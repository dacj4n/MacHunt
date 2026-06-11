use crate::db::Db;
use crate::filters::{is_excluded, ExcludeRules};
use crate::utils::{should_skip_path, Logger};
use core_foundation_sys::runloop::{
    CFRunLoopGetCurrent, CFRunLoopRef, CFRunLoopRun, CFRunLoopStop,
};
use std::ffi::{c_void, CStr};
use std::os::raw::{c_char, c_double, c_ulong};
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use walkdir::WalkDir;

#[allow(non_camel_case_types)]
type FSEventStreamRef = *mut c_void;
#[allow(non_camel_case_types)]
type FSEventStreamCallback = unsafe extern "C" fn(
    stream_ref: FSEventStreamRef,
    client_callback_info: *mut c_void,
    num_events: usize,
    event_paths: *mut c_void,
    event_flags: *const u32,
    event_ids: *const u64,
);

#[repr(C)]
struct FSEventStreamContext {
    version: c_ulong,
    info: *mut c_void,
    retain: *const c_void,
    release: *const c_void,
    copy_description: *const c_void,
}

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn FSEventStreamCreate(
        allocator: *const c_void,
        callback: FSEventStreamCallback,
        context: *mut FSEventStreamContext,
        paths_to_watch: *const c_void,
        since_when: u64,
        latency: c_double,
        flags: u32,
    ) -> FSEventStreamRef;

    fn FSEventStreamScheduleWithRunLoop(
        stream_ref: FSEventStreamRef,
        run_loop: CFRunLoopRef,
        run_loop_mode: *const c_void,
    );

    fn FSEventStreamStart(stream_ref: FSEventStreamRef) -> bool;
    fn FSEventStreamStop(stream_ref: FSEventStreamRef);
    fn FSEventStreamInvalidate(stream_ref: FSEventStreamRef);
    fn FSEventStreamRelease(stream_ref: FSEventStreamRef);
    pub fn FSEventsGetCurrentEventId() -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFArrayCreate(
        allocator: *const c_void,
        values: *const *const c_void,
        num_values: isize,
        callbacks: *const c_void,
    ) -> *const c_void;

    fn CFStringCreateWithCString(
        allocator: *const c_void,
        c_str: *const c_char,
        encoding: u32,
    ) -> *const c_void;

    fn CFRelease(cf: *const c_void);

    static kCFRunLoopDefaultMode: *const c_void;
    static kCFTypeArrayCallBacks: c_void;
}

const FSEVENT_SINCE_NOW: u64 = u64::MAX;
const KCF_STRING_ENCODING_UTF8: u32 = 0x08000100;

const FLAG_HISTORY_DONE: u32 = 0x0000_2000;
const FLAG_ITEM_CREATED: u32 = 0x0000_0100;
const FLAG_ITEM_REMOVED: u32 = 0x0000_0200;
const FLAG_ITEM_RENAMED: u32 = 0x0000_0800;
const FLAG_ITEM_MODIFIED: u32 = 0x0000_1000;
const FLAG_ITEM_IS_FILE: u32 = 0x0001_0000;

const STREAM_FLAG_FILE_EVENTS: u32 = 0x0000_0010;
const STREAM_FLAG_WATCH_ROOT: u32 = 0x0000_0004;

struct WatchContext {
    db: Db,
    logger: Logger,
    last_event_id: Arc<AtomicU64>,
    include_dirs: bool,
    exclude_rules: Arc<ExcludeRules>,
}

#[derive(Default)]
struct WatchRuntime {
    stream_ref: usize,
    run_loop_ref: usize,
    running: bool,
}

fn watch_runtime() -> &'static Mutex<WatchRuntime> {
    static RUNTIME: OnceLock<Mutex<WatchRuntime>> = OnceLock::new();
    RUNTIME.get_or_init(|| Mutex::new(WatchRuntime::default()))
}

fn lock_watch_runtime() -> std::sync::MutexGuard<'static, WatchRuntime> {
    watch_runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn upsert_file(ctx: &WatchContext, path: &Path) {
    if should_skip_path(path) {
        return;
    }
    if is_excluded(path, path.is_dir(), &ctx.exclude_rules) {
        return;
    }
    if !ctx.include_dirs && path.is_dir() {
        return;
    }
    let file_name_lower = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.to_lowercase(),
        None => return,
    };
    // Direct DB insert — UNIQUE constraint handles dedup.
    if let Some(rowid) = ctx.db.insert(&file_name_lower, path, path.is_dir()) {
        ctx.db.insert_fts(rowid, &file_name_lower);
    }
    if ctx.logger.enabled() {
        ctx.logger.log(&format!("[+] {}", path.display()));
    }
}

fn remove_file(ctx: &WatchContext, path: &Path) {
    ctx.db.delete(path);
    if ctx.logger.enabled() {
        ctx.logger.log(&format!("[-] {}", path.display()));
    }
}

fn remove_tree(ctx: &WatchContext, root: &Path) {
    ctx.db.delete_under_root(root);
    if ctx.logger.enabled() {
        ctx.logger.log(&format!("[-] tree {}", root.display()));
    }
}

/// On rename events, clean up stale DB entries in the parent directory.
/// APFS may not send a REMOVED event for the old name, leaving dead paths.
fn clean_dead_in_dir(ctx: &WatchContext, dir: &Path) {
    let dir_str = dir.to_string_lossy();
    let rows = ctx.db.list_files_in_dir(&dir_str);
    for (name, full_path) in rows {
        let p = Path::new(&full_path);
        if !p.exists() {
            ctx.db.delete_by_dir_and_name(&dir_str, &name);
            if ctx.logger.enabled() {
                ctx.logger.log(&format!("[-] stale rename {}", full_path));
            }
        }
    }
}

fn index_directory(ctx: &WatchContext, root: &Path) {
    if should_skip_path(root) {
        return;
    }
    if is_excluded(root, true, &ctx.exclude_rules) {
        return;
    }

    for entry in WalkDir::new(root)
        .follow_links(false)
        .min_depth(0)
        .into_iter()
        .filter_entry(|e| {
            let path = e.path();
            !should_skip_path(path)
                && !is_excluded(path, e.file_type().is_dir(), &ctx.exclude_rules)
        })
        .filter_map(Result::ok)
    {
        if !ctx.include_dirs && entry.file_type().is_dir() {
            continue;
        }
        upsert_file(ctx, entry.path());
    }
}

unsafe extern "C" fn fsevent_callback(
    _stream_ref: FSEventStreamRef,
    client_info: *mut c_void,
    num_events: usize,
    event_paths: *mut c_void,
    event_flags: *const u32,
    event_ids: *const u64,
) {
    let ctx = &*(client_info as *const WatchContext);
    let paths_ptr = event_paths as *const *const c_char;

    for i in 0..num_events {
        let flags = *event_flags.add(i);
        let event_id = *event_ids.add(i);
        let path_cstr = CStr::from_ptr(*paths_ptr.add(i));
        let path_str = match path_cstr.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };

        ctx.last_event_id.store(event_id, Ordering::Relaxed);

        if flags & FLAG_HISTORY_DONE != 0 {
            println!(
                "History playback completed (EventID: {}), entering real-time monitoring",
                event_id
            );
            ctx.db.save_last_event_id(event_id);
            continue;
        }

        // Fast-path skip on the raw C string before any allocation.
        // During bootstrap builds, WAL file writes can generate millions
        // of self-events that must be discarded cheaply.
        if path_str.contains("/.Spotlight-V100")
            || path_str.contains("/.fseventsd")
            || path_str.contains("/Library/Caches/MacHunt")
        {
            continue;
        }

        let path = PathBuf::from(path_str);

        if should_skip_path(path.as_path()) {
            continue;
        }

        // Directory events
        if flags & FLAG_ITEM_IS_FILE == 0 {
            if flags & FLAG_ITEM_REMOVED != 0 {
                remove_tree(ctx, path.as_path());
            } else if flags & (FLAG_ITEM_CREATED | FLAG_ITEM_RENAMED | FLAG_ITEM_MODIFIED) != 0 {
                if path.is_dir() {
                    index_directory(ctx, path.as_path());
                } else if path.exists() {
                    upsert_file(ctx, path.as_path());
                }
            }
            continue;
        }

        // File events
        if flags & FLAG_ITEM_REMOVED != 0 {
            remove_file(ctx, path.as_path());
            continue;
        }

        let is_create = flags & FLAG_ITEM_CREATED != 0;
        let is_rename = flags & FLAG_ITEM_RENAMED != 0;

        if is_rename && !is_create {
            // Pure rename: APFS may not send REMOVED for the old name.
            // Clean up any stale entries in the parent directory.
            if let Some(parent) = path.parent() {
                clean_dead_in_dir(ctx, parent);
            }
        }

        if flags & (FLAG_ITEM_CREATED | FLAG_ITEM_RENAMED | FLAG_ITEM_MODIFIED) != 0 {
            if path.is_file() {
                upsert_file(ctx, path.as_path());
            } else {
                remove_file(ctx, path.as_path());
            }
        }
    }
}

pub fn start_watch(
    db: Db,
    logger: Logger,
    last_event_id: Arc<AtomicU64>,
    include_dirs: bool,
    since_event_id: Option<u64>,
    watch_roots: Vec<String>,
    exclude_rules: Arc<ExcludeRules>,
) {
    {
        let mut runtime = lock_watch_runtime();
        if runtime.running {
            return;
        }
        runtime.running = true;
        runtime.stream_ref = 0;
        runtime.run_loop_ref = 0;
    }

    let since = since_event_id.unwrap_or(FSEVENT_SINCE_NOW);
    thread::spawn(move || unsafe {
        let mut c_paths = Vec::new();
        for root in watch_roots {
            if let Ok(c) = std::ffi::CString::new(root) {
                c_paths.push(c);
            }
        }
        if c_paths.is_empty() {
            if let Ok(root) = std::ffi::CString::new("/") {
                c_paths.push(root);
            }
        }

        let mut cf_paths = Vec::<*const c_void>::new();
        for path_cstr in &c_paths {
            let cf_path = CFStringCreateWithCString(
                std::ptr::null(),
                path_cstr.as_ptr(),
                KCF_STRING_ENCODING_UTF8,
            );
            cf_paths.push(cf_path);
        }

        let paths_array = CFArrayCreate(
            std::ptr::null(),
            cf_paths.as_ptr(),
            cf_paths.len() as isize,
            &kCFTypeArrayCallBacks as *const _,
        );

        let ctx = Box::new(WatchContext {
            db,
            logger,
            last_event_id,
            include_dirs,
            exclude_rules,
        });
        let ctx_ptr = Box::into_raw(ctx);
        let mut fsevent_ctx = FSEventStreamContext {
            version: 0,
            info: ctx_ptr as *mut c_void,
            retain: std::ptr::null(),
            release: std::ptr::null(),
            copy_description: std::ptr::null(),
        };

        let stream = FSEventStreamCreate(
            std::ptr::null(),
            fsevent_callback,
            &mut fsevent_ctx,
            paths_array,
            since,
            0.3, // 300ms coalescing window for better batch efficiency
            STREAM_FLAG_FILE_EVENTS | STREAM_FLAG_WATCH_ROOT,
        );

        // FSEventStreamCreate retains the array; release our references.
        for cf_path in &cf_paths {
            CFRelease(*cf_path);
        }
        CFRelease(paths_array);

        if stream.is_null() {
            let mut runtime = lock_watch_runtime();
            runtime.running = false;
            runtime.stream_ref = 0;
            runtime.run_loop_ref = 0;
            drop(Box::from_raw(ctx_ptr));
            eprintln!("Failed to create FSEvent stream");
            return;
        }

        let run_loop = CFRunLoopGetCurrent();
        {
            let mut runtime = lock_watch_runtime();
            runtime.stream_ref = stream as usize;
            runtime.run_loop_ref = run_loop as usize;
        }

        FSEventStreamScheduleWithRunLoop(stream, run_loop, kCFRunLoopDefaultMode);

        if !FSEventStreamStart(stream) {
            FSEventStreamInvalidate(stream);
            FSEventStreamRelease(stream);
            drop(Box::from_raw(ctx_ptr));
            let mut runtime = lock_watch_runtime();
            runtime.running = false;
            runtime.stream_ref = 0;
            runtime.run_loop_ref = 0;
            eprintln!("Failed to start FSEvent stream");
            return;
        }

        println!(
            "FSEvents monitoring started, since_event_id={:?}",
            since_event_id
        );

        CFRunLoopRun();

        FSEventStreamStop(stream);
        FSEventStreamInvalidate(stream);
        FSEventStreamRelease(stream);
        drop(Box::from_raw(ctx_ptr));

        let mut runtime = lock_watch_runtime();
        runtime.running = false;
        runtime.stream_ref = 0;
        runtime.run_loop_ref = 0;
        println!("FSEvents monitoring stopped");
    });
}

pub fn stop_watch() -> bool {
    let (running, stream_ref, run_loop_ref) = {
        let runtime = lock_watch_runtime();
        (runtime.running, runtime.stream_ref, runtime.run_loop_ref)
    };

    if !running || run_loop_ref == 0 {
        return false;
    }

    unsafe {
        if stream_ref != 0 {
            FSEventStreamStop(stream_ref as FSEventStreamRef);
        }
        CFRunLoopStop(run_loop_ref as CFRunLoopRef);
    }

    true
}

pub fn is_watch_running() -> bool {
    lock_watch_runtime().running
}

use crate::db::Db;
use crate::utils::{should_skip_path, Logger};
use core_foundation_sys::runloop::{
    CFRunLoopGetCurrent, CFRunLoopRef, CFRunLoopRun, CFRunLoopStop,
};
use dashmap::DashMap;
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
    index: Arc<DashMap<String, Vec<PathBuf>>>,
    db: Db,
    logger: Logger,
    last_event_id: Arc<AtomicU64>,
    index_write_lock: Arc<Mutex<()>>,
    include_dirs: bool,
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

fn upsert_path(ctx: &WatchContext, path: &Path) {
    if should_skip_path(path) {
        return;
    }
    if !ctx.include_dirs && path.is_dir() {
        return;
    }
    let _guard = ctx
        .index_write_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let file_name_lower = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.to_lowercase(),
        None => return,
    };
    let path_buf = path.to_path_buf();
    let mut entry = ctx.index.entry(file_name_lower.clone()).or_default();
    if !entry.contains(&path_buf) {
        entry.push(path_buf.clone());
        ctx.db.insert(&file_name_lower, path);
        if ctx.logger.enabled() {
            ctx.logger.log(&format!("[+] {}", path.display()));
        }
    }
}

fn remove_path(ctx: &WatchContext, path: &Path) {
    let _guard = ctx
        .index_write_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let file_name_lower = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.to_lowercase(),
        None => return,
    };
    if let Some(mut v) = ctx.index.get_mut(&file_name_lower) {
        v.retain(|p| p != path);
        if v.is_empty() {
            drop(v);
            ctx.index.remove(&file_name_lower);
        }
    }
    ctx.db.delete(path);
    if ctx.logger.enabled() {
        ctx.logger.log(&format!("[-] {}", path.display()));
    }
}

fn remove_path_tree(ctx: &WatchContext, root: &Path) {
    let _guard = ctx
        .index_write_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root_str = root.to_string_lossy();
    let prefix = format!("{}/", root_str);
    ctx.index.retain(|_, v| {
        v.retain(|p| {
            let path_str = p.to_string_lossy();
            if p.as_path() == root || path_str.starts_with(&prefix) {
                ctx.db.delete(p.as_path());
                if ctx.logger.enabled() {
                    ctx.logger.log(&format!("[-] {}", p.display()));
                }
                false
            } else {
                true
            }
        });
        !v.is_empty()
    });
}

fn index_directory_tree(ctx: &WatchContext, root: &Path) {
    if should_skip_path(root) {
        return;
    }
    for entry in WalkDir::new(root)
        .follow_links(false)
        .min_depth(0)
        .into_iter()
        .filter_entry(|e| !should_skip_path(e.path()))
        .filter_map(Result::ok)
    {
        if !ctx.include_dirs && entry.file_type().is_dir() {
            continue;
        }
        upsert_path(ctx, entry.path());
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

        let path = PathBuf::from(path_str);

        if flags & FLAG_ITEM_IS_FILE == 0 {
            if flags & FLAG_ITEM_REMOVED != 0 {
                remove_path_tree(ctx, path.as_path());
            } else if flags & (FLAG_ITEM_CREATED | FLAG_ITEM_RENAMED | FLAG_ITEM_MODIFIED) != 0 {
                if path.is_dir() {
                    index_directory_tree(ctx, path.as_path());
                } else if path.exists() {
                    upsert_path(ctx, path.as_path());
                }
            }
            continue;
        }

        if flags & FLAG_ITEM_REMOVED != 0 {
            remove_path(ctx, path.as_path());
            continue;
        }

        if flags & (FLAG_ITEM_CREATED | FLAG_ITEM_RENAMED | FLAG_ITEM_MODIFIED) != 0 {
            if path.is_file() {
                upsert_path(ctx, path.as_path());
            } else {
                remove_path(ctx, path.as_path());
            }
        }
    }
}

pub fn start_watch(
    index: Arc<DashMap<String, Vec<PathBuf>>>,
    db: Db,
    logger: Logger,
    last_event_id: Arc<AtomicU64>,
    index_write_lock: Arc<Mutex<()>>,
    include_dirs: bool,
    since_event_id: Option<u64>,
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
        let path_cstr = std::ffi::CString::new("/").unwrap();
        let cf_path = CFStringCreateWithCString(
            std::ptr::null(),
            path_cstr.as_ptr(),
            KCF_STRING_ENCODING_UTF8,
        );
        let paths_array = CFArrayCreate(
            std::ptr::null(),
            &cf_path as *const _,
            1,
            &kCFTypeArrayCallBacks as *const _,
        );

        let ctx = Box::new(WatchContext {
            index,
            db,
            logger,
            last_event_id,
            index_write_lock,
            include_dirs,
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
            0.05,
            STREAM_FLAG_FILE_EVENTS | STREAM_FLAG_WATCH_ROOT,
        );

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

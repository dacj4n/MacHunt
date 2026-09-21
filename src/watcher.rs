use crate::db::Db;
use crate::filters::{is_excluded, is_file_excluded, ExcludeRules, FileExcludeRules};
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

// Event flags, taken from CoreServices/FSEvents.framework/Headers/FSEvents.h.
// A wrong bit here fails silently — the branch it guards simply never fires —
// so every value is spelled out next to the constant it must equal.
/// The daemon could not deliver every event and flagged the enclosing directory
/// instead, expecting the client to rescan that subtree. `USER_DROPPED` /
/// `KERNEL_DROPPED` say whose queue overflowed.
const FLAG_MUST_SCAN_SUBDIRS: u32 = 0x0000_0001;
const FLAG_USER_DROPPED: u32 = 0x0000_0002;
const FLAG_KERNEL_DROPPED: u32 = 0x0000_0004;
/// Event IDs wrapped around, so a stored resume ID is no longer meaningful.
const FLAG_EVENT_IDS_WRAPPED: u32 = 0x0000_0008;
/// End of the historical replay. This is 0x10. It used to be written as
/// 0x2000, which is `kFSEventStreamEventFlagItemFinderInfoMod` — so the replay
/// completion was never detected (and the event ID was never persisted at that
/// point), while every Finder-info change looked like the end of a replay.
const FLAG_HISTORY_DONE: u32 = 0x0000_0010;
const FLAG_ITEM_CREATED: u32 = 0x0000_0100;
const FLAG_ITEM_REMOVED: u32 = 0x0000_0200;
const FLAG_ITEM_INODE_META_MOD: u32 = 0x0000_0400;
const FLAG_ITEM_RENAMED: u32 = 0x0000_0800;
const FLAG_ITEM_MODIFIED: u32 = 0x0000_1000;
const FLAG_ITEM_XATTR_MOD: u32 = 0x0000_8000;
const FLAG_ITEM_IS_FILE: u32 = 0x0001_0000;

const STREAM_FLAG_FILE_EVENTS: u32 = 0x0000_0010;
const STREAM_FLAG_WATCH_ROOT: u32 = 0x0000_0004;

struct WatchContext {
    db: Db,
    logger: Logger,
    last_event_id: Arc<AtomicU64>,
    include_dirs: bool,
    exclude_rules: Arc<ExcludeRules>,
    file_exclude_rules: Arc<FileExcludeRules>,
    history_done: std::sync::atomic::AtomicBool,
    /// The roots the stream was created for. `reconcile_dir` needs them so a
    /// drop notification that lands on a root does not turn into a rescan of an
    /// entire volume from inside the event callback.
    watch_roots: Arc<Vec<String>>,
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

fn upsert_file(ctx: &WatchContext, path: &Path, meta: Option<&std::fs::Metadata>) {
    if should_skip_path(path) {
        return;
    }

    // Only stat when the caller could not hand metadata over: the directory walk
    // already has it, and an FSEvents callback is per file (low frequency, so one
    // stat each is a fair price for keeping size/mtime accurate).
    let fallback = match meta {
        Some(_) => None,
        None => std::fs::metadata(path).ok(),
    };
    let meta = match meta.or(fallback.as_ref()) {
        Some(m) => m,
        // The path is already gone (vanished between the event and now). Do not
        // create a row for something that is not there.
        None => return,
    };

    let is_dir = meta.is_dir();
    if is_excluded(path, is_dir, &ctx.exclude_rules) {
        return;
    }
    if is_file_excluded(path, is_dir, &ctx.file_exclude_rules) {
        return;
    }
    if !ctx.include_dirs && is_dir {
        return;
    }
    let file_name_lower = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.to_lowercase(),
        None => return,
    };

    // Store the real metadata. Without it a freshly indexed file would be NULL
    // in the DB, which makes the client-side size/time filters drop it and the
    // size/time sorts treat it as 0.
    let size_bytes = if meta.is_file() { Some(meta.len()) } else { None };
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_millis()).ok());

    // Direct DB insert — UNIQUE constraint handles dedup.
    if let Some(rowid) = ctx.db.insert(&file_name_lower, path, is_dir, size_bytes, modified_ms) {
        ctx.db.insert_fts(rowid, &file_name_lower);
    }
    if ctx.logger.enabled() {
        ctx.logger
            .log_path(path, &format!("[+] {}", path.display()));
    }
}

fn remove_file(ctx: &WatchContext, path: &Path) {
    ctx.db.delete(path);
    if ctx.logger.enabled() {
        ctx.logger
            .log_path(path, &format!("[-] {}", path.display()));
    }
}

/// Re-read size/mtime for a path that is already indexed.
///
/// A file created by a copy is indexed the instant it appears — before any data
/// has been written — so it lands with size 0 and the current time. Finishing a
/// copy then restores mtime (and xattrs) from the source, which FSEvents reports
/// as a metadata notification rather than a MODIFIED one. Ignoring those left the
/// 0-byte metadata in place permanently, which is why a copied file could show no
/// size at all.
///
/// This deliberately never creates rows: metadata notifications can be frequent
/// (Spotlight, backups, sync clients), and they must not grow the index.
fn refresh_file_metadata(ctx: &WatchContext, path: &Path) {
    if !path.is_file() {
        return;
    }
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return,
    };
    let size_bytes = Some(meta.len());
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_millis()).ok());

    if ctx.db.refresh_metadata(path, size_bytes, modified_ms) && ctx.logger.enabled() {
        ctx.logger.log_path(
            path,
            &format!("[~] {} ({} bytes)", path.display(), meta.len()),
        );
    }
}

fn remove_tree(ctx: &WatchContext, root: &Path) {
    ctx.db.delete_under_root(root);
    if ctx.logger.enabled() {
        ctx.logger
            .log_path(root, &format!("[-] tree {}", root.display()));
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
                ctx.logger.log_path(
                    Path::new(&full_path),
                    &format!("[-] stale rename {}", full_path),
                );
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
        // WalkDir already stat'ed this entry — pass the metadata through instead
        // of making `upsert_file` stat the same path a second time.
        let meta = entry.metadata().ok();
        upsert_file(ctx, entry.path(), meta.as_ref());
    }
}

/// Re-scan a directory the event stream admits it did not fully report.
///
/// Called when FSEvents sets `MustScanSubDirs` / `UserDropped` /
/// `KernelDropped` / `EventIdsWrapped`: the daemon coalesced or dropped events
/// while a burst was happening (a Finder paste or drag of several files is
/// exactly that) and expects the client to rescan the flagged subtree instead.
///
/// Skipping this does not just delay indexing — the dropped events are gone for
/// good, so files created during the gap stay unindexed until the next full
/// rebuild. That is why a batch copy could leave only some of its files
/// searchable, with the missing ones changing from run to run.
///
/// The upsert pass is recursive; `clean_dead_in_dir` sweeps the flagged
/// directory itself, which is the same one-level stale clean the rename path
/// uses.
fn reconcile_dir(ctx: &WatchContext, dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    if should_skip_path(dir) || is_excluded(dir, true, &ctx.exclude_rules) {
        return;
    }
    // Running a deep scan from inside the event callback blocks the stream,
    // which is itself a way to cause more drops — a feedback loop. A drop
    // flagged on a watch root would mean rescanning a whole volume, so that case
    // is left to the app's own verification pass. Burst copies land in ordinary
    // directories, and those are cheap enough to repair here.
    if ctx.watch_roots.iter().any(|root| dir == Path::new(root)) {
        if ctx.logger.enabled() {
            ctx.logger.log_path(
                dir,
                &format!(
                    "[!] {} is a watch root — not rescanning from the event callback",
                    dir.display()
                ),
            );
        }
        return;
    }
    index_directory(ctx, dir);
    clean_dead_in_dir(ctx, dir);
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
            ctx.history_done.store(true, std::sync::atomic::Ordering::Relaxed);
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

        // Exclusion rules take absolute priority — even if the watch root
        // was explicitly configured, any path matching an exclude pattern
        // must be silently dropped before any indexing or removal.
        let is_dir = flags & FLAG_ITEM_IS_FILE == 0;
        if is_excluded(path.as_path(), is_dir, &ctx.exclude_rules) {
            // Files only: an excluded file is the other way a real file can
            // silently never appear, and logging directories here would drown
            // the log in system churn.
            if !is_dir && ctx.logger.enabled() {
                ctx.logger.log_path(
                    path.as_path(),
                    &format!("[excl] flags=0x{:08x} path={}", flags, path_str),
                );
            }
            continue;
        }

        // FSEvents says it could not report everything, and names the directory
        // to rescan. Handled *before* the flag dispatch below, because a drop
        // notification carries none of the CREATED/REMOVED/RENAMED/MODIFIED
        // bits — it looks like an idle directory event, and the dispatch would
        // swallow it silently.
        if flags & (FLAG_MUST_SCAN_SUBDIRS
            | FLAG_USER_DROPPED
            | FLAG_KERNEL_DROPPED
            | FLAG_EVENT_IDS_WRAPPED)
            != 0
        {
            if ctx.logger.enabled() {
                let mut cause = String::new();
                if flags & FLAG_USER_DROPPED != 0 {
                    cause.push_str(", user queue");
                }
                if flags & FLAG_KERNEL_DROPPED != 0 {
                    cause.push_str(", kernel queue");
                }
                if flags & FLAG_EVENT_IDS_WRAPPED != 0 {
                    cause.push_str(", ids wrapped");
                }
                ctx.logger.log_path(
                    path.as_path(),
                    &format!(
                        "[!] events dropped (flags=0x{:08x}{}) — rescanning {}",
                        flags, cause, path_str
                    ),
                );
            }
            reconcile_dir(ctx, path.as_path());
            continue;
        }

        // Directory events
        if is_dir {
            if flags & FLAG_ITEM_REMOVED != 0 {
                remove_tree(ctx, path.as_path());
                continue;
            }
            // Only CREATED needs recursive indexing (new directory with
            // files inside). MODIFIED means something inside changed —
            // file-level events handle those individually. Re-indexing
            // the entire directory on every MODIFIED event is the #1
            // cause of 100% CPU during history replay.
            if flags & FLAG_ITEM_CREATED != 0 {
                if path.is_dir() {
                    index_directory(ctx, path.as_path());
                } else if path.exists() {
                    upsert_file(ctx, path.as_path(), None);
                }
            } else if flags & FLAG_ITEM_RENAMED != 0 {
                // Renamed directory: clean stale entries in parent,
                // then index the renamed directory if it exists.
                if let Some(parent) = path.parent() {
                    clean_dead_in_dir(ctx, parent);
                }
                if path.is_dir() {
                    index_directory(ctx, path.as_path());
                } else if path.exists() {
                    upsert_file(ctx, path.as_path(), None);
                }
            } else if flags & (FLAG_ITEM_INODE_META_MOD | FLAG_ITEM_XATTR_MOD) != 0
                && ctx.logger.enabled()
            {
                // A metadata-only notification on a *directory*. This is the
                // shape Finder's drag-and-drop copy leaves behind: the
                // destination folder is touched, and no usable file event is
                // sent for the file that was just created in it. Logged so the
                // next test can confirm the flags actually observed.
                ctx.logger.log_path(
                    path.as_path(),
                    &format!("[dirmeta] flags=0x{:08x} path={}", flags, path_str),
                );
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
                upsert_file(ctx, path.as_path(), None);
            } else {
                remove_file(ctx, path.as_path());
            }
        } else if flags & (FLAG_ITEM_INODE_META_MOD | FLAG_ITEM_XATTR_MOD) != 0 {
            refresh_file_metadata(ctx, path.as_path());
        } else if ctx.logger.enabled() {
            // Nothing recognised this event. A file that never reaches the index
            // hides exactly here, so record the flags FSEvents actually sent —
            // with logging off this costs one branch, nothing more.
            ctx.logger.log_path(
                path.as_path(),
                &format!("[skip] flags=0x{:08x} path={}", flags, path_str),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn start_watch(
    db: Db,
    logger: Logger,
    last_event_id: Arc<AtomicU64>,
    include_dirs: bool,
    since_event_id: Option<u64>,
    watch_roots: Vec<String>,
    exclude_rules: Arc<ExcludeRules>,
    file_exclude_rules: Arc<FileExcludeRules>,
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
        let roots_for_reconcile: Arc<Vec<String>> = Arc::new(watch_roots.clone());
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
            file_exclude_rules,
            history_done: std::sync::atomic::AtomicBool::new(false),
            watch_roots: roots_for_reconcile,
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

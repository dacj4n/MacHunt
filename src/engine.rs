use crate::builder;
use crate::db::Db;
use crate::filters::{
    compile_exclude_rules, sanitize_owned_rules, sanitize_roots, validate_pattern_rules,
};
use crate::model::SearchOptions;
use crate::search;
use crate::utils::{get_root_directories, num_cpus, Logger};
use crate::watcher;
use dashmap::DashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const STARTUP_CLEANUP_MAX_DELETE_PER_ROUND: usize = 2000;
const STARTUP_CLEANUP_SCAN_BATCH: usize = 8000;

#[derive(Clone)]
pub struct Engine {
    index: Arc<DashMap<String, Vec<PathBuf>>>,
    db: Db,
    logger: Logger,
    last_event_id: Arc<AtomicU64>,
    index_write_lock: Arc<Mutex<()>>,
    include_dirs: Arc<AtomicBool>,
    exclude_exact_dirs: Arc<Mutex<Vec<String>>>,
    exclude_pattern_dirs: Arc<Mutex<Vec<String>>>,
    watch_roots: Arc<Mutex<Vec<String>>>,
    dirty_roots: Arc<Mutex<Vec<PathBuf>>>,
    dirty_worker_running: Arc<AtomicBool>,
    cleanup_running: Arc<AtomicBool>,
    cleanup_cursor_id: Arc<AtomicU64>,
}

impl Engine {
    pub fn new(logs_enabled: bool) -> Self {
        let db = Db::init_default();
        // Pre-allocate for large indexes to reduce startup rehash/resize churn.
        let index = Arc::new(DashMap::with_capacity(1_048_576));
        let logger = Logger::new(logs_enabled);
        let last_event_id = Arc::new(AtomicU64::new(0));
        let index_write_lock = Arc::new(Mutex::new(()));
        let include_dirs = Arc::new(AtomicBool::new(db.load_include_dirs().unwrap_or(true)));
        let exclude_exact_dirs = Arc::new(Mutex::new(db.load_exclude_exact_dirs()));
        let exclude_pattern_dirs = Arc::new(Mutex::new(db.load_exclude_pattern_dirs()));

        let mut watch_roots = sanitize_roots(db.load_watch_roots());
        if watch_roots.is_empty() {
            watch_roots = default_watch_roots();
            db.save_watch_roots(&watch_roots);
        }

        Self {
            index,
            db,
            logger,
            last_event_id,
            index_write_lock,
            include_dirs,
            exclude_exact_dirs,
            exclude_pattern_dirs,
            watch_roots: Arc::new(Mutex::new(watch_roots)),
            dirty_roots: Arc::new(Mutex::new(Vec::new())),
            dirty_worker_running: Arc::new(AtomicBool::new(false)),
            cleanup_running: Arc::new(AtomicBool::new(false)),
            cleanup_cursor_id: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn load_index_from_db(&self) -> usize {
        if !self.db.path().exists() {
            return 0;
        }

        let start = Instant::now();
        println!("Loading index from database...");
        let _guard = self
            .index_write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let count = self.db.load_index(&self.index);
        println!("Loaded {} records, took {:?}", count, start.elapsed());
        count
    }

    pub fn build_index(
        &self,
        path: Option<String>,
        _rebuild: bool,
        include_dirs: bool,
        auto_vacuum_on_rebuild: bool,
    ) -> usize {
        self.include_dirs.store(include_dirs, Ordering::Relaxed);
        self.db.save_include_dirs(include_dirs);
        let exclude_exact_dirs = self
            .exclude_exact_dirs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let exclude_pattern_dirs = self
            .exclude_pattern_dirs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let watch_roots = self
            .watch_roots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();

        let filters = builder::BuildFilterSettings {
            include_dirs,
            exclude_exact_dirs,
            exclude_pattern_dirs,
            watch_roots: Some(watch_roots),
        };

        // Use a temp DB + atomic swap for full rebuilds, or for
        // build/rebuild when no specific incremental path is given.
        // The only incremental (non-temp) path is the dirty-root worker,
        // which passes auto_vacuum_on_rebuild=false and a specific path.
        let is_incremental = !auto_vacuum_on_rebuild && path.is_some();

        if !is_incremental {
            self.db.begin_rebuild();
        }

        let count = builder::build_index(
            &self.db,
            &self.index,
            &self.index_write_lock,
            path,
            !is_incremental,             // treat as fresh DB if not incremental
            &filters,
        );

        if !is_incremental {
            if let Err(e) = self.db.finish_rebuild() {
                eprintln!("finish_rebuild failed: {}", e);
            }
        } else {
            self.db.checkpoint_truncate();
        }

        let current_event_id = unsafe { watcher::FSEventsGetCurrentEventId() };
        self.db.save_last_event_id(current_event_id);
        println!(
            "Saved EventID: {}, next watch will use incremental sync",
            current_event_id
        );
        count
    }

    pub fn get_exclude_dir_settings(&self) -> (Vec<String>, Vec<String>) {
        let exact_dirs = self
            .exclude_exact_dirs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let pattern_dirs = self
            .exclude_pattern_dirs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        (exact_dirs, pattern_dirs)
    }

    pub fn set_exclude_dir_settings(
        &self,
        exact_dirs: Vec<String>,
        pattern_dirs: Vec<String>,
    ) -> Result<(Vec<String>, Vec<String>), String> {
        let sanitized_exact = sanitize_owned_rules(exact_dirs);
        let sanitized_pattern = sanitize_owned_rules(pattern_dirs);
        validate_pattern_rules(&sanitized_pattern)?;

        {
            let mut guard = self
                .exclude_exact_dirs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = sanitized_exact.clone();
        }
        {
            let mut guard = self
                .exclude_pattern_dirs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = sanitized_pattern.clone();
        }

        self.db.save_exclude_exact_dirs(&sanitized_exact);
        self.db.save_exclude_pattern_dirs(&sanitized_pattern);

        Ok((sanitized_exact, sanitized_pattern))
    }

    pub fn get_watch_roots(&self) -> Vec<String> {
        let roots = self
            .watch_roots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if roots.is_empty() {
            default_watch_roots()
        } else {
            roots
        }
    }

    pub fn set_watch_roots(&self, roots: Vec<String>) -> Vec<String> {
        let sanitized = normalize_watch_roots(roots);
        let final_roots = if sanitized.is_empty() {
            default_watch_roots()
        } else {
            sanitized
        };

        {
            let mut guard = self
                .watch_roots
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = final_roots.clone();
        }

        self.db.save_watch_roots(&final_roots);
        final_roots
    }

    pub fn start_watch(&self, since_event_id: Option<u64>) {
        let watch_roots = self.get_watch_roots();
        let exclude_exact_dirs = self
            .exclude_exact_dirs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let exclude_pattern_dirs = self
            .exclude_pattern_dirs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let exclude_rules = compile_exclude_rules(&exclude_exact_dirs, &exclude_pattern_dirs);

        watcher::start_watch(
            self.index.clone(),
            self.db.clone(),
            self.logger.clone(),
            self.last_event_id.clone(),
            self.index_write_lock.clone(),
            self.include_dirs.load(Ordering::Relaxed),
            since_event_id,
            watch_roots,
            Arc::new(exclude_rules),
            self.dirty_roots.clone(),
        );
        self.start_dirty_root_worker();
    }

    pub fn stop_watch(&self) -> bool {
        watcher::stop_watch()
    }

    pub fn is_watch_running(&self) -> bool {
        watcher::is_watch_running()
    }

    pub fn search(&self, options: SearchOptions) -> Vec<PathBuf> {
        search::search(&self.index, options)
    }

    pub fn load_last_event_id(&self) -> Option<u64> {
        self.db.load_last_event_id()
    }

    pub fn has_persisted_index(&self) -> bool {
        self.db.has_any_files()
    }

    pub fn checkpoint_wal(&self) {
        self.db.checkpoint_truncate();
    }

    pub fn vacuum(&self) {
        self.db.vacuum();
    }

    pub fn save_last_event_id_from_runtime(&self) {
        let id = self.last_event_id.load(Ordering::Relaxed);
        if id > 0 {
            self.db.save_last_event_id(id);
            println!("\nSaved EventID: {}", id);
        }
    }

    pub fn cleanup_dead_paths_background(&self) {
        if self
            .cleanup_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let db = self.db.clone();
        let index = self.index.clone();
        let logger = self.logger.clone();
        let index_write_lock = self.index_write_lock.clone();
        let running = self.cleanup_running.clone();
        let cursor = self.cleanup_cursor_id.clone();

        thread::spawn(move || {
            let start = Instant::now();
            let mut removed_total = 0usize;
            let mut scanned_total = 0usize;
            let mut rounds = 0usize;

            loop {
                rounds += 1;
                let mut removed_this_round = 0usize;

                while removed_this_round < STARTUP_CLEANUP_MAX_DELETE_PER_ROUND {
                    let last_id = cursor.load(Ordering::Relaxed);
                    let rows = db.list_paths_after_id(last_id, STARTUP_CLEANUP_SCAN_BATCH);
                    if rows.is_empty() {
                        cursor.store(0, Ordering::Relaxed);
                        if removed_total > 0 {
                            println!(
                                "[Startup Validation] Cleaned up {} dead paths (scanned {}, rounds {}), took {:?}",
                                removed_total,
                                scanned_total,
                                rounds,
                                start.elapsed()
                            );
                        }
                        running.store(false, Ordering::SeqCst);
                        return;
                    }

                    let mut dead = Vec::<PathBuf>::new();
                    let mut max_id_seen = last_id;
                    let chunk_size = (rows.len() / num_cpus()).max(1);
                    let dead_shared = Arc::new(std::sync::Mutex::new(Vec::<PathBuf>::new()));

                    let handles: Vec<_> = rows
                        .chunks(chunk_size)
                        .map(|chunk| {
                            let chunk = chunk.to_vec();
                            let dead_shared = dead_shared.clone();
                            thread::spawn(move || {
                                let local_dead: Vec<PathBuf> = chunk
                                    .into_iter()
                                    .filter_map(|(_, _, path_str)| {
                                        let p = PathBuf::from(path_str);
                                        if !p.exists() {
                                            Some(p)
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                dead_shared.lock().unwrap().extend(local_dead);
                            })
                        })
                        .collect();

                    for (id, _, _) in &rows {
                        max_id_seen = max_id_seen.max(*id as u64);
                    }

                    for h in handles {
                        let _ = h.join();
                    }

                    if let Ok(guard) = dead_shared.lock() {
                        dead.extend(guard.iter().cloned());
                    }

                    scanned_total += rows.len();
                    cursor.store(max_id_seen, Ordering::Relaxed);

                    if dead.is_empty() {
                        continue;
                    }

                    if removed_this_round + dead.len() > STARTUP_CLEANUP_MAX_DELETE_PER_ROUND {
                        dead.truncate(STARTUP_CLEANUP_MAX_DELETE_PER_ROUND - removed_this_round);
                    }

                    {
                        let _guard = index_write_lock
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        for path in &dead {
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            if let Some(mut v) = index.get_mut(&name) {
                                v.retain(|p| p != path);
                                if v.is_empty() {
                                    drop(v);
                                    index.remove(&name);
                                }
                            }
                            db.delete(path);
                            if logger.enabled() {
                                logger.log(&format!("[-] {}", path.display()));
                            }
                        }
                    }

                    removed_this_round += dead.len();
                    removed_total += dead.len();
                }

                // Cap startup pressure; continue cleanup in background slices.
                thread::sleep(Duration::from_millis(250));
            }
        });
    }

    fn start_dirty_root_worker(&self) {
        if self
            .dirty_worker_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let this = self.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(600));

                if !this.is_watch_running() {
                    this.dirty_worker_running.store(false, Ordering::SeqCst);
                    return;
                }

                let pending = {
                    let mut guard = this
                        .dirty_roots
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    if guard.is_empty() {
                        Vec::new()
                    } else {
                        std::mem::take(&mut *guard)
                    }
                };

                if pending.is_empty() {
                    continue;
                }

                for root in collapse_dirty_roots(pending) {
                    if !root.exists() {
                        continue;
                    }
                    let root_text = root.to_string_lossy().to_string();
                    let _ = this.build_index(
                        Some(root_text),
                        false,
                        this.include_dirs.load(Ordering::Relaxed),
                        false,
                    );
                }
            }
        });
    }
}

fn default_watch_roots() -> Vec<String> {
    let roots = get_root_directories()
        .into_iter()
        .filter_map(|p| p.to_str().map(|s| s.to_string()))
        .collect::<Vec<_>>();
    if roots.is_empty() {
        vec!["/".to_string()]
    } else {
        roots
    }
}

fn normalize_watch_roots(roots: Vec<String>) -> Vec<String> {
    sanitize_roots(roots)
        .into_iter()
        .filter(|root| Path::new(root).is_dir())
        .collect()
}

fn collapse_dirty_roots(mut roots: Vec<PathBuf>) -> Vec<PathBuf> {
    roots.sort_by_key(|p| p.components().count());
    let mut out = Vec::<PathBuf>::new();
    for root in roots {
        if out.iter().any(|existing| root.starts_with(existing)) {
            continue;
        }
        out.retain(|existing| !existing.starts_with(&root));
        out.push(root);
    }
    out
}

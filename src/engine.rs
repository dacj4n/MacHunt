use crate::builder;
use crate::db::Db;
use crate::model::SearchOptions;
use crate::search;
use crate::utils::{num_cpus, Logger};
use crate::watcher;
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

#[derive(Clone)]
pub struct Engine {
    index: Arc<DashMap<String, Vec<PathBuf>>>,
    db: Db,
    logger: Logger,
    last_event_id: Arc<AtomicU64>,
    index_write_lock: Arc<Mutex<()>>,
    include_dirs: Arc<AtomicBool>,
}

impl Engine {
    pub fn new(logs_enabled: bool) -> Self {
        let db = Db::init_default();
        let index = Arc::new(DashMap::new());
        let logger = Logger::new(logs_enabled);
        let last_event_id = Arc::new(AtomicU64::new(0));
        let index_write_lock = Arc::new(Mutex::new(()));
        let include_dirs = Arc::new(AtomicBool::new(db.load_include_dirs().unwrap_or(true)));
        Self {
            index,
            db,
            logger,
            last_event_id,
            index_write_lock,
            include_dirs,
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

    pub fn build_index(&self, path: Option<String>, rebuild: bool, include_dirs: bool) -> usize {
        self.include_dirs.store(include_dirs, Ordering::Relaxed);
        self.db.save_include_dirs(include_dirs);
        let count = builder::build_index(
            &self.db,
            &self.index,
            &self.index_write_lock,
            path,
            rebuild,
            include_dirs,
        );
        let current_event_id = unsafe { watcher::FSEventsGetCurrentEventId() };
        self.db.save_last_event_id(current_event_id);
        self.db.checkpoint_truncate();
        println!(
            "Saved EventID: {}, next watch will use incremental sync",
            current_event_id
        );
        count
    }

    pub fn start_watch(&self, since_event_id: Option<u64>) {
        watcher::start_watch(
            self.index.clone(),
            self.db.clone(),
            self.logger.clone(),
            self.last_event_id.clone(),
            self.index_write_lock.clone(),
            self.include_dirs.load(Ordering::Relaxed),
            since_event_id,
        );
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
        let db = self.db.clone();
        let index = self.index.clone();
        let logger = self.logger.clone();

        thread::spawn(move || {
            let start = Instant::now();
            let paths = db.list_all_paths();
            if paths.is_empty() {
                return;
            }

            let dead_shared = Arc::new(std::sync::Mutex::new(Vec::<PathBuf>::new()));
            let chunk_size = (paths.len() / num_cpus()).max(1);
            let handles: Vec<_> = paths
                .chunks(chunk_size)
                .map(|chunk| {
                    let chunk = chunk.to_vec();
                    let dead_shared = dead_shared.clone();
                    thread::spawn(move || {
                        let local_dead: Vec<PathBuf> = chunk
                            .into_iter()
                            .filter_map(|(_, path_str)| {
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

            for h in handles {
                let _ = h.join();
            }

            let dead = Arc::try_unwrap(dead_shared)
                .ok()
                .map(|m| m.into_inner().unwrap())
                .unwrap_or_default();
            let dead_count = dead.len();
            if dead_count == 0 {
                return;
            }

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

            println!(
                "[Startup Validation] Cleaned up {} dead paths, took {:?}",
                dead_count,
                start.elapsed()
            );
        });
    }
}

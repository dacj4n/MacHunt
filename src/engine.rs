use crate::builder;
use crate::db::Db;
use crate::filters::{
    compile_exclude_rules, sanitize_owned_rules, sanitize_roots, validate_pattern_rules,
};
use crate::model::{SearchMode, SearchOptions, SortKey, VolumeEvent};
use crate::search;
use crate::utils::{get_root_directories, Logger};
use crate::watcher;
use crossbeam::channel::Sender;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

#[derive(Clone)]
pub struct Engine {
    db: Db,
    logger: Logger,
    last_event_id: Arc<AtomicU64>,
    include_dirs: Arc<AtomicBool>,
    exclude_exact_dirs: Arc<Mutex<Vec<String>>>,
    exclude_pattern_dirs: Arc<Mutex<Vec<String>>>,
    watch_roots: Arc<Mutex<Vec<String>>>,
    cleanup_running: Arc<AtomicBool>,
    volume_event_tx: Arc<Mutex<Option<Sender<VolumeEvent>>>>,
}

impl Engine {
    pub fn new(logs_enabled: bool) -> Self {
        let db = Db::init_default();
        // Ensure FTS5 index is in sync. Batch-mode avoids long locks.
        let synced = db.sync_fts_batched(50_000);
        if synced > 0 {
            println!("[sync_fts] indexed {} new rows", synced);
        }
        let logger = Logger::new(logs_enabled);
        let last_event_id = Arc::new(AtomicU64::new(0));
        let include_dirs = Arc::new(AtomicBool::new(db.load_include_dirs().unwrap_or(true)));
        let exclude_exact_dirs = Arc::new(Mutex::new(db.load_exclude_exact_dirs()));
        let exclude_pattern_dirs = Arc::new(Mutex::new(db.load_exclude_pattern_dirs()));

        let mut watch_roots = sanitize_roots(db.load_watch_roots());
        if watch_roots.is_empty() {
            watch_roots = default_watch_roots();
            db.save_watch_roots(&watch_roots);
        }

        Self {
            db,
            logger,
            last_event_id,
            include_dirs,
            exclude_exact_dirs,
            exclude_pattern_dirs,
            watch_roots: Arc::new(Mutex::new(watch_roots)),
            cleanup_running: Arc::new(AtomicBool::new(false)),
            volume_event_tx: Arc::new(Mutex::new(None)),
        }
    }

    pub fn load_index_from_db(&self) -> usize {
        if !self.db.path().exists() {
            return 0;
        }
        self.db.count_files()
    }

    /// Set the channel for volume mount/unmount/indexing events.
    /// These are forwarded to the Tauri frontend for status bar updates.
    pub fn set_volume_event_tx(&self, tx: Sender<VolumeEvent>) {
        *self.volume_event_tx.lock().unwrap() = Some(tx);
    }

    pub fn get_include_dirs(&self) -> bool {
        self.include_dirs.load(Ordering::Relaxed)
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

        let is_incremental = !auto_vacuum_on_rebuild && path.is_some();

        if !is_incremental {
            self.db.begin_rebuild();
        }

        let count = builder::build_index(&self.db, path, !is_incremental, &filters);

        if !is_incremental {
            if let Err(e) = self.db.finish_rebuild() {
                eprintln!("finish_rebuild failed: {}", e);
            }
            self.db.rebuild_fts();
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
            self.db.clone(),
            self.logger.clone(),
            self.last_event_id.clone(),
            self.include_dirs.load(Ordering::Relaxed),
            since_event_id,
            watch_roots,
            Arc::new(exclude_rules),
        );

        // Start lazy dead-path GC to compensate for removing the
        // per-search `exists()` check. Runs infrequently and at low
        // priority to avoid competing with search/watcher operations.
        self.start_lazy_gc();

        // Auto-detect newly mounted volumes (SMB, WebDAV, external disks)
        // and trigger incremental indexing when they appear.
        self.start_volume_poller();
    }

    /// Spawn a low-frequency background thread that cleans dead paths.
    /// Processes paths in batches to avoid loading millions of rows into
    /// a single Vec (which would consume 300+ MB per GC cycle).
    fn start_lazy_gc(&self) {
        let db = self.db.clone();
        let logger = self.logger.clone();
        let cleanup_running = self.cleanup_running.clone();

        thread::spawn(move || {
            thread::sleep(std::time::Duration::from_secs(600));
            loop {
                if !watcher::is_watch_running() {
                    break;
                }
                if cleanup_running
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    thread::sleep(std::time::Duration::from_secs(3600));
                    continue;
                }
                // Process in 10K-row batches — each batch ~1 MB, vs
                // 300+ MB if loaded all at once for 3M+ files.
                const BATCH: usize = 10_000;
                let mut last_id: u64 = 0;
                let mut total_removed = 0usize;
                loop {
                    let rows = db.list_paths_after_id(last_id, BATCH);
                    if rows.is_empty() {
                        break;
                    }
                    last_id = rows.last().map(|(id, _)| *id as u64).unwrap_or(0);
                    for (_, path_str) in &rows {
                        if !std::path::Path::new(path_str).exists() {
                            db.delete(std::path::Path::new(path_str));
                            if logger.enabled() {
                                logger.log(&format!("[gc] {}", path_str));
                            }
                            total_removed += 1;
                        }
                    }
                }
                if total_removed > 0 {
                    println!("[GC] removed {} dead paths", total_removed);
                }
                cleanup_running.store(false, Ordering::SeqCst);
                thread::sleep(std::time::Duration::from_secs(7200));
            }
        });
    }

    /// Periodically scan `/Volumes/` for newly mounted network/external drives
    /// and automatically trigger incremental indexing on them. FSEvents does not
    /// monitor SMB/WebDAV volumes, so this poller fills the gap.
    fn start_volume_poller(&self) {
        let engine = self.clone();
        let db = self.db.clone();
        let include_dirs = self.include_dirs.clone();
        let event_tx = self.volume_event_tx.clone();

        // Helper to get a clone of the sender (if set).
        fn get_tx(tx_arc: &Arc<Mutex<Option<Sender<VolumeEvent>>>>) -> Option<Sender<VolumeEvent>> {
            tx_arc.lock().unwrap().clone()
        }

        thread::spawn(move || {
            use std::collections::HashSet;

            let vol_root = std::path::Path::new("/Volumes");
            let mut known: HashSet<String> = HashSet::new();
            // Initialize with currently mounted volumes
            if let Ok(entries) = std::fs::read_dir(vol_root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        known.insert(path.to_string_lossy().to_string());
                    }
                }
            }

            // Poll every 10 seconds — fast enough to catch remounts,
            // slow enough to avoid I/O overhead.
            let is_external = |v: &String| -> bool {
                let p = std::path::Path::new(v);
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name != "Macintosh HD" && name != "System"
                    && !p.to_string_lossy().contains("/.timemachine")
            };

            let vol_name = |path: &str| -> String {
                std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path)
                    .to_string()
            };

            loop {
                thread::sleep(std::time::Duration::from_secs(10));

                if !watcher::is_watch_running() {
                    break;
                }

                let mut current: HashSet<String> = HashSet::new();
                if let Ok(entries) = std::fs::read_dir(vol_root) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            current.insert(path.to_string_lossy().to_string());
                        }
                    }
                }

                // ── New mounts → auto-index ──
                let new_volumes: Vec<String> = current
                    .difference(&known)
                    .filter(|&x| is_external(x))
                    .cloned()
                    .collect();

                for vol_path in &new_volumes {
                    let name = vol_name(vol_path);
                    if let Some(tx) = get_tx(&event_tx) {
                        let _ = tx.send(serde_json::json!({
                            "type": "mountDetected",
                            "path": vol_path,
                            "name": name
                        }));
                    }
                    println!(
                        "[VolumePoller] detected new mount: {} — starting background index",
                        vol_path
                    );
                    let engine_bg = engine.clone();
                    let db_bg = db.clone();
                    let vol = vol_path.clone();
                    let inc_dirs = include_dirs.load(Ordering::Relaxed);
                    let tx_bg = get_tx(&event_tx);
                    thread::spawn(move || {
                        let count = engine_bg.build_index(Some(vol.clone()), false, inc_dirs, false);
                        let total = db_bg.count_files();
                        if let Some(tx) = tx_bg {
                            let _ = tx.send(serde_json::json!({
                                "type": "indexComplete",
                                "path": vol,
                                "fileCount": count,
                                "totalIndexed": total
                            }));
                        }
                    });
                }

                // ── Unmounted volumes → delete index immediately ──
                let removed_volumes: Vec<String> = known
                    .difference(&current)
                    .filter(|&x| is_external(x))
                    .cloned()
                    .collect();

                for vol_path in &removed_volumes {
                    println!(
                        "[VolumePoller] volume unmounted: {} — removing from index",
                        vol_path
                    );
                    db.delete_under_root(std::path::Path::new(vol_path));
                    let total = db.count_files();
                    if let Some(tx) = get_tx(&event_tx) {
                        let _ = tx.send(serde_json::json!({
                            "type": "volumeRemoved",
                            "path": vol_path.clone(),
                            "name": vol_name(vol_path),
                            "totalIndexed": total
                        }));
                    }
                }

                known = current;
            }
        });
    }

    pub fn stop_watch(&self) -> bool {
        watcher::stop_watch()
    }

    pub fn is_watch_running(&self) -> bool {
        watcher::is_watch_running()
    }

    pub fn search(&self, options: SearchOptions) -> Vec<PathBuf> {
        let limit = options.limit.unwrap_or(500);

        match options.mode {
            SearchMode::Substring => self.search_substring(&options, limit),
            SearchMode::Pattern => self.search_pattern(&options, limit),
            SearchMode::Fuzzy => self.search_fuzzy(&options, limit),
        }
    }

    fn search_fuzzy(&self, options: &SearchOptions, limit: usize) -> Vec<PathBuf> {
        let query = if options.case_sensitive {
            options.query.clone()
        } else {
            options.query.to_lowercase()
        };
        if query.is_empty() {
            return Vec::new();
        }

        // Split into space-separated tokens — each must be found as a substring.
        let tokens: Vec<&str> = query.split_whitespace().collect();
        if tokens.is_empty() {
            return Vec::new();
        }
        // SQL does ALL token matching — each token becomes LIKE '%token%'.
        let token_strings: Vec<String> = if options.case_sensitive {
            tokens.iter().map(|t| t.to_string()).collect()
        } else {
            tokens.iter().map(|t| t.to_lowercase()).collect()
        };
        let candidates = self
            .db
            .search_fuzzy_candidates(&token_strings, options.path_prefix.as_deref().and_then(|p| p.to_str()), options.extensions.as_deref(), 100_000, options.include_files, options.include_dirs);

        let mut out: Vec<PathBuf> = Vec::new();
        for (dir_path, file_name) in candidates {

            let full_path = if dir_path == "/" {
                PathBuf::from(format!("/{}", file_name))
            } else {
                PathBuf::from(format!("{}/{}", dir_path, file_name))
            };
            if !prefix_allowed(&full_path, &options.path_prefix) {
                continue;
            }
            if !include_allowed(&full_path, options.include_files, options.include_dirs) {
                continue;
            }
            out.push(full_path);
        }
        // Sort by user-selected key (same as other search modes).
        let sort_k = options.sort_key;
        let sort_asc = options.sort_ascending;
        match sort_k {
            SortKey::Name => out.sort_by(|a, b| {
                let na = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let nb = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if sort_asc { na.cmp(nb) } else { nb.cmp(na) }
            }),
            SortKey::Path => out.sort_by(|a, b| {
                if sort_asc { a.cmp(b) } else { b.cmp(a) }
            }),
            SortKey::Type => out.sort_by(|a, b| {
                let ea = a.extension().and_then(|e| e.to_str()).unwrap_or("");
                let eb = b.extension().and_then(|e| e.to_str()).unwrap_or("");
                if sort_asc { ea.cmp(eb) } else { eb.cmp(ea) }
            }),
            SortKey::Size | SortKey::Modified => {
                out = self.sort_by_metadata(out, sort_k, sort_asc);
            }
        }
        out.truncate(limit);
        out
    }

    fn search_substring(&self, options: &SearchOptions, limit: usize) -> Vec<PathBuf> {
        let query = if options.case_sensitive {
            options.query.clone()
        } else {
            options.query.to_lowercase()
        };
        let needs_meta_sort = matches!(options.sort_key, SortKey::Size | SortKey::Modified);
        // Type filtering (file/dir) now happens at the SQL level via is_dir column,
        // so fetch_limit only needs to compensate for dead-path cleanup and meta-sort.
        let fetch_limit = if needs_meta_sort { limit * 3 } else { limit * 2 };
        let results = self
            .db
            .search_fts(
                &query,
                options.case_sensitive,
                options.path_prefix.as_deref().and_then(|p| p.to_str()),
                options.extensions.as_deref(),
                options.sort_key,
                options.sort_ascending,
                fetch_limit,
                options.include_files,
                options.include_dirs,
            );
        let mut out = self.build_results(results, options);
        if needs_meta_sort {
            out = self.sort_by_metadata(out, options.sort_key, options.sort_ascending);
        }
        out.truncate(limit);
        out
    }

    fn search_pattern(&self, options: &SearchOptions, limit: usize) -> Vec<PathBuf> {
        let regex = match search::convert_wildcard_to_regex(&options.query, options.case_sensitive) {
            Ok(re) => re,
            Err(_) => return Vec::new(),
        };

        // Extract a literal fragment for DB pre-filtering.
        let fragment = extract_literal(&options.query);
        let pattern = if fragment.len() >= 2 {
            if options.case_sensitive {
                format!("%{}%", fragment)
            } else {
                format!("%{}%", fragment.to_lowercase())
            }
        } else {
            "%".to_string()
        };

        let needs_meta_sort = matches!(options.sort_key, SortKey::Size | SortKey::Modified);
        // Type filtering happens at SQL level via is_dir column.
        let fetch_limit = if needs_meta_sort { limit * 3 } else { limit * 2 };

        // Use LIKE with the literal fragment to get candidates, then filter by regex.
        let results = self
            .db
            .search_like(
                &pattern,
                options.case_sensitive,
                options.path_prefix.as_deref().and_then(|p| p.to_str()),
                options.extensions.as_deref(),
                options.sort_key,
                options.sort_ascending,
                fetch_limit,
                options.include_files,
                options.include_dirs,
            );
        let mut out = Vec::new();
        for (dir_path, file_name) in results {
            let target = if options.case_sensitive {
                file_name.clone()
            } else {
                file_name.to_lowercase()
            };
            if !regex.is_match(&target) {
                continue;
            }
            let full_path = if dir_path == "/" {
                PathBuf::from(format!("/{}", file_name))
            } else {
                PathBuf::from(format!("{}/{}", dir_path, file_name))
            };
            if !prefix_allowed(&full_path, &options.path_prefix) {
                continue;
            }
            if !include_allowed(&full_path, options.include_files, options.include_dirs) {
                continue;
            }
            out.push(full_path);
        }
        if needs_meta_sort {
            out = self.sort_by_metadata(out, options.sort_key, options.sort_ascending);
        }
        out.truncate(limit);
        out
    }

    fn build_results(
        &self,
        results: Vec<(String, String)>,
        options: &SearchOptions,
    ) -> Vec<PathBuf> {
        let mut out = Vec::with_capacity(results.len());
        for (dir_path, file_name) in results {
            let full_path = if dir_path == "/" {
                PathBuf::from(format!("/{}", file_name))
            } else {
                PathBuf::from(format!("{}/{}", dir_path, file_name))
            };
            // Skip `exists()` check — stat() on every candidate is the
            // #1 search-time CPU cost. Dead paths are cleaned by the
            // watcher (rename/remove events) and periodic GC instead.
            if !prefix_allowed(&full_path, &options.path_prefix) {
                continue;
            }
            if !include_allowed(&full_path, options.include_files, options.include_dirs) {
                continue;
            }
            out.push(full_path);
        }
        out
    }

    /// Re-sort results by filesystem metadata (size or modified time).
    /// Called after SQL fetch when sort_key is Size or Modified.
    fn sort_by_metadata(
        &self,
        paths: Vec<PathBuf>,
        sort_key: SortKey,
        ascending: bool,
    ) -> Vec<PathBuf> {
        let mut with_meta: Vec<(PathBuf, u64)> = paths
            .into_iter()
            .map(|p| {
                let val = std::fs::metadata(&p)
                    .ok()
                    .and_then(|meta| match sort_key {
                        SortKey::Size => {
                            if meta.is_file() {
                                Some(meta.len())
                            } else {
                                Some(0)
                            }
                        }
                        SortKey::Modified => meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .and_then(|d| u64::try_from(d.as_millis()).ok()),
                        _ => Some(0),
                    })
                    .unwrap_or(0); // keep path even if metadata unavailable (e.g. unmounted network drive)
                (p, val)
            })
            .collect();
        with_meta.sort_by(|a, b| {
            let cmp = a.1.cmp(&b.1);
            if ascending { cmp } else { cmp.reverse() }
        });
        with_meta.into_iter().map(|(p, _)| p).collect()
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
        let logger = self.logger.clone();
        let running = self.cleanup_running.clone();

        thread::spawn(move || {
            let start = Instant::now();
            let rows = db.list_all_paths();
            let mut removed = 0usize;

            let dead: Vec<PathBuf> = rows
                .into_iter()
                .filter_map(|(_, path_str)| {
                    let p = PathBuf::from(path_str);
                    if !p.exists() { Some(p) } else { None }
                })
                .collect();

            for path in &dead {
                db.delete(path.as_path());
                if logger.enabled() {
                    logger.log(&format!("[-] {}", path.display()));
                }
                removed += 1;
            }

            if removed > 0 {
                println!(
                    "[Startup Validation] Cleaned up {} dead paths, took {:?}",
                    removed,
                    start.elapsed()
                );
            }
            running.store(false, Ordering::SeqCst);
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

fn prefix_allowed(path: &Path, prefix: &Option<PathBuf>) -> bool {
    match prefix {
        Some(p) => path.starts_with(p),
        None => true,
    }
}

/// Extract the longest literal (non-wildcard) fragment from a pattern for DB pre-filtering.
fn extract_literal(pattern: &str) -> String {
    let mut best = String::new();
    let mut current = String::new();
    for ch in pattern.chars() {
        match ch {
            '*' | '?' | '{' | '}' | ',' | '[' | ']' | '\\' => {
                if current.len() > best.len() {
                    best = current.clone();
                }
                current.clear();
            }
            c => current.push(c),
        }
    }
    if current.len() > best.len() {
        best = current;
    }
    best
}

fn include_allowed(path: &Path, include_files: bool, include_dirs: bool) -> bool {
    if include_files && include_dirs {
        return true;
    }
    if include_files {
        return path.is_file();
    }
    if include_dirs {
        return path.is_dir();
    }
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_fuzzy_token_match() {
        // Simulate the fuzzy search token-matching logic.
        let filename = "AgentSyste代理商管理系统 Struts2 远程代码执行漏洞";
        let query = "系统 Struts2 远程";
        let lowered_name = filename.to_lowercase();
        let lowered_query = query.to_lowercase();
        let tokens: Vec<&str> = lowered_query.split_whitespace().collect();
        let all_match = tokens.iter().all(|t| lowered_name.contains(t));
        assert!(all_match, "tokens {:?} should match '{}'", tokens, lowered_name);

        let query2 = "代理 执行 struts2";
        let lowered_query2 = query2.to_lowercase();
        let tokens2: Vec<&str> = lowered_query2.split_whitespace().collect();
        let all_match2 = tokens2.iter().all(|t| lowered_name.contains(t));
        assert!(all_match2, "tokens {:?} should match '{}'", tokens2, lowered_name);

        // Verify DB pre-filter: "struts2" first 2 chars = "st", LIKE '%st%' should match.
        let prefilter = tokens2.iter().max_by_key(|t| t.chars().count()).unwrap();
        assert_eq!(*prefilter, "struts2");
        let prefix: String = prefilter.chars().take(2).collect();
        assert_eq!(prefix, "st");
        assert!(lowered_name.contains(&prefix), "LIKE '%{}%' should match '{}'", prefix, lowered_name);

        // Test that non-matching query is rejected.
        let query3 = "不存在 关键词";
        let lowered_query3 = query3.to_lowercase();
        let tokens3: Vec<&str> = lowered_query3.split_whitespace().collect();
        let all_match3 = tokens3.iter().all(|t| lowered_name.contains(t));
        assert!(!all_match3);
    }
}


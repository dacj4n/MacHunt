use crate::builder;
use crate::db::Db;
use crate::filters::{
    compile_exclude_rules, sanitize_owned_rules, sanitize_roots, validate_pattern_rules,
};
use crate::model::{SearchMode, SearchOptions, SortKey};
use crate::search;
use crate::utils::{get_root_directories, Logger};
use crate::watcher;
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
}

impl Engine {
    pub fn new(logs_enabled: bool) -> Self {
        let db = Db::init_default();
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
        }
    }

    pub fn load_index_from_db(&self) -> usize {
        if !self.db.path().exists() {
            return 0;
        }
        self.db.count_files()
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

        // Use broad LIKE to get candidates, then filter by edit distance.
        // Candidate pre-filter always uses lowered prefix because f.name_lower is lowered.
        let query_lower = query.to_lowercase();
        let candidates = self
            .db
            .search_fuzzy_candidates(&query_lower, options.path_prefix.as_deref().and_then(|p| p.to_str()), options.extensions.as_deref(), 2000);
        let q_len = query.chars().count();
        let mut scored: Vec<(PathBuf, usize)> = Vec::new();

        for (dir_path, file_name) in candidates {
            let name_cmp = if options.case_sensitive {
                file_name.clone()
            } else {
                file_name.to_lowercase()
            };

            // Fast length pre-filter: skip names too far from query length.
            let n_len = name_cmp.chars().count();
            if n_len.abs_diff(q_len) > 3 {
                continue;
            }

            let dist = levenshtein(&name_cmp, &query);
            // Allow up to (query_len / 3) + 1 edits; minimum tolerance of 1 for short queries.
            let max_dist = (query.len() / 3).max(1);
            if dist > max_dist {
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
            scored.push((full_path, dist));
            if scored.len() >= limit * 3 {
                break;
            }
        }

        // Sort by edit distance (best match first), then by path length.
        scored.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| {
            a.0.as_os_str().len().cmp(&b.0.as_os_str().len())
        }));
        scored.truncate(limit);
        scored.into_iter().map(|(p, _)| p).collect()
    }

    fn search_substring(&self, options: &SearchOptions, limit: usize) -> Vec<PathBuf> {
        let query = if options.case_sensitive {
            options.query.clone()
        } else {
            options.query.to_lowercase()
        };
        let needs_meta_sort = matches!(options.sort_key, SortKey::Size | SortKey::Modified);
        // Fetch more rows than the display limit to compensate for build_results
        // filtering out non-existent paths and non-matching types (file vs dir).
        // Need even more for size/modified since SQL can't sort those.
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
        // For size/modified sorts fetch more since SQL can't sort those.
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
            if out.len() >= fetch_limit {
                break;
            }
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
            .filter_map(|p| {
                let meta = std::fs::metadata(&p).ok()?;
                let val = match sort_key {
                    SortKey::Size => {
                        if meta.is_file() {
                            meta.len()
                        } else {
                            0
                        }
                    }
                    SortKey::Modified => meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0),
                    _ => 0,
                };
                Some((p, val))
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

/// Levenshtein (edit) distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();
    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0usize; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1)        // deletion
                .min(curr[j] + 1)                    // insertion
                .min(prev[j] + cost);                // substitution
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_len]
}

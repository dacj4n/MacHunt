use crate::db::Db;
use crate::utils::{get_root_directories, should_skip_path};
use crossbeam::channel::Sender;
use dashmap::DashMap;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use walkdir::WalkDir;

const BATCH_SIZE: usize = 50_000;

#[derive(Clone, Debug)]
pub struct BuildFilterSettings {
    pub include_dirs: bool,
    pub exclude_exact_dirs: Vec<String>,
    pub exclude_pattern_dirs: Vec<String>,
}

struct ExcludeRules {
    exact_dirs: Vec<PathBuf>,
    regex_dirs: Vec<Regex>,
}

impl ExcludeRules {
    fn compile(exact_dirs: &[String], regex_dirs: &[String]) -> Self {
        let exact_dirs = sanitize_rules(exact_dirs)
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();

        let mut compiled_regex = Vec::new();
        for raw in sanitize_rules(regex_dirs) {
            if let Ok(re) = compile_regex_rule(&raw) {
                compiled_regex.push(re);
            }
        }

        Self {
            exact_dirs,
            regex_dirs: compiled_regex,
        }
    }
}

fn sanitize_rules(values: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for value in values {
        let normalized = value.trim();
        if normalized.is_empty() {
            continue;
        }
        if out.iter().any(|existing| existing == normalized) {
            continue;
        }
        out.push(normalized.to_string());
    }
    out
}

fn wildcard_to_regex(pattern: &str) -> Result<Regex, regex::Error> {
    let mut regex_pattern = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        match chars[i] {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    regex_pattern.push_str(".*");
                    i += 2;
                } else {
                    regex_pattern.push_str(".*");
                    i += 1;
                }
            }
            '?' => {
                regex_pattern.push('.');
                i += 1;
            }
            c => {
                regex_pattern.push_str(&regex::escape(&c.to_string()));
                i += 1;
            }
        }
    }

    Regex::new(&format!("(?i)^{}$", regex_pattern))
}

fn compile_regex_rule(pattern: &str) -> Result<Regex, String> {
    Regex::new(pattern)
        .or_else(|_| wildcard_to_regex(pattern))
        .map_err(|err| err.to_string())
}

fn to_matchable_path(path: &Path, is_dir: bool) -> String {
    let mut s = path.to_string_lossy().to_string();
    if is_dir && !s.ends_with('/') {
        s.push('/');
    }
    s
}

fn is_excluded(path: &Path, is_dir: bool, rules: &ExcludeRules) -> bool {
    if rules.exact_dirs.iter().any(|dir| path.starts_with(dir)) {
        return true;
    }

    if rules.regex_dirs.is_empty() {
        return false;
    }

    let path_text = to_matchable_path(path, is_dir);
    rules.regex_dirs.iter().any(|re| re.is_match(&path_text))
}

fn scan_root(
    root: PathBuf,
    tx: Sender<Vec<(String, PathBuf)>>,
    include_dirs: bool,
    exclude_rules: Arc<ExcludeRules>,
) {
    let mut batch = Vec::with_capacity(BATCH_SIZE);

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| {
            let path = e.path();
            let is_dir = e.file_type().is_dir();
            !should_skip_path(path) && !is_excluded(path, is_dir, &exclude_rules)
        })
        .filter_map(|e| e.ok())
    {
        let file_type = entry.file_type();
        if is_excluded(entry.path(), file_type.is_dir(), &exclude_rules) {
            continue;
        }
        if !(file_type.is_file() || include_dirs && file_type.is_dir()) {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            batch.push((name.to_lowercase(), entry.into_path()));
            if batch.len() >= BATCH_SIZE {
                let _ = tx.send(std::mem::replace(
                    &mut batch,
                    Vec::with_capacity(BATCH_SIZE),
                ));
            }
        }
    }

    if !batch.is_empty() {
        let _ = tx.send(batch);
    }
}

fn apply_batch_to_index(
    index: &Arc<DashMap<String, Vec<PathBuf>>>,
    entries: &[(String, PathBuf)],
    dedupe: bool,
) {
    for (name, path) in entries {
        let mut bucket = index.entry(name.clone()).or_default();
        if !dedupe || !bucket.iter().any(|existing| existing == path) {
            bucket.push(path.clone());
        }
    }
}

pub fn build_index(
    db: &Db,
    index: &Arc<DashMap<String, Vec<PathBuf>>>,
    index_write_lock: &Arc<Mutex<()>>,
    path: Option<String>,
    rebuild: bool,
    filters: &BuildFilterSettings,
) -> usize {
    if rebuild {
        let _guard = index_write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        db.clear_files();
        index.clear();
    }

    let start = Instant::now();
    println!("Building file index...");
    let use_incremental_memory_update = path.is_some();

    let roots = if let Some(p) = path {
        vec![PathBuf::from(p)]
    } else {
        get_root_directories()
    };
    let exclude_rules = Arc::new(ExcludeRules::compile(
        &filters.exclude_exact_dirs,
        &filters.exclude_pattern_dirs,
    ));
    let include_dirs = filters.include_dirs;

    let (tx, rx) = crossbeam::channel::bounded::<Vec<(String, PathBuf)>>(256);

    let handles: Vec<_> = roots
        .into_iter()
        .map(|root| {
            let tx = tx.clone();
            let exclude_rules = exclude_rules.clone();
            thread::spawn(move || scan_root(root, tx, include_dirs, exclude_rules))
        })
        .collect();
    drop(tx);

    let mut db_batch: Vec<(String, PathBuf)> = Vec::with_capacity(BATCH_SIZE);
    let mut count = 0usize;

    for chunk in rx {
        count += chunk.len();
        db_batch.extend(chunk);

        if db_batch.len() >= BATCH_SIZE {
            db.insert_batch(&db_batch);
            if use_incremental_memory_update {
                let _guard = index_write_lock
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                apply_batch_to_index(index, &db_batch, !rebuild);
            }
            db_batch.clear();
        }

        if count % 500_000 < BATCH_SIZE {
            println!("Scanned {} files, took {:?}", count, start.elapsed());
        }
    }
    if !db_batch.is_empty() {
        db.insert_batch(&db_batch);
        if use_incremental_memory_update {
            let _guard = index_write_lock
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            apply_batch_to_index(index, &db_batch, !rebuild);
        }
    }

    for h in handles {
        let _ = h.join();
    }

    println!("Scan + DB write took: {:?}", start.elapsed());

    if use_incremental_memory_update {
        println!("Path-scoped build: in-memory index updated incrementally; skipped full reload");
    } else {
        let t_mem = Instant::now();
        let _guard = index_write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let loaded = db.load_index(index);
        println!(
            "Write to memory took: {:?} ({} records)",
            t_mem.elapsed(),
            loaded
        );
    }

    println!(
        "Index build completed, {} files total, took {:?}",
        count,
        start.elapsed()
    );
    count
}

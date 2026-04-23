use crate::db::Db;
use crate::filters::{compile_exclude_rules, is_excluded, ExcludeRules};
use crate::utils::{get_root_directories, normalize_path_for_index, should_skip_path};
use crossbeam::channel::Sender;
use dashmap::DashMap;
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
    pub watch_roots: Option<Vec<String>>,
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
            let normalized_path = normalize_path_for_index(entry.path());
            batch.push((name.to_lowercase(), normalized_path));
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
    if !rebuild {
        if let Some(ref p) = path {
            let root = PathBuf::from(p);
            if root.exists() {
                db.delete_under_root(root.as_path());
                let _guard = index_write_lock
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                purge_index_under_root(index, root.as_path());
            }
        }
    }

    if rebuild {
        let _guard = index_write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        db.clear_files();
        index.clear();
    }

    let start = Instant::now();
    println!("Building file index...");
    let roots = if let Some(p) = path {
        vec![PathBuf::from(p)]
    } else if let Some(watch_roots) = &filters.watch_roots {
        let configured: Vec<PathBuf> = watch_roots.iter().map(PathBuf::from).collect();
        if configured.is_empty() {
            get_root_directories()
        } else {
            configured
        }
    } else {
        get_root_directories()
    };
    let exclude_rules = Arc::new(compile_exclude_rules(
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
            let _guard = index_write_lock
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            apply_batch_to_index(index, &db_batch, !rebuild);
            db_batch.clear();
        }

        if count % 500_000 < BATCH_SIZE {
            println!("Scanned {} files, took {:?}", count, start.elapsed());
        }
    }
    if !db_batch.is_empty() {
        db.insert_batch(&db_batch);
        let _guard = index_write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        apply_batch_to_index(index, &db_batch, !rebuild);
    }

    for h in handles {
        let _ = h.join();
    }

    println!("Scan + DB write took: {:?}", start.elapsed());

    println!("In-memory index updated incrementally during scan");

    println!(
        "Index build completed, {} files total, took {:?}",
        count,
        start.elapsed()
    );
    count
}

fn purge_index_under_root(index: &Arc<DashMap<String, Vec<PathBuf>>>, root: &Path) {
    if root == Path::new("/") {
        index.clear();
        return;
    }
    let root_text = root.to_string_lossy();
    let prefix = format!("{}/", root_text);
    index.retain(|_, v| {
        v.retain(|p| {
            let text = p.to_string_lossy();
            p.as_path() != root && !text.starts_with(&prefix)
        });
        !v.is_empty()
    });
}

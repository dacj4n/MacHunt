use crate::db::Db;
use crate::filters::{compile_exclude_rules, is_excluded, ExcludeRules};
use crate::utils::{get_root_directories, normalize_path_for_index, should_skip_path};
use crossbeam::channel::Sender;
use std::path::PathBuf;
use std::sync::Arc;
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
            let normalized = normalize_path_for_index(entry.path());
            batch.push((name.to_lowercase(), normalized));
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

pub fn build_index(
    db: &Db,
    path: Option<String>,
    rebuild: bool,
    filters: &BuildFilterSettings,
) -> usize {
    if !rebuild {
        if let Some(ref p) = path {
            let root = PathBuf::from(p);
            if root.exists() {
                db.delete_under_root(root.as_path());
            }
        }
    }

    if rebuild {
        db.clear_files();
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
            db_batch.clear();
        }

        if count % 500_000 < BATCH_SIZE {
            println!("Scanned {} files, took {:?}", count, start.elapsed());
        }
    }
    if !db_batch.is_empty() {
        db.insert_batch(&db_batch);
    }

    for h in handles {
        let _ = h.join();
    }

    println!("Build completed, {} files total, took {:?}", count, start.elapsed());
    count
}

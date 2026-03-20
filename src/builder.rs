use crate::db::Db;
use crate::utils::{get_root_directories, should_skip_path};
use crossbeam::channel::Sender;
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use walkdir::WalkDir;

const BATCH_SIZE: usize = 50_000;

fn scan_root(root: PathBuf, tx: Sender<Vec<(String, PathBuf)>>, include_dirs: bool) {
    let mut batch = Vec::with_capacity(BATCH_SIZE);

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| !should_skip_path(e.path()))
        .filter_map(|e| e.ok())
    {
        let file_type = entry.file_type();
        if !file_type.is_file() && !(include_dirs && file_type.is_dir()) {
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
    include_dirs: bool,
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

    let (tx, rx) = crossbeam::channel::bounded::<Vec<(String, PathBuf)>>(256);

    let handles: Vec<_> = roots
        .into_iter()
        .map(|root| {
            let tx = tx.clone();
            thread::spawn(move || scan_root(root, tx, include_dirs))
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
        println!(
            "Path-scoped build: in-memory index updated incrementally; skipped full reload"
        );
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

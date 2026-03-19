use crate::db::Db;
use crate::utils::{get_root_directories, should_skip_path};
use crossbeam::channel::Sender;
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use walkdir::WalkDir;

const BATCH_SIZE: usize = 50_000;

fn scan_root(root: PathBuf, tx: Sender<Vec<(String, PathBuf)>>) {
    let mut batch = Vec::with_capacity(BATCH_SIZE);

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| !should_skip_path(e.path()))
        .filter_map(|e| e.ok())
    {
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

pub fn build_index(
    db: &Db,
    index: &Arc<DashMap<String, Vec<PathBuf>>>,
    path: Option<String>,
    rebuild: bool,
) -> usize {
    if rebuild {
        db.clear_files();
        index.clear();
    }

    let start = Instant::now();
    println!("Building file index...");

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
            thread::spawn(move || scan_root(root, tx))
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

    println!("Scan + DB write took: {:?}", start.elapsed());

    let t_mem = Instant::now();
    let loaded = db.load_index(index);
    println!(
        "Write to memory took: {:?} ({} records)",
        t_mem.elapsed(),
        loaded
    );

    println!(
        "Index build completed, {} files total, took {:?}",
        count,
        start.elapsed()
    );
    count
}

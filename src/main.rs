use clap::{Parser, Subcommand};
use dashmap::DashMap;
use fuzzy_matcher::FuzzyMatcher;
use notify::{EventKind, Watcher};
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{Write, BufWriter, BufReader};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use walkdir::WalkDir;
use rusqlite::{Connection, params};
use parking_lot::Mutex;

static LOG_FILE: OnceCell<String> = OnceCell::new();
static LOG_ENABLED: OnceCell<bool> = OnceCell::new();

static DB_PATH: OnceCell<PathBuf> = OnceCell::new();
static DB_CONN: OnceCell<Arc<Mutex<Connection>>> = OnceCell::new();

#[allow(dead_code)]
static LAST_EVENT_ID: std::sync::atomic::AtomicU64 = 
    std::sync::atomic::AtomicU64::new(0);

fn set_db_path() {
    let data_dir = std::env::current_dir().unwrap().join("data");
    fs::create_dir_all(&data_dir).ok();
    DB_PATH.set(data_dir.join("index.db")).unwrap();
}

fn init_db() {
    let path = DB_PATH.get().unwrap();
    let conn = Connection::open(path).unwrap();

    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA cache_size=-65536;
        PRAGMA temp_store=MEMORY;
        PRAGMA mmap_size=268435456;
        CREATE TABLE IF NOT EXISTS files (
            id   INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE
        );
        CREATE INDEX IF NOT EXISTS idx_name ON files(name);

        CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
    ").unwrap();

    DB_CONN.set(Arc::new(Mutex::new(conn))).unwrap();
}

fn db_create_index() {
    let conn = get_db().lock();
    println!("建立索引...");
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_name ON files(name);"
    ).unwrap();
}

fn get_db() -> &'static Arc<Mutex<Connection>> {
    DB_CONN.get().unwrap()
}

fn db_insert(name: &str, path: &PathBuf) {
    let conn = get_db().lock();
    conn.execute(
        "INSERT OR IGNORE INTO files (name, path) VALUES (?1, ?2)",
        params![name, path.to_string_lossy().as_ref()],
    ).ok();
}

fn db_delete(path: &PathBuf) {
    let conn = get_db().lock();
    conn.execute(
        "DELETE FROM files WHERE path = ?1",
        params![path.to_string_lossy().as_ref()],
    ).ok();
}

fn db_insert_batch(entries: &[(String, PathBuf)]) {
      if entries.is_empty() { return; }
      
      let mut conn = get_db().lock();
      
      conn.execute_batch("PRAGMA synchronous=OFF;").ok();
      
      let tx = conn.transaction().unwrap();
      {
          let mut stmt = tx.prepare_cached(
              "INSERT OR IGNORE INTO files (name, path) VALUES (?1, ?2)"
          ).unwrap();
          for (name, path) in entries {
              stmt.execute(params![name, path.to_string_lossy().as_ref()]).ok();
          }
      }
      tx.commit().unwrap();
      
      conn.execute_batch("PRAGMA synchronous=NORMAL;").ok();
  }

  #[allow(dead_code)]
  fn save_last_event_id(event_id: u64) {
      let conn = get_db().lock();
      conn.execute(
          "INSERT OR REPLACE INTO meta (key, value) VALUES ('last_event_id', ?1)",
          params![event_id.to_string()],
      ).ok();
  }

  #[allow(dead_code)]
  fn load_last_event_id() -> Option<u64> {
      let conn = get_db().lock();
      conn.query_row(
          "SELECT value FROM meta WHERE key = 'last_event_id'",
          [],
          |row| row.get::<_, String>(0),
      ).ok()?.parse().ok()
  }

  #[derive(Serialize, Deserialize)]
  #[allow(dead_code)]
  struct IndexData {
      files: HashMap<String, Vec<String>>,
  }

  #[derive(Parser)]
#[command(name = "mac_find")]
#[command(about = "macOS 全局文件搜索工具，类似 Everything")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, default_value = ".")]
    path: String,

    #[arg(short, long)]
    regex: bool,

    #[arg(short, long)]
    fuzzy: bool,
    
    #[arg(long)]
    logs: bool,
}

#[derive(Subcommand)]
enum Commands {
    Build {
        #[arg(short, long)]
        path: Option<String>,
    },
    Watch,
}

static FILE_INDEX: OnceCell<Arc<DashMap<String, Vec<PathBuf>>>> = OnceCell::new();
  static INDEX_PATH: OnceCell<PathBuf> = OnceCell::new();

  #[allow(dead_code)]
  fn get_index_path() -> &'static PathBuf {
      INDEX_PATH.get().unwrap()
  }

  #[allow(dead_code)]
  fn init_index() {
      let index = Arc::new(DashMap::new());
      FILE_INDEX.set(index).unwrap();
  }

  #[allow(dead_code)]
  fn set_index_path() {
      let current_dir = std::env::current_dir().unwrap();
      let data_dir = current_dir.join("data");
      
      if !data_dir.exists() {
          fs::create_dir_all(&data_dir).ok();
      }
      
      let path = data_dir.join("index.json");
      INDEX_PATH.set(path).unwrap();
  }

  fn load_index_from_db() -> bool {
      let path = DB_PATH.get().unwrap();
      if !path.exists() {
          return false;
      }
      let start = Instant::now();
      println!("从数据库加载索引...");

      let conn = get_db().lock();
      let mut stmt = conn
          .prepare("SELECT name, path FROM files")
          .unwrap();

      let index = get_index();
      let mut count = 0usize;

      let rows = stmt.query_map([], |row| {
          Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
      }).unwrap();

      for row in rows {
          let (name, path_str) = row.unwrap();
          index.entry(name).or_default().push(PathBuf::from(path_str));
          count += 1;
      }

      println!("加载完成，共 {} 条记录，耗时 {:?}", count, start.elapsed());
      count > 0
  }

  fn get_index() -> &'static Arc<DashMap<String, Vec<PathBuf>>> {
    FILE_INDEX.get().unwrap()
}

#[allow(dead_code)]
  fn load_index() -> bool {
    let index_path = get_index_path();
    if !index_path.exists() {
        return false;
    }

    let start = Instant::now();
    println!("正在从文件加载索引...");

    match File::open(index_path) {
        Ok(file) => {
            let reader = BufReader::new(file);
            let index_data: IndexData = match serde_json::from_reader(reader) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("加载索引失败: {}", e);
                    return false;
                }
            };

            let index = get_index();
            for (file_name, paths) in index_data.files {
                let path_vec: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
                index.insert(file_name, path_vec);
            }

            let duration = start.elapsed();
            println!("索引加载完成，耗时 {:?}", duration);
            true
        }
        Err(e) => {
            eprintln!("打开索引文件失败: {}", e);
            false
        }
    }
}

const BATCH_SIZE: usize = 50000;

fn get_timestamp() -> String {
    let now = SystemTime::now();
    let since_epoch = now.duration_since(UNIX_EPOCH).unwrap();
    let timestamp = since_epoch.as_secs();
    timestamp.to_string()
}

fn init_logging(enabled: bool) {
    if enabled {
        let log_file = format!("logs/mac_find_{}.log", get_timestamp());
        LOG_FILE.set(log_file).unwrap();
        LOG_ENABLED.set(true).unwrap();
    } else {
        LOG_ENABLED.set(false).unwrap();
    }
}

fn log_message(message: &str) {
    if *LOG_ENABLED.get().unwrap_or(&false) {
        if let Some(log_file) = LOG_FILE.get() {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file)
                .unwrap();
            
            let mut writer = BufWriter::new(file);
            writeln!(writer, "{}", message).unwrap();
        }
    }
}

fn get_root_directories() -> Vec<PathBuf> {
    let root = PathBuf::from("/");
    let mut dirs = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let path_str = path.to_string_lossy();
                if !matches!(
                    path_str.as_ref(),
                    "/dev" | "/proc" | "/sys"
                    | "/private/var/vm"
                    | "/private/var/run"
                    | "/private/var/folders"
                    | "/System/Volumes/Data"
                    | "/System/Volumes/Preboot"
                    | "/System/Volumes/Recovery"
                    | "/System/Volumes/VM"
                ) && !path_str.contains("/.Spotlight-V100")
                && !path_str.contains("/.fseventsd")
                {
                    dirs.push(path);
                }
            }
        }
    }
    
    dirs
}

fn scan_root(root: PathBuf, tx: crossbeam::channel::Sender<Vec<(String, PathBuf)>>) {
    let mut batch = Vec::with_capacity(BATCH_SIZE);

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| {
            let p = e.path();
            let path_str = p.to_str().unwrap_or("");
            !matches!(
                path_str,
                "/dev" | "/proc" | "/sys"
                | "/private/var/vm"
                | "/private/var/run"
                | "/private/var/folders"
                | "/System/Volumes/Data"
                | "/System/Volumes/Preboot"
                | "/System/Volumes/Recovery"
                | "/System/Volumes/VM"
            ) && !path_str.contains("/.Spotlight-V100")
            && !path_str.contains("/.fseventsd")
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if let Some(name) = entry.file_name().to_str() {
            batch.push((name.to_lowercase(), entry.into_path()));
            if batch.len() >= BATCH_SIZE {
                tx.send(std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE)))
                    .ok();
            }
        }
    }
    if !batch.is_empty() {
        tx.send(batch).ok();
    }
}

fn build_index() {
    let start = Instant::now();
    println!("开始构建文件索引...");

    let t1 = Instant::now();
    let index = get_index().clone();
    let (tx, rx) = crossbeam::channel::bounded::<Vec<(String, PathBuf)>>(256);

    let roots = get_root_directories();

    let handles: Vec<_> = roots
        .into_iter()
        .map(|root| {
            let tx = tx.clone();
            thread::spawn(move || scan_root(root, tx))
        })
        .collect();

    drop(tx);

    let mut local_map: std::collections::HashMap<String, Vec<PathBuf>> = std::collections::HashMap::with_capacity(500_000);
    let mut count = 0usize;

    for batch in rx {
        count += batch.len();
        for (name, path) in batch {
            local_map.entry(name).or_default().push(path);
        }
        if count % 500_000 < BATCH_SIZE {
            println!(
                "已扫描 {} 个文件，耗时 {:?}",
                count,
                start.elapsed()
            );
        }
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("扫描耗时: {:?}", t1.elapsed());

    let t2 = Instant::now();
    for (k, v) in local_map {
        index.insert(k.clone(), v.clone());
        db_insert_batch(&v.into_iter().map(|p| (k.clone(), p)).collect::<Vec<_>>());
    }
    println!("写内存耗时: {:?}", t2.elapsed());

    let t3 = Instant::now();
    db_create_index();
    println!("建索引耗时: {:?}", t3.elapsed());

    let duration = start.elapsed();
    let total_files: usize = index.iter().map(|r| r.value().len()).sum();
    println!("索引构建完成，共索引 {} 个文件，总耗时 {:?}", total_files, duration);
}

fn search_substring(index: &Arc<DashMap<String, Vec<PathBuf>>>, query: &str) -> Vec<PathBuf> {
    let mut results = vec![];
    let query_lower = query.to_lowercase();
    
    for r in index.iter() {
        let (file_name, paths) = r.pair();
        if file_name.contains(&query_lower) {
            results.extend(paths.clone());
        }
    }
    
    results
}

fn search_regex(index: &Arc<DashMap<String, Vec<PathBuf>>>, pattern: &str) -> Vec<PathBuf> {
    let mut results = vec![];
    
    let regex = match Regex::new(pattern) {
        Ok(re) => re,
        Err(e) => {
            eprintln!("正则表达式错误: {}", e);
            return results;
        }
    };

    for r in index.iter() {
        let (file_name, paths) = r.pair();
        if regex.is_match(file_name) {
            results.extend(paths.clone());
        }
    }
    
    results
}

fn search_fuzzy(index: &Arc<DashMap<String, Vec<PathBuf>>>, query: &str) -> Vec<PathBuf> {
    let mut results = vec![];
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
    
    for r in index.iter() {
        let (file_name, paths) = r.pair();
        if matcher.fuzzy_match(file_name, query).is_some() {
            results.extend(paths.clone());
        }
    }
    
    results
}

fn search_files(query: &str, use_regex: bool, use_fuzzy: bool) {
    let start = Instant::now();
    let index = get_index();

    let results = if use_regex {
        search_regex(index, query)
    } else if use_fuzzy {
        search_fuzzy(index, query)
    } else {
        search_substring(index, query)
    };

    let duration = start.elapsed();
    println!("搜索完成，找到 {} 个匹配文件，耗时 {:?}", results.len(), duration);

    for path in results {
        println!("{}", path.display());
    }
}

fn watch_changes() -> notify::RecommendedWatcher {
    let index = get_index().clone();
    let (tx, rx) = crossbeam::channel::unbounded::<notify::Result<notify::Event>>();

    let mut watcher = notify::recommended_watcher(move |event| {
        tx.send(event).unwrap();
    })
    .unwrap();

    let watch_roots = vec!["/"];
    for root in watch_roots {
        let path = std::path::Path::new(root);
        if path.exists() {
            match watcher.watch(path, notify::RecursiveMode::Recursive) {
                Ok(_) => println!("已开始监听: {}", root),
                Err(e) => eprintln!("监听 {} 失败: {:?}", root, e),
            }
        }
    }

    thread::spawn(move || {
        for event in rx {
            let e = match event {
                Ok(event) => event,
                Err(e) => {
                    eprintln!("监听错误: {:?}", e);
                    continue;
                }
            };

            for path in &e.paths {
                let file_name_lower = match path.file_name().and_then(|n| n.to_str()) {
                    Some(name) => name.to_lowercase(),
                    None => continue,
                };

                match e.kind {
                    EventKind::Create(_) => {
                        if path.is_file() {
                            let file_name_lower_clone = file_name_lower.clone();
                            let path_clone = path.clone();
                            let mut entry = index
                                .entry(file_name_lower)
                                .or_insert_with(Vec::new);
                            if !entry.contains(&path) {
                                entry.push(path.clone());
                                db_insert(&file_name_lower_clone, &path_clone);
                                if *LOG_ENABLED.get().unwrap_or(&false) {
                                    log_message(&format!("[+] 新增: {}", path.display()));
                                }
                            }
                        }
                    }

                    EventKind::Remove(_) => {
                        let path_clone = path.clone();
                        if let Some(mut paths) = index.get_mut(&file_name_lower) {
                            paths.retain(|p| p != path);
                            db_delete(&path_clone);
                            if *LOG_ENABLED.get().unwrap_or(&false) {
                                log_message(&format!("[-] 删除: {}", path.display()));
                            }
                            if paths.is_empty() {
                                drop(paths);
                                index.remove(&file_name_lower);
                            }
                        }
                    }

                    EventKind::Modify(_) | EventKind::Any => {
                        let path_clone = path.clone();
                        let file_name_lower_clone = file_name_lower.clone();
                        if path.is_file() {
                            let mut entry = index.entry(file_name_lower).or_default();
                            if !entry.contains(&path) {
                                entry.push(path.clone());
                                db_insert(&file_name_lower_clone, &path_clone);
                                if *LOG_ENABLED.get().unwrap_or(&false) {
                                    log_message(&format!("[~] 变更/重命名到: {}", path.display()));
                                }
                            }
                        } else {
                            if let Some(mut paths) = index.get_mut(&file_name_lower) {
                                paths.retain(|p| p != path);
                                db_delete(&path_clone);
                                if *LOG_ENABLED.get().unwrap_or(&false) {
                                    log_message(&format!("[~] 变更/重命名离开: {}", path.display()));
                                }
                                if paths.is_empty() {
                                    drop(paths);
                                    index.remove(&file_name_lower);
                                }
                            }
                        }
                    }

                    _ => {}
                }
            }
        }
    });

    watcher
}

fn real_time_search() {
    println!("实时搜索模式，输入搜索词（按Ctrl+C退出）:");

    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        search_files(input, false, false);
    }
}

fn main() {
    set_index_path();
    set_db_path();
    init_db();
    init_index();

    let cli = Cli::parse();
    init_logging(cli.logs);

    match cli.command {
        Some(Commands::Build { path }) => {
            if path.is_some() {
                println!("使用指定路径构建索引...");
            }
            build_index();
        }
        Some(Commands::Watch) => {
            let has_index = load_index_from_db();
            let _watcher = watch_changes();
            
            if !has_index {
                println!("首次运行，后台构建索引中，可先搜索（结果可能不完整）...");
                thread::spawn(|| build_index());
            }
            
            real_time_search();
        }
        None => {
            if !load_index_from_db() {
                println!("未找到索引文件，开始构建索引...");
                build_index();
            }
            search_files(&cli.path, cli.regex, cli.fuzzy);
        }
    }
}

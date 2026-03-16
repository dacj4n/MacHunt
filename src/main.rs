use clap::{Parser, Subcommand};
use dashmap::DashMap;
use fuzzy_matcher::FuzzyMatcher;
use once_cell::sync::OnceCell;
use regex::Regex;
use std::ffi::{c_void, CStr};
use std::fs::{self, OpenOptions};
use std::io::{Write, BufWriter};
use std::os::raw::{c_char, c_double, c_ulong};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;
use rusqlite::{Connection, params};
use parking_lot::Mutex;
use core_foundation_sys::runloop::{
    CFRunLoopGetCurrent, CFRunLoopRun, CFRunLoopRef,
};

#[allow(non_camel_case_types)]
type FSEventStreamRef = *mut c_void;
#[allow(non_camel_case_types)]
type FSEventStreamCallback = unsafe extern "C" fn(
    stream_ref: FSEventStreamRef,
    client_callback_info: *mut c_void,
    num_events: usize,
    event_paths: *mut c_void,
    event_flags: *const u32,
    event_ids: *const u64,
);

#[repr(C)]
struct FSEventStreamContext {
    version: c_ulong,
    info: *mut c_void,
    retain: *const c_void,
    release: *const c_void,
    copy_description: *const c_void,
}

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn FSEventStreamCreate(
        allocator: *const c_void,
        callback: FSEventStreamCallback,
        context: *mut FSEventStreamContext,
        paths_to_watch: *const c_void,
        since_when: u64,
        latency: c_double,
        flags: u32,
    ) -> FSEventStreamRef;

    fn FSEventStreamScheduleWithRunLoop(
        stream_ref: FSEventStreamRef,
        run_loop: CFRunLoopRef,
        run_loop_mode: *const c_void,
    );

    fn FSEventStreamStart(stream_ref: FSEventStreamRef) -> bool;
    fn FSEventsGetCurrentEventId() -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFArrayCreate(
        allocator: *const c_void,
        values: *const *const c_void,
        num_values: isize,
        callbacks: *const c_void,
    ) -> *const c_void;

    fn CFStringCreateWithCString(
        allocator: *const c_void,
        c_str: *const c_char,
        encoding: u32,
    ) -> *const c_void;

    static kCFRunLoopDefaultMode: *const c_void;
    static kCFTypeArrayCallBacks: c_void;
}

const FSEVENT_SINCE_NOW: u64      = u64::MAX;
const KCF_STRING_ENCODING_UTF8: u32 = 0x08000100;

const FLAG_HISTORY_DONE: u32 = 0x00002000;
const FLAG_ITEM_CREATED:  u32 = 0x00000100;
const FLAG_ITEM_REMOVED:  u32 = 0x00000200;
const FLAG_ITEM_RENAMED:  u32 = 0x00000800;
const FLAG_ITEM_MODIFIED: u32 = 0x00001000;
const FLAG_ITEM_IS_FILE:  u32 = 0x00010000;

const STREAM_FLAG_FILE_EVENTS: u32 = 0x00000010;
const STREAM_FLAG_WATCH_ROOT:  u32 = 0x00000004;

struct WatchContext {
    index: Arc<DashMap<String, Vec<PathBuf>>>,
}

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

      let index = get_index();
      index.clear();

      let conn = get_db().lock();
      let mut stmt = conn
          .prepare("SELECT name, path FROM files")
          .unwrap();

      let mut count = 0usize;

      let rows = stmt.query_map([], |row| {
          Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
      }).unwrap();

      for row in rows.flatten() {
          let (name, path_str) = row;
          index.entry(name).or_default().push(PathBuf::from(path_str));
          count += 1;
      }

      println!("加载完成，共 {} 条记录，耗时 {:?}", count, start.elapsed());
      count > 0
  }

  fn get_index() -> &'static Arc<DashMap<String, Vec<PathBuf>>> {
    FILE_INDEX.get().unwrap()
}

const BATCH_SIZE: usize = 50000;

fn get_timestamp() -> String {
    let now = SystemTime::now();
    let since_epoch = now.duration_since(UNIX_EPOCH).unwrap();
    let timestamp = since_epoch.as_secs();
    timestamp.to_string()
}

fn num_cpus() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}

fn cleanup_dead_paths_background() {
    thread::spawn(|| {
        let start = Instant::now();
        
        let paths: Vec<(String, String)> = {
            let conn = get_db().lock();
            let mut stmt = conn.prepare("SELECT name, path FROM files").unwrap();
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .flatten()
                .collect()
        };

        let index = get_index();
        let dead_shared = Arc::new(std::sync::Mutex::new(vec![]));

        let chunk_size = (paths.len() / num_cpus()).max(1);
        let handles: Vec<_> = paths.chunks(chunk_size).map(|chunk| {
            let chunk = chunk.to_vec();
            let dead_shared = dead_shared.clone();
            thread::spawn(move || {
                let local_dead: Vec<PathBuf> = chunk.into_iter()
                    .filter_map(|(_, path_str)| {
                        let p = PathBuf::from(path_str);
                        if !p.exists() { Some(p) } else { None }
                    })
                    .collect();
                dead_shared.lock().unwrap().extend(local_dead);
            })
        }).collect();

        for h in handles { h.join().unwrap(); }
        let dead = Arc::try_unwrap(dead_shared).unwrap().into_inner().unwrap();
        let dead_count = dead.len();

        if dead_count == 0 {
            return;
        }

        let mut conn = get_db().lock();
        let tx = conn.transaction().unwrap();
        {
            let mut stmt = tx.prepare_cached("DELETE FROM files WHERE path = ?1").unwrap();
            for path in &dead {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                if let Some(mut v) = index.get_mut(&name) {
                    v.retain(|p| p != path);
                    if v.is_empty() { drop(v); index.remove(&name); }
                }
                stmt.execute(params![path.to_string_lossy().as_ref()]).ok();
            }
        }
        tx.commit().unwrap();

        println!("[启动校验] 清理 {} 条失效路径，耗时 {:?}", dead_count, start.elapsed());
    });
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

    let (tx, rx) = crossbeam::channel::bounded::<Vec<(String, PathBuf)>>(256);
    let roots = get_root_directories();

    let handles: Vec<_> = roots.into_iter().map(|root| {
        let tx = tx.clone();
        thread::spawn(move || scan_root(root, tx))
    }).collect();
    drop(tx);

    let mut db_batch: Vec<(String, PathBuf)> = Vec::with_capacity(50_000);
    let mut count = 0usize;

    for chunk in rx {
        count += chunk.len();
        db_batch.extend(chunk);

        if db_batch.len() >= 50_000 {
            db_insert_batch(&db_batch);
            db_batch.clear();
        }

        if count % 500_000 < BATCH_SIZE {
            println!("已扫描 {} 个文件，耗时 {:?}", count, start.elapsed());
        }
    }
    if !db_batch.is_empty() {
        db_insert_batch(&db_batch);
    }

    for h in handles { h.join().unwrap(); }
    println!("扫描+写库耗时: {:?}", start.elapsed());

    let t_mem = Instant::now();
    load_index_from_db();
    println!("写内存耗时: {:?}", t_mem.elapsed());

    let current_event_id = unsafe { FSEventsGetCurrentEventId() };
    save_last_event_id(current_event_id);
    println!("已记录 EventID: {}，下次 watch 将从此点增量同步", current_event_id);

    println!("索引构建完成，共 {} 个文件，总耗时 {:?}", count, start.elapsed());
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

unsafe extern "C" fn fsevent_callback(
    _stream_ref: FSEventStreamRef,
    client_info: *mut c_void,
    num_events: usize,
    event_paths: *mut c_void,
    event_flags: *const u32,
    event_ids: *const u64,
) {
    let ctx = &*(client_info as *const WatchContext);
    let paths_ptr = event_paths as *const *const c_char;

    for i in 0..num_events {
        let flags = *event_flags.add(i);
        let event_id = *event_ids.add(i);
        let path_cstr = CStr::from_ptr(*paths_ptr.add(i));
        let path_str = match path_cstr.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };

        LAST_EVENT_ID.store(event_id, std::sync::atomic::Ordering::Relaxed);

        if flags & FLAG_HISTORY_DONE != 0 {
            println!("历史回放完成（EventID: {}），进入实时监听", event_id);
            save_last_event_id(event_id);
            continue;
        }

        let path = PathBuf::from(path_str);

        if flags & FLAG_ITEM_IS_FILE == 0 {
            if flags & FLAG_ITEM_REMOVED != 0 {
                let prefix = format!("{}/", path_str);
                ctx.index.retain(|_, v| {
                    v.retain(|p| {
                        if p.to_string_lossy().starts_with(&prefix) {
                            db_delete(p);
                            false
                        } else {
                            true
                        }
                    });
                    !v.is_empty()
                });
            }
            continue;
        }

        let file_name_lower = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_lowercase(),
            None => continue,
        };

        if flags & FLAG_ITEM_REMOVED != 0 {
            if let Some(mut v) = ctx.index.get_mut(&file_name_lower) {
                v.retain(|p| p != &path);
                db_delete(&path);
                if *LOG_ENABLED.get().unwrap_or(&false) {
                    log_message(&format!("[-] {}", path.display()));
                }
                if v.is_empty() {
                    drop(v);
                    ctx.index.remove(&file_name_lower);
                }
            }
            continue;
        }

        if flags & (FLAG_ITEM_CREATED | FLAG_ITEM_RENAMED | FLAG_ITEM_MODIFIED) != 0 {
            if path.is_file() {
                let mut entry = ctx.index.entry(file_name_lower.clone()).or_default();
                if !entry.contains(&path) {
                    entry.push(path.clone());
                    db_insert(&file_name_lower, &path);
                    if *LOG_ENABLED.get().unwrap_or(&false) {
                        log_message(&format!("[+] {}", path.display()));
                    }
                }
            } else {
                if let Some(mut v) = ctx.index.get_mut(&file_name_lower) {
                    v.retain(|p| p != &path);
                    db_delete(&path);
                    if v.is_empty() {
                        drop(v);
                        ctx.index.remove(&file_name_lower);
                    }
                }
            }
        }
    }
}

fn watch_with_history(since_event_id: Option<u64>) {
    let index = get_index().clone();
    let since = since_event_id.unwrap_or(FSEVENT_SINCE_NOW);

    thread::spawn(move || {
        unsafe {
            let path_cstr = std::ffi::CString::new("/").unwrap();
            let cf_path = CFStringCreateWithCString(
                std::ptr::null(),
                path_cstr.as_ptr(),
                KCF_STRING_ENCODING_UTF8,
            );
            let paths_array = CFArrayCreate(
                std::ptr::null(),
                &cf_path as *const _ as *const *const c_void,
                1,
                &kCFTypeArrayCallBacks as *const _ as *const c_void,
            );

            let ctx = Box::new(WatchContext { index });
            let mut fsevent_ctx = FSEventStreamContext {
                version: 0,
                info: Box::into_raw(ctx) as *mut c_void,
                retain: std::ptr::null(),
                release: std::ptr::null(),
                copy_description: std::ptr::null(),
            };

            let stream = FSEventStreamCreate(
                std::ptr::null(),
                fsevent_callback,
                &mut fsevent_ctx,
                paths_array,
                since,
                0.05,
                STREAM_FLAG_FILE_EVENTS | STREAM_FLAG_WATCH_ROOT,
            );

            FSEventStreamScheduleWithRunLoop(
                stream,
                CFRunLoopGetCurrent(),
                kCFRunLoopDefaultMode as *const c_void,
            );

            FSEventStreamStart(stream);
            println!("FSEvents 监听启动，since_event_id={:?}", since_event_id);

            CFRunLoopRun();
        }
    });
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
            let last_event_id = load_last_event_id();
            
            if !has_index {
                println!("首次运行，后台构建索引中...");
                watch_with_history(None);
                thread::spawn(|| build_index());
            } else {
                match last_event_id {
                    Some(id) => {
                        println!("从上次退出点恢复（EventID: {}），回放离线变更...", id);
                        watch_with_history(Some(id));
                    }
                    None => {
                        println!("后台校验中...");
                        watch_with_history(None);
                        cleanup_dead_paths_background();
                    }
                }
            }
            
            ctrlc::set_handler(|| {
                let id = LAST_EVENT_ID.load(std::sync::atomic::Ordering::Relaxed);
                if id > 0 {
                    save_last_event_id(id);
                    println!("\n已保存 EventID: {}", id);
                }
                std::process::exit(0);
            }).unwrap();
            
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

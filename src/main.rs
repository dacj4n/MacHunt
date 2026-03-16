use crossbeam::channel::{unbounded, Receiver, Sender};
use once_cell::sync::OnceCell;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use walkdir::WalkDir;

// 全局索引，使用OnceCell确保只初始化一次
static FILE_INDEX: OnceCell<Arc<Mutex<HashMap<String, Vec<PathBuf>>>>> = OnceCell::new();

// 初始化文件索引
fn init_index() {
    let index = Arc::new(Mutex::new(HashMap::new()));
    FILE_INDEX.set(index).unwrap();
}

// 获取文件索引
fn get_index() -> &'static Arc<Mutex<HashMap<String, Vec<PathBuf>>>> {
    FILE_INDEX.get().unwrap()
}

// 并行遍历文件系统并构建索引
fn build_index() {
    let start = Instant::now();
    println!("开始构建文件索引...");
    
    // 获取需要遍历的目录
    let roots = vec![
        PathBuf::from("/"),
        // 可以添加其他需要搜索的根目录
    ];
    
    let index = get_index().clone();
    let (tx, rx): (Sender<(String, PathBuf)>, Receiver<(String, PathBuf)>) = unbounded();
    
    // 启动多个线程进行文件遍历
    let mut handles = vec![];
    for root in roots {
        let tx = tx.clone();
        let handle = thread::spawn(move || {
            // 遍历文件系统，设置更大的缓冲区和并行度
            for entry in WalkDir::new(root)
                .follow_links(false)
                .min_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                
                // 排除重复的系统路径
                if let Some(path_str) = path.to_str() {
                    // 排除 /System/Volumes/Data/Users (实际就是 /Users)
                    if path_str.starts_with("/System/Volumes/Data/Users/") {
                        continue;
                    }
                    // 排除 /System/Volumes/Data/Applications (实际就是 /Applications)
                    if path_str.starts_with("/System/Volumes/Data/Applications/") {
                        continue;
                    }
                    // 排除 /System/Volumes/Data/Volumes (实际就是 /Volumes)
                    if path_str.starts_with("/System/Volumes/Data/Volumes/") {
                        continue;
                    }
                }
                
                if path.is_file() {
                    if let Some(file_name) = path.file_name() {
                        if let Some(file_name_str) = file_name.to_str() {
                            let file_name_lower = file_name_str.to_lowercase();
                            tx.send((file_name_lower, path.to_path_buf())).unwrap();
                        }
                    }
                }
            }
        });
        handles.push(handle);
    }
    
    // 关闭发送端
    drop(tx);
    
    // 接收并处理文件信息
    let mut count = 0;
    for (file_name, path) in rx {
        let mut index = index.lock().unwrap();
        // 同一文件名可能有多个路径，所以使用Vec存储
        index.entry(file_name).or_insert_with(Vec::new).push(path);
        count += 1;
        
        // 每处理10000个文件打印一次进度
        if count % 10000 == 0 {
            println!("已索引 {} 个文件...", count);
        }
    }
    
    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }
    
    let duration = start.elapsed();
    let total_files = index.lock().unwrap().values().map(|v| v.len()).sum::<usize>();
    println!("索引构建完成，共索引 {} 个文件，耗时 {:?}", total_files, duration);
}

// 搜索文件
fn search_files(pattern: &str) {
    let start = Instant::now();
    let index = get_index();
    let index = index.lock().unwrap();
    
    // 构建正则表达式
    let regex_pattern = format!("{}", pattern);
    let regex = Regex::new(&regex_pattern).unwrap();
    
    // 搜索匹配的文件
    let mut results = vec![];
    for (file_name, paths) in index.iter() {
        if regex.is_match(file_name) {
            results.extend(paths);
        }
    }
    
    let duration = start.elapsed();
    println!("搜索完成，找到 {} 个匹配文件，耗时 {:?}", results.len(), duration);
    
    // 打印搜索结果
    for path in results {
        println!("{}", path.display());
    }
}

// 实时搜索功能
fn real_time_search() {
    println!("实时搜索模式，输入搜索词（按Ctrl+C退出）:");
    
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        search_files(input);
    }
}

fn main() {
    // 初始化索引
    init_index();
    
    // 构建文件索引
    build_index();
    
    // 进入实时搜索模式
    real_time_search();
}

use parking_lot::Mutex;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct Logger {
    enabled: bool,
    writer: Option<Arc<Mutex<BufWriter<std::fs::File>>>>,
}

impl Logger {
    pub fn new(enabled: bool) -> Self {
        if !enabled {
            return Self {
                enabled: false,
                writer: None,
            };
        }

        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let logs_dir = PathBuf::from(home_dir).join(".machunt").join("logs");
        let _ = fs::create_dir_all(&logs_dir);

        let log_file = logs_dir.join(format!("machunt_{}.log", timestamp_secs()));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .unwrap();

        Self {
            enabled: true,
            writer: Some(Arc::new(Mutex::new(BufWriter::new(file)))),
        }
    }

    pub fn log(&self, message: &str) {
        if !self.enabled {
            return;
        }
        if let Some(writer) = &self.writer {
            let mut w = writer.lock();
            let _ = writeln!(w, "{}", message);
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }
}

pub fn timestamp_secs() -> String {
    let now = SystemTime::now();
    let since_epoch = now.duration_since(UNIX_EPOCH).unwrap();
    since_epoch.as_secs().to_string()
}

pub fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

pub fn get_root_directories() -> Vec<PathBuf> {
    let root = PathBuf::from("/");
    let mut dirs = Vec::new();

    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !should_skip_path(&path) {
                dirs.push(path);
            }
        }
    }

    dirs
}

pub fn should_skip_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    matches!(
        path_str.as_ref(),
        "/dev"
            | "/proc"
            | "/sys"
            | "/private/var/vm"
            | "/private/var/run"
            | "/private/var/folders"
            | "/System/Volumes/Data"
            | "/System/Volumes/Preboot"
            | "/System/Volumes/Recovery"
            | "/System/Volumes/VM"
    ) || path_str.contains("/.Spotlight-V100")
        || path_str.contains("/.fseventsd")
}

use parking_lot::Mutex;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// `~/Library/Caches/MacHunt` — the index, the GUI settings and the logs all
/// live here.
pub fn cache_dir() -> PathBuf {
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home_dir)
        .join("Library")
        .join("Caches")
        .join("MacHunt")
}

/// Whether to record diagnostics, and optionally what subtree to restrict them
/// to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogSettings {
    pub enabled: bool,
    /// `Some(prefix)` means only messages about paths inside `prefix` are kept.
    pub scope: Option<PathBuf>,
}

/// Resolve the opt-in for diagnostics logging.
///
/// Off unless asked for, so a shipped build never grows a log file. Turn it on
/// without rebuilding by either exporting `MACHUNT_LOG=1` or creating
/// `~/Library/Caches/MacHunt/watch-log-on`; either one may hold a path to scope
/// the log to.
///
/// The scope matters: the watcher watches `/`, and recording every file event on
/// the volume would both bury the signal and slow the watcher thread down — and
/// a slow callback is itself a way to cause the very event drops being
/// investigated.
///
/// The flag file existed before this function and was read by nothing, so the
/// watcher could not be observed in any build.
pub fn log_settings() -> LogSettings {
    if let Ok(raw) = std::env::var("MACHUNT_LOG") {
        let value = raw.trim().to_string();
        if matches!(
            value.to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "off" | "no"
        ) {
            return LogSettings::default();
        }
        return LogSettings {
            enabled: true,
            scope: value.starts_with('/').then(|| PathBuf::from(&value)),
        };
    }

    match fs::read_to_string(cache_dir().join("watch-log-on")) {
        Ok(contents) => {
            let trimmed = contents.trim();
            LogSettings {
                enabled: true,
                scope: trimmed.starts_with('/').then(|| PathBuf::from(trimmed)),
            }
        }
        Err(_) => LogSettings::default(),
    }
}

/// How many log files to keep around. A file is opened per launch and only ever
/// appended to, so without retention the directory grows for as long as
/// diagnostics stay switched on.
const MAX_LOG_FILES: usize = 10;

/// Cap on a single log file, so one long session with the switch left on cannot
/// grow without limit either.
const MAX_LOG_BYTES: u64 = 32 * 1024 * 1024;

/// Drop the oldest log files, keeping `MAX_LOG_FILES - 1` so the file about to
/// be created still fits inside the budget.
fn prune_old_logs(logs_dir: &Path) {
    let Ok(entries) = fs::read_dir(logs_dir) else {
        return;
    };
    let mut logs: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".log"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    if logs.len() < MAX_LOG_FILES {
        return;
    }
    logs.sort_by_key(|(modified, _)| *modified); // oldest first
    let excess = logs.len() - (MAX_LOG_FILES - 1);
    for (_, path) in logs.iter().take(excess) {
        let _ = fs::remove_file(path);
    }
}

/// Buffered log file plus the accounting needed to stop at [`MAX_LOG_BYTES`].
struct LogSink {
    writer: BufWriter<std::fs::File>,
    written: u64,
    capped: bool,
}

/// Persist the logging choice, keeping the flag file the single source of truth
/// that [`log_settings`] reads — so the CLI, the GUI and the logger itself can
/// never disagree. An empty file means "on, no scope".
pub fn write_log_settings(enabled: bool, scope: &str) -> std::io::Result<()> {
    let flag = cache_dir().join("watch-log-on");
    if !enabled {
        return match fs::remove_file(&flag) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        };
    }
    fs::create_dir_all(cache_dir())?;
    fs::write(&flag, scope.trim())
}

/// Shared mutable half of a logger.
///
/// Behind one `Arc` so every clone — the engine holds one, the watcher context
/// holds another — sees a settings change immediately instead of having to
/// restart the app.
struct LoggerState {
    /// Fast path, read before any lock is taken.
    enabled: AtomicBool,
    /// `Some(prefix)` while the log is scoped to a subtree.
    scope: Mutex<Option<PathBuf>>,
    /// Opened on demand so logging can be switched on at runtime.
    sink: Mutex<Option<LogSink>>,
}

#[derive(Clone)]
pub struct Logger {
    state: Arc<LoggerState>,
}

/// Open a fresh log file, pruning older ones first so the directory stays
/// bounded. `None` on any I/O failure — logging degrades to off rather than
/// taking the app down.
fn open_sink() -> Option<LogSink> {
    let logs_dir = cache_dir().join("logs");
    fs::create_dir_all(&logs_dir).ok()?;
    prune_old_logs(&logs_dir);

    let log_file = logs_dir.join(format!("machunt_{}.log", timestamp_secs()));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .ok()?;

    Some(LogSink {
        writer: BufWriter::new(file),
        written: 0,
        capped: false,
    })
}

impl Logger {
    pub fn new(settings: LogSettings) -> Self {
        let logger = Self {
            state: Arc::new(LoggerState {
                enabled: AtomicBool::new(false),
                scope: Mutex::new(None),
                sink: Mutex::new(None),
            }),
        };
        logger.apply(settings);
        logger
    }

    /// Re-read the opt-in from disk so a settings toggle takes effect at once.
    pub fn refresh(&self) {
        self.apply(log_settings());
    }

    fn apply(&self, settings: LogSettings) {
        if !settings.enabled {
            // Stop accepting writes *before* dropping the sink, so a concurrent
            // writer cannot end up appending to a file that is being closed.
            self.state.enabled.store(false, Ordering::Relaxed);
            *self.state.sink.lock() = None; // dropping flushes and closes
            *self.state.scope.lock() = None;
            return;
        }

        *self.state.scope.lock() = settings.scope;
        let mut sink = self.state.sink.lock();
        if sink.is_none() {
            *sink = open_sink();
        }
        let opened = sink.is_some();
        drop(sink);
        self.state.enabled.store(opened, Ordering::Relaxed);
    }

    /// Record a line that is not about one particular path — the watcher
    /// starting, a replay finishing. These are always kept so a scoped log is
    /// still readable as a sequence of events.
    pub fn log(&self, message: &str) {
        self.write(message);
    }

    /// Record a line about `path`, skipped when the log is scoped to a prefix
    /// that does not contain it.
    pub fn log_path(&self, path: &Path, message: &str) {
        let in_scope = match self.state.scope.lock().as_ref() {
            Some(scope) => path.starts_with(scope),
            None => true,
        };
        if in_scope {
            self.write(message);
        }
    }

    fn write(&self, message: &str) {
        if !self.state.enabled.load(Ordering::Relaxed) {
            return;
        }
        let mut guard = self.state.sink.lock();
        let Some(sink) = guard.as_mut() else {
            return;
        };

        if sink.written >= MAX_LOG_BYTES {
            if !sink.capped {
                sink.capped = true;
                let _ = writeln!(
                    sink.writer,
                    "[log] reached {} MiB, no further messages will be recorded",
                    MAX_LOG_BYTES / (1024 * 1024)
                );
                let _ = sink.writer.flush();
            }
            return;
        }

        // Flushed on every line: the point of this log is to be readable while
        // the app is still running. A BufWriter owned by the long-lived Engine
        // would otherwise hold every line in memory until the process exits.
        let _ = writeln!(sink.writer, "{}", message);
        let _ = sink.writer.flush();
        sink.written += message.len() as u64 + 1;
    }

    pub fn enabled(&self) -> bool {
        self.state.enabled.load(Ordering::Relaxed)
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

pub fn normalize_path_for_index(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    let normalized = if raw == "/System/Volumes/Data" || raw.starts_with("/System/Volumes/Data/") {
        Some(raw.trim_start_matches("/System/Volumes/Data"))
    } else if raw == "/Volumes/System/Volumes/Data"
        || raw.starts_with("/Volumes/System/Volumes/Data/")
    {
        Some(raw.trim_start_matches("/Volumes/System/Volumes/Data"))
    } else if raw == "/Volumes/Macintosh HD" || raw.starts_with("/Volumes/Macintosh HD/") {
        Some(raw.trim_start_matches("/Volumes/Macintosh HD"))
    } else {
        None
    };

    if let Some(rest) = normalized {
        if rest.is_empty() {
            return PathBuf::from("/");
        }
        return PathBuf::from(rest);
    }

    path.to_path_buf()
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
            | "/Volumes/System/Volumes/Data"
            | "/Volumes/Macintosh HD"
    ) || path_str.contains("/.Spotlight-V100")
        || path_str.contains("/.fseventsd")
        || path_str.contains("/Library/Caches/MacHunt")
}


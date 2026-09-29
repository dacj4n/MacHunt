use parking_lot::Mutex;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

/// `~/Library/Application Support/MacHunt` — persistent application data.
///
/// Unlike `Caches`, the system does not purge this under disk pressure, which
/// is what an index that costs a full filesystem scan to build requires. It is
/// also included in Time Machine by default, so bulk data placed here is marked
/// excluded from backup when it is first created
/// (see `Db::exclude_from_backup`).
pub fn support_dir() -> PathBuf {
    home_dir()
        .join("Library")
        .join("Application Support")
        .join("MacHunt")
}

/// `~/Library/Application Support/MacHunt/Data` — the index and its SQLite
/// sidecars.
///
/// Kept in its own subdirectory so settings and bulk data can evolve
/// independently: a second database, a versioned index or a migration marker
/// needs no further change to the layout.
pub fn data_dir() -> PathBuf {
    support_dir().join("Data")
}

/// `~/Library/Caches/MacHunt` — regenerable data only: the diagnostics logs and
/// the switch that turns them on.
///
/// This held the index too before 0.6.8; [`crate::db`] migrates it out on first
/// run.
pub fn cache_dir() -> PathBuf {
    home_dir().join("Library").join("Caches").join("MacHunt")
}

/// Whether a raw path belongs to MacHunt itself.
///
/// Indexing our own files makes the index feed on its own writes: during a
/// rebuild the WAL grows continuously, and the resulting event storm is both
/// pointless and expensive. Both locations are covered because the index has
/// lived in each of them.
pub fn is_own_data_path(path: &str) -> bool {
    path.contains("/Library/Caches/MacHunt")
        || path.contains("/Library/Application Support/MacHunt")
}

/// Whether `path` is a symlink, judged by the link itself rather than by what it
/// points at — the distinction `Path::is_dir` and `Path::is_file` silently
/// resolve away.
pub fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

/// Whether `path` is a directory in its own right and not reached through a
/// symlink.
///
/// This is the check that keeps a directory-shaped symlink from being walked as
/// if it were the directory. Walking one indexes the entire target tree a second
/// time under the link's path, and a tool that keeps one link per process — `fnm`
/// does exactly that — multiplies the index by the number of links.
pub fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false)
}

/// Per-process alias farms.
///
/// `fnm` keeps one symlink per shell under `fnm_multishells`, all pointing at
/// the same Node install, creating and deleting them as shells come and go.
/// Every entry aliases content that is already indexed at its real path, so the
/// farm is churn rather than data and is skipped outright.
pub fn is_alias_farm_path(path: &str) -> bool {
    path.contains("/fnm_multishells/") || path.ends_with("/fnm_multishells")
}

/// Whether to record diagnostics, and optionally what subtree to restrict them
/// to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogSettings {
    pub enabled: bool,
    /// `Some(prefix)` means only messages about paths inside `prefix` are kept.
    pub scope: Option<PathBuf>,
}

/// Resolve the diagnostics-log opt-in from the environment.
///
/// This is how the CLI turns logging on, and an override for the GUI, whose
/// switch lives in `settings.json` — the GUI hands that value straight to
/// [`Logger::set`] rather than having the logger read a second source of truth.
///
/// Off unless asked for, so a shipped build never grows a log file.
/// `MACHUNT_LOG` may hold `1` to record everything, or a path to scope the log
/// to that subtree. The scope matters: the watcher watches `/`, and recording
/// every file event on the volume would both bury the signal and slow the
/// watcher thread down — and a slow callback is itself a way to cause the very
/// event drops being investigated.
pub fn log_settings() -> LogSettings {
    let Ok(raw) = std::env::var("MACHUNT_LOG") else {
        return LogSettings::default();
    };
    let value = raw.trim().to_string();
    if matches!(
        value.to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "off" | "no"
    ) {
        return LogSettings::default();
    }
    LogSettings {
        enabled: true,
        scope: value.starts_with('/').then(|| PathBuf::from(&value)),
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
        logger.set(settings);
        logger
    }

    /// Apply a configuration to a logger that may already be running, so a
    /// settings change takes effect without a restart. Enabling opens the file
    /// on demand; disabling closes it, flushing whatever was buffered.
    pub fn set(&self, settings: LogSettings) {
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
        || is_own_data_path(path_str.as_ref())
        || is_alias_farm_path(path_str.as_ref())
}


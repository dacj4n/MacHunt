use machunt::{Engine, SearchMode, SearchOptions};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::Emitter;

struct AppState {
    engine: Engine,
    watch_started: AtomicBool,
    index_loaded: AtomicBool,
    preview_process: Arc<Mutex<Option<u32>>>,
    preview_session_seq: AtomicU64,
}

impl AppState {
    fn new() -> Self {
        Self {
            engine: Engine::new(false),
            watch_started: AtomicBool::new(false),
            index_loaded: AtomicBool::new(false),
            preview_process: Arc::new(Mutex::new(None)),
            preview_session_seq: AtomicU64::new(0),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchRequest {
    query: String,
    mode: SearchMode,
    regex_enabled: Option<bool>,
    case_sensitive: Option<bool>,
    path_prefix: Option<String>,
    include_files: Option<bool>,
    include_dirs: Option<bool>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResultItem {
    name: String,
    path: String,
    parent: String,
    is_dir: bool,
    is_file: bool,
    size_bytes: Option<u64>,
    modified_unix_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResponse {
    items: Vec<SearchResultItem>,
    total: usize,
    took_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InitResponse {
    indexed: usize,
    has_index: bool,
    last_event_id: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildResponse {
    indexed: usize,
    took_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildEvent {
    phase: String,
    indexed: Option<usize>,
    took_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewStatusEvent {
    phase: String,
    session_id: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WatchResponse {
    running: bool,
    mode: String,
    message: String,
    last_event_id: Option<u64>,
}

fn watch_response(running: bool, mode: &str, last_event_id: Option<u64>) -> WatchResponse {
    let message = if running {
        match last_event_id {
            Some(id) => format!("Watcher running (EventID {})", id),
            None => "Watcher running".to_string(),
        }
    } else {
        "Watcher stopped".to_string()
    };

    WatchResponse {
        running,
        mode: mode.to_string(),
        message,
        last_event_id,
    }
}

fn to_search_options(req: &SearchRequest, mode: SearchMode, limit: Option<usize>) -> SearchOptions {
    SearchOptions {
        query: req.query.clone(),
        mode,
        case_sensitive: req.case_sensitive.unwrap_or(false),
        path_prefix: req.path_prefix.as_ref().map(PathBuf::from),
        include_files: req.include_files.unwrap_or(true),
        include_dirs: req.include_dirs.unwrap_or(true),
        limit,
    }
}

fn map_result(path: PathBuf) -> SearchResultItem {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();

    let parent = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let metadata = std::fs::metadata(&path).ok();
    let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
    let is_file = metadata.as_ref().map(|m| m.is_file()).unwrap_or(false);

    let size_bytes = if is_file {
        metadata.as_ref().map(|m| m.len())
    } else {
        None
    };

    let modified_unix_ms = metadata
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_millis()).ok());

    SearchResultItem {
        name,
        path: path.to_string_lossy().to_string(),
        parent,
        is_dir,
        is_file,
        size_bytes,
        modified_unix_ms,
    }
}

fn add_path_if_dir(out: &mut BTreeSet<String>, path: &Path) {
    if matches!(
        path.to_str(),
        Some("/Volumes") | Some("/Volumes/Macintosh HD")
    ) {
        return;
    }
    if path.is_dir() {
        out.insert(path.to_string_lossy().to_string());
    }
}

#[tauri::command]
fn list_path_suggestions() -> Vec<String> {
    let mut out = BTreeSet::new();

    add_path_if_dir(&mut out, Path::new("/"));

    if let Ok(home) = std::env::var("HOME") {
        add_path_if_dir(&mut out, PathBuf::from(home).as_path());
    }

    if let Ok(entries) = fs::read_dir("/Volumes") {
        for entry in entries.flatten().take(8) {
            add_path_if_dir(&mut out, entry.path().as_path());
        }
    }

    out.into_iter().collect()
}

#[tauri::command]
fn pick_path_in_finder() -> Option<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg("POSIX path of (choose folder with prompt \"Select a search path\")")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    if raw == "/" {
        return Some(raw);
    }

    Some(raw.trim_end_matches('/').to_string())
}

#[tauri::command]
fn open_search_result(path: String) -> Result<(), String> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err("Target path does not exist".to_string());
    }

    let status = if target.is_dir() {
        Command::new("open")
            .arg("-a")
            .arg("Finder")
            .arg(&target)
            .status()
            .map_err(|e| e.to_string())?
    } else {
        Command::new("open")
            .arg(&target)
            .status()
            .map_err(|e| e.to_string())?
    };

    if status.success() {
        Ok(())
    } else {
        Err("Failed to open target".to_string())
    }
}

#[tauri::command]
fn preview_search_result(
    paths: Vec<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }

    if let Ok(mut running_pid) = state.preview_process.lock() {
        if let Some(pid) = running_pid.take() {
            let _ = Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .status();
        }
    }

    let session_id = state.preview_session_seq.fetch_add(1, Ordering::SeqCst) + 1;
    let mut cmd = Command::new("qlmanage");
    cmd.arg("-p");
    let mut valid_count = 0usize;
    for path in paths {
        let target = PathBuf::from(path);
        if !target.exists() {
            continue;
        }
        valid_count += 1;
        cmd.arg(target);
    }
    if valid_count == 0 {
        return Err("Target path does not exist".to_string());
    }

    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let pid = child.id();

    if let Ok(mut running_pid) = state.preview_process.lock() {
        *running_pid = Some(pid);
    }

    let _ = app.emit(
        "preview://status",
        PreviewStatusEvent {
            phase: "opened".to_string(),
            session_id,
        },
    );

    let preview_process = state.preview_process.clone();
    let app_handle = app.clone();
    std::thread::spawn(move || {
        let _ = child.wait();
        if let Ok(mut running_pid) = preview_process.lock() {
            if running_pid.as_ref().copied() == Some(pid) {
                *running_pid = None;
            }
        }
        let _ = app_handle.emit(
            "preview://status",
            PreviewStatusEvent {
                phase: "closed".to_string(),
                session_id,
            },
        );
    });

    Ok(())
}

#[tauri::command]
fn reveal_in_finder(path: String) -> Result<(), String> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err("Target path does not exist".to_string());
    }

    let status = if target.is_dir() {
        Command::new("open")
            .arg("-a")
            .arg("Finder")
            .arg(&target)
            .status()
            .map_err(|e| e.to_string())?
    } else {
        Command::new("open")
            .arg("-R")
            .arg(&target)
            .status()
            .map_err(|e| e.to_string())?
    };

    if status.success() {
        Ok(())
    } else {
        Err("Failed to reveal in Finder".to_string())
    }
}

#[tauri::command]
fn open_in_qspace(path: String) -> Result<(), String> {
    let open_target = open_container_path(&path)?;

    let status = Command::new("open")
        .arg("-a")
        .arg("QSpace Pro")
        .arg(open_target)
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err("Failed to open in QSpace Pro (check whether QSpace Pro is installed)".to_string())
    }
}

fn open_container_path(path: &str) -> Result<PathBuf, String> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err("Target path does not exist".to_string());
    }

    if target.is_dir() {
        return Ok(target);
    }

    target
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "Unable to resolve parent directory".to_string())
}

#[tauri::command]
fn open_in_terminal(path: String) -> Result<(), String> {
    let open_target = open_container_path(&path)?;

    let status = Command::new("open")
        .arg("-a")
        .arg("Terminal")
        .arg(open_target)
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err("Failed to open in Terminal".to_string())
    }
}

#[tauri::command]
fn open_in_wezterm(path: String) -> Result<(), String> {
    let open_target = open_container_path(&path)?;

    let status = Command::new("open")
        .arg("-a")
        .arg("WezTerm")
        .arg("--args")
        .arg("start")
        .arg("--cwd")
        .arg(open_target)
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err("Failed to open in WezTerm (check whether WezTerm is installed)".to_string())
    }
}

#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| e.to_string())?;
    } else {
        return Err("Unable to access clipboard pipe".to_string());
    }

    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Failed to copy to clipboard".to_string())
    }
}

#[tauri::command]
fn move_to_trash(path: String) -> Result<(), String> {
    let target = PathBuf::from(&path);
    if !target.exists() {
        return Err("Target path does not exist".to_string());
    }

    let escaped = path.replace('\\', "\\\\").replace('\"', "\\\"");
    let script = format!(
        "tell application \"Finder\" to delete POSIX file \"{}\"",
        escaped
    );
    let status = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err("Failed to move item to Trash".to_string())
    }
}

#[tauri::command]
async fn initialize(state: tauri::State<'_, AppState>) -> Result<InitResponse, String> {
    let engine = state.engine.clone();
    let indexed = tauri::async_runtime::spawn_blocking(move || engine.load_index_from_db())
        .await
        .map_err(|e| e.to_string())?;
    state.index_loaded.store(true, Ordering::Relaxed);

    let last_event_id = state.engine.load_last_event_id();

    Ok(InitResponse {
        indexed,
        has_index: indexed > 0,
        last_event_id,
    })
}

#[tauri::command]
async fn search(
    request: SearchRequest,
    state: tauri::State<'_, AppState>,
) -> Result<SearchResponse, String> {
    let engine = state.engine.clone();
    let query_limit = request.limit;
    let regex_enabled = request.regex_enabled.unwrap_or(false);

    let started = Instant::now();
    let mut items = tauri::async_runtime::spawn_blocking(move || {
        let paths = if regex_enabled {
            let substring_options = to_search_options(&request, SearchMode::Substring, query_limit);
            let regex_options = to_search_options(&request, SearchMode::Pattern, query_limit);

            let mut merged = Vec::<PathBuf>::new();
            let mut seen = HashSet::<PathBuf>::new();

            for path in engine.search(substring_options) {
                if seen.insert(path.clone()) {
                    merged.push(path);
                }
                if let Some(limit) = query_limit {
                    if merged.len() >= limit {
                        break;
                    }
                }
            }

            if !matches!(query_limit, Some(0))
                && query_limit.map(|limit| merged.len() < limit).unwrap_or(true)
            {
                for path in engine.search(regex_options) {
                    if seen.insert(path.clone()) {
                        merged.push(path);
                    }
                    if let Some(limit) = query_limit {
                        if merged.len() >= limit {
                            break;
                        }
                    }
                }
            }

            merged
        } else {
            let options = to_search_options(&request, request.mode, query_limit);
            engine.search(options)
        };
        let mut out: Vec<SearchResultItem> = paths.into_iter().map(map_result).collect();
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    })
    .await
    .map_err(|e| e.to_string())?;

    let total = items.len();
    if items.len() > 5000 {
        items.truncate(5000);
    }

    Ok(SearchResponse {
        items,
        total,
        took_ms: started.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
async fn build_index(
    path: Option<String>,
    rebuild: bool,
    include_dirs: Option<bool>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<BuildResponse, String> {
    let _ = app.emit(
        "index://build-status",
        BuildEvent {
            phase: "started".to_string(),
            indexed: None,
            took_ms: None,
        },
    );

    let engine = state.engine.clone();
    let include_dirs = include_dirs.unwrap_or(true);
    let response = tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let indexed = engine.build_index(path, rebuild, include_dirs);
        BuildResponse {
            indexed,
            took_ms: started.elapsed().as_millis() as u64,
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    state
        .index_loaded
        .store(response.indexed > 0, Ordering::Relaxed);

    let _ = app.emit(
        "index://build-status",
        BuildEvent {
            phase: "finished".to_string(),
            indexed: Some(response.indexed),
            took_ms: Some(response.took_ms),
        },
    );

    Ok(response)
}

#[tauri::command]
fn start_watch_auto(state: tauri::State<'_, AppState>) -> WatchResponse {
    if state.engine.is_watch_running() {
        return WatchResponse {
            running: true,
            mode: "active".to_string(),
            message: "Watcher is already running".to_string(),
            last_event_id: state.engine.load_last_event_id(),
        };
    }
    state.watch_started.store(true, Ordering::SeqCst);

    if !state.index_loaded.load(Ordering::Relaxed) {
        let loaded = state.engine.load_index_from_db();
        state.index_loaded.store(loaded > 0, Ordering::Relaxed);
    }

    let has_index = state.engine.has_persisted_index();
    let last_event_id = state.engine.load_last_event_id();

    if !has_index {
        state.engine.start_watch(None);
        let engine_bg = state.engine.clone();
        std::thread::spawn(move || {
            let _ = engine_bg.build_index(None, true, true);
        });

        return WatchResponse {
            running: true,
            mode: "bootstrap".to_string(),
            message: "Watcher started; initial index build runs in background".to_string(),
            last_event_id: None,
        };
    }

    match last_event_id {
        Some(id) => {
            state.engine.start_watch(Some(id));
            WatchResponse {
                running: true,
                mode: "resume".to_string(),
                message: format!("Watcher resumed from EventID {}", id),
                last_event_id: Some(id),
            }
        }
        None => {
            state.engine.start_watch(None);
            state.engine.cleanup_dead_paths_background();
            WatchResponse {
                running: true,
                mode: "validate".to_string(),
                message: "Watcher started with startup validation".to_string(),
                last_event_id: None,
            }
        }
    }
}

#[tauri::command]
fn watch_status(state: tauri::State<'_, AppState>) -> WatchResponse {
    let running = state.engine.is_watch_running();
    state.watch_started.store(running, Ordering::Relaxed);
    watch_response(running, "status", state.engine.load_last_event_id())
}

#[tauri::command]
fn stop_watch(state: tauri::State<'_, AppState>) -> WatchResponse {
    let running = state.engine.is_watch_running() || state.watch_started.load(Ordering::Relaxed);
    if !running {
        return WatchResponse {
            running: false,
            mode: "inactive".to_string(),
            message: "Watcher is not running".to_string(),
            last_event_id: state.engine.load_last_event_id(),
        };
    }

    if state.engine.stop_watch() {
        state.engine.save_last_event_id_from_runtime();
        state.watch_started.store(false, Ordering::SeqCst);
        return watch_response(false, "stopped", state.engine.load_last_event_id());
    }

    WatchResponse {
        running: true,
        mode: "stopping".to_string(),
        message: "Watcher is stopping...".to_string(),
        last_event_id: state.engine.load_last_event_id(),
    }
}

#[tauri::command]
fn persist_watch_cursor(state: tauri::State<'_, AppState>) {
    state.engine.save_last_event_id_from_runtime();
}

const MENU_OPEN_SETTINGS_ID: &str = "open_settings";
const EVENT_OPEN_SETTINGS: &str = "app://open-settings";

fn settings_menu_text() -> &'static str {
    "Preferences"
}

#[tauri::command]
fn set_menu_language(_language: String, app: tauri::AppHandle) -> Result<(), String> {
    if let Some(menu) = app.menu() {
        if let Some(item) = menu.get(MENU_OPEN_SETTINGS_ID) {
            if let Some(menu_item) = item.as_menuitem() {
                menu_item
                    .set_text(settings_menu_text())
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .menu(|app| {
            #[cfg(target_os = "macos")]
            {
                let app_menu = Submenu::with_items(
                    app,
                    app.package_info().name.clone(),
                    true,
                    &[
                        &PredefinedMenuItem::about(app, None::<&str>, None)?,
                        &PredefinedMenuItem::separator(app)?,
                        &MenuItem::with_id(
                            app,
                            MENU_OPEN_SETTINGS_ID,
                            settings_menu_text(),
                            true,
                            Some("CmdOrCtrl+,"),
                        )?,
                        &PredefinedMenuItem::separator(app)?,
                        &PredefinedMenuItem::services(app, None)?,
                        &PredefinedMenuItem::separator(app)?,
                        &PredefinedMenuItem::hide(app, None)?,
                        &PredefinedMenuItem::hide_others(app, None)?,
                        &PredefinedMenuItem::separator(app)?,
                        &PredefinedMenuItem::quit(app, None)?,
                    ],
                )?;
                let file_menu = Submenu::with_items(
                    app,
                    "File",
                    true,
                    &[&PredefinedMenuItem::close_window(app, None)?],
                )?;
                let edit_menu = Submenu::with_items(
                    app,
                    "Edit",
                    true,
                    &[
                        &PredefinedMenuItem::undo(app, None)?,
                        &PredefinedMenuItem::redo(app, None)?,
                        &PredefinedMenuItem::separator(app)?,
                        &PredefinedMenuItem::cut(app, None)?,
                        &PredefinedMenuItem::copy(app, None)?,
                        &PredefinedMenuItem::paste(app, None)?,
                        &PredefinedMenuItem::select_all(app, None)?,
                    ],
                )?;
                let view_menu = Submenu::with_items(
                    app,
                    "View",
                    true,
                    &[&PredefinedMenuItem::fullscreen(app, None)?],
                )?;
                let window_menu = Submenu::with_items(
                    app,
                    "Window",
                    true,
                    &[
                        &PredefinedMenuItem::minimize(app, None)?,
                        &PredefinedMenuItem::maximize(app, None)?,
                        &PredefinedMenuItem::separator(app)?,
                        &PredefinedMenuItem::close_window(app, None)?,
                    ],
                )?;
                let help_menu = Submenu::with_items(app, "Help", true, &[])?;
                return Menu::with_items(
                    app,
                    &[
                        &app_menu,
                        &file_menu,
                        &edit_menu,
                        &view_menu,
                        &window_menu,
                        &help_menu,
                    ],
                );
            }
            #[cfg(not(target_os = "macos"))]
            {
                let menu = Menu::default(app)?;
                let open_settings = MenuItem::with_id(
                    app,
                    MENU_OPEN_SETTINGS_ID,
                    "Settings",
                    true,
                    Some("Ctrl+,"),
                )?;
                let settings_submenu =
                    Submenu::with_items(app, "Settings", true, &[&open_settings])?;
                menu.append(&settings_submenu)?;
                return Ok(menu);
            }
        })
        .on_menu_event(|app, event| {
            if event.id() == MENU_OPEN_SETTINGS_ID {
                let _ = app.emit(EVENT_OPEN_SETTINGS, ());
            }
        })
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            initialize,
            search,
            build_index,
            start_watch_auto,
            stop_watch,
            watch_status,
            list_path_suggestions,
            pick_path_in_finder,
            open_search_result,
            preview_search_result,
            reveal_in_finder,
            open_in_qspace,
            open_in_terminal,
            open_in_wezterm,
            copy_to_clipboard,
            move_to_trash,
            set_menu_language,
            persist_watch_cursor
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}

use machunt::{Engine, SearchMode, SearchOptions};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::Emitter;

struct AppState {
    engine: Engine,
    watch_started: AtomicBool,
}

impl AppState {
    fn new() -> Self {
        Self {
            engine: Engine::new(false),
            watch_started: AtomicBool::new(false),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchRequest {
    query: String,
    mode: SearchMode,
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

fn to_search_options(req: SearchRequest) -> SearchOptions {
    SearchOptions {
        query: req.query,
        mode: req.mode,
        case_sensitive: req.case_sensitive.unwrap_or(false),
        path_prefix: req.path_prefix.map(PathBuf::from),
        include_files: req.include_files.unwrap_or(true),
        include_dirs: req.include_dirs.unwrap_or(true),
        limit: req.limit,
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

#[tauri::command]
async fn initialize(state: tauri::State<'_, AppState>) -> Result<InitResponse, String> {
    let engine = state.engine.clone();
    let indexed = tauri::async_runtime::spawn_blocking(move || engine.load_index_from_db())
        .await
        .map_err(|e| e.to_string())?;

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
    let options = to_search_options(request);

    let started = Instant::now();
    let mut items = tauri::async_runtime::spawn_blocking(move || {
        let paths = engine.search(options);
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
    let response = tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let indexed = engine.build_index(path, rebuild);
        BuildResponse {
            indexed,
            took_ms: started.elapsed().as_millis() as u64,
        }
    })
    .await
    .map_err(|e| e.to_string())?;

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

    let has_index = state.engine.load_index_from_db() > 0;
    let last_event_id = state.engine.load_last_event_id();

    if !has_index {
        state.engine.start_watch(None);
        let engine_bg = state.engine.clone();
        std::thread::spawn(move || {
            let _ = engine_bg.build_index(None, true);
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
            set_menu_language,
            persist_watch_cursor
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}

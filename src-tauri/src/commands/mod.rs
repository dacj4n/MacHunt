use crate::settings::{AppState, save_gui_settings, snapshot_gui_settings};
use crate::startup::apply_launch_settings;
use crate::window;
use machunt::{SearchMode, SearchOptions, SortKey};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tauri::Emitter;

// ── Macro for simple getter/setter pattern ──
macro_rules! simple_getter_setter {
    (
        $(#[$meta:meta])*
        $get_fn:ident, $set_fn:ident, $field:ident, $ty:ty, $resp:ident, $resp_field:ident,
        $param_name:ident
    ) => {
        $(#[$meta])*
        #[tauri::command]
        pub fn $get_fn(state: tauri::State<'_, AppState>) -> Result<$resp, String> {
            let val = *state
                .$field
                .lock()
                .map_err(|_| format!("Failed to access {} setting", stringify!($field)))?;
            Ok($resp { $resp_field: val })
        }

        $(#[$meta])*
        #[tauri::command]
        #[allow(non_snake_case)]
        pub fn $set_fn(
            $param_name: $ty,
            state: tauri::State<'_, AppState>,
        ) -> Result<$resp, String> {
            {
                let mut guard = state
                    .$field
                    .lock()
                    .map_err(|_| format!("Failed to access {} setting", stringify!($field)))?;
                *guard = $param_name;
            }

            let settings = snapshot_gui_settings(&state)?;
            save_gui_settings(&settings)?;

            Ok($resp { $resp_field: $param_name })
        }
    };
}

// ── Response types ──
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchSettingsResponse {
    pub launch_at_login: bool,
    pub silent_start: bool,
    pub show_dock_icon: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoVacuumSettingsResponse {
    pub auto_vacuum_on_rebuild: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoCheckUpdateResponse {
    pub auto_check_update: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaxResultsResponse {
    pub max_results: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcludeDirSettingsResponse {
    pub exact_dirs: Vec<String>,
    pub pattern_dirs: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcludeFileSettingsResponse {
    pub exclude_dot_files: bool,
    pub file_patterns: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchRootsSettingsResponse {
    pub roots: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileManagerSettingsResponse {
    pub default_folder_action: String,
    pub default_terminal_action: String,
    pub custom_folder_app: String,
    pub custom_terminal_app: String,
}

// ── Simple getter/setters using macro ──
simple_getter_setter!(
    get_auto_vacuum_settings, set_auto_vacuum_settings,
    auto_vacuum_on_rebuild, bool, AutoVacuumSettingsResponse, auto_vacuum_on_rebuild,
    autoVacuumOnRebuild
);

simple_getter_setter!(
    get_auto_check_update, set_auto_check_update,
    auto_check_update, bool, AutoCheckUpdateResponse, auto_check_update,
    autoCheckUpdate
);

simple_getter_setter!(
    get_max_results, set_max_results,
    max_results, usize, MaxResultsResponse, max_results,
    maxResults
);

// ── Window toggle shortcut (needs extra normalization + shortcut re-register) ──
#[tauri::command]
pub fn get_window_toggle_shortcut(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let shortcut = state
        .window_toggle_shortcut
        .lock()
        .map_err(|_| "Failed to access shortcut setting".to_string())?
        .clone();
    Ok(shortcut)
}

#[tauri::command]
pub fn set_window_toggle_shortcut(
    shortcut: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let normalized = window::normalize_shortcut_input(&shortcut)?;
    window::register_window_toggle_shortcut(&app, &normalized)?;
    {
        let mut guard = state
            .window_toggle_shortcut
            .lock()
            .map_err(|_| "Failed to access shortcut setting".to_string())?;
        *guard = normalized.clone();
    }

    let settings = snapshot_gui_settings(&state)?;
    save_gui_settings(&settings)?;

    Ok(normalized)
}

// ── Launch settings (needs system calls) ──
#[tauri::command]
pub fn get_launch_settings(state: tauri::State<'_, AppState>) -> Result<LaunchSettingsResponse, String> {
    let launch_at_login = *state.launch_at_login.lock().map_err(|_| "Failed to access launch-at-login setting".to_string())?;
    let silent_start = *state.silent_start.lock().map_err(|_| "Failed to access silent-start setting".to_string())?;
    let show_dock_icon = *state.show_dock_icon.lock().map_err(|_| "Failed to access show-dock-icon setting".to_string())?;
    Ok(LaunchSettingsResponse { launch_at_login, silent_start, show_dock_icon })
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn set_launch_settings(
    launchAtLogin: bool,
    silentStart: bool,
    showDockIcon: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<LaunchSettingsResponse, String> {
    apply_launch_settings(launchAtLogin)?;

    {
        let mut guard = state.launch_at_login.lock().map_err(|_| "Failed to access launch-at-login setting".to_string())?;
        *guard = launchAtLogin;
    }
    {
        let mut guard = state.silent_start.lock().map_err(|_| "Failed to access silent-start setting".to_string())?;
        *guard = silentStart;
    }
    {
        let mut guard = state.show_dock_icon.lock().map_err(|_| "Failed to access show-dock-icon setting".to_string())?;
        *guard = showDockIcon;
    }

    #[cfg(target_os = "macos")]
    {
        unsafe { crate::ffi::set_dock_flag(showDockIcon); }
        if showDockIcon {
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
            let _ = app.set_dock_visibility(true);
        } else {
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let _ = app.set_dock_visibility(false);
        }
    }

    let settings = snapshot_gui_settings(&state)?;
    save_gui_settings(&settings)?;

    Ok(LaunchSettingsResponse { launch_at_login: launchAtLogin, silent_start: silentStart, show_dock_icon: showDockIcon })
}

// ── Exclude dir settings (needs engine sync) ──
#[tauri::command]
pub fn get_exclude_dir_settings(state: tauri::State<'_, AppState>) -> Result<ExcludeDirSettingsResponse, String> {
    let exact_dirs = state.exclude_exact_dirs.lock().map_err(|_| "Failed to access exact exclude directories".to_string())?.clone();
    let pattern_dirs = state.exclude_pattern_dirs.lock().map_err(|_| "Failed to access pattern exclude directories".to_string())?.clone();
    Ok(ExcludeDirSettingsResponse { exact_dirs, pattern_dirs })
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn set_exclude_dir_settings(
    exactDirs: Vec<String>,
    patternDirs: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<ExcludeDirSettingsResponse, String> {
    let (saved_exact_dirs, saved_pattern_dirs) = state.engine.set_exclude_dir_settings(exactDirs, patternDirs)?;
    {
        let mut guard = state.exclude_exact_dirs.lock().map_err(|_| "Failed to access exact exclude directories".to_string())?;
        *guard = saved_exact_dirs.clone();
    }
    {
        let mut guard = state.exclude_pattern_dirs.lock().map_err(|_| "Failed to access pattern exclude directories".to_string())?;
        *guard = saved_pattern_dirs.clone();
    }
    let settings = snapshot_gui_settings(&state)?;
    save_gui_settings(&settings)?;
    Ok(ExcludeDirSettingsResponse { exact_dirs: saved_exact_dirs, pattern_dirs: saved_pattern_dirs })
}

// ── Exclude file settings (needs engine sync) ──
#[tauri::command]
pub fn get_exclude_file_settings(state: tauri::State<'_, AppState>) -> Result<ExcludeFileSettingsResponse, String> {
    let (exclude_dot_files, file_patterns) = state.engine.get_exclude_file_settings();
    Ok(ExcludeFileSettingsResponse { exclude_dot_files, file_patterns })
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn set_exclude_file_settings(
    excludeDotFiles: bool,
    filePatterns: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<ExcludeFileSettingsResponse, String> {
    let (saved_dot_files, saved_patterns) = state.engine.set_exclude_file_settings(excludeDotFiles, filePatterns)?;
    {
        let mut guard = state.exclude_dot_files.lock().map_err(|_| "Failed to access exclude dot files setting".to_string())?;
        *guard = saved_dot_files;
    }
    {
        let mut guard = state.exclude_file_patterns.lock().map_err(|_| "Failed to access exclude file patterns setting".to_string())?;
        *guard = saved_patterns.clone();
    }
    let settings = snapshot_gui_settings(&state)?;
    save_gui_settings(&settings)?;
    Ok(ExcludeFileSettingsResponse { exclude_dot_files: saved_dot_files, file_patterns: saved_patterns })
}

// ── Watch roots (needs engine sync) ──
#[tauri::command]
pub fn get_watch_roots_settings(state: tauri::State<'_, AppState>) -> Result<WatchRootsSettingsResponse, String> {
    let roots = state.watch_roots.lock().map_err(|_| "Failed to access watch roots".to_string())?.clone();
    Ok(WatchRootsSettingsResponse { roots })
}

#[tauri::command]
pub fn set_watch_roots_settings(
    roots: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<WatchRootsSettingsResponse, String> {
    let saved_roots = state.engine.set_watch_roots(roots);
    {
        let mut guard = state.watch_roots.lock().map_err(|_| "Failed to access watch roots".to_string())?;
        *guard = saved_roots.clone();
    }
    let settings = snapshot_gui_settings(&state)?;
    save_gui_settings(&settings)?;
    Ok(WatchRootsSettingsResponse { roots: saved_roots })
}

// ── File manager settings ──
#[tauri::command]
pub fn get_file_manager_settings(state: tauri::State<'_, AppState>) -> Result<FileManagerSettingsResponse, String> {
    let default_folder_action = state.default_folder_action.lock().map_err(|_| "Failed to access default folder action".to_string())?.clone();
    let default_terminal_action = state.default_terminal_action.lock().map_err(|_| "Failed to access default terminal action".to_string())?.clone();
    let custom_folder_app = state.custom_folder_app.lock().map_err(|_| "Failed to access custom folder app".to_string())?.clone();
    let custom_terminal_app = state.custom_terminal_app.lock().map_err(|_| "Failed to access custom terminal app".to_string())?.clone();
    Ok(FileManagerSettingsResponse { default_folder_action, default_terminal_action, custom_folder_app, custom_terminal_app })
}

#[tauri::command]
pub fn set_file_manager_settings(
    default_folder_action: String,
    default_terminal_action: String,
    custom_folder_app: String,
    custom_terminal_app: String,
    state: tauri::State<'_, AppState>,
) -> Result<FileManagerSettingsResponse, String> {
    {
        let mut guard = state.default_folder_action.lock().map_err(|_| "Failed to access default folder action".to_string())?;
        *guard = default_folder_action.clone();
    }
    {
        let mut guard = state.default_terminal_action.lock().map_err(|_| "Failed to access default terminal action".to_string())?;
        *guard = default_terminal_action.clone();
    }
    {
        let mut guard = state.custom_folder_app.lock().map_err(|_| "Failed to access custom folder app".to_string())?;
        *guard = custom_folder_app.clone();
    }
    {
        let mut guard = state.custom_terminal_app.lock().map_err(|_| "Failed to access custom terminal app".to_string())?;
        *guard = custom_terminal_app.clone();
    }

    let settings = snapshot_gui_settings(&state)?;
    save_gui_settings(&settings)?;

    Ok(FileManagerSettingsResponse { default_folder_action, default_terminal_action, custom_folder_app, custom_terminal_app })
}

// ── Search / Index / Watch commands ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    query: String,
    mode: SearchMode,
    regex_enabled: Option<bool>,
    case_sensitive: Option<bool>,
    path_prefix: Option<String>,
    include_files: Option<bool>,
    include_dirs: Option<bool>,
    limit: Option<usize>,
    extensions: Option<Vec<String>>,
    sort_key: Option<String>,
    sort_ascending: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
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
pub struct SearchResponse {
    items: Vec<SearchResultItem>,
    total: usize,
    took_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitResponse {
    indexed: usize,
    has_index: bool,
    last_event_id: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildResponse {
    indexed: usize,
    took_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildEvent {
    phase: String,
    indexed: Option<usize>,
    took_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchResponse {
    running: bool,
    mode: String,
    code: String,
    message: String,
    last_event_id: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResponse {
    has_update: bool,
    latest_version: String,
}

fn watch_response(running: bool, mode: &str, last_event_id: Option<u64>) -> WatchResponse {
    let code = if running { "running" } else { "stopped" };
    let message = if running {
        match last_event_id {
            Some(id) => format!("Watcher running (EventID {})", id),
            None => "Watcher running".to_string(),
        }
    } else {
        "Watcher stopped".to_string()
    };
    WatchResponse { running, mode: mode.to_string(), code: code.to_string(), message, last_event_id }
}

fn to_search_options(req: &SearchRequest, mode: SearchMode, limit: Option<usize>) -> SearchOptions {
    let sort_key = match req.sort_key.as_deref() {
        Some("path") => SortKey::Path,
        Some("type") => SortKey::Type,
        Some("size") => SortKey::Size,
        Some("modified") => SortKey::Modified,
        _ => SortKey::Name,
    };
    SearchOptions {
        query: req.query.clone(),
        mode,
        case_sensitive: req.case_sensitive.unwrap_or(false),
        path_prefix: req.path_prefix.as_ref().map(PathBuf::from),
        include_files: req.include_files.unwrap_or(true),
        include_dirs: req.include_dirs.unwrap_or(true),
        limit,
        extensions: req.extensions.clone(),
        sort_key,
        sort_ascending: req.sort_ascending.unwrap_or(true),
    }
}

fn sort_results(items: &mut [SearchResultItem], key: SortKey, ascending: bool) {
    match key {
        SortKey::Name => items.sort_by(|a, b| {
            let cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
            if ascending { cmp } else { cmp.reverse() }
        }),
        SortKey::Path => items.sort_by(|a, b| {
            let cmp = a.parent.cmp(&b.parent).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            if ascending { cmp } else { cmp.reverse() }
        }),
        SortKey::Type => items.sort_by(|a, b| {
            let ext_a = a.name.rfind('.').map(|i| &a.name[i+1..]).unwrap_or("");
            let ext_b = b.name.rfind('.').map(|i| &b.name[i+1..]).unwrap_or("");
            let cmp = ext_a.cmp(ext_b).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            if ascending { cmp } else { cmp.reverse() }
        }),
        SortKey::Size => items.sort_by(|a, b| {
            let cmp = a.size_bytes.unwrap_or(0).cmp(&b.size_bytes.unwrap_or(0));
            if ascending { cmp } else { cmp.reverse() }
        }),
        SortKey::Modified => items.sort_by(|a, b| {
            let cmp = a.modified_unix_ms.unwrap_or(0).cmp(&b.modified_unix_ms.unwrap_or(0));
            if ascending { cmp } else { cmp.reverse() }
        }),
    }
}

fn map_result(path: PathBuf) -> SearchResultItem {
    use std::time::UNIX_EPOCH;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    let parent = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let metadata = std::fs::metadata(&path).ok();
    let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
    let is_file = metadata.as_ref().map(|m| m.is_file()).unwrap_or(false);
    let size_bytes = if is_file { metadata.as_ref().map(|m| m.len()) } else { None };
    let modified_unix_ms = metadata.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(UNIX_EPOCH).ok()).and_then(|d| u64::try_from(d.as_millis()).ok());
    SearchResultItem { name, path: path.to_string_lossy().to_string(), parent, is_dir, is_file, size_bytes, modified_unix_ms }
}

#[tauri::command]
pub fn list_path_suggestions() -> Vec<String> {
    fn add_path_if_dir(out: &mut BTreeSet<String>, path: &std::path::Path) {
        if matches!(path.to_str(), Some("/Volumes") | Some("/Volumes/Macintosh HD")) { return; }
        if path.is_dir() { out.insert(path.to_string_lossy().to_string()); }
    }
    let mut out = BTreeSet::new();
    if let Ok(home) = std::env::var("HOME") { add_path_if_dir(&mut out, PathBuf::from(home).as_path()); }
    if let Ok(entries) = fs::read_dir("/Volumes") {
        for entry in entries.flatten().take(8) { add_path_if_dir(&mut out, entry.path().as_path()); }
    }
    out.into_iter().collect()
}

#[tauri::command]
pub async fn initialize(state: tauri::State<'_, AppState>) -> Result<InitResponse, String> {
    let engine = state.engine.clone();
    let indexed = tauri::async_runtime::spawn_blocking(move || engine.load_index_from_db())
        .await.map_err(|e| e.to_string())?;
    state.index_loaded.store(true, std::sync::atomic::Ordering::Relaxed);
    let last_event_id = state.engine.load_last_event_id();
    Ok(InitResponse { indexed, has_index: indexed > 0, last_event_id })
}

#[tauri::command]
pub async fn search(request: SearchRequest, state: tauri::State<'_, AppState>) -> Result<SearchResponse, String> {
    let engine = state.engine.clone();
    let query_limit = request.limit;
    let regex_enabled = request.regex_enabled.unwrap_or(false);
    let started = Instant::now();
    let mut items = tauri::async_runtime::spawn_blocking(move || {
        if regex_enabled && request.mode == SearchMode::Pattern {
            let substring_options = to_search_options(&request, SearchMode::Substring, query_limit);
            let regex_options = to_search_options(&request, SearchMode::Pattern, query_limit);
            let mut merged = Vec::<SearchResultItem>::new();
            let mut seen = HashSet::<PathBuf>::new();
            for path in engine.search(substring_options) {
                if seen.insert(path.clone()) { merged.push(map_result(path)); }
                if let Some(limit) = query_limit { if merged.len() >= limit { break; } }
            }
            if !matches!(query_limit, Some(0)) && query_limit.map(|limit| merged.len() < limit).unwrap_or(true) {
                for path in engine.search(regex_options) {
                    if seen.insert(path.clone()) { merged.push(map_result(path)); }
                    if let Some(limit) = query_limit { if merged.len() >= limit { break; } }
                }
            }
            let sort_key = match request.sort_key.as_deref() {
                Some("path") => SortKey::Path, Some("type") => SortKey::Type,
                Some("size") => SortKey::Size, Some("modified") => SortKey::Modified,
                _ => SortKey::Name,
            };
            sort_results(&mut merged, sort_key, request.sort_ascending.unwrap_or(true));
            merged
        } else {
            let options = to_search_options(&request, request.mode, query_limit);
            engine.search(options).into_iter().map(map_result).collect()
        }
    }).await.map_err(|e| e.to_string())?;
    let total = items.len();
    let max_results = *state.max_results.lock().map_err(|_| "Failed to access max_results setting".to_string())?;
    items.truncate(max_results);
    Ok(SearchResponse { items, total, took_ms: started.elapsed().as_millis() as u64 })
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn build_index(
    path: Option<String>, rebuild: bool, includeDirs: Option<bool>,
    app: tauri::AppHandle, state: tauri::State<'_, AppState>,
) -> Result<BuildResponse, String> {
    let _ = app.emit("index://build-status", BuildEvent { phase: "started".to_string(), indexed: None, took_ms: None });
    let engine = state.engine.clone();
    let include_dirs = includeDirs.unwrap_or(true);
    let auto_vacuum_on_rebuild = *state.auto_vacuum_on_rebuild.lock().map_err(|_| "Failed to access auto-vacuum setting".to_string())?;
    let response = tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let indexed = engine.build_index(path, rebuild, include_dirs, auto_vacuum_on_rebuild);
        BuildResponse { indexed, took_ms: started.elapsed().as_millis() as u64 }
    }).await.map_err(|e| e.to_string())?;
    state.index_loaded.store(response.indexed > 0, std::sync::atomic::Ordering::Relaxed);
    let _ = app.emit("index://build-status", BuildEvent { phase: "finished".to_string(), indexed: Some(response.indexed), took_ms: Some(response.took_ms) });
    Ok(response)
}

#[tauri::command]
pub fn start_watch_auto(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> WatchResponse {
    use std::sync::atomic::Ordering;
    if state.engine.is_watch_running() {
        return WatchResponse { running: true, mode: "active".to_string(), code: "already_running".to_string(), message: "Watcher is already running".to_string(), last_event_id: state.engine.load_last_event_id() };
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
        let auto_vacuum_on_rebuild = *state.auto_vacuum_on_rebuild.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = app.emit("index://build-status", BuildEvent { phase: "started".to_string(), indexed: None, took_ms: None });
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let _ = engine_bg.build_index(None, true, true, auto_vacuum_on_rebuild);
            let indexed = engine_bg.load_index_from_db();
            let took_ms = started.elapsed().as_millis() as u64;
            let _ = app.emit("index://build-status", BuildEvent { phase: "finished".to_string(), indexed: Some(indexed), took_ms: Some(took_ms) });
        });
        return WatchResponse { running: true, mode: "bootstrap".to_string(), code: "bootstrap".to_string(), message: "Watcher started; initial index build runs in background".to_string(), last_event_id: None };
    }
    match last_event_id {
        Some(id) => {
            let current_id = unsafe { machunt::watcher::FSEventsGetCurrentEventId() };
            let stale = id < current_id && current_id - id > 50_000;
            let since = if stale {
                let engine_bg = state.engine.clone();
                let include_dirs = engine_bg.get_include_dirs();
                std::thread::spawn(move || { engine_bg.build_index(None, false, include_dirs, false); });
                None
            } else { Some(id) };
            state.engine.start_watch(since);
            WatchResponse {
                running: true,
                mode: if stale { "active".to_string() } else { "resume".to_string() },
                code: if stale { "active".to_string() } else { "resume".to_string() },
                message: if stale { format!("Watcher started (gap: {}, background catch-up running)", current_id - id) } else { format!("Watcher resumed from EventID {}", id) },
                last_event_id: if stale { None } else { Some(id) },
            }
        }
        None => {
            state.engine.start_watch(None);
            WatchResponse { running: true, mode: "active".to_string(), code: "active".to_string(), message: "Watcher started (startup cleanup skipped for fast launch)".to_string(), last_event_id: None }
        }
    }
}

#[tauri::command]
pub fn watch_status(state: tauri::State<'_, AppState>) -> WatchResponse {
    use std::sync::atomic::Ordering;
    let running = state.engine.is_watch_running();
    state.watch_started.store(running, Ordering::Relaxed);
    watch_response(running, "status", state.engine.load_last_event_id())
}

#[tauri::command]
pub fn stop_watch(state: tauri::State<'_, AppState>) -> WatchResponse {
    use std::sync::atomic::Ordering;
    let running = state.engine.is_watch_running() || state.watch_started.load(Ordering::Relaxed);
    if !running {
        return WatchResponse { running: false, mode: "inactive".to_string(), code: "not_running".to_string(), message: "Watcher is not running".to_string(), last_event_id: state.engine.load_last_event_id() };
    }
    if state.engine.stop_watch() {
        state.engine.save_last_event_id_from_runtime();
        state.watch_started.store(false, Ordering::SeqCst);
        return watch_response(false, "stopped", state.engine.load_last_event_id());
    }
    WatchResponse { running: true, mode: "stopping".to_string(), code: "stopping".to_string(), message: "Watcher is stopping...".to_string(), last_event_id: state.engine.load_last_event_id() }
}

#[tauri::command]
pub fn persist_watch_cursor(state: tauri::State<'_, AppState>) {
    state.engine.save_last_event_id_from_runtime();
}

#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<UpdateCheckResponse, String> {
    let current = app.package_info().version.to_string();
    let url = "https://api.github.com/repos/dacj4n/MacHunt/releases/latest";
    let client = reqwest::Client::builder().user_agent(format!("MacHunt/{}", current)).build().map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let resp = client.get(url).send().await.map_err(|e| format!("Failed to check for updates: {}", e))?;
    let json: serde_json::Value = resp.json().await.map_err(|e| format!("Failed to parse update response: {}", e))?;
    let tag = json["tag_name"].as_str().unwrap_or("").trim_start_matches('v');
    let has_update = match (semver(&current), semver(tag)) {
        (Some(cur), Some(latest)) => latest > cur,
        _ => tag != current,
    };
    Ok(UpdateCheckResponse { has_update, latest_version: if has_update { format!("v{}", tag) } else { format!("v{}", current) } })
}

fn semver(s: &str) -> Option<(u32, u32, u32)> {
    // Extract the first three numeric components, ignoring suffixes like "_fix"
    let numeric: Vec<u32> = s.split('.')
        .flat_map(|p| p.split(|c: char| !c.is_ascii_digit()).next())
        .filter_map(|n| n.parse::<u32>().ok())
        .take(3)
        .collect();
    if numeric.len() == 3 {
        Some((numeric[0], numeric[1], numeric[2]))
    } else {
        None
    }
}

#[tauri::command]
pub fn toggle_main_window(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<bool, String> {
    window::toggle_main_window_internal(&app, &state)
}

#[tauri::command]
pub fn get_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn set_menu_language(language: String, app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Persist the choice in AppState
    {
        let mut guard = state.language.lock().map_err(|_| "Failed to access language setting".to_string())?;
        *guard = language.clone();
    }

    let lang_ref = language.as_str();

    // Update the macOS application menu ("Preferences" / "偏好设置")
    use crate::menu::{MENU_OPEN_SETTINGS_ID, settings_menu_text};
    if let Some(menu) = app.menu() {
        if let Some(item) = menu.get(MENU_OPEN_SETTINGS_ID) {
            if let Some(menu_item) = item.as_menuitem() {
                menu_item.set_text(settings_menu_text(Some(lang_ref)))
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    // Update the existing tray menu text — no new NSStatusItem
    crate::tray::update_tray_menu_language(&app);

    Ok(())
}

// ── App groups ──
use machunt::{app_for_extension, app_for_extension_en, group_by_app, group_by_app_en};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppGroupItem {
    app_name: String,
    count: usize,
    extensions: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppGroupResponse {
    groups: Vec<AppGroupItem>,
}

#[tauri::command]
pub async fn list_app_groups(state: tauri::State<'_, AppState>, language: Option<String>) -> Result<AppGroupResponse, String> {
    use std::collections::HashMap;
    let engine = state.engine.clone();
    let paths = tauri::async_runtime::spawn_blocking(move || {
        engine.search(SearchOptions {
            query: String::new(), mode: SearchMode::Substring, case_sensitive: false,
            path_prefix: None, include_files: true, include_dirs: false,
            limit: Some(50000), extensions: None, sort_key: SortKey::Name, sort_ascending: true,
        })
    }).await.map_err(|e| e.to_string())?;
    let is_en = language.as_deref() == Some("en");
    let mut groups: HashMap<String, (usize, Vec<String>)> = HashMap::new();
    for path in &paths {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let ext = machunt::extension_of(name);
        if ext.is_empty() { continue; }
        let app = if is_en { app_for_extension_en(&ext) } else { app_for_extension(&ext) };
        let entry = groups.entry(app).or_default();
        entry.0 += 1;
        if !entry.1.contains(&ext) { entry.1.push(ext); }
    }
    let mut group_vec: Vec<AppGroupItem> = groups.into_iter().map(|(app_name, (count, exts))| {
        let mut sorted_exts = exts; sorted_exts.sort();
        AppGroupItem { app_name, count, extensions: sorted_exts }
    }).collect();
    group_vec.sort_by(|a, b| b.count.cmp(&a.count));
    Ok(AppGroupResponse { groups: group_vec })
}

#[tauri::command]
pub fn group_results_by_app(paths: Vec<String>, language: Option<String>) -> Result<Vec<(String, Vec<String>)>, String> {
    let is_en = language.as_deref() == Some("en");
    if is_en { Ok(group_by_app_en(&paths)) } else { Ok(group_by_app(&paths)) }
}

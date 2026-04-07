use machunt::{Engine, SearchMode, SearchOptions};
#[cfg(target_os = "macos")]
use objc2_service_management::{SMAppService, SMAppServiceStatus};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
#[cfg(target_os = "macos")]
use std::ffi::CString;
use std::fs;
use std::io::Write;
#[cfg(target_os = "macos")]
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Instant, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const DEFAULT_WINDOW_TOGGLE_SHORTCUT: &str = "CmdOrCtrl+Shift+KeyF";
const EVENT_OPEN_SETTINGS: &str = "app://open-settings";
const EVENT_FOCUS_SEARCH: &str = "app://focus-search";
const MENU_OPEN_SETTINGS_ID: &str = "open_settings";
#[cfg(target_os = "macos")]
const AUTOSTART_LAUNCH_AGENT_LABEL: &str = "com.dacj4n.machunt.autostart";
#[cfg(target_os = "macos")]
const DEFAULT_LOGIN_ITEM_NAME: &str = "MacHunt";

fn default_auto_vacuum_on_rebuild() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct GuiSettings {
    window_toggle_shortcut: String,
    launch_at_login: bool,
    silent_start: bool,
    #[serde(default = "default_auto_vacuum_on_rebuild")]
    auto_vacuum_on_rebuild: bool,
    exclude_exact_dirs: Vec<String>,
    exclude_pattern_dirs: Vec<String>,
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            window_toggle_shortcut: DEFAULT_WINDOW_TOGGLE_SHORTCUT.to_string(),
            launch_at_login: false,
            silent_start: false,
            auto_vacuum_on_rebuild: default_auto_vacuum_on_rebuild(),
            exclude_exact_dirs: Vec::new(),
            exclude_pattern_dirs: Vec::new(),
        }
    }
}

fn gui_settings_path() -> PathBuf {
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home_dir)
        .join(".machunt")
        .join("gui")
        .join("settings.json")
}

fn load_gui_settings() -> GuiSettings {
    let path = gui_settings_path();
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(_) => return GuiSettings::default(),
    };

    let mut settings = serde_json::from_str::<GuiSettings>(&raw).unwrap_or_default();
    if settings.window_toggle_shortcut.trim().is_empty() {
        settings.window_toggle_shortcut = DEFAULT_WINDOW_TOGGLE_SHORTCUT.to_string();
    }
    settings
}

fn save_gui_settings(settings: &GuiSettings) -> Result<(), String> {
    let path = gui_settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

fn snapshot_gui_settings(state: &AppState) -> Result<GuiSettings, String> {
    let window_toggle_shortcut = state
        .window_toggle_shortcut
        .lock()
        .map_err(|_| "Failed to access shortcut setting".to_string())?
        .clone();
    let launch_at_login = *state
        .launch_at_login
        .lock()
        .map_err(|_| "Failed to access launch-at-login setting".to_string())?;
    let silent_start = *state
        .silent_start
        .lock()
        .map_err(|_| "Failed to access silent-start setting".to_string())?;
    let exclude_exact_dirs = state
        .exclude_exact_dirs
        .lock()
        .map_err(|_| "Failed to access exact exclude directories".to_string())?
        .clone();
    let exclude_pattern_dirs = state
        .exclude_pattern_dirs
        .lock()
        .map_err(|_| "Failed to access pattern exclude directories".to_string())?
        .clone();
    let auto_vacuum_on_rebuild = *state
        .auto_vacuum_on_rebuild
        .lock()
        .map_err(|_| "Failed to access auto-vacuum setting".to_string())?;

    Ok(GuiSettings {
        window_toggle_shortcut,
        launch_at_login,
        silent_start,
        auto_vacuum_on_rebuild,
        exclude_exact_dirs,
        exclude_pattern_dirs,
    })
}

struct AppState {
    engine: Engine,
    watch_started: AtomicBool,
    index_loaded: AtomicBool,
    window_toggle_shortcut: Mutex<String>,
    launch_at_login: Mutex<bool>,
    silent_start: Mutex<bool>,
    auto_vacuum_on_rebuild: Mutex<bool>,
    exclude_exact_dirs: Mutex<Vec<String>>,
    exclude_pattern_dirs: Mutex<Vec<String>>,
    is_quitting: AtomicBool,
}

impl AppState {
    fn new() -> Self {
        let mut settings = load_gui_settings();
        let engine = Engine::new(false);
        let (legacy_exact_dirs, legacy_pattern_dirs) = engine.get_exclude_dir_settings();
        if settings.exclude_exact_dirs.is_empty()
            && settings.exclude_pattern_dirs.is_empty()
            && (!legacy_exact_dirs.is_empty() || !legacy_pattern_dirs.is_empty())
        {
            settings.exclude_exact_dirs = legacy_exact_dirs;
            settings.exclude_pattern_dirs = legacy_pattern_dirs;
            let _ = save_gui_settings(&settings);
        }
        let (exclude_exact_dirs, exclude_pattern_dirs) = engine
            .set_exclude_dir_settings(
                settings.exclude_exact_dirs.clone(),
                settings.exclude_pattern_dirs.clone(),
            )
            .unwrap_or_else(|_| (Vec::new(), Vec::new()));
        Self {
            engine,
            watch_started: AtomicBool::new(false),
            index_loaded: AtomicBool::new(false),
            window_toggle_shortcut: Mutex::new(settings.window_toggle_shortcut),
            launch_at_login: Mutex::new(settings.launch_at_login),
            silent_start: Mutex::new(settings.silent_start),
            auto_vacuum_on_rebuild: Mutex::new(settings.auto_vacuum_on_rebuild),
            exclude_exact_dirs: Mutex::new(exclude_exact_dirs),
            exclude_pattern_dirs: Mutex::new(exclude_pattern_dirs),
            is_quitting: AtomicBool::new(false),
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn open_quicklook(paths: *const *const c_char, len: usize, index: usize) -> bool;
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WatchResponse {
    running: bool,
    mode: String,
    message: String,
    last_event_id: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchSettingsResponse {
    launch_at_login: bool,
    silent_start: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AutoVacuumSettingsResponse {
    auto_vacuum_on_rebuild: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExcludeDirSettingsResponse {
    exact_dirs: Vec<String>,
    pattern_dirs: Vec<String>,
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

fn normalize_shortcut_input(raw: &str) -> Result<String, String> {
    let shortcut = raw.trim();
    if shortcut.is_empty() {
        return Err("Shortcut cannot be empty".to_string());
    }
    let _: tauri_plugin_global_shortcut::Shortcut = shortcut
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| e.to_string())?;
    Ok(shortcut.to_string())
}

fn show_main_window_internal<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        let _ = app.set_dock_visibility(true);
    }

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    window.show().map_err(|e| e.to_string())?;
    let _ = window.unminimize();
    window.set_focus().map_err(|e| e.to_string())?;
    let _ = app.emit(EVENT_FOCUS_SEARCH, ());
    Ok(())
}

fn hide_main_window_internal<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        let _ = app.set_dock_visibility(false);
    }

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    window.hide().map_err(|e| e.to_string())
}

fn toggle_main_window_internal<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<bool, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    if window.is_visible().map_err(|e| e.to_string())? {
        hide_main_window_internal(app)?;
        return Ok(false);
    }

    show_main_window_internal(app)?;
    Ok(true)
}

fn register_window_toggle_shortcut<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    shortcut: &str,
) -> Result<(), String> {
    let manager = app.global_shortcut();
    manager.unregister_all().map_err(|e| e.to_string())?;
    manager
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = toggle_main_window_internal(app);
            }
        })
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn applescript_escape(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('\"', "\\\"")
}

#[cfg(target_os = "macos")]
fn legacy_launch_agent_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{}.plist", AUTOSTART_LAUNCH_AGENT_LABEL)),
    )
}

#[cfg(target_os = "macos")]
fn cleanup_legacy_launch_agent_file() {
    if let Some(path) = legacy_launch_agent_path() {
        let _ = fs::remove_file(path);
    }
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str) -> Result<(), String> {
    let status = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Failed to update login item via System Events".to_string())
    }
}

#[cfg(target_os = "macos")]
fn macos_major_version() -> Option<u32> {
    let output = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    let major = raw.trim().split('.').next()?;
    major.parse::<u32>().ok()
}

#[cfg(target_os = "macos")]
fn supports_smappservice() -> bool {
    macos_major_version()
        .map(|major| major >= 13)
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn apply_launch_settings_via_service_management(launch_at_login: bool) -> Result<(), String> {
    let service = unsafe { SMAppService::mainAppService() };
    let status = unsafe { service.status() };
    let is_registered = status.0 == SMAppServiceStatus::Enabled.0
        || status.0 == SMAppServiceStatus::RequiresApproval.0;

    if launch_at_login {
        if is_registered {
            return Ok(());
        }
        unsafe { service.registerAndReturnError() }.map_err(|err| {
            format!("Failed to register login item via ServiceManagement: {err:?}")
        })?;
        return Ok(());
    }

    if !is_registered {
        return Ok(());
    }

    unsafe { service.unregisterAndReturnError() }
        .map_err(|err| format!("Failed to unregister login item via ServiceManagement: {err:?}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn current_bundle_path() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundle = exe_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .ok_or_else(|| "Failed to resolve bundle path".to_string())?
        .to_path_buf();
    if bundle.extension().and_then(|ext| ext.to_str()) != Some("app") {
        return Err("Launch at login requires running the bundled .app".to_string());
    }
    Ok(bundle)
}

#[cfg(target_os = "macos")]
fn remove_login_item_by_name(name: &str) -> Result<(), String> {
    let escaped_name = applescript_escape(name);
    let script = format!(
        "tell application \"System Events\"\n\
         if exists login item \"{name}\" then\n\
           delete login item \"{name}\"\n\
         end if\n\
         end tell",
        name = escaped_name
    );
    run_osascript(&script)
}

#[cfg(target_os = "macos")]
fn apply_launch_settings_via_system_events(launch_at_login: bool) -> Result<(), String> {
    let mut cleanup_names = vec![DEFAULT_LOGIN_ITEM_NAME.to_string()];
    if let Ok(bundle_path) = current_bundle_path() {
        if let Some(bundle_name) = bundle_path.file_stem().and_then(|s| s.to_str()) {
            if !cleanup_names.iter().any(|name| name == bundle_name) {
                cleanup_names.push(bundle_name.to_string());
            }
        }
    }

    for name in &cleanup_names {
        remove_login_item_by_name(name)?;
    }

    if !launch_at_login {
        return Ok(());
    }

    let bundle_path = current_bundle_path()?;
    let bundle_name = bundle_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(DEFAULT_LOGIN_ITEM_NAME);
    let script = format!(
        "tell application \"System Events\"\n\
         make login item at end with properties {{name:\"{name}\", path:\"{path}\"}}\n\
         end tell",
        name = applescript_escape(bundle_name),
        path = applescript_escape(&bundle_path.to_string_lossy())
    );
    run_osascript(&script)
}

#[cfg(target_os = "macos")]
fn apply_launch_settings(launch_at_login: bool) -> Result<(), String> {
    cleanup_legacy_launch_agent_file();

    if supports_smappservice() {
        return apply_launch_settings_via_service_management(launch_at_login);
    }

    apply_launch_settings_via_system_events(launch_at_login)
}

#[cfg(not(target_os = "macos"))]
fn apply_launch_settings(_launch_at_login: bool) -> Result<(), String> {
    Ok(())
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
fn preview_search_result(paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let mut c_paths = Vec::new();
        for path in paths {
            let target = PathBuf::from(path);
            if !target.exists() {
                continue;
            }
            let c_path = CString::new(target.to_string_lossy().into_owned())
                .map_err(|_| "Target path contains NUL byte".to_string())?;
            c_paths.push(c_path);
        }
        if c_paths.is_empty() {
            return Err("Target path does not exist".to_string());
        }

        let raw_paths: Vec<*const c_char> = c_paths.iter().map(|p| p.as_ptr()).collect();
        let opened = unsafe { open_quicklook(raw_paths.as_ptr(), raw_paths.len(), 0) };
        if opened {
            Ok(())
        } else {
            Err("Failed to open native Quick Look preview".to_string())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = paths;
        Err("Quick Look preview is only supported on macOS".to_string())
    }
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
        let _ = activate_application("Finder");
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
        let _ = activate_application("QSpace Pro");
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

fn is_process_running(process_name: &str) -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg(process_name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn is_wezterm_running() -> bool {
    is_process_running("wezterm-gui")
        || is_process_running("wezterm")
        || is_process_running("WezTerm")
}

#[cfg(target_os = "macos")]
fn activate_application(app_name: &str) -> Result<(), String> {
    let script = format!(
        "tell application \"{}\" to activate",
        applescript_escape(app_name)
    );
    let status = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Failed to activate {}", app_name))
    }
}

#[cfg(not(target_os = "macos"))]
fn activate_application(_app_name: &str) -> Result<(), String> {
    Ok(())
}

fn wezterm_executable_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from("wezterm")];

    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from(
            "/Applications/WezTerm.app/Contents/MacOS/wezterm",
        ));
        if let Ok(home) = std::env::var("HOME") {
            candidates.push(
                PathBuf::from(home)
                    .join("Applications")
                    .join("WezTerm.app")
                    .join("Contents")
                    .join("MacOS")
                    .join("wezterm"),
            );
        }
    }

    candidates
}

fn try_spawn_wezterm_tab(open_target: &Path) -> bool {
    for executable in wezterm_executable_candidates() {
        let status = Command::new(&executable)
            .arg("cli")
            .arg("spawn")
            .arg("--cwd")
            .arg(open_target)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        if let Ok(status) = status {
            if status.success() {
                return true;
            }
        }
    }

    false
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
        let _ = activate_application("Terminal");
        Ok(())
    } else {
        Err("Failed to open in Terminal".to_string())
    }
}

#[tauri::command]
fn open_in_wezterm(path: String) -> Result<(), String> {
    let open_target = open_container_path(&path)?;

    if is_wezterm_running() && try_spawn_wezterm_tab(&open_target) {
        let _ = activate_application("WezTerm");
        return Ok(());
    }

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
        let _ = activate_application("WezTerm");
        Ok(())
    } else {
        Err("Failed to open in WezTerm (check whether WezTerm is installed)".to_string())
    }
}

#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    let mut child = match Command::new("/usr/bin/pbcopy")
        .stdin(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => Command::new("pbcopy")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to launch pbcopy: {}", e))?,
    };

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("Failed to write clipboard data: {}", e))?;
    } else {
        return Err("Unable to access clipboard pipe".to_string());
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed waiting for pbcopy: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "pbcopy exited with status {}",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ))
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
                && query_limit
                    .map(|limit| merged.len() < limit)
                    .unwrap_or(true)
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
    let auto_vacuum_on_rebuild = *state
        .auto_vacuum_on_rebuild
        .lock()
        .map_err(|_| "Failed to access auto-vacuum setting".to_string())?;
    let response = tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let indexed = engine.build_index(path, rebuild, include_dirs, auto_vacuum_on_rebuild);
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
        let auto_vacuum_on_rebuild = *state
            .auto_vacuum_on_rebuild
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::thread::spawn(move || {
            let _ = engine_bg.build_index(None, true, true, auto_vacuum_on_rebuild);
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

#[tauri::command]
fn get_window_toggle_shortcut(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let shortcut = state
        .window_toggle_shortcut
        .lock()
        .map_err(|_| "Failed to access shortcut setting".to_string())?
        .clone();
    Ok(shortcut)
}

#[tauri::command]
fn set_window_toggle_shortcut(
    shortcut: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let normalized = normalize_shortcut_input(&shortcut)?;
    register_window_toggle_shortcut(&app, &normalized)?;
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

#[tauri::command]
fn get_launch_settings(
    state: tauri::State<'_, AppState>,
) -> Result<LaunchSettingsResponse, String> {
    let launch_at_login = *state
        .launch_at_login
        .lock()
        .map_err(|_| "Failed to access launch-at-login setting".to_string())?;
    let silent_start = *state
        .silent_start
        .lock()
        .map_err(|_| "Failed to access silent-start setting".to_string())?;

    Ok(LaunchSettingsResponse {
        launch_at_login,
        silent_start,
    })
}

#[tauri::command]
fn set_launch_settings(
    launch_at_login: bool,
    silent_start: bool,
    state: tauri::State<'_, AppState>,
) -> Result<LaunchSettingsResponse, String> {
    apply_launch_settings(launch_at_login)?;

    {
        let mut guard = state
            .launch_at_login
            .lock()
            .map_err(|_| "Failed to access launch-at-login setting".to_string())?;
        *guard = launch_at_login;
    }
    {
        let mut guard = state
            .silent_start
            .lock()
            .map_err(|_| "Failed to access silent-start setting".to_string())?;
        *guard = silent_start;
    }

    let settings = snapshot_gui_settings(&state)?;
    save_gui_settings(&settings)?;

    Ok(LaunchSettingsResponse {
        launch_at_login,
        silent_start,
    })
}

#[tauri::command]
fn get_auto_vacuum_settings(
    state: tauri::State<'_, AppState>,
) -> Result<AutoVacuumSettingsResponse, String> {
    let auto_vacuum_on_rebuild = *state
        .auto_vacuum_on_rebuild
        .lock()
        .map_err(|_| "Failed to access auto-vacuum setting".to_string())?;
    Ok(AutoVacuumSettingsResponse {
        auto_vacuum_on_rebuild,
    })
}

#[tauri::command]
fn set_auto_vacuum_settings(
    auto_vacuum_on_rebuild: bool,
    state: tauri::State<'_, AppState>,
) -> Result<AutoVacuumSettingsResponse, String> {
    {
        let mut guard = state
            .auto_vacuum_on_rebuild
            .lock()
            .map_err(|_| "Failed to access auto-vacuum setting".to_string())?;
        *guard = auto_vacuum_on_rebuild;
    }

    let settings = snapshot_gui_settings(&state)?;
    save_gui_settings(&settings)?;

    Ok(AutoVacuumSettingsResponse {
        auto_vacuum_on_rebuild,
    })
}

#[tauri::command]
fn get_exclude_dir_settings(
    state: tauri::State<'_, AppState>,
) -> Result<ExcludeDirSettingsResponse, String> {
    let exact_dirs = state
        .exclude_exact_dirs
        .lock()
        .map_err(|_| "Failed to access exact exclude directories".to_string())?
        .clone();
    let pattern_dirs = state
        .exclude_pattern_dirs
        .lock()
        .map_err(|_| "Failed to access pattern exclude directories".to_string())?
        .clone();

    Ok(ExcludeDirSettingsResponse {
        exact_dirs,
        pattern_dirs,
    })
}

#[tauri::command]
fn set_exclude_dir_settings(
    exact_dirs: Vec<String>,
    pattern_dirs: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<ExcludeDirSettingsResponse, String> {
    let (saved_exact_dirs, saved_pattern_dirs) = state
        .engine
        .set_exclude_dir_settings(exact_dirs, pattern_dirs)?;

    {
        let mut guard = state
            .exclude_exact_dirs
            .lock()
            .map_err(|_| "Failed to access exact exclude directories".to_string())?;
        *guard = saved_exact_dirs.clone();
    }
    {
        let mut guard = state
            .exclude_pattern_dirs
            .lock()
            .map_err(|_| "Failed to access pattern exclude directories".to_string())?;
        *guard = saved_pattern_dirs.clone();
    }

    let settings = snapshot_gui_settings(&state)?;
    save_gui_settings(&settings)?;

    Ok(ExcludeDirSettingsResponse {
        exact_dirs: saved_exact_dirs,
        pattern_dirs: saved_pattern_dirs,
    })
}

#[tauri::command]
fn toggle_main_window(app: tauri::AppHandle) -> Result<bool, String> {
    toggle_main_window_internal(&app)
}

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
    let app = tauri::Builder::default()
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
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .on_menu_event(|app, event| {
            if event.id() == MENU_OPEN_SETTINGS_ID {
                let _ = app.emit(EVENT_OPEN_SETTINGS, ());
            }
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }

            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.app_handle().state::<AppState>();
                if state.is_quitting.load(Ordering::SeqCst) {
                    return;
                }
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .manage(AppState::new())
        .setup(|app| {
            let initial_shortcut = app
                .state::<AppState>()
                .window_toggle_shortcut
                .lock()
                .map(|value| value.clone())
                .unwrap_or_else(|_| DEFAULT_WINDOW_TOGGLE_SHORTCUT.to_string());

            if register_window_toggle_shortcut(&app.handle().clone(), &initial_shortcut).is_err() {
                let fallback = DEFAULT_WINDOW_TOGGLE_SHORTCUT.to_string();
                let _ = register_window_toggle_shortcut(&app.handle().clone(), &fallback);
                if let Ok(mut guard) = app.state::<AppState>().window_toggle_shortcut.lock() {
                    *guard = fallback.clone();
                }
                if let Ok(settings) = snapshot_gui_settings(&app.state::<AppState>()) {
                    let _ = save_gui_settings(&settings);
                }
            }

            let launch_at_login = app
                .state::<AppState>()
                .launch_at_login
                .lock()
                .map(|value| *value)
                .unwrap_or(false);
            let silent_start = app
                .state::<AppState>()
                .silent_start
                .lock()
                .map(|value| *value)
                .unwrap_or(false);
            let _ = apply_launch_settings(launch_at_login);

            if silent_start {
                let _ = hide_main_window_internal(&app.handle().clone());
            }

            Ok(())
        })
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
            persist_watch_cursor,
            get_window_toggle_shortcut,
            set_window_toggle_shortcut,
            get_launch_settings,
            set_launch_settings,
            get_auto_vacuum_settings,
            set_auto_vacuum_settings,
            get_exclude_dir_settings,
            set_exclude_dir_settings,
            toggle_main_window
        ])
        .build(tauri::generate_context!())
        .expect("error while building Tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } => {
            let state = app_handle.state::<AppState>();
            state.is_quitting.store(true, Ordering::SeqCst);
            state.engine.save_last_event_id_from_runtime();
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen {
            has_visible_windows,
            ..
        } => {
            if !has_visible_windows {
                let _ = show_main_window_internal(app_handle);
            }
        }
        _ => {}
    });
}

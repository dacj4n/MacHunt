use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

const DEFAULT_WINDOW_TOGGLE_SHORTCUT: &str = "CmdOrCtrl+Shift+KeyD";

fn default_auto_vacuum_on_rebuild() -> bool {
    true
}

fn default_auto_check_update() -> bool {
    true
}

fn default_exclude_pattern_dirs() -> Vec<String> {
    vec![
        "/System/**".to_string(),
        "/private/var/**".to_string(),
        "/private/tmp/**".to_string(),
        "/.Spotlight-V100/**".to_string(),
        "/.fseventsd/**".to_string(),
        "/dev/**".to_string(),
        "/proc/**".to_string(),
    ]
}

fn default_exclude_file_patterns() -> Vec<String> {
    vec![".DS_Store".to_string()]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GuiSettings {
    pub window_toggle_shortcut: String,
    pub launch_at_login: bool,
    pub silent_start: bool,
    pub show_dock_icon: bool,
    #[serde(default = "default_auto_vacuum_on_rebuild")]
    pub auto_vacuum_on_rebuild: bool,
    #[serde(default = "default_auto_check_update")]
    pub auto_check_update: bool,
    pub exclude_exact_dirs: Vec<String>,
    pub exclude_pattern_dirs: Vec<String>,
    pub exclude_dot_files: bool,
    #[serde(default = "default_exclude_file_patterns")]
    pub exclude_file_patterns: Vec<String>,
    pub watch_roots: Vec<String>,
    pub default_folder_action: String,
    pub default_terminal_action: String,
    pub custom_folder_app: String,
    pub custom_terminal_app: String,
    pub max_results: usize,
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            window_toggle_shortcut: DEFAULT_WINDOW_TOGGLE_SHORTCUT.to_string(),
            launch_at_login: false,
            silent_start: false,
            show_dock_icon: true,
            auto_vacuum_on_rebuild: default_auto_vacuum_on_rebuild(),
            auto_check_update: default_auto_check_update(),
            exclude_exact_dirs: Vec::new(),
            exclude_pattern_dirs: default_exclude_pattern_dirs(),
            exclude_dot_files: false,
            exclude_file_patterns: default_exclude_file_patterns(),
            watch_roots: Vec::new(),
            default_folder_action: "Finder".to_string(),
            default_terminal_action: "Terminal".to_string(),
            custom_folder_app: String::new(),
            custom_terminal_app: String::new(),
            max_results: 500,
        }
    }
}

pub fn gui_settings_path() -> PathBuf {
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home_dir)
        .join("Library")
        .join("Application Support")
        .join("MacHunt")
        .join("settings.json")
}

pub fn load_gui_settings() -> GuiSettings {
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

pub fn save_gui_settings(settings: &GuiSettings) -> Result<(), String> {
    let path = gui_settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

pub struct AppState {
    pub engine: machunt::Engine,
    pub watch_started: AtomicBool,
    pub index_loaded: AtomicBool,
    pub window_toggle_shortcut: Mutex<String>,
    pub launch_at_login: Mutex<bool>,
    pub silent_start: Mutex<bool>,
    pub show_dock_icon: Mutex<bool>,
    pub auto_vacuum_on_rebuild: Mutex<bool>,
    pub auto_check_update: Mutex<bool>,
    pub exclude_exact_dirs: Mutex<Vec<String>>,
    pub exclude_pattern_dirs: Mutex<Vec<String>>,
    pub exclude_dot_files: Mutex<bool>,
    pub exclude_file_patterns: Mutex<Vec<String>>,
    pub watch_roots: Mutex<Vec<String>>,
    pub default_folder_action: Mutex<String>,
    pub default_terminal_action: Mutex<String>,
    pub custom_folder_app: Mutex<String>,
    pub custom_terminal_app: Mutex<String>,
    pub max_results: Mutex<usize>,
    pub is_quitting: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        let mut settings = load_gui_settings();
        let engine = machunt::Engine::new(false);
        let (legacy_exact_dirs, legacy_pattern_dirs) = engine.get_exclude_dir_settings();
        if settings.exclude_exact_dirs.is_empty()
            && settings.exclude_pattern_dirs.is_empty()
            && (!legacy_exact_dirs.is_empty() || !legacy_pattern_dirs.is_empty())
        {
            settings.exclude_exact_dirs = legacy_exact_dirs;
            settings.exclude_pattern_dirs = legacy_pattern_dirs;
            let _ = save_gui_settings(&settings);
        }
        let legacy_watch_roots = engine.get_watch_roots();
        if settings.watch_roots.is_empty() && !legacy_watch_roots.is_empty() {
            settings.watch_roots = legacy_watch_roots;
            let _ = save_gui_settings(&settings);
        }
        let (exclude_exact_dirs, exclude_pattern_dirs) = engine
            .set_exclude_dir_settings(
                settings.exclude_exact_dirs.clone(),
                settings.exclude_pattern_dirs.clone(),
            )
            .unwrap_or_else(|_| (Vec::new(), Vec::new()));
        let (_exclude_dot_files, exclude_file_patterns) = engine
            .set_exclude_file_settings(
                settings.exclude_dot_files,
                settings.exclude_file_patterns.clone(),
            )
            .unwrap_or_else(|_| (false, Vec::new()));
        let watch_roots = engine.set_watch_roots(settings.watch_roots.clone());
        Self {
            engine,
            watch_started: AtomicBool::new(false),
            index_loaded: AtomicBool::new(false),
            window_toggle_shortcut: Mutex::new(settings.window_toggle_shortcut),
            launch_at_login: Mutex::new(settings.launch_at_login),
            silent_start: Mutex::new(settings.silent_start),
            show_dock_icon: Mutex::new(settings.show_dock_icon),
            auto_vacuum_on_rebuild: Mutex::new(settings.auto_vacuum_on_rebuild),
            auto_check_update: Mutex::new(settings.auto_check_update),
            exclude_exact_dirs: Mutex::new(exclude_exact_dirs),
            exclude_pattern_dirs: Mutex::new(exclude_pattern_dirs),
            exclude_dot_files: Mutex::new(settings.exclude_dot_files),
            exclude_file_patterns: Mutex::new(exclude_file_patterns),
            watch_roots: Mutex::new(watch_roots),
            default_folder_action: Mutex::new(settings.default_folder_action),
            default_terminal_action: Mutex::new(settings.default_terminal_action),
            custom_folder_app: Mutex::new(settings.custom_folder_app),
            custom_terminal_app: Mutex::new(settings.custom_terminal_app),
            max_results: Mutex::new(settings.max_results),
            is_quitting: AtomicBool::new(false),
        }
    }
}

pub fn snapshot_gui_settings(state: &AppState) -> Result<GuiSettings, String> {
    macro_rules! lock_get {
        ($field:ident, $ty:ty) => {
            state.$field.lock().map_err(|_| format!("Failed to access {} setting", stringify!($field)))?.clone()
        };
    }

    Ok(GuiSettings {
        window_toggle_shortcut: lock_get!(window_toggle_shortcut, String),
        launch_at_login: *state.launch_at_login.lock().map_err(|_| "Failed to access launch-at-login setting".to_string())?,
        silent_start: *state.silent_start.lock().map_err(|_| "Failed to access silent-start setting".to_string())?,
        show_dock_icon: *state.show_dock_icon.lock().map_err(|_| "Failed to access show-dock-icon setting".to_string())?,
        auto_vacuum_on_rebuild: *state.auto_vacuum_on_rebuild.lock().map_err(|_| "Failed to access auto-vacuum setting".to_string())?,
        auto_check_update: *state.auto_check_update.lock().map_err(|_| "Failed to access auto-check-update setting".to_string())?,
        exclude_exact_dirs: lock_get!(exclude_exact_dirs, Vec<String>),
        exclude_pattern_dirs: lock_get!(exclude_pattern_dirs, Vec<String>),
        exclude_dot_files: *state.exclude_dot_files.lock().map_err(|_| "Failed to access exclude-dot-files setting".to_string())?,
        exclude_file_patterns: lock_get!(exclude_file_patterns, Vec<String>),
        watch_roots: lock_get!(watch_roots, Vec<String>),
        default_folder_action: lock_get!(default_folder_action, String),
        default_terminal_action: lock_get!(default_terminal_action, String),
        custom_folder_app: lock_get!(custom_folder_app, String),
        custom_terminal_app: lock_get!(custom_terminal_app, String),
        max_results: *state.max_results.lock().map_err(|_| "Failed to access max_results setting".to_string())?,
    })
}

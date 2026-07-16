use crate::settings::AppState;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const EVENT_OPEN_SETTINGS: &str = "app://open-settings";
pub const EVENT_FOCUS_SEARCH: &str = "app://focus-search";

pub fn normalize_shortcut_input(raw: &str) -> Result<String, String> {
    let shortcut = raw.trim();
    if shortcut.is_empty() {
        return Err("Shortcut cannot be empty".to_string());
    }
    let _: tauri_plugin_global_shortcut::Shortcut = shortcut
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| e.to_string())?;
    Ok(shortcut.to_string())
}

pub fn show_main_window_internal<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    let show_dock = *state
        .show_dock_icon
        .lock()
        .map_err(|_| "Failed to access show-dock-icon setting".to_string())?;

    #[cfg(target_os = "macos")]
    {
        if show_dock {
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
            let _ = app.set_dock_visibility(true);
        } else {
            unsafe {
                crate::ffi::activate_ignoring_other_apps();
            }
        }
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

pub fn hide_main_window_internal<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    let show_dock = *state
        .show_dock_icon
        .lock()
        .map_err(|_| "Failed to access show-dock-icon setting".to_string())?;

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    window.hide().map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    {
        if show_dock {
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let _ = app.set_dock_visibility(false);
        } else {
            unsafe {
                crate::ffi::deactivate_app();
            }
        }
    }

    Ok(())
}

pub fn toggle_main_window_internal<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
) -> Result<bool, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    let visible = window.is_visible().map_err(|e| e.to_string())?;
    if !visible {
        show_main_window_internal(app, state)?;
        return Ok(true);
    }

    let minimized = window.is_minimized().map_err(|e| e.to_string())?;
    let focused = window.is_focused().map_err(|e| e.to_string())?;
    if minimized || !focused {
        show_main_window_internal(app, state)?;
        return Ok(true);
    }

    hide_main_window_internal(app, state)?;
    Ok(false)
}

pub fn register_window_toggle_shortcut<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    shortcut: &str,
) -> Result<(), String> {
    let manager = app.global_shortcut();
    manager.unregister_all().map_err(|e| e.to_string())?;
    manager
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let state = app.state::<AppState>();
                let _ = toggle_main_window_internal(app, &state);
            }
        })
        .map_err(|e| e.to_string())
}

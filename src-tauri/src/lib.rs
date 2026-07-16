mod settings;
mod window;
mod startup;
mod file_ops;
mod menu;
mod commands;
mod ffi;

use crate::settings::AppState;
use crate::window::{register_window_toggle_shortcut, show_main_window_internal, hide_main_window_internal};
use crate::startup::apply_launch_settings;
use crate::menu::build_menu;
use tauri::{Emitter, Manager};
use std::sync::atomic::Ordering;

const DEFAULT_WINDOW_TOGGLE_SHORTCUT: &str = "CmdOrCtrl+Shift+KeyD";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "macos")]
    unsafe {
        ffi::force_accessory_policy();
    }

    let app = tauri::Builder::default()
        .menu(|app| build_menu(app))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .on_menu_event(|app, event| {
            if event.id() == menu::MENU_OPEN_SETTINGS_ID {
                let _ = app.emit(window::EVENT_OPEN_SETTINGS, ());
            }
        })
        .on_window_event(|window, event| {
            if window.label() != "main" { return; }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.app_handle().state::<AppState>();
                if state.is_quitting.load(Ordering::SeqCst) { return; }
                api.prevent_close();
                let app_handle = window.app_handle().clone();
                let _ = hide_main_window_internal(&app_handle, &state);
            }
        })
        .manage(AppState::new())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                let show_dock = app.state::<AppState>().show_dock_icon.lock().map(|v| *v).unwrap_or(false);
                unsafe { ffi::set_dock_flag(show_dock); ffi::install_policy_guard(); }
                if show_dock {
                    let _ = app.handle().set_activation_policy(tauri::ActivationPolicy::Regular);
                    let _ = app.handle().set_dock_visibility(true);
                } else {
                    let _ = app.handle().set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }

            let initial_shortcut = app.state::<AppState>().window_toggle_shortcut.lock()
                .map(|value| value.clone())
                .unwrap_or_else(|_| DEFAULT_WINDOW_TOGGLE_SHORTCUT.to_string());

            if register_window_toggle_shortcut(&app.handle().clone(), &initial_shortcut).is_err() {
                let fallback = DEFAULT_WINDOW_TOGGLE_SHORTCUT.to_string();
                let _ = register_window_toggle_shortcut(&app.handle().clone(), &fallback);
                if let Ok(mut guard) = app.state::<AppState>().window_toggle_shortcut.lock() { *guard = fallback.clone(); }
                if let Ok(settings) = settings::snapshot_gui_settings(&app.state::<AppState>()) {
                    let _ = settings::save_gui_settings(&settings);
                }
            }

            let launch_at_login = app.state::<AppState>().launch_at_login.lock().map(|value| *value).unwrap_or(false);
            let silent_start = app.state::<AppState>().silent_start.lock().map(|value| *value).unwrap_or(false);
            let _ = apply_launch_settings(launch_at_login);

            // Volume event channel
            {
                let (tx, rx) = crossbeam::channel::unbounded::<serde_json::Value>();
                let state = app.state::<AppState>();
                state.engine.set_volume_event_tx(tx);
                let app_handle = app.handle().clone();
                std::thread::spawn(move || { for event in rx { let _ = app_handle.emit("volume://event", &event); } });
            }

            if !silent_start {
                let state = app.state::<AppState>();
                let _ = show_main_window_internal(&app.handle().clone(), &state);
            }

            // Make window draggable by background (no titlebar)
            let _ = window::make_window_movable_by_background(&app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::initialize,
            commands::search,
            commands::build_index,
            commands::start_watch_auto,
            commands::stop_watch,
            commands::watch_status,
            commands::list_path_suggestions,
            file_ops::pick_path_in_finder,
            file_ops::pick_app,
            file_ops::open_search_result,
            file_ops::preview_search_result,
            file_ops::reveal_in_finder,
            file_ops::open_in_qspace,
            file_ops::open_in_terminal,
            file_ops::open_in_wezterm,
            file_ops::open_in_default_terminal,
            file_ops::copy_to_clipboard,
            file_ops::copy_search_results,
            file_ops::move_to_trash,
            commands::set_menu_language,
            commands::persist_watch_cursor,
            commands::get_window_toggle_shortcut,
            commands::set_window_toggle_shortcut,
            commands::get_launch_settings,
            commands::set_launch_settings,
            commands::get_auto_vacuum_settings,
            commands::set_auto_vacuum_settings,
            commands::get_auto_check_update,
            commands::set_auto_check_update,
            commands::get_max_results,
            commands::set_max_results,
            commands::check_for_update,
            commands::get_exclude_dir_settings,
            commands::set_exclude_dir_settings,
            commands::get_watch_roots_settings,
            commands::set_watch_roots_settings,
            commands::get_file_manager_settings,
            commands::set_file_manager_settings,
            commands::toggle_main_window,
            commands::get_version,
            commands::list_app_groups,
            commands::group_results_by_app,
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
        tauri::RunEvent::Reopen { has_visible_windows, .. } => {
            let state = app_handle.state::<AppState>();
            let show_dock = state.show_dock_icon.lock().map(|v| *v).unwrap_or(false);
            if show_dock && !has_visible_windows {
                let _ = show_main_window_internal(app_handle, &state);
            } else if !show_dock {
                let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
        }
        _ => {}
    });
}

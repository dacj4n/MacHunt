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

/// Hide traffic light buttons, install NSVisualEffectView for system-level Liquid Glass,
/// and make the window movable by header drag.
pub fn make_window_movable_by_background(app: &tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::rc::Retained;
        use objc2::runtime::Bool;
        use objc2_foundation::{NSObject, NSRect};

        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "Main window not found".to_string())?;
        let ns_window_ptr = window.ns_window().map_err(|e| e.to_string())?;

        unsafe {
            let ns_window: Retained<NSObject> = Retained::retain(ns_window_ptr as *mut _).unwrap();

            // Hide traffic light buttons
            let _: () = msg_send![&ns_window, setTitlebarAppearsTransparent: true];
            let close_btn: *mut NSObject = msg_send![&ns_window, standardWindowButton: 0u32];
            let mini_btn: *mut NSObject = msg_send![&ns_window, standardWindowButton: 1u32];
            let zoom_btn: *mut NSObject = msg_send![&ns_window, standardWindowButton: 2u32];
            if !close_btn.is_null() { let _: () = msg_send![close_btn, setHidden: true]; }
            if !mini_btn.is_null() { let _: () = msg_send![mini_btn, setHidden: true]; }
            if !zoom_btn.is_null() { let _: () = msg_send![zoom_btn, setHidden: true]; }

            // Full size content view + transparent background
            let current_mask: u64 = msg_send![&ns_window, styleMask];
            let _: () = msg_send![&ns_window, setStyleMask: current_mask | (1u64 << 15)];
            let _: () = msg_send![&ns_window, setOpaque: Bool::NO];
            let _: () = msg_send![&ns_window, setHasShadow: Bool::NO];
            let _: () = msg_send![&ns_window, setMovableByWindowBackground: Bool::NO];
            let clear_color: *mut NSObject = msg_send![objc2::class!(NSColor), clearColor];
            let _: () = msg_send![&ns_window, setBackgroundColor: clear_color];

            // Get the WKWebView inside the content view
            let content_view: *mut NSObject = msg_send![&*ns_window, contentView];
            if !content_view.is_null() {
                let cv: Retained<NSObject> = Retained::retain(content_view).unwrap();
                let bounds: NSRect = msg_send![&*cv, bounds];
                let _: () = msg_send![&*cv, setWantsLayer: true];

                // Set content view's layer corner radius so window corners are rounded
                let cv_layer: *mut NSObject = msg_send![&*cv, layer];
                if !cv_layer.is_null() {
                    let _: () = msg_send![cv_layer, setCornerRadius: 24.0_f64];
                    let _: () = msg_send![cv_layer, setMasksToBounds: true];
                }

                // Find the WKWebView — it's the first subview of the content view in Tauri 2
                let subviews: *mut NSObject = msg_send![&*cv, subviews];
                let webview_ptr: *mut NSObject = msg_send![subviews, firstObject];
                if !webview_ptr.is_null() {
                    let wv: Retained<NSObject> = Retained::retain(webview_ptr).unwrap();

                    // Make the WKWebView layer-backed and set opaque=NO so NSVisualEffectView shows through
                    let _: () = msg_send![&*wv, setWantsLayer: true];

                    // Set WKWebView background to transparent via its layer
                    let wv_layer: *mut NSObject = msg_send![&*wv, layer];
                    if !wv_layer.is_null() {
                        let _: () = msg_send![wv_layer, setBackgroundColor: std::ptr::null::<NSObject>()];
                    }

                    // Create NSVisualEffectView for system-managed Liquid Glass
                    let vv_alloc: *mut NSObject = msg_send![objc2::class!(NSVisualEffectView), alloc];
                    let vv_ptr: *mut NSObject = msg_send![vv_alloc, initWithFrame: bounds];
                    if !vv_ptr.is_null() {
                        let vv: Retained<NSObject> = Retained::retain(vv_ptr).unwrap();

                        // NSVisualEffectMaterialToolTip = 17 (deep dark with vibrancy)
                        let _: () = msg_send![&*vv, setMaterial: 17i64];
                        // NSVisualEffectStateActive = 1
                        let _: () = msg_send![&*vv, setState: 1i64];
                        // NSVisualEffectBlendingModeBehindWindow = 0 (translucent)
                        let _: () = msg_send![&*vv, setBlendingMode: 0i64];
                        // NSViewWidthSizable(2) | NSViewHeightSizable(16) = 18
                        let _: () = msg_send![&*vv, setAutoresizingMask: 18u64];
                        let _: () = msg_send![&*vv, setWantsLayer: true];

                        // Add NSVisualEffectView BELOW the WKWebView
                        // positioned: NSWindowBelow = -1
                        let _: () = msg_send![&*cv, addSubview: &*vv, positioned: -1i64, relativeTo: &*wv];

                        // Bring WKWebView back to front (above the visual effect view)
                        // Send to front so it captures clicks
                        let _: () = msg_send![&*wv, setAutoresizingMask: 18u64];
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
    Ok(())
}

/// Set the window titlebar appearance to dark or light.
#[tauri::command]
pub fn set_window_appearance(app: tauri::AppHandle, dark: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::rc::Retained;
        use objc2_app_kit::NSAppearance;
        use objc2_foundation::NSObject;

        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "Main window not found".to_string())?;
        let ns_window_ptr = window.ns_window().map_err(|e| e.to_string())?;

        let name = unsafe {
            if dark {
                objc2_app_kit::NSAppearanceNameDarkAqua
            } else {
                objc2_app_kit::NSAppearanceNameAqua
            }
        };

        let appearance = NSAppearance::appearanceNamed(name);
        if let Some(appearance) = appearance {
            unsafe {
                let ns_window: Retained<NSObject> = Retained::retain(ns_window_ptr as *mut _).unwrap();
                let _: () = msg_send![&ns_window, setAppearance: &*appearance];
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, dark);
    }
    Ok(())
}

/// Start dragging the window. Call from header mousedown (excluding inputs/buttons).
#[tauri::command]
pub fn start_dragging(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    window.start_dragging().map_err(|e| e.to_string())
}

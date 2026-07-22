/// macOS menu bar tray icon.
/// Left-click: toggle main window → show search.
/// Right-click: context menu with "Search" / "Settings" / "Quit" (localised).

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use crate::menu::tray_labels;
use crate::window;

/// Shared helper: build the tray menu with labels matching the given language.
fn build_tray_menu(app: &tauri::AppHandle, language: Option<&str>) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let labels = tray_labels(language);

    let search_item = MenuItemBuilder::with_id("tray_search", labels.search)
        .build(app)?;

    let pinned_item = MenuItemBuilder::with_id("tray_pinned", labels.pinned)
        .build(app)?;

    let settings_item = MenuItemBuilder::with_id("tray_settings", labels.settings)
        .accelerator("Cmd+,")
        .build(app)?;

    let quit_item = MenuItemBuilder::with_id("tray_quit", labels.quit)
        .accelerator("Cmd+Q")
        .build(app)?;

    let tray_menu = MenuBuilder::new(app)
        .item(&search_item)
        .item(&pinned_item)
        .separator()
        .item(&settings_item)
        .separator()
        .item(&quit_item)
        .build()?;

    Ok(tray_menu)
}

/// Create the tray icon (called once during setup).
pub fn create_tray_icon(app: &tauri::AppHandle) -> tauri::Result<()> {
    let language = {
        let state = app.state::<crate::settings::AppState>();
        state.language.lock().ok().map(|g| g.clone())
    };
    let lang_ref = language.as_deref();

    let tray_menu = build_tray_menu(app, lang_ref)?;

    // Decode PNG icon to RGBA bytes for Tauri Image.
    let icon_png = include_bytes!("../icons/tray-icon@2x.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(icon_png));
    let mut reader = decoder.read_info().expect("Failed to read PNG info");
    let info = reader.info().clone();
    let mut buf = vec![0u8; reader.output_buffer_size()];
    reader.next_frame(&mut buf).expect("Failed to decode PNG frame");
    let icon = Image::new_owned(buf, info.width, info.height);

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .icon_as_template(true)
        .menu(&tray_menu)
        .tooltip("MacHunt")
        .on_menu_event(move |app, event| {
            match event.id().as_ref() {
                "tray_search" => {
                    let state = app.state::<crate::settings::AppState>();
                    let _ = app.emit(crate::window::EVENT_FOCUS_SEARCH, ());
                    let _ = window::show_main_window_internal(&app, &state);
                }
                "tray_pinned" => {
                    let state = app.state::<crate::settings::AppState>();
                    let _ = app.emit(crate::window::EVENT_OPEN_PINNED, ());
                    let _ = window::show_main_window_internal(&app, &state);
                }
                "tray_settings" => {
                    let state = app.state::<crate::settings::AppState>();
                    let _ = app.emit(crate::window::EVENT_OPEN_SETTINGS, ());
                    let _ = window::show_main_window_internal(&app, &state);
                }
                "tray_quit" => {
                    let state = app.state::<crate::settings::AppState>();
                    state.is_quitting.store(true, std::sync::atomic::Ordering::SeqCst);
                    let _ = app.cleanup_before_exit();
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(move |tray, event| {
            // Left-click: toggle window → show search
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    if w.is_visible().unwrap_or(false) {
                        let _ = w.hide();
                    } else {
                        let state = app.state::<crate::settings::AppState>();
                        let _ = window::show_main_window_internal(&app, &state);
                        let _ = app.emit(crate::window::EVENT_FOCUS_SEARCH, ());
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Update the tray menu text when language changes, WITHOUT creating a second icon.
pub fn update_tray_menu_language(app: &tauri::AppHandle) {
    let language = {
        let state = app.state::<crate::settings::AppState>();
        state.language.lock().ok().map(|g| g.clone())
    };
    let lang_ref = language.as_deref();

    // Rebuild the menu and attach it to the existing tray icon via its set_menu method.
    if let Ok(tray_menu) = build_tray_menu(app, lang_ref) {
        if let Some(tray) = app.tray_by_id("main-tray") {
            if let Err(e) = tray.set_menu(Some(tray_menu)) {
                eprintln!("Failed to update tray menu: {}", e);
            }
        }
    }
}

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};

pub const MENU_OPEN_SETTINGS_ID: &str = "open_settings";

pub fn settings_menu_text() -> &'static str {
    "Preferences"
}

pub fn build_menu(app: &tauri::AppHandle) -> Result<Menu<tauri::Wry>, tauri::Error> {
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
        Menu::with_items(
            app,
            &[
                &app_menu,
                &file_menu,
                &edit_menu,
                &view_menu,
                &window_menu,
                &help_menu,
            ],
        )
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
        Ok(menu)
    }
}

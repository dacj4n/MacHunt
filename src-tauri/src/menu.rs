use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};

pub const MENU_OPEN_SETTINGS_ID: &str = "open_settings";

pub fn settings_menu_text(language: Option<&str>) -> &'static str {
    match language {
        Some("zh") => "偏好设置",
        _ => "Preferences",
    }
}

/// Localised tray menu labels.
pub struct TrayLabels {
    pub search: &'static str,
    pub pinned: &'static str,
    pub settings: &'static str,
    pub quit: &'static str,
}

pub fn tray_labels(language: Option<&str>) -> TrayLabels {
    match language {
        Some("zh") => TrayLabels {
            search: "搜索",
            pinned: "收藏",
            settings: "设置...",
            quit: "退出 MacHunt",
        },
        _ => TrayLabels {
            search: "Search",
            pinned: "Pinned",
            settings: "Settings...",
            quit: "Quit MacHunt",
        },
    }
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
                    settings_menu_text(None),
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

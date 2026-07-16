use crate::settings::AppState;
#[cfg(target_os = "macos")]
use std::ffi::CString;
use std::io::Write;
#[cfg(target_os = "macos")]
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::Manager;

pub fn open_container_path(path: &str) -> Result<PathBuf, String> {
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
pub fn activate_application(app_name: &str) -> Result<(), String> {
    let script = format!(
        "tell application \"{}\" to activate",
        app_name.replace('\\', "\\\\").replace('\"', "\\\"")
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
pub fn activate_application(_app_name: &str) -> Result<(), String> {
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

pub fn open_with_terminal_internal(open_target: &Path) -> Result<(), String> {
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

pub fn open_with_wezterm_internal(open_target: &Path) -> Result<(), String> {
    if is_wezterm_running() && try_spawn_wezterm_tab(open_target) {
        let _ = activate_application("WezTerm");
        return Ok(());
    }

    let status = Command::new("open")
        .arg("-a")
        .arg("WezTerm")
        .status()
        .map_err(|e| e.to_string())?;

    if !status.success() {
        return Err("Failed to open WezTerm".to_string());
    }

    std::thread::sleep(std::time::Duration::from_millis(500));
    if try_spawn_wezterm_tab(open_target) {
        let _ = activate_application("WezTerm");
        Ok(())
    } else {
        Ok(())
    }
}

pub fn open_with_app_internal(app_name: &str, open_target: &Path) -> Result<(), String> {
    let status = Command::new("open")
        .arg("-a")
        .arg(app_name)
        .arg(open_target)
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        let _ = activate_application(app_name);
        Ok(())
    } else {
        Err(format!("Failed to open in {} (may not be installed)", app_name))
    }
}

fn open_custom_terminal_app(app_path: &str, open_target: &Path) -> Result<(), String> {
    let status = Command::new("open")
        .arg("-a")
        .arg(app_path)
        .arg(open_target)
        .status()
        .map_err(|e| format!("Failed to open in '{}': {}", app_path, e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Failed to open in '{}' (may not be installed)", app_path))
    }
}

/// Resolve the actual .app path from either the action value or the custom app field.
/// The format is "AppName|/path/to/App.app" (from `pick_app`).
pub fn resolve_app_path(action: &str, custom: &str) -> String {
    let source = if custom.is_empty() { action } else { custom };
    if let Some(idx) = source.rfind('|') {
        source[idx + 1..].to_string()
    } else {
        source.to_string()
    }
}

#[tauri::command]
pub fn open_search_result(path: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err("Target path does not exist".to_string());
    }

    if !target.is_dir() {
        let status = Command::new("open")
            .arg(&target)
            .status()
            .map_err(|e| e.to_string())?;
        return if status.success() {
            Ok(())
        } else {
            Err("Failed to open target".to_string())
        };
    }

    let folder_action = state
        .default_folder_action
        .lock()
        .map_err(|_| "Failed to access default folder action".to_string())?
        .clone();
    let custom_app = state
        .custom_folder_app
        .lock()
        .map_err(|_| "Failed to access custom folder app".to_string())?
        .clone();

    match folder_action.as_str() {
        "QSpace Pro" => open_with_app_internal("QSpace Pro", &target),
        "Finder" => open_with_app_internal("Finder", &target),
        _ => {
            let app_path = resolve_app_path(&folder_action, &custom_app);
            if !app_path.is_empty() {
                Command::new("open")
                    .arg("-a")
                    .arg(&app_path)
                    .arg(&target)
                    .status()
                    .map_err(|e| format!("Failed to open in '{}': {}", app_path, e))?;
                Ok(())
            } else {
                open_with_app_internal("Finder", &target)
            }
        }
    }
}

#[tauri::command]
pub fn preview_search_result(
    app: tauri::AppHandle,
    paths: Vec<String>,
    icon_x: Option<f64>,
    icon_y: Option<f64>,
    icon_w: Option<f64>,
    icon_h: Option<f64>,
) -> Result<(), String> {
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

        let win = app
            .get_webview_window("main")
            .ok_or("Window not found")?;
        let win_pos = win
            .outer_position()
            .map_err(|e| format!("Failed to get window position: {e}"))?;

        let scale = win
            .current_monitor()
            .map_err(|e| format!("{e}"))?
            .map(|m| m.scale_factor())
            .unwrap_or(1.0);

        let win_x = win_pos.x as f64 / scale;
        let win_y = win_pos.y as f64 / scale;

        let screen_h = win
            .current_monitor()
            .map_err(|e| format!("{e}"))?
            .map(|m| m.size().height as f64 / scale)
            .unwrap_or(900.0);

        let win_size = win
            .outer_size()
            .map_err(|e| format!("Failed to get window size: {e}"))?;
        let inner_size = win
            .inner_size()
            .map_err(|e| format!("Failed to get inner size: {e}"))?;
        let raw_chrome = (win_size.height as f64 - inner_size.height as f64) / scale;
        let chrome_h = if raw_chrome > 0.0 { raw_chrome } else { 28.0 };

        let ix = icon_x.unwrap_or(0.0);
        let iy = icon_y.unwrap_or(0.0);
        let iw = icon_w.unwrap_or(28.0);
        let ih = icon_h.unwrap_or(28.0);

        let sx = win_x + ix;
        let sy = screen_h - (win_y + chrome_h + iy + ih);
        let sw = iw;
        let sh = ih;

        let raw_paths: Vec<*const c_char> = c_paths.iter().map(|p| p.as_ptr()).collect();
        let opened = unsafe {
            crate::ffi::open_quicklook(raw_paths.as_ptr(), raw_paths.len(), 0, sx, sy, sw, sh)
        };
        if opened {
            Ok(())
        } else {
            Err("Failed to open native Quick Look preview".to_string())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (paths, icon_x, icon_y, icon_w, icon_h);
        Err("Quick Look preview is only supported on macOS".to_string())
    }
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
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
pub fn open_in_qspace(path: String) -> Result<(), String> {
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

#[tauri::command]
pub fn open_in_terminal(path: String) -> Result<(), String> {
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
pub fn open_in_wezterm(path: String) -> Result<(), String> {
    let open_target = open_container_path(&path)?;
    open_with_wezterm_internal(&open_target)
}

#[tauri::command]
pub fn open_in_default_terminal(path: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let open_target = open_container_path(&path)?;

    let terminal_action = state
        .default_terminal_action
        .lock()
        .map_err(|_| "Failed to access default terminal action".to_string())?
        .clone();
    let custom_app = state
        .custom_terminal_app
        .lock()
        .map_err(|_| "Failed to access custom terminal app".to_string())?
        .clone();

    match terminal_action.as_str() {
        "WezTerm" => open_with_wezterm_internal(&open_target),
        "Terminal" => open_with_terminal_internal(&open_target),
        _ => {
            let app_path = resolve_app_path(&terminal_action, &custom_app);
            if !app_path.is_empty() {
                open_custom_terminal_app(&app_path, &open_target)
            } else {
                open_with_terminal_internal(&open_target)
            }
        }
    }
}

#[tauri::command]
pub fn pick_path_in_finder(app: tauri::AppHandle) -> Option<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg("POSIX path of (choose folder with prompt \"Select a search path\")")
        .output()
        .ok()?;

    let _ = app.show();
    #[cfg(target_os = "macos")]
    unsafe {
        crate::ffi::activate_ignoring_other_apps();
    }

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
pub fn pick_app(app: tauri::AppHandle) -> Option<String> {
    let script = r#"POSIX path of (choose file with prompt "Select an application" default location (path to applications folder))"#;
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;

    let _ = app.show();
    #[cfg(target_os = "macos")]
    unsafe {
        crate::ffi::activate_ignoring_other_apps();
    }

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }

    let path = PathBuf::from(&raw);
    if path.extension().and_then(|ext| ext.to_str()) != Some("app") {
        return None;
    }

    let app_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&raw)
        .to_string();

    Some(format!("{}|{}", app_name, raw.trim_end_matches('/')))
}

#[tauri::command]
pub fn copy_to_clipboard(text: String) -> Result<(), String> {
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
pub fn copy_search_results(paths: Vec<String>) -> Result<(), String> {
    let selected_paths: Vec<String> = paths
        .into_iter()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect();

    if selected_paths.is_empty() {
        return Err("No paths selected".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let mut c_paths = Vec::new();
        for path in selected_paths {
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
        let copied = unsafe { crate::ffi::copy_files_to_clipboard(raw_paths.as_ptr(), raw_paths.len()) };
        if copied {
            return Ok(());
        }
        return Err("Failed to copy search results to pasteboard".to_string());
    }

    #[cfg(not(target_os = "macos"))]
    {
        copy_to_clipboard(selected_paths.join("\n"))
    }
}

#[tauri::command]
pub fn move_to_trash(path: String) -> Result<(), String> {
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

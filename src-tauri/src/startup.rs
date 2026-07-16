#[cfg(target_os = "macos")]
use objc2_service_management::{SMAppService, SMAppServiceStatus};
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
use std::process::Command;

#[cfg(target_os = "macos")]
const AUTOSTART_LAUNCH_AGENT_LABEL: &str = "com.dacj4n.machunt.autostart";
#[cfg(target_os = "macos")]
const DEFAULT_LOGIN_ITEM_NAME: &str = "MacHunt";

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
pub fn apply_launch_settings(launch_at_login: bool) -> Result<(), String> {
    cleanup_legacy_launch_agent_file();

    if supports_smappservice() {
        return apply_launch_settings_via_service_management(launch_at_login);
    }

    apply_launch_settings_via_system_events(launch_at_login)
}

#[cfg(not(target_os = "macos"))]
pub fn apply_launch_settings(_launch_at_login: bool) -> Result<(), String> {
    Ok(())
}

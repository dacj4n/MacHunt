#[cfg(target_os = "macos")]
use std::os::raw::c_char;

#[cfg(target_os = "macos")]
unsafe extern "C" {
    pub fn open_quicklook(
        paths: *const *const c_char,
        len: usize,
        index: usize,
        source_x: f64,
        source_y: f64,
        source_w: f64,
        source_h: f64,
    ) -> bool;
    pub fn copy_files_to_clipboard(paths: *const *const c_char, len: usize) -> bool;
    pub fn set_dock_flag(v: bool);
    pub fn install_policy_guard();
    pub fn force_accessory_policy() -> bool;
    pub fn activate_ignoring_other_apps() -> bool;
    pub fn deactivate_app() -> bool;
}

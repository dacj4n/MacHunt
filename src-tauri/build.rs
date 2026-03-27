fn main() {
    tauri_build::build();

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        println!("cargo:rerun-if-changed=macos/quicklook_bridge.m");
        cc::Build::new()
            .file("macos/quicklook_bridge.m")
            .flag("-fobjc-arc")
            .compile("machunt_quicklook_bridge");

        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=QuickLookUI");
    }
}

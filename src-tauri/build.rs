fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=src/system/macos_tray.m");
        cc::Build::new()
            .file("src/system/macos_tray.m")
            .compile("macos_tray");

    }
    tauri_build::build()
}


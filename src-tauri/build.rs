fn main() {
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("src/system/macos_tray.m")
            .compile("macos_tray");
    }
    tauri_build::build()
}


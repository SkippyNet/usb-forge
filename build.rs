//! Embeds the application icon and version info into the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico")
            .set("ProductName", "USB Forge")
            .set("FileDescription", "USB Forge - multi-boot USB builder")
            .set("LegalCopyright", "MIT License");
        if let Err(e) = res.compile() {
            panic!("Failed to embed Windows resources: {e}");
        }
    }
}

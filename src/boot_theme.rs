//! The USB Forge boot menu theme, written to the data partition after the ISOs.
//!
//! The boot loader reads `/ventoy/ventoy.json` from the data partition and supports a GRUB
//! gfxmenu theme through its theme plugin, so branding needs no change to the boot loader
//! itself (and nothing about Secure Boot). Images come from `assets/boot-theme/make-theme.ps1`.

use std::path::Path;

/// Folder on the data partition that holds the theme, relative to the drive root.
const THEME_DIR: &str = "ventoy/theme/usb-forge";

macro_rules! theme_files {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!("../assets/boot-theme/", $name)) as &[u8])),*]
    };
}

const FILES: &[(&str, &[u8])] = theme_files![
    "theme.txt",
    "background.png",
    "menu_c.png",
    "menu_n.png",
    "menu_s.png",
    "menu_e.png",
    "menu_w.png",
    "menu_ne.png",
    "menu_nw.png",
    "menu_se.png",
    "menu_sw.png",
    "select_c.png",
    "select_w.png",
    "slider_c.png",
    "slider_n.png",
    "slider_s.png",
    "terminal_box_c.png",
    "terminal_box_n.png",
    "terminal_box_s.png",
    "terminal_box_e.png",
    "terminal_box_w.png",
    "terminal_box_ne.png",
    "terminal_box_nw.png",
    "terminal_box_se.png",
    "terminal_box_sw.png",
];

/// Background colour at the bottom of `background.png`. The boot loader always prints its own
/// name and version line; drawing it in this colour hides it.
const HIDDEN_TEXT_COLOR: &str = "#0f1117";

/// Plugin config: use the theme in graphical mode, with the boot loader's version line hidden.
fn plugin_config() -> String {
    format!(
        r##"{{
    "theme": {{
        "file": "/{THEME_DIR}/theme.txt",
        "display_mode": "GUI",
        "ventoy_left": "5%",
        "ventoy_top": "95%",
        "ventoy_color": "{HIDDEN_TEXT_COLOR}"
    }}
}}
"##
    )
}

/// Write the theme and the plugin config onto the drive mounted at `root`.
pub fn install(root: &Path) -> Result<(), String> {
    let dir = root.join(THEME_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    for (name, bytes) in FILES {
        let path = dir.join(name);
        std::fs::write(&path, bytes)
            .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    }
    let config = root.join("ventoy").join("ventoy.json");
    std::fs::write(&config, plugin_config())
        .map_err(|e| format!("Cannot write {}: {e}", config.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_points_at_the_bundled_theme() {
        let v: serde_json::Value = serde_json::from_str(&plugin_config()).expect("valid JSON");
        let file = v["theme"]["file"].as_str().unwrap();
        let name = file.strip_prefix(&format!("/{THEME_DIR}/")).unwrap();
        assert!(FILES.iter().any(|(n, _)| *n == name));
        assert_eq!(v["theme"]["ventoy_color"], HIDDEN_TEXT_COLOR);
        assert!(
            std::str::from_utf8(FILES[0].1)
                .unwrap()
                .contains(&format!("desktop-color: \"{HIDDEN_TEXT_COLOR}\"")),
            "hidden text colour must match the theme background"
        );
    }

    #[test]
    fn every_image_the_theme_uses_is_bundled() {
        let theme = std::str::from_utf8(FILES[0].1).unwrap();
        let bundled = |n: &str| FILES.iter().any(|(f, _)| *f == n);
        for pattern in theme.split('"').filter(|s| s.ends_with(".png")) {
            if let Some(prefix) = pattern.strip_suffix("*.png") {
                assert!(bundled(&format!("{prefix}c.png")), "{pattern}");
            } else {
                assert!(bundled(pattern), "{pattern}");
            }
        }
        for (name, bytes) in FILES.iter().filter(|(n, _)| n.ends_with(".png")) {
            assert!(bytes.starts_with(b"\x89PNG"), "{name} is not a PNG");
        }
    }

    #[test]
    fn installs_into_a_drive_folder() {
        let root = std::env::temp_dir().join(format!("usbf-theme-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        install(&root).unwrap();
        assert!(root.join("ventoy/ventoy.json").is_file());
        assert!(root.join(THEME_DIR).join("background.png").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }
}

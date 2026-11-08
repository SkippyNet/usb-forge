//! USB Forge: build a verified multi-boot USB thumb drive (Ventoy + official ISOs).
//! Runs on Windows 10+ and mainstream Linux desktops.
//!
//! Command-line diagnostics (no changes are made to any disk):
//!   usb-forge --version
//!   usb-forge --list-disks   show every disk and whether it is a safe target
//!   usb-forge --resolve      look up the latest version + checksum of every catalog item

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(not(any(windows, target_os = "linux")))]
compile_error!("USB Forge supports Windows and Linux.");

mod app;
mod boot_theme;
mod catalog;
mod disks;
mod job;
mod net;
mod platform;
mod release_msg;
mod sys;
mod theme;
mod updater;
mod ventoy;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("usb-forge {}", updater::CURRENT_VERSION);
        return Ok(());
    }
    if args.iter().any(|a| a == "--list-disks") {
        list_disks();
        return Ok(());
    }
    if args.iter().any(|a| a == "--resolve") {
        resolve_all();
        return Ok(());
    }

    platform::cleanup_after_update();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("USB Forge")
        .with_app_id("usb-forge")
        .with_inner_size([1140.0, 780.0])
        .with_min_inner_size([940.0, 640.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "USB Forge",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(app::UsbForgeApp::new(cc)))
        }),
    )
}

fn list_disks() {
    match disks::all_disks() {
        Ok(list) => {
            for d in list {
                println!(
                    "{:<14} {:<32} {:<5} {:>9}  boot={:<5} system={:<5} os={:<5}  {}  -> {}",
                    d.display_id(),
                    d.name,
                    d.bus,
                    sys::human_size(d.size),
                    d.is_boot,
                    d.is_system,
                    d.is_os_disk,
                    d.mounts_label(),
                    if d.is_safe_target() {
                        "SAFE TARGET"
                    } else {
                        "hidden"
                    }
                );
            }
        }
        Err(e) => eprintln!("{e}"),
    }
}

fn resolve_all() {
    match ventoy::latest_release() {
        Ok(r) => println!(
            "{:<22} {:<12} {}\n{:<22} sha256 {}",
            "Boot loader", r.version, r.url, "", r.sha256
        ),
        Err(e) => println!("{:<22} ERROR {e}", "Boot loader"),
    }
    for t in catalog::TOOLS {
        let channels: &[catalog::Channel] = if t.has_channel {
            &[catalog::Channel::Lts, catalog::Channel::Latest]
        } else {
            &[catalog::Channel::Latest]
        };
        for ch in channels {
            let label = if t.has_channel {
                format!("{} ({})", t.name, ch.label())
            } else {
                t.name.to_string()
            };
            match catalog::resolve(t.id, *ch) {
                Ok(r) => println!(
                    "{label:<22} {:<12} {:>9}  {}\n{:<22} sha256 {}",
                    r.version,
                    r.size.map(sys::human_size).unwrap_or_else(|| "?".into()),
                    r.url,
                    "",
                    r.sha256
                ),
                Err(e) => println!("{label:<22} ERROR {e}"),
            }
        }
    }
}

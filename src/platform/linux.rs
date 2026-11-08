//! Linux backend: lsblk for discovery, udisks2 for (un)mounting as the desktop user, and
//! polkit (`pkexec`) to run only the Ventoy installer with root rights.
//!
//! Tested targets: Ubuntu, Debian, Fedora, Linux Mint and Arch (any distro with util-linux,
//! udisks2 and polkit, which every mainstream desktop ships).

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

use crate::disks::Disk;
use crate::ventoy::FileSystem;

pub const OS_NAME: &str = "Linux";
pub const VENTOY_ASSET_SUFFIX: &str = "-linux.tar.gz";
pub const VENTOY_TOOL: &str = "Ventoy2Disk.sh";
pub const UPDATE_ASSET: &str = "usb-forge-linux-x86_64";
/// Ventoy's Linux installer always formats the data partition as exFAT.
pub const SUPPORTED_FS: &[FileSystem] = &[FileSystem::ExFat];

pub fn hidden(program: &str) -> Command {
    Command::new(program)
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false)
}

/// The GUI runs as the normal user; only the format step asks for a password via polkit.
pub fn needs_admin_restart() -> bool {
    false
}

pub fn privilege_note() -> &'static str {
    if is_root() {
        "Running as root"
    } else {
        "Asks for your password to format"
    }
}

pub fn relaunch_elevated() -> Result<(), String> {
    Err("Not needed on Linux: you will be asked for your password when formatting.".into())
}

/// `$XDG_CACHE_HOME/usb-forge` or `~/.cache/usb-forge`
pub fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
        .join("usb-forge")
}

/// Whether UEFI Secure Boot is on for this PC. `None` when it can't be determined.
pub fn secure_boot_enabled() -> Option<bool> {
    let vars = Path::new("/sys/firmware/efi/efivars");
    if !Path::new("/sys/firmware/efi").exists() {
        // Booted in legacy BIOS mode: Secure Boot doesn't exist.
        return Some(false);
    }
    let entry = std::fs::read_dir(vars)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with("SecureBoot-"))?;
    // efivarfs: 4 bytes of attributes, then the 1-byte value.
    let data = std::fs::read(entry.path()).ok()?;
    data.last().map(|b| *b == 1)
}

/// Open a web page in the default browser.
pub fn open_url(url: &str) {
    let _ = Command::new("xdg-open").arg(url).spawn();
}

// ---- disks -------------------------------------------------------------------------------

fn lsblk(target: Option<&str>) -> Result<Value, String> {
    let mut cmd = Command::new("lsblk");
    cmd.args([
        "-J",
        "-b",
        "-o",
        "NAME,PATH,SIZE,TRAN,TYPE,MODEL,VENDOR,MOUNTPOINTS,LABEL,PTTYPE",
    ]);
    if let Some(t) = target {
        cmd.arg(t);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("Could not run lsblk: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "lsblk failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("Could not read lsblk output: {e}"))
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn number(v: &Value) -> u64 {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

/// Every mount point on this device and its descendants (partitions, LVM, LUKS...).
fn collect_mounts(dev: &Value, out: &mut Vec<String>) {
    for key in ["mountpoints", "mountpoint"] {
        match &dev[key] {
            Value::Array(list) => {
                out.extend(list.iter().filter_map(|m| m.as_str()).map(String::from))
            }
            Value::String(s) => out.push(s.clone()),
            _ => {}
        }
    }
    if let Some(children) = dev["children"].as_array() {
        for c in children {
            collect_mounts(c, out);
        }
    }
}

/// Mounts the desktop creates for removable media. Anything else means the system uses the disk.
fn is_removable_mount(m: &str) -> bool {
    m.starts_with("/media/") || m.starts_with("/run/media/") || m.starts_with("/mnt/")
}

pub fn all_disks() -> Result<Vec<Disk>, String> {
    let v = lsblk(None)?;
    let devices = v["blockdevices"].as_array().cloned().unwrap_or_default();
    Ok(devices
        .iter()
        .filter(|d| text(&d["type"]) == "disk")
        .map(|d| {
            let mut mounts = Vec::new();
            collect_mounts(d, &mut mounts);
            let vendor = text(&d["vendor"]);
            let model = text(&d["model"]);
            let name = match (vendor.is_empty(), model.is_empty()) {
                (false, false) => format!("{vendor} {model}"),
                (true, false) => model,
                (false, true) => vendor,
                (true, true) => text(&d["name"]),
            };
            let style = match text(&d["pttype"]).as_str() {
                "dos" => "MBR".to_string(),
                "gpt" => "GPT".to_string(),
                "" => "RAW".to_string(),
                other => other.to_uppercase(),
            };
            Disk {
                id: text(&d["path"]),
                name,
                bus: text(&d["tran"]).to_uppercase(),
                size: number(&d["size"]),
                is_boot: mounts
                    .iter()
                    .any(|m| m == "/boot" || m == "/boot/efi" || m == "/efi"),
                is_system: mounts.iter().any(|m| !is_removable_mount(m)),
                is_os_disk: mounts.iter().any(|m| m == "/"),
                style,
                mounts,
            }
        })
        .collect())
}

/// Device paths of the disk's partitions, in order.
fn partitions(disk: &str) -> Result<Vec<(String, String, Vec<String>)>, String> {
    let v = lsblk(Some(disk))?;
    let dev = &v["blockdevices"][0];
    Ok(dev["children"]
        .as_array()
        .map(|kids| {
            kids.iter()
                .filter(|k| text(&k["type"]) == "part")
                .map(|k| {
                    let mut m = Vec::new();
                    collect_mounts(k, &mut m);
                    (text(&k["path"]), text(&k["label"]), m)
                })
                .collect()
        })
        .unwrap_or_default())
}

fn valid_device(path: &str) -> bool {
    path.strip_prefix("/dev/")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric()))
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

// ---- Ventoy ------------------------------------------------------------------------------

pub fn extract_ventoy(archive: &Path, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(dir)
        .status()
        .map_err(|e| format!("Could not run tar: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Extracting Ventoy failed ({status})"))
    }
}

/// Format the disk and install Ventoy on it. This ERASES the disk.
pub fn install_ventoy(
    ventoy_dir: &Path,
    disk: &Disk,
    _fs: FileSystem,
    gpt: bool,
    progress: &mut dyn FnMut(f32),
) -> Result<(), String> {
    if !valid_device(&disk.id) {
        return Err(format!("Refusing unusual device path '{}'", disk.id));
    }
    // Unmount as the desktop user first (no password needed for removable media).
    for (part, _, mounts) in partitions(&disk.id)? {
        if !mounts.is_empty() {
            let _ = Command::new("udisksctl")
                .args(["unmount", "--no-user-interaction", "-b", &part])
                .output();
        }
    }

    let dir = ventoy_dir.to_string_lossy();
    let dev = shell_quote(&disk.id);
    let script = format!(
        "set -e\ncd {dir}\nfor p in $(lsblk -lnpo NAME {dev} | tail -n +2); do umount \"$p\" 2>/dev/null || true; done\nprintf 'y\\ny\\n' | sh ./Ventoy2Disk.sh -I {gpt}{dev}\n",
        dir = shell_quote(&dir),
        gpt = if gpt { "-g " } else { "" },
    );
    let mut cmd = if is_root() {
        Command::new("sh")
    } else {
        let mut c = Command::new("pkexec");
        c.arg("sh");
        c
    };
    cmd.arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    progress(0.1);
    let mut child = cmd.spawn().map_err(|e| {
        format!("Could not start the privileged installer (is polkit/pkexec installed?): {e}")
    })?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(script.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let log = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    match out.status.code() {
        Some(126) | Some(127) if !is_root() => {
            return Err("Authorization was cancelled or denied. Nothing was changed.".into());
        }
        Some(0) => {}
        _ => {
            return Err(format!(
                "Ventoy install failed ({}).\n{}",
                out.status,
                tail(&log)
            ));
        }
    }
    // Ventoy's script exits 0 even when it aborts, so confirm the result on disk.
    let _ = Command::new("udevadm").arg("settle").status();
    for _ in 0..15 {
        if let Ok(parts) = partitions(&disk.id)
            && parts.first().is_some_and(|(_, label, _)| label == "Ventoy")
        {
            progress(1.0);
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err(format!(
        "Ventoy did not create its partition.\n{}",
        tail(&log)
    ))
}

fn tail(log: &str) -> String {
    let lines: Vec<&str> = log.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(15)..].join("\n")
}

/// Mount Ventoy's data partition (as the desktop user, via udisks2) and return its path.
pub fn data_partition(disk: &Disk) -> Result<PathBuf, String> {
    for _ in 0..20 {
        if let Some((part, label, mounts)) = partitions(&disk.id)?.into_iter().next() {
            if label != "Ventoy" {
                return Err(format!(
                    "Partition {part} is labelled '{label}', expected 'Ventoy'."
                ));
            }
            if let Some(m) = mounts.first() {
                return Ok(PathBuf::from(m));
            }
            let _ = Command::new("udisksctl")
                .args(["mount", "--no-user-interaction", "-b", &part])
                .output();
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err("Could not mount the new Ventoy partition (is udisks2 running?).".into())
}

// ---- self-update -------------------------------------------------------------------------

/// Atomically replace the running executable with the verified `new_exe`.
pub fn swap_executable(new_exe: &Path) -> Result<PathBuf, String> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    std::fs::set_permissions(new_exe, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    std::fs::rename(new_exe, &current)
        .map_err(|e| format!("Cannot install the update (is the folder writable?): {e}"))?;
    Ok(current)
}

pub fn cleanup_after_update() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lsblk_and_flags_system_disks() {
        let json = r#"{"blockdevices":[
          {"name":"nvme0n1","path":"/dev/nvme0n1","size":1000204886016,"tran":"nvme","type":"disk","model":"Samsung SSD","vendor":null,"mountpoints":[null],"label":null,"pttype":"gpt",
           "children":[{"name":"nvme0n1p1","path":"/dev/nvme0n1p1","size":536870912,"tran":null,"type":"part","mountpoints":["/boot/efi"]},
                       {"name":"nvme0n1p2","path":"/dev/nvme0n1p2","size":999,"tran":null,"type":"part","mountpoints":["/"]}]},
          {"name":"sdb","path":"/dev/sdb","size":"31457280000","tran":"usb","type":"disk","model":"UDisk","vendor":"General ","mountpoints":[null],"label":null,"pttype":"dos",
           "children":[{"name":"sdb1","path":"/dev/sdb1","size":31457280000,"tran":null,"type":"part","label":"SAMDATA","mountpoints":["/media/me/SAMDATA"]}]},
          {"name":"sdc","path":"/dev/sdc","size":2000000000000,"tran":"usb","type":"disk","model":"Backup","vendor":"WD","mountpoints":[null],"pttype":"gpt",
           "children":[{"name":"sdc1","path":"/dev/sdc1","type":"part","mountpoints":["[SWAP]"]}]}
        ]}"#;
        let v: Value = serde_json::from_str(json).unwrap();
        let disks: Vec<Disk> = v["blockdevices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                let mut mounts = Vec::new();
                collect_mounts(d, &mut mounts);
                Disk {
                    id: text(&d["path"]),
                    name: text(&d["model"]),
                    bus: text(&d["tran"]).to_uppercase(),
                    size: number(&d["size"]),
                    is_boot: mounts.iter().any(|m| m == "/boot/efi"),
                    is_system: mounts.iter().any(|m| !is_removable_mount(m)),
                    is_os_disk: mounts.iter().any(|m| m == "/"),
                    style: String::new(),
                    mounts,
                }
            })
            .collect();
        assert!(!disks[0].is_safe_target(), "internal NVMe must be hidden");
        assert!(
            disks[1].is_safe_target(),
            "USB stick mounted under /media is a target"
        );
        assert_eq!(disks[1].size, 31_457_280_000);
        assert!(
            !disks[2].is_safe_target(),
            "USB disk used for swap must be hidden"
        );
    }

    #[test]
    fn device_paths_are_validated_and_quoted() {
        assert!(valid_device("/dev/sdb"));
        assert!(valid_device("/dev/mmcblk0"));
        assert!(!valid_device("/dev/sdb; rm -rf /"));
        assert!(!valid_device("/dev/../etc"));
        assert_eq!(shell_quote("a'b"), r"'a'\''b'");
    }
}

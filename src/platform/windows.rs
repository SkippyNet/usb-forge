//! Windows 10+ backend: PowerShell storage cmdlets, Ventoy2Disk.exe, UAC elevation.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use crate::disks::Disk;
use crate::ventoy::{DATA_LABEL, FileSystem};

pub const OS_NAME: &str = "Windows";
pub const VENTOY_ASSET_SUFFIX: &str = "-windows.zip";
pub const VENTOY_TOOL: &str = "Ventoy2Disk.exe";
pub const UPDATE_ASSET: &str = "usb-forge-windows-x86_64.exe";
pub const SUPPORTED_FS: &[FileSystem] = &[FileSystem::ExFat, FileSystem::Ntfs, FileSystem::Fat32];

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Label the upstream boot loader installer gives the data partition; renamed to `DATA_LABEL`.
const INSTALLER_LABEL: &str = "Ventoy";

/// A `Command` that never flashes a console window.
pub fn hidden(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Run a PowerShell script and return its stdout (UTF-8).
pub fn powershell(script: &str) -> Result<String, String> {
    let full = format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8\n{script}");
    let out = hidden("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &full,
        ])
        .output()
        .map_err(|e| format!("Could not start PowerShell: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "PowerShell failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `fltmc` only succeeds for elevated callers, which makes it a cheap, reliable probe.
pub fn is_elevated() -> bool {
    hidden("fltmc.exe")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Windows must run the whole app elevated to format disks.
pub fn needs_admin_restart() -> bool {
    !is_elevated()
}

/// Shown in the header when no restart is needed.
pub fn privilege_note() -> &'static str {
    "Administrator"
}

/// Start a new elevated copy of this executable (one UAC prompt).
pub fn relaunch_elevated() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot locate executable: {e}"))?;
    let path = exe.display().to_string().replace('\'', "''");
    powershell(&format!("Start-Process -FilePath '{path}' -Verb RunAs")).map(|_| ())
}

/// `%LOCALAPPDATA%\UsbForge\cache`
pub fn cache_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("UsbForge")
        .join("cache")
}

/// Whether UEFI Secure Boot is on for this PC. `None` when it can't be determined.
/// The registry value is readable without administrator rights.
pub fn secure_boot_enabled() -> Option<bool> {
    let out = hidden("reg.exe")
        .args([
            "query",
            r"HKLM\SYSTEM\CurrentControlSet\Control\SecureBoot\State",
            "/v",
            "UEFISecureBootEnabled",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        // Key missing: legacy BIOS boot, where Secure Boot doesn't exist.
        return Some(false);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    if text.contains("0x1") {
        Some(true)
    } else if text.contains("0x0") {
        Some(false)
    } else {
        None
    }
}

/// Open a web page in the default browser.
pub fn open_url(url: &str) {
    let _ = hidden("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
}

/// Show a folder in File Explorer.
pub fn open_folder(path: &Path) {
    let _ = Command::new("explorer.exe").arg(path).spawn();
}

// ---- disks -------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WinDisk {
    number: u32,
    name: String,
    bus: String,
    size: u64,
    is_boot: bool,
    is_system: bool,
    is_os_disk: bool,
    style: String,
    letters: Vec<String>,
}

const LIST_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$os = (Get-Partition -DriveLetter $env:SystemDrive.Substring(0,1)).DiskNumber
$list = @(Get-Disk | ForEach-Object {
  $n = $_.Number
  [pscustomobject]@{
    Number   = [int]$n
    Name     = ([string]$_.FriendlyName).Trim()
    Bus      = [string]$_.BusType
    Size     = [uint64]$_.Size
    IsBoot   = [bool]$_.IsBoot
    IsSystem = [bool]$_.IsSystem
    IsOsDisk = ($null -ne $os -and $n -eq $os)
    Style    = [string]$_.PartitionStyle
    Letters  = [string[]]@(Get-Partition -DiskNumber $n | Where-Object { [int][char]$_.DriveLetter -gt 0 } | ForEach-Object { [string]$_.DriveLetter })
  }
})
ConvertTo-Json -InputObject $list -Depth 4 -Compress
"#;

pub fn all_disks() -> Result<Vec<Disk>, String> {
    let json = powershell(LIST_SCRIPT)?;
    let json = json.trim();
    if json.is_empty() {
        return Ok(Vec::new());
    }
    let list: Vec<WinDisk> =
        serde_json::from_str(json).map_err(|e| format!("Could not read the disk list: {e}"))?;
    Ok(list
        .into_iter()
        .map(|d| Disk {
            id: d.number.to_string(),
            name: d.name,
            bus: d.bus,
            size: d.size,
            is_boot: d.is_boot,
            is_system: d.is_system,
            is_os_disk: d.is_os_disk,
            style: d.style,
            mounts: d.letters.into_iter().map(|l| format!("{l}:")).collect(),
        })
        .collect())
}

// ---- Ventoy ------------------------------------------------------------------------------

pub fn extract_ventoy(archive: &Path, dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive)
        .map_err(|e| format!("Cannot open the boot loader package: {e}"))?;
    zip::ZipArchive::new(file)
        .and_then(|mut z| z.extract(dir))
        .map_err(|e| format!("Cannot extract the boot loader: {e}"))
}

/// Format the disk and install Ventoy on it. This ERASES the disk.
pub fn install_ventoy(
    ventoy_dir: &Path,
    disk: &Disk,
    fs: FileSystem,
    gpt: bool,
    progress: &mut dyn FnMut(f32),
) -> Result<(), String> {
    let number: u32 = disk
        .id
        .parse()
        .map_err(|_| format!("Invalid disk number '{}'", disk.id))?;
    let done_file = ventoy_dir.join("cli_done.txt");
    let pct_file = ventoy_dir.join("cli_percent.txt");
    let _ = std::fs::remove_file(&done_file);
    let _ = std::fs::remove_file(&pct_file);

    let mut args = vec![
        "VTOYCLI".to_string(),
        "/I".into(),
        format!("/PhyDrive:{number}"),
        format!("/FS:{}", fs.cli()),
    ];
    if gpt {
        args.push("/GPT".into());
    }
    let exe = ventoy_dir.join(VENTOY_TOOL);
    let mut child = hidden(&exe.to_string_lossy())
        .args(&args)
        .current_dir(ventoy_dir)
        .spawn()
        .map_err(|e| format!("Could not start the boot loader installer: {e}"))?;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            let done = std::fs::read_to_string(&done_file).unwrap_or_default();
            if done.trim() == "0" {
                progress(1.0);
                return Ok(());
            }
            let log = std::fs::read_to_string(ventoy_dir.join("log.txt")).unwrap_or_default();
            let tail: Vec<&str> = log.lines().rev().take(12).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            return Err(format!(
                "Boot loader install failed (exit {status}, result '{}').\n{}",
                done.trim(),
                tail.join("\n")
            ));
        }
        if let Ok(p) = std::fs::read_to_string(&pct_file)
            && let Ok(p) = p.trim().parse::<f32>()
        {
            progress((p / 100.0).clamp(0.0, 1.0));
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// Locate (assigning if needed) the drive letter of the data partition, and give it the
/// USB Forge label (the installer names it after itself by default).
pub fn data_partition(disk: &Disk) -> Result<PathBuf, String> {
    let number: u32 = disk
        .id
        .parse()
        .map_err(|_| "Invalid disk number".to_string())?;
    let script = format!(
        r#"
$ErrorActionPreference = 'SilentlyContinue'
Update-HostStorageCache
$p = Get-Partition -DiskNumber {number} -PartitionNumber 1
if ($p -and [int][char]$p.DriveLetter -eq 0) {{
  $p | Add-PartitionAccessPath -AssignDriveLetter | Out-Null
  $p = Get-Partition -DiskNumber {number} -PartitionNumber 1
}}
$v = $p | Get-Volume
if ($v -and $v.FileSystemLabel -eq '{INSTALLER_LABEL}') {{
  Set-Volume -DriveLetter $p.DriveLetter -NewFileSystemLabel '{DATA_LABEL}' | Out-Null
  $v = $p | Get-Volume
}}
if ($p -and [int][char]$p.DriveLetter -gt 0) {{ "$($p.DriveLetter)|$($v.FileSystemLabel)" }}
"#
    );
    for _ in 0..30 {
        if let Ok(out) = powershell(&script) {
            let parts: Vec<&str> = out.trim().split('|').collect();
            if parts.len() == 2 && parts[0].len() == 1 {
                if !parts[1].eq_ignore_ascii_case(DATA_LABEL) {
                    return Err(format!(
                        "Partition 1 on disk {number} is labelled '{}', expected '{DATA_LABEL}'.",
                        parts[1]
                    ));
                }
                return Ok(PathBuf::from(format!("{}:\\", parts[0])));
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err("Windows did not mount the new data partition.".into())
}

// ---- self-update -------------------------------------------------------------------------

/// Put the verified `new_exe` in place of the running executable and return its path.
/// Windows allows renaming (but not overwriting) a running .exe, so the old one is moved aside.
pub fn swap_executable(new_exe: &Path) -> Result<PathBuf, String> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let old = current.with_extension("exe.old");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&current, &old)
        .map_err(|e| format!("Cannot move the running app aside: {e}"))?;
    if let Err(e) = std::fs::rename(new_exe, &current) {
        let _ = std::fs::rename(&old, &current);
        return Err(format!("Cannot install the update: {e}"));
    }
    Ok(current)
}

/// Remove the previous executable left behind by an update.
pub fn cleanup_after_update() {
    if let Ok(current) = std::env::current_exe() {
        let _ = std::fs::remove_file(current.with_extension("exe.old"));
    }
}

//! USB disk discovery with strict safety filtering (platform backends in `platform`).

use crate::platform;
use crate::sys::human_size;

#[derive(Debug, Clone, PartialEq)]
pub struct Disk {
    /// Windows: physical disk number ("1"). Linux: device path ("/dev/sdb").
    pub id: String,
    pub name: String,
    pub bus: String,
    pub size: u64,
    /// Holds boot files (EFI system partition, /boot).
    pub is_boot: bool,
    /// Holds anything the running system uses (Windows system disk; Linux non-removable mounts, swap).
    pub is_system: bool,
    /// Holds the running operating system (C: / the root filesystem).
    pub is_os_disk: bool,
    pub style: String,
    /// Drive letters (Windows) or mount points (Linux).
    pub mounts: Vec<String>,
}

/// Space kept free on the drive for the boot partition and filesystem overhead.
pub const RESERVED_BYTES: u64 = 64 * 1024 * 1024;

/// Anything larger than this is more likely an external hard drive than a thumb drive.
pub const THUMB_DRIVE_MAX: u64 = 256 * 1024 * 1024 * 1024;

impl Disk {
    /// Only USB disks that the running system does not boot from or use qualify.
    pub fn is_safe_target(&self) -> bool {
        self.bus.eq_ignore_ascii_case("USB")
            && !self.is_boot
            && !self.is_system
            && !self.is_os_disk
            && self.size > 0
    }

    pub fn looks_like_thumb_drive(&self) -> bool {
        self.size <= THUMB_DRIVE_MAX
    }

    pub fn display_id(&self) -> String {
        if cfg!(windows) {
            format!("Disk {}", self.id)
        } else {
            self.id.clone()
        }
    }

    pub fn mounts_label(&self) -> String {
        if self.mounts.is_empty() {
            "not mounted".into()
        } else {
            self.mounts.join(" ")
        }
    }
}

/// Every disk the OS knows about (unfiltered).
pub fn all_disks() -> Result<Vec<Disk>, String> {
    platform::all_disks()
}

/// Re-query the OS right before formatting and make sure the id still refers to the same
/// safe USB device the user picked.
pub fn reverify(expected: &Disk) -> Result<Disk, String> {
    let now = all_disks()?
        .into_iter()
        .find(|d| d.id == expected.id)
        .ok_or_else(|| format!("{} is no longer connected.", expected.display_id()))?;
    if !now.is_safe_target() {
        return Err(format!(
            "{} no longer passes the USB safety checks (bus {}, boot {}, system {}). Nothing was changed.",
            now.display_id(),
            now.bus,
            now.is_boot,
            now.is_system
        ));
    }
    if now.name != expected.name || now.size != expected.size {
        return Err(format!(
            "{} changed (was {} / {}, now {} / {}). Nothing was changed.",
            now.display_id(),
            expected.name,
            human_size(expected.size),
            now.name,
            human_size(now.size)
        ));
    }
    Ok(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usb() -> Disk {
        Disk {
            id: "1".into(),
            name: "General UDisk".into(),
            bus: "USB".into(),
            size: 32_000_000_000,
            is_boot: false,
            is_system: false,
            is_os_disk: false,
            style: "MBR".into(),
            mounts: vec![],
        }
    }

    #[test]
    fn only_plain_usb_disks_are_targets() {
        assert!(usb().is_safe_target());
        assert!(
            !Disk {
                bus: "NVMe".into(),
                ..usb()
            }
            .is_safe_target()
        );
        assert!(
            !Disk {
                bus: "SATA".into(),
                ..usb()
            }
            .is_safe_target()
        );
        assert!(
            !Disk {
                is_boot: true,
                ..usb()
            }
            .is_safe_target()
        );
        assert!(
            !Disk {
                is_system: true,
                ..usb()
            }
            .is_safe_target()
        );
        assert!(
            !Disk {
                is_os_disk: true,
                ..usb()
            }
            .is_safe_target()
        );
        assert!(!Disk { size: 0, ..usb() }.is_safe_target());
    }
}

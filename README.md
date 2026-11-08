<p align="center"><img src="assets/icon.png" width="96" alt="USB Forge icon"></p>

# USB Forge

Build a multi-boot USB thumb drive in a few clicks. USB Forge installs
[Ventoy](https://www.ventoy.net), then adds the operating systems and tools you pick, each one
downloaded from its official project and verified against that project's published SHA-256.

Runs on **Windows 10/11** and **Linux** (Ubuntu, Debian, Fedora, Linux Mint, Arch and other
mainstream desktops).

## Download

Get the latest build from [Releases](https://github.com/SkippyNet/usb-forge/releases/latest):

| OS | File |
|---|---|
| Windows 10/11 (x64) | `usb-forge-windows-x86_64.exe` |
| Linux (x86_64) | `usb-forge-linux-x86_64` |

The app checks for new versions when it starts. When one is available it shows
**Update & restart**. It installs the update only if the release's Ed25519 signature checks
out against the key built into the app (see [SECURITY.md](SECURITY.md)).

### Windows

Run the `.exe`. Formatting needs administrator rights, so click **Restart as administrator**
in the top-right corner.

### Linux

```sh
chmod +x usb-forge-linux-x86_64
./usb-forge-linux-x86_64
```

The app runs as your normal user and asks for your password (via polkit) only for the format
step. Requirements: `util-linux` (`lsblk`), `udisks2`, `polkit` and a kernel with exFAT
support. Every distro listed above ships these by default.

Optional desktop launcher:

```sh
install -Dm755 usb-forge-linux-x86_64 ~/.local/bin/usb-forge
install -Dm644 assets/icon.png ~/.local/share/icons/hicolor/256x256/apps/usb-forge.png
install -Dm644 packaging/linux/usb-forge.desktop ~/.local/share/applications/usb-forge.desktop
```

## Catalog

| Item | Where the version + checksum come from |
|---|---|
| Ubuntu Desktop / Server (LTS or Latest) | releases.ubuntu.com `SHA256SUMS` |
| Fedora Workstation | fedoraproject.org `releases.json` |
| Linux Mint Cinnamon | kernel.org mirror `sha256sum.txt` |
| Debian netinst | cdimage.debian.org `SHA256SUMS` |
| Hiren's BootCD PE | hirensbootcd.org download page |
| SystemRescue | system-rescue.org `.sha256` |
| Rescuezilla | GitHub release `SHA256SUM` |
| Clonezilla Live | clonezilla.org `CHECKSUMS.TXT` |
| GParted Live | gparted.org `CHECKSUMS.TXT` |
| Memtest86+ | memtest.org `sha256sum.txt` |

## Disk safety

- Only USB disks are listed. The disk that holds the running OS, the boot disk, and any disk the
  system uses (Windows system disk; on Linux, anything mounted outside `/media`, `/run/media` or
  `/mnt`, including swap) are hidden.
- Everything is downloaded and verified **before** the drive is touched.
- The target disk is checked again right before formatting, and you have to type `ERASE`.
- Each ISO is hashed again after it is copied to the drive.

## Building from source

```sh
cargo build --release          # uses the toolchain pinned in rust-toolchain.toml
cargo test --workspace
```

Diagnostics that never write to a disk:

```sh
usb-forge --version
usb-forge --list-disks   # every disk and whether it counts as a safe target
usb-forge --resolve      # latest version, URL and SHA-256 of every catalog item
```

To regenerate the icon on Windows, run `powershell -File assets/make-icon.ps1`.

## Releasing (maintainers)

1. Bump `version` in `Cargo.toml` through a PR and merge it.
2. Tag the merge commit: `git tag v0.2.0 && git push origin v0.2.0` (only admins can push `v*` tags).
3. Approve the **release** environment deployment in the Actions run. CI builds both binaries,
   signs them and publishes the GitHub release.

## Contributing

Pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md). Licensed under [MIT](LICENSE).

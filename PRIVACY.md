# Privacy Policy

USB Forge runs on your computer. It has no telemetry, no analytics, no crash reporting, no
advertising and no account, and there is no USB Forge server for it to talk to. It downloads
files; it never uploads anything.

This page lists every case in which USB Forge uses the network, what is sent, and what it keeps
on your computer.

## What every request contains

Each request is an ordinary web request that names the program and its version (for example
`UsbForge/0.3.1`). Like any web request, it comes from your IP address. USB Forge sends nothing
about you, your computer, your disks or your files. It has no installation ID and stores no
cookies.

## What USB Forge does by itself

When the app starts, it looks up two things:

1. **Is there a newer USB Forge?** It asks `api.github.com` for the latest release of
   [SkippyNet/usb-forge](https://github.com/SkippyNet/usb-forge).
2. **What is the current version of each item in the catalog?** It asks each project for its
   current version, its published SHA-256 and the size of the download. This covers every item
   in the catalog, whether or not you tick it, so the requests do not show which items you
   choose.

| Catalog item | Asked at start |
|---|---|
| Ubuntu Desktop / Server | `releases.ubuntu.com` |
| Fedora Workstation | `fedoraproject.org` |
| Linux Mint | `mirrors.edge.kernel.org` |
| Debian | `cdimage.debian.org` |
| Hiren's BootCD PE | `www.hirensbootcd.org` |
| SystemRescue | `www.system-rescue.org`, `fastly-cdn.system-rescue.org` |
| Rescuezilla | `api.github.com`, `github.com` |
| Clonezilla Live | `clonezilla.org`, `downloads.sourceforge.net` |
| GParted Live | `gparted.org`, `downloads.sourceforge.net` |
| Memtest86+ | `memtest.org` |

A size request can be redirected to one of that project's download mirrors.

This version has no setting that turns these lookups off. Without a network connection the app
still starts; the lookups fail and nothing else happens. `usb-forge --version` and
`usb-forge --list-disks` never use the network.

## What USB Forge does only when you ask

- **Build a drive.** USB Forge asks `api.github.com` for the latest release of the boot loader
  ([Ventoy](https://www.ventoy.net)) and downloads it from `github.com`. It then downloads the
  items you ticked from the official addresses above, which may redirect to a mirror. Every
  download is checked against the SHA-256 its project publishes.
- **Update & restart.** USB Forge downloads the new version and its signature from `github.com`
  and installs it only if the signature matches the key built into the app
  (see [SECURITY.md](SECURITY.md)).
- **Check versions.** The same catalog lookups as at start.
- **`usb-forge --resolve`.** The same catalog lookups, plus the boot loader lookup.
- **Links.** A link you click in the app opens in your web browser.

## What stays on your computer

- **Saved downloads.** ISOs and the boot loader package (with an unpacked copy) are kept in the
  downloads folder (`%LOCALAPPDATA%\UsbForge\cache` on Windows, `~/.cache/usb-forge` on Linux)
  so that building another drive does not download them again. On Windows the boot loader's
  installer also keeps its own log there. Delete the folder whenever you like; **Open** next to
  "Saved downloads" shows it.
- **Nothing else.** USB Forge has no settings file and writes no log of its own; the progress
  log lives in the window and is gone when you close the app.
- **Your disks.** The list of disks is read on your computer to find safe targets. It is never
  sent anywhere.

The finished USB drive holds the ISOs you chose and a small `ventoy` folder with the boot menu's
theme and settings. USB Forge writes nothing about you or the computer that built it onto the
drive.

## Other parties

When USB Forge uses the network, the service on the other end sees the request and handles it
under its own policy:

- GitHub (update check, USB Forge downloads, the boot loader, Rescuezilla):
  [GitHub General Privacy Statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement)
- Each project in the catalog, and the mirrors it uses, under that project's own policy. The
  projects are listed with links in the [credits](README.md#credits).

The website [skippynet.github.io/usb-forge](https://skippynet.github.io/usb-forge/) is hosted by
GitHub Pages. It loads no trackers, no third-party scripts and no cookies.

## Changes and questions

Changes to this policy are made in the open, in the
[history of this file](https://github.com/SkippyNet/usb-forge/commits/main/PRIVACY.md).

Questions: [Discussions](https://github.com/SkippyNet/usb-forge/discussions). Problems:
[open an issue](https://github.com/SkippyNet/usb-forge/issues).

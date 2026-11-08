//! The software catalog. Every entry is resolved live from its official source, and every
//! download is pinned to a SHA-256 published by that project.

use regex::Regex;

use crate::net::{find_sha256, get_text, is_sha256, remote_size};
use crate::sys::version_key;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ToolId {
    UbuntuDesktop,
    UbuntuServer,
    Fedora,
    Mint,
    Debian,
    Hirens,
    SystemRescue,
    Rescuezilla,
    Clonezilla,
    GParted,
    Memtest,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Channel {
    Lts,
    Latest,
}

impl Channel {
    pub fn label(self) -> &'static str {
        match self {
            Channel::Lts => "LTS",
            Channel::Latest => "Latest",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Installers,
    Rescue,
    DiskTools,
    Diagnostics,
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::Installers,
        Category::Rescue,
        Category::DiskTools,
        Category::Diagnostics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Installers => "Operating system installers",
            Category::Rescue => "Rescue & recovery",
            Category::DiskTools => "Backup, cloning & partitioning",
            Category::Diagnostics => "Diagnostics",
        }
    }
}

pub struct ToolInfo {
    pub id: ToolId,
    pub name: &'static str,
    pub blurb: &'static str,
    pub category: Category,
    /// Offers an LTS / Latest choice.
    pub has_channel: bool,
    pub default_on: bool,
}

pub const TOOLS: &[ToolInfo] = &[
    ToolInfo {
        id: ToolId::UbuntuDesktop,
        name: "Ubuntu Desktop",
        blurb: "The most popular Linux desktop. Try it live or install it.",
        category: Category::Installers,
        has_channel: true,
        default_on: true,
    },
    ToolInfo {
        id: ToolId::UbuntuServer,
        name: "Ubuntu Server",
        blurb: "Text-mode installer for servers and headless machines.",
        category: Category::Installers,
        has_channel: true,
        default_on: false,
    },
    ToolInfo {
        id: ToolId::Fedora,
        name: "Fedora Workstation",
        blurb: "Cutting-edge GNOME desktop from the Fedora Project.",
        category: Category::Installers,
        has_channel: false,
        default_on: false,
    },
    ToolInfo {
        id: ToolId::Mint,
        name: "Linux Mint Cinnamon",
        blurb: "Friendly, Windows-like desktop. Great for first-time Linux users.",
        category: Category::Installers,
        has_channel: false,
        default_on: false,
    },
    ToolInfo {
        id: ToolId::Debian,
        name: "Debian (netinst)",
        blurb: "Small network installer for rock-solid Debian stable.",
        category: Category::Installers,
        has_channel: false,
        default_on: false,
    },
    ToolInfo {
        id: ToolId::Hirens,
        name: "Hiren's BootCD PE",
        blurb: "Windows PE toolkit: password reset, malware removal, data recovery.",
        category: Category::Rescue,
        has_channel: false,
        default_on: true,
    },
    ToolInfo {
        id: ToolId::SystemRescue,
        name: "SystemRescue",
        blurb: "Linux rescue system for repairing unbootable machines.",
        category: Category::Rescue,
        has_channel: false,
        default_on: false,
    },
    ToolInfo {
        id: ToolId::Rescuezilla,
        name: "Rescuezilla",
        blurb: "Point-and-click backup, restore and recovery (Clonezilla-compatible).",
        category: Category::DiskTools,
        has_channel: false,
        default_on: false,
    },
    ToolInfo {
        id: ToolId::Clonezilla,
        name: "Clonezilla Live",
        blurb: "Disk imaging and cloning, trusted by sysadmins for decades.",
        category: Category::DiskTools,
        has_channel: false,
        default_on: false,
    },
    ToolInfo {
        id: ToolId::GParted,
        name: "GParted Live",
        blurb: "Resize, move, create and repair partitions.",
        category: Category::DiskTools,
        has_channel: false,
        default_on: false,
    },
    ToolInfo {
        id: ToolId::Memtest,
        name: "Memtest86+",
        blurb: "Thorough RAM tester. Tiny, and works on almost any PC.",
        category: Category::Diagnostics,
        has_channel: false,
        default_on: false,
    },
];

pub fn info(id: ToolId) -> &'static ToolInfo {
    TOOLS
        .iter()
        .find(|t| t.id == id)
        .expect("every ToolId has catalog info")
}

/// A concrete, checksum-pinned download.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub version: String,
    pub url: String,
    /// Name of the downloaded file (in the cache).
    pub download_name: String,
    /// Name of the ISO placed on the USB drive.
    pub iso_name: String,
    pub sha256: String,
    pub size: Option<u64>,
    /// The download is a zip containing the ISO (Memtest86+).
    pub zipped: bool,
}

impl Resolved {
    fn new(version: String, url: String, file: String, sha256: String) -> Self {
        Resolved {
            version,
            url,
            download_name: file.clone(),
            iso_name: file,
            sha256,
            size: None,
            zipped: false,
        }
    }
}

pub fn resolve(id: ToolId, channel: Channel) -> Result<Resolved, String> {
    let mut r = match id {
        ToolId::UbuntuDesktop => ubuntu("desktop", channel),
        ToolId::UbuntuServer => ubuntu("live-server", channel),
        ToolId::Fedora => fedora(),
        ToolId::Mint => mint(),
        ToolId::Debian => debian(),
        ToolId::Hirens => hirens(),
        ToolId::SystemRescue => systemrescue(),
        ToolId::Rescuezilla => rescuezilla(),
        ToolId::Clonezilla => clonezilla(),
        ToolId::GParted => gparted(),
        ToolId::Memtest => memtest(),
    }?;
    if r.size.is_none() {
        r.size = remote_size(&r.url);
    }
    Ok(r)
}

// ---------------------------------------------------------------------------------------

fn sorted_desc(mut versions: Vec<String>) -> Vec<String> {
    versions.sort_by_key(|v| std::cmp::Reverse(version_key(v)));
    versions.dedup();
    versions
}

fn captures(re: &str, text: &str) -> Vec<String> {
    let re = Regex::new(re).expect("valid regex");
    re.captures_iter(text).map(|c| c[1].to_string()).collect()
}

fn ubuntu(kind: &str, channel: Channel) -> Result<Resolved, String> {
    let index = get_text("https://releases.ubuntu.com/")?;
    let versions = captures(r#"href="(\d{2}\.\d{2}(?:\.\d+)*)/""#, &index)
        .into_iter()
        .filter(|v| {
            let k = version_key(v);
            channel == Channel::Latest || (k.len() >= 2 && k[0].is_multiple_of(2) && k[1] == 4)
        })
        .collect();
    for v in sorted_desc(versions).iter().take(6) {
        let Ok(sums) = get_text(&format!("https://releases.ubuntu.com/{v}/SHA256SUMS")) else {
            continue;
        };
        let name = format!("ubuntu-{v}-{kind}-amd64.iso");
        if let Some(hash) = find_sha256(&sums, &name) {
            let url = format!("https://releases.ubuntu.com/{v}/{name}");
            let label = if channel == Channel::Lts {
                format!("{v} LTS")
            } else {
                v.clone()
            };
            return Ok(Resolved::new(label, url, name, hash));
        }
    }
    Err("No Ubuntu release with a published checksum was found.".into())
}

fn fedora() -> Result<Resolved, String> {
    let json = get_text("https://fedoraproject.org/releases.json")?;
    let list: Vec<serde_json::Value> =
        serde_json::from_str(&json).map_err(|e| format!("Fedora release list: {e}"))?;
    let best = list
        .iter()
        .filter(|e| {
            e["variant"] == "Workstation"
                && e["arch"] == "x86_64"
                && e["link"].as_str().is_some_and(|l| l.ends_with(".iso"))
                && e["version"]
                    .as_str()
                    .is_some_and(|v| v.parse::<u32>().is_ok())
        })
        .max_by_key(|e| e["version"].as_str().and_then(|v| v.parse::<u32>().ok()))
        .ok_or("No Fedora Workstation ISO found.")?;
    let url = best["link"].as_str().unwrap_or_default().to_string();
    let sha = best["sha256"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !is_sha256(&sha) {
        return Err("Fedora did not publish a SHA-256 for its ISO.".into());
    }
    let name = url.rsplit('/').next().unwrap_or("fedora.iso").to_string();
    let mut r = Resolved::new(
        best["version"].as_str().unwrap_or("?").into(),
        url,
        name,
        sha,
    );
    r.size = best["size"].as_str().and_then(|s| s.parse().ok());
    Ok(r)
}

fn mint() -> Result<Resolved, String> {
    let base = "https://mirrors.edge.kernel.org/linuxmint/stable";
    let index = get_text(&format!("{base}/"))?;
    for v in sorted_desc(captures(r#"href="(\d+(?:\.\d+)*)/""#, &index))
        .iter()
        .take(3)
    {
        let Ok(sums) = get_text(&format!("{base}/{v}/sha256sum.txt")) else {
            continue;
        };
        let name = format!("linuxmint-{v}-cinnamon-64bit.iso");
        if let Some(hash) = find_sha256(&sums, &name) {
            return Ok(Resolved::new(
                v.clone(),
                format!("{base}/{v}/{name}"),
                name,
                hash,
            ));
        }
    }
    Err("No Linux Mint release with a published checksum was found.".into())
}

fn debian() -> Result<Resolved, String> {
    let base = "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd";
    let sums = get_text(&format!("{base}/SHA256SUMS"))?;
    let re = Regex::new(r"^debian-(\d+\.\d+\.\d+)-amd64-netinst\.iso$").expect("valid regex");
    for line in sums.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(hash), Some(name)) = (parts.next(), parts.next())
            && let Some(c) = re.captures(name)
            && is_sha256(hash)
        {
            return Ok(Resolved::new(
                c[1].to_string(),
                format!("{base}/{name}"),
                name.to_string(),
                hash.to_ascii_lowercase(),
            ));
        }
    }
    Err("Debian netinst checksum not found.".into())
}

fn hirens() -> Result<Resolved, String> {
    let page = get_text("https://www.hirensbootcd.org/download/")?;
    let hash = Regex::new(r"\b[0-9a-fA-F]{64}\b")
        .expect("valid regex")
        .find(&page)
        .map(|m| m.as_str().to_ascii_lowercase())
        .ok_or("Hiren's BootCD checksum not found on the download page.")?;
    let version = captures(r"v(\d+\.\d+(?:\.\d+)?)\)\s*-\s*ISO Content", &page)
        .into_iter()
        .next()
        .unwrap_or_else(|| "latest".into());
    Ok(Resolved::new(
        version,
        "https://www.hirensbootcd.org/files/HBCD_PE_x64.iso".into(),
        "HBCD_PE_x64.iso".into(),
        hash,
    ))
}

fn systemrescue() -> Result<Resolved, String> {
    let page = get_text("https://www.system-rescue.org/Download/")?;
    let v = sorted_desc(captures(
        r"https://fastly-cdn\.system-rescue\.org/releases/([\d.]+)/systemrescue-[\d.]+-amd64\.iso",
        &page,
    ))
    .into_iter()
    .next()
    .ok_or("SystemRescue download link not found.")?;
    let name = format!("systemrescue-{v}-amd64.iso");
    let url = format!("https://fastly-cdn.system-rescue.org/releases/{v}/{name}");
    let sums = get_text(&format!("{url}.sha256"))?;
    let hash = find_sha256(&sums, &name).ok_or("SystemRescue checksum not found.")?;
    Ok(Resolved::new(v, url, name, hash))
}

fn rescuezilla() -> Result<Resolved, String> {
    let json = get_text("https://api.github.com/repos/rescuezilla/rescuezilla/releases/latest")?;
    let rel: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("Rescuezilla release info: {e}"))?;
    let tag = rel["tag_name"]
        .as_str()
        .ok_or("Rescuezilla release has no tag.")?;
    let base = format!("https://github.com/rescuezilla/rescuezilla/releases/download/{tag}");
    let sums = get_text(&format!("{base}/SHA256SUM"))?;
    let isos: Vec<&str> = sums
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .filter(|n| n.ends_with(".iso"))
        .collect();
    // Prefer the build based on the newest Ubuntu LTS, for the widest hardware support.
    let name = ["resolute", "noble"]
        .iter()
        .find_map(|code| isos.iter().find(|n| n.contains(code)))
        .or(isos.last())
        .ok_or("No Rescuezilla ISO listed.")?
        .to_string();
    let hash = find_sha256(&sums, &name).ok_or("Rescuezilla checksum not found.")?;
    Ok(Resolved::new(
        tag.into(),
        format!("{base}/{name}"),
        name,
        hash,
    ))
}

/// First SHA-256 line in a checksum listing whose file name matches `name_re`. Returns
/// (hash, file name, first capture group). Other hash types are skipped by their length.
fn sha256_line_matching(text: &str, name_re: &str) -> Option<(String, String, String)> {
    let re = Regex::new(name_re).expect("valid regex");
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?;
        let c = re.captures(name)?;
        is_sha256(hash).then(|| {
            (
                hash.to_ascii_lowercase(),
                name.to_string(),
                c[1].to_string(),
            )
        })
    })
}

fn clonezilla() -> Result<Resolved, String> {
    let sums = get_text("https://clonezilla.org/downloads/stable/data/CHECKSUMS.TXT")?;
    let (hash, name, v) = sha256_line_matching(&sums, r"^clonezilla-live-([\d.\-]+)-amd64\.iso$")
        .ok_or("Clonezilla checksum not found.")?;
    let url = format!(
        "https://downloads.sourceforge.net/project/clonezilla/clonezilla_live_stable/{v}/{name}"
    );
    Ok(Resolved::new(v, url, name, hash))
}

fn gparted() -> Result<Resolved, String> {
    let sums = get_text("https://gparted.org/gparted-live/stable/CHECKSUMS.TXT")?;
    let (hash, name, v) = sha256_line_matching(&sums, r"^gparted-live-([\d.\-]+)-amd64\.iso$")
        .ok_or("GParted checksum not found.")?;
    let url = format!("https://downloads.sourceforge.net/gparted/{name}");
    Ok(Resolved::new(v, url, name, hash))
}

fn memtest() -> Result<Resolved, String> {
    let page = get_text("https://memtest.org/")?;
    let v = sorted_desc(captures(r"download/v([\d.]+)/", &page))
        .into_iter()
        .next()
        .ok_or("Memtest86+ version not found.")?;
    let zip = format!("mt86plus_{v}_x86_64.iso.zip");
    let sums = get_text(&format!("https://memtest.org/download/v{v}/sha256sum.txt"))?;
    let hash = find_sha256(&sums, &zip).ok_or("Memtest86+ checksum not found.")?;
    Ok(Resolved {
        version: v.clone(),
        url: format!("https://memtest.org/download/v{v}/{zip}"),
        download_name: zip,
        iso_name: format!("memtest86plus-{v}-x86_64.iso"),
        sha256: hash,
        size: None,
        zipped: true,
    })
}

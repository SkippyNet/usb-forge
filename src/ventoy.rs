//! Ventoy: fetch the latest verified release for this OS. Installation lives in `platform`.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::net::{self, find_sha256, get_text};
use crate::platform;

/// Volume label of the data partition that holds the ISOs.
pub const DATA_LABEL: &str = "USB Forge";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
// Ventoy's Linux installer only formats exFAT, so the other variants exist only on Windows.
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub enum FileSystem {
    ExFat,
    Ntfs,
    Fat32,
}

impl FileSystem {
    pub fn cli(self) -> &'static str {
        match self {
            FileSystem::ExFat => "exFAT",
            FileSystem::Ntfs => "NTFS",
            FileSystem::Fat32 => "FAT32",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            FileSystem::ExFat => "exFAT (recommended): big files, readable everywhere",
            FileSystem::Ntfs => "NTFS: big files, best on Windows",
            FileSystem::Fat32 => "FAT32: most compatible, 4 GB file limit",
        }
    }

    /// Largest single file the filesystem can hold.
    pub fn max_file(self) -> Option<u64> {
        match self {
            FileSystem::Fat32 => Some(4 * 1024 * 1024 * 1024 - 1),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub file_name: String,
    pub sha256: String,
}

pub fn latest_release() -> Result<Release, String> {
    let json = get_text("https://api.github.com/repos/ventoy/Ventoy/releases/latest")?;
    let rel: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("Boot loader release info: {e}"))?;
    let tag = rel["tag_name"]
        .as_str()
        .ok_or("Boot loader release has no tag.")?;
    let assets = rel["assets"]
        .as_array()
        .ok_or("Boot loader release has no assets.")?;
    let asset_url = |pred: &dyn Fn(&str) -> bool| {
        assets.iter().find_map(|a| {
            let name = a["name"].as_str()?;
            let url = a["browser_download_url"].as_str()?;
            pred(name).then(|| (name.to_string(), url.to_string()))
        })
    };
    let (file_name, url) = asset_url(&|n| n.ends_with(platform::VENTOY_ASSET_SUFFIX))
        .ok_or("Boot loader package for this OS not found in the release.")?;
    let (_, sums_url) =
        asset_url(&|n| n == "sha256.txt").ok_or("Boot loader release has no checksum list.")?;
    let sums = get_text(&sums_url)?;
    let sha256 = find_sha256(&sums, &file_name).ok_or("Boot loader checksum not found.")?;
    Ok(Release {
        version: tag.trim_start_matches('v').to_string(),
        url,
        file_name,
        sha256,
    })
}

/// Download (or reuse) the verified Ventoy package and extract it. Returns the folder that
/// contains the Ventoy installer.
pub fn prepare(
    rel: &Release,
    cache: &Path,
    progress: net::Progress<'_>,
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let archive = cache.join(&rel.file_name);
    let cached_ok = archive.exists()
        && net::sha256_file(&archive, &mut |_, _| {}, cancel)?.eq_ignore_ascii_case(&rel.sha256);
    if !cached_ok {
        net::download_verified(&rel.url, &archive, &rel.sha256, progress, cancel)?;
    }
    let dir = cache.join(format!(
        "ventoy-{}{}",
        rel.version,
        platform::VENTOY_ASSET_SUFFIX.replace('.', "_")
    ));
    if let Some(found) = find_tool(&dir) {
        return Ok(found);
    }
    platform::extract_ventoy(&archive, &dir)?;
    find_tool(&dir).ok_or_else(|| format!("{} missing from the package.", platform::VENTOY_TOOL))
}

fn find_tool(dir: &Path) -> Option<PathBuf> {
    if dir.join(platform::VENTOY_TOOL).is_file() {
        return Some(dir.to_path_buf());
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .find_map(|p| find_tool(&p))
}

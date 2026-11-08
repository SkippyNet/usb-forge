//! HTTP helpers: text fetches, verified downloads with progress, SHA-256 hashing.

use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

const CHUNK: usize = 1024 * 1024;

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .user_agent(concat!("UsbForge/", env!("CARGO_PKG_VERSION")))
            .timeout_connect(Some(Duration::from_secs(20)))
            .max_redirects(10)
            .build()
            .into()
    })
}

/// Fetch a small text resource (index pages, checksum lists, JSON).
pub fn get_text(url: &str) -> Result<String, String> {
    let mut resp = agent()
        .get(url)
        .call()
        .map_err(|e| format!("Could not fetch {url}: {e}"))?;
    resp.body_mut()
        .with_config()
        .limit(32 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("Could not read {url}: {e}"))
}

/// Size of a remote file, if the server reports it.
pub fn remote_size(url: &str) -> Option<u64> {
    let resp = agent().head(url).call().ok()?;
    resp.headers()
        .get("content-length")?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

pub type Progress<'a> = &'a mut dyn FnMut(u64, Option<u64>);

fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Cancelled.".into())
    } else {
        Ok(())
    }
}

/// Download `url` to `dest`, hashing as it streams, and fail unless the SHA-256 matches.
/// The file is written to `dest.part` first and only renamed into place once verified.
pub fn download_verified(
    url: &str,
    dest: &Path,
    expected_sha256: &str,
    progress: Progress<'_>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let part = dest.with_extension("part");
    let got = download_to(url, &part, progress, cancel)?;
    if !got.eq_ignore_ascii_case(expected_sha256) {
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "Checksum mismatch for {url}\n  expected {expected_sha256}\n  got      {got}"
        ));
    }
    std::fs::rename(&part, dest).map_err(|e| format!("Cannot finalize {}: {e}", dest.display()))
}

/// Stream `url` into `part` and return the SHA-256 of what was received.
/// The file is removed again if the download fails or is cancelled.
pub fn download_to(
    url: &str,
    part: &Path,
    progress: Progress<'_>,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let result = stream_to(url, part, progress, cancel);
    if result.is_err() {
        let _ = std::fs::remove_file(part);
    }
    result
}

fn stream_to(
    url: &str,
    part: &Path,
    progress: Progress<'_>,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let resp = agent()
        .get(url)
        .call()
        .map_err(|e| format!("Download failed for {url}: {e}"))?;
    let total = resp.body().content_length();
    let mut reader = resp.into_body().into_with_config().limit(u64::MAX).reader();
    let mut out =
        File::create(part).map_err(|e| format!("Cannot write {}: {e}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    loop {
        cancelled(cancel)?;
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Download interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])
            .map_err(|e| format!("Disk write failed: {e}"))?;
        done += n as u64;
        progress(done, total);
    }
    out.sync_all()
        .map_err(|e| format!("Disk flush failed: {e}"))?;
    Ok(hex::encode(hasher.finalize()))
}

/// SHA-256 of a local file, with progress.
pub fn sha256_file(
    path: &Path,
    progress: Progress<'_>,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let total = file.metadata().map(|m| m.len()).ok();
    let mut reader = BufReader::with_capacity(CHUNK, file);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    loop {
        cancelled(cancel)?;
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Read failed for {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
        progress(done, total);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Copy `reader` into a new file at `dest`, returning the SHA-256 of what was written.
pub fn copy_hashed(
    mut reader: impl Read,
    total: Option<u64>,
    dest: &Path,
    progress: Progress<'_>,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let mut out =
        File::create(dest).map_err(|e| format!("Cannot create {}: {e}", dest.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 4 * CHUNK];
    let mut done = 0u64;
    loop {
        cancelled(cancel)?;
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Read failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])
            .map_err(|e| format!("Write to USB failed: {e}"))?;
        done += n as u64;
        progress(done, total);
    }
    out.sync_all()
        .map_err(|e| format!("Flush to USB failed: {e}"))?;
    Ok(hex::encode(hasher.finalize()))
}

/// Find `name` in a `sha256sum`-style listing (`<hash> [*]<name>`), returning the lowercase hash.
pub fn find_sha256(listing: &str, name: &str) -> Option<String> {
    listing.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let file = parts.next()?.trim_start_matches('*');
        let file = file.rsplit('/').next().unwrap_or(file);
        (file == name && is_sha256(hash)).then(|| hash.to_ascii_lowercase())
    })
}

pub fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

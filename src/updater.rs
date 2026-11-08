//! Signed self-update from GitHub Releases.
//!
//! A release is installed only if its binary carries a valid Ed25519 signature from the
//! USB Forge release key, whose public half is compiled into this executable. The signed
//! message binds the version and asset name, so an attacker who controls the download (or even
//! the GitHub repository) cannot substitute a different binary, replay an older signed one,
//! or swap platforms. GitHub's own checksums are not trusted on their own.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use ed25519_dalek::{Signature, VerifyingKey};

use crate::net::{self, get_text};
use crate::platform;
use crate::release_msg::release_message;
use crate::sys::version_key;

pub const REPO: &str = "SkippyNet/usb-forge";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const PUBLIC_KEY_HEX: &str = include_str!("../keys/release-signing.pub");

#[derive(Clone, Debug)]
pub struct Update {
    pub version: String,
    pub asset_url: String,
    pub sig_url: String,
    pub page_url: String,
}

/// Look for a newer, non-prerelease version published for this platform.
pub fn check() -> Result<Option<Update>, String> {
    let json = get_text(&format!(
        "https://api.github.com/repos/{REPO}/releases/latest"
    ))?;
    let rel: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("Release info: {e}"))?;
    if rel["draft"].as_bool() == Some(true) || rel["prerelease"].as_bool() == Some(true) {
        return Ok(None);
    }
    let version = rel["tag_name"]
        .as_str()
        .ok_or("Release has no tag.")?
        .trim_start_matches('v')
        .to_string();
    if version_key(&version) <= version_key(CURRENT_VERSION) {
        return Ok(None);
    }
    let assets = rel["assets"].as_array().cloned().unwrap_or_default();
    let url_of = |name: &str| {
        assets.iter().find_map(|a| {
            (a["name"].as_str() == Some(name))
                .then(|| a["browser_download_url"].as_str().map(String::from))
                .flatten()
        })
    };
    let (Some(asset_url), Some(sig_url)) = (
        url_of(platform::UPDATE_ASSET),
        url_of(&format!("{}.sig", platform::UPDATE_ASSET)),
    ) else {
        return Ok(None);
    };
    Ok(Some(Update {
        version,
        asset_url,
        sig_url,
        page_url: rel["html_url"].as_str().unwrap_or_default().to_string(),
    }))
}

/// Download, verify and install `update`. Returns the path of the new executable; the caller
/// starts it and exits.
pub fn install(
    update: &Update,
    progress: net::Progress<'_>,
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = current.parent().ok_or("Cannot locate the app folder.")?;
    let staged = dir.join(format!("{}.download", platform::UPDATE_ASSET));
    let sha = net::download_to(&update.asset_url, &staged, progress, cancel)?;
    let sig = get_text(&update.sig_url)?;
    if let Err(e) = verify(
        &update.version,
        platform::UPDATE_ASSET,
        &sha,
        &sig,
        PUBLIC_KEY_HEX,
    ) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("Update rejected: {e}. Nothing was changed."));
    }
    platform::swap_executable(&staged)
}

/// Start the freshly installed executable.
pub fn restart(exe: &Path) -> Result<(), String> {
    platform::hidden(&exe.to_string_lossy())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Updated, but could not restart automatically: {e}"))
}

fn verify(
    version: &str,
    asset: &str,
    sha256_hex: &str,
    sig_hex: &str,
    public_hex: &str,
) -> Result<(), String> {
    let public: [u8; 32] = hex::decode(public_hex.trim())
        .map_err(|_| "embedded public key is malformed")?
        .try_into()
        .map_err(|_| "embedded public key has the wrong length")?;
    let key = VerifyingKey::from_bytes(&public).map_err(|_| "embedded public key is invalid")?;
    let sig: [u8; 64] = hex::decode(sig_hex.trim())
        .map_err(|_| "signature is not hex")?
        .try_into()
        .map_err(|_| "signature has the wrong length")?;
    key.verify_strict(
        &release_message(version, asset, sha256_hex),
        &Signature::from_bytes(&sig),
    )
    .map_err(|_| "signature does not match the USB Forge release key".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn keypair() -> (SigningKey, String) {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let public = hex::encode(key.verifying_key().to_bytes());
        (key, public)
    }

    #[test]
    fn accepts_a_correct_signature() {
        let (key, public) = keypair();
        let sha = "ab".repeat(32);
        let sig = hex::encode(
            key.sign(&release_message("1.2.3", "a.exe", &sha))
                .to_bytes(),
        );
        assert!(verify("1.2.3", "a.exe", &sha, &sig, &public).is_ok());
        assert!(verify("v1.2.3", "a.exe", &sha, &sig, &public).is_ok());
    }

    #[test]
    fn rejects_tampering_replay_and_platform_swap() {
        let (key, public) = keypair();
        let sha = "ab".repeat(32);
        let sig = hex::encode(
            key.sign(&release_message("1.2.3", "a.exe", &sha))
                .to_bytes(),
        );
        assert!(
            verify("1.2.3", "a.exe", &"cd".repeat(32), &sig, &public).is_err(),
            "modified binary"
        );
        assert!(
            verify("9.9.9", "a.exe", &sha, &sig, &public).is_err(),
            "replayed as newer version"
        );
        assert!(
            verify("1.2.3", "b", &sha, &sig, &public).is_err(),
            "other platform's asset"
        );
        let other = hex::encode(
            SigningKey::from_bytes(&[9u8; 32])
                .verifying_key()
                .to_bytes(),
        );
        assert!(
            verify("1.2.3", "a.exe", &sha, &sig, &other).is_err(),
            "wrong key"
        );
        assert!(
            verify("1.2.3", "a.exe", &sha, "zz", &public).is_err(),
            "garbage signature"
        );
    }

    #[test]
    fn embedded_key_is_well_formed() {
        let bytes = hex::decode(PUBLIC_KEY_HEX.trim()).expect("hex");
        let arr: [u8; 32] = bytes.try_into().expect("32 bytes");
        assert!(VerifyingKey::from_bytes(&arr).is_ok());
    }
}

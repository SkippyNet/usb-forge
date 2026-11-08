//! The exact bytes a release signature covers. Shared (via `#[path]`) with `tools/release-sign`,
//! so the signer and the updater can never disagree.
//!
//! Binding the version and asset name into the signed message stops a validly signed *old*
//! binary from being replayed as a "new" release (downgrade attack), and stops one platform's
//! binary from being served as another's.

pub fn release_message(version: &str, asset: &str, sha256_hex: &str) -> Vec<u8> {
    format!(
        "usb-forge-release-v1\n{}\n{}\n{}\n",
        version.trim_start_matches('v'),
        asset,
        sha256_hex.to_ascii_lowercase()
    )
    .into_bytes()
}

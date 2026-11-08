//! release-sign: key generation and signing for USB Forge releases.
//!
//!   release-sign keygen <dir>
//!       Writes <dir>/release-signing.key (SECRET, hex) and <dir>/release-signing.pub (hex).
//!
//!   release-sign sign <file> <version> <asset-name>
//!       Reads the hex secret key from the USB_FORGE_SIGNING_KEY environment variable and writes
//!       <file>.sig (hex Ed25519 signature over the release message).
//!
//!   release-sign verify <file> <version> <asset-name> <public-key-hex>

use std::path::Path;
use std::process::ExitCode;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

#[path = "../../../src/release_msg.rs"]
mod release_msg;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["keygen", dir] => keygen(Path::new(dir)),
        ["sign", file, version, asset] => sign(Path::new(file), version, asset),
        ["verify", file, version, asset, public] => verify(Path::new(file), version, asset, public),
        _ => Err("usage: release-sign keygen <dir> | sign <file> <version> <asset> | verify <file> <version> <asset> <pubhex>".into()),
    };
    match result {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn keygen(dir: &Path) -> Result<String, String> {
    let secret_path = dir.join("release-signing.key");
    if secret_path.exists() {
        return Err(format!(
            "{} already exists; refusing to overwrite",
            secret_path.display()
        ));
    }
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| format!("no OS randomness: {e}"))?;
    let key = SigningKey::from_bytes(&seed);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(&secret_path, hex::encode(key.to_bytes())).map_err(|e| e.to_string())?;
    let public_hex = hex::encode(key.verifying_key().to_bytes());
    std::fs::write(dir.join("release-signing.pub"), &public_hex).map_err(|e| e.to_string())?;
    Ok(format!(
        "public key: {public_hex}\nsecret key written to {}",
        secret_path.display()
    ))
}

fn file_sha256(file: &Path) -> Result<String, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

fn sign(file: &Path, version: &str, asset: &str) -> Result<String, String> {
    let secret_hex =
        std::env::var("USB_FORGE_SIGNING_KEY").map_err(|_| "USB_FORGE_SIGNING_KEY is not set")?;
    let secret: [u8; 32] = hex::decode(secret_hex.trim())
        .map_err(|e| format!("bad key hex: {e}"))?
        .try_into()
        .map_err(|_| "signing key must be 32 bytes")?;
    let key = SigningKey::from_bytes(&secret);
    let sha = file_sha256(file)?;
    let sig = key.sign(&release_msg::release_message(version, asset, &sha));
    let sig_path = format!("{}.sig", file.display());
    std::fs::write(&sig_path, hex::encode(sig.to_bytes())).map_err(|e| e.to_string())?;
    Ok(format!(
        "signed {asset} {version} (sha256 {sha}) -> {sig_path}"
    ))
}

fn verify(file: &Path, version: &str, asset: &str, public_hex: &str) -> Result<String, String> {
    let public: [u8; 32] = hex::decode(public_hex.trim())
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|_| "public key must be 32 bytes")?;
    let key = VerifyingKey::from_bytes(&public).map_err(|e| e.to_string())?;
    let sig_hex =
        std::fs::read_to_string(format!("{}.sig", file.display())).map_err(|e| e.to_string())?;
    let sig: [u8; 64] = hex::decode(sig_hex.trim())
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|_| "signature must be 64 bytes")?;
    let sha = file_sha256(file)?;
    key.verify_strict(
        &release_msg::release_message(version, asset, &sha),
        &Signature::from_bytes(&sig),
    )
    .map_err(|e| format!("INVALID signature: {e}"))?;
    Ok(format!("valid signature for {asset} {version}"))
}

# Security Policy

USB Forge formats disks and updates itself, so we treat its supply chain as security-critical.

## Reporting a vulnerability

Please **do not open a public issue.** Use GitHub's
[private vulnerability reporting](https://github.com/SkippyNet/usb-forge/security/advisories/new)
instead. We aim to reply within 7 days.

## How releases are protected

| Layer | Protection |
|---|---|
| Source | `main` only accepts changes through pull requests, which need code-owner approval, passing CI and resolved review threads. Force-pushes and branch deletion are blocked. |
| CI | Workflows run with a read-only token. PRs use `pull_request` (never `pull_request_target`), so fork code can't reach secrets. Workflow runs from outside contributors need approval. Actions are restricted to GitHub-owned ones and pinned to full commit SHAs. |
| Tags | Only repository admins can create, move or delete `v*` tags. |
| Signing | The Ed25519 release key lives only in the protected `release` environment. That environment needs maintainer approval and only accepts deployments from `v*` tags. |
| Updates | The app installs an update only if the binary's signature verifies against the public key **compiled into the app** (`keys/release-signing.pub`). The signature covers the version and asset name, so tampered binaries, replayed old releases and cross-platform swaps are all rejected, even if the GitHub repository itself is compromised. |
| Downloads | Every ISO and Ventoy package is checked against the SHA-256 published by its own project before the drive is touched. |
| Dependencies | `Cargo.lock` is committed, builds use `--locked`, `cargo audit` runs on every PR, and Dependabot proposes updates as reviewable PRs. |

## Verifying a release manually

```sh
cargo run -p release-sign -- verify usb-forge-linux-x86_64 0.1.0 usb-forge-linux-x86_64 "$(cat keys/release-signing.pub)"
```

The `.sig` file must sit next to the binary.

## Supported versions

Only the latest release gets security fixes.

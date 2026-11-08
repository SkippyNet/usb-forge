# USB Forge: project instructions

## Every feature or fix: bump the version before merging

Every PR that changes the app (feature, fix or user-visible text) **must include a version bump
before it is merged**:

1. Bump `version` in the root `Cargo.toml` using semver:
   - **patch** (0.1.1 → 0.1.2): fixes, wording, small tweaks
   - **minor** (0.1.x → 0.2.0): new features or catalog items
   - **major**: breaking changes (on-disk layout, update/signing format)
2. Run `cargo build` so `Cargo.lock` picks up the new version, and commit both files in the same PR.
3. Make sure the new version is higher than the latest tag (`git tag --sort=-v:refname | head -1`)
   and than the version on `main`.
4. Put the new version in the PR title, e.g. `Secure Boot notice and vendor guide (v0.1.2)`.

PRs that only touch docs, CI or Dependabot dependency updates don't need a bump unless they ship
in a release.

## Releasing (after the PR is merged)

1. Tag the merge commit on `main` with the bumped version: `git tag -a vX.Y.Z -m "USB Forge vX.Y.Z"`
   then `git push origin vX.Y.Z`. The tag must match `Cargo.toml`, or the release workflow fails.
2. The Release workflow builds Windows and Linux, then waits at the protected `release`
   environment. **Only approve that deployment when the user explicitly asks.** Otherwise tell
   them it's waiting for their approval.
3. Check the published release: assets, `SHA256SUMS.txt`, and signatures with
   `release-sign verify` against `keys/release-signing.pub`.

## Workflow rules

- `main` only accepts PRs (code-owner approval, CI on Windows + Linux, audit). Never push to
  `main` directly. Merge with the admin bypass only when the user asks.
- Before opening a PR, both must be clean on Windows **and** Linux (WSL: `~/src/usb-forge`):
  `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`.
- Pin GitHub Actions to full commit SHAs (repo policy requires it).
- Never commit secrets. Signing keys live in `%USERPROFILE%\.usb-forge-signing\` and in the
  `release` environment secret only.

## Product rules

- User-facing text must not mention Ventoy. Refer to "the boot loader" or "the boot menu".
- Don't build anything that bypasses or weakens Secure Boot verification. When Secure Boot is
  on, show the notice and link `docs/SECURE_BOOT.md`.
- Every download must be pinned to a SHA-256 published by the upstream project.

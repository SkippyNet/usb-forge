# Contributing to USB Forge

Thanks for helping! This tool erases disks and updates itself on users' machines, so every
change goes through review.

## Workflow

1. **Fork** the repository and create a branch.
2. Make your change. Run these and make sure they pass:
   ```sh
   cargo fmt --all
   cargo clippy --workspace --all-targets --locked -- -D warnings
   cargo test --workspace --locked
   ```
3. **Bump the version** in `Cargo.toml` (semver: patch for fixes and wording, minor for
   features) and commit the updated `Cargo.lock` in the same PR. Every feature or fix ships with
   its own version, so the version must be cut before the PR is merged. (Docs-only and CI-only
   PRs are exempt.)
4. Open a **pull request** against `main` and fill in the template.
5. A maintainer reviews it. Before merging, `main` requires:
   - approval from a code owner, given after your latest push
   - passing CI on Windows and Linux, plus the dependency audit
   - all review conversations resolved
   - squash merge (linear history)

Nobody can push directly to `main`, maintainers included. CI on PRs from first-time contributors
starts only after a maintainer approves the run.

## Adding software to the catalog

New entries must:

- download from the project's **official** site or official mirror
- verify against a **SHA-256 published by that project** (no hard-coded hashes of files we
  downloaded ourselves)
- be open source or freely redistributable, and widely trusted

## Changes that get extra scrutiny

Disk detection and safety filters (`src/disks.rs`, `src/platform/`), the updater and signing
(`src/updater.rs`, `src/release_msg.rs`, `keys/`, `tools/`), CI (`.github/`), and new
dependencies. Please explain why these changes are needed.

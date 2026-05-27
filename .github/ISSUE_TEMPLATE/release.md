---
name: Release
about: Track a new version release
title: 'Release vX.Y.Z'
labels: release
assignees: ''

---

## Release Version

**Version:** vX.Y.Z

## Pre-Release Checklist

- [ ] All tests passing: `cargo test`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Code is formatted: `cargo fmt --check`
- [ ] CHANGELOG.md updated with new version changes
- [ ] Version bumped in `Cargo.toml`
- [ ] `dist-workspace.toml` in sync with installed cargo-dist version

## Create Release

- [ ] Commit version bump: `git commit -m "chore: release X.Y.Z"`
- [ ] Push to main: `git push`
- [ ] Create version tag: `git tag vX.Y.Z`
- [ ] Push tag: `git push origin vX.Y.Z`
- [ ] Wait for GitHub Actions workflows to complete (~10-15 minutes)

## Automated Release Verification

### Core Release (`.github/workflows/release.yml`)
- [ ] GitHub Release created at https://github.com/bgreenwell/jotdown-rs/releases/tag/vX.Y.Z
- [ ] All artifacts present (binaries, tarballs, installers, checksums)
- [ ] Homebrew formula published to [homebrew-jotdown-rs](https://github.com/bgreenwell/homebrew-jotdown-rs)
- [ ] Published to [crates.io](https://crates.io/crates/jotdown-rs)

### Scoop Publishing (`.github/workflows/publish-scoop.yml`)
- [ ] Manifest updated in [scoop-bucket](https://github.com/bgreenwell/scoop-bucket)

### AUR Publishing (`.github/workflows/publish-aur.yml`)
- [ ] PKGBUILD updated in [jotdown-rs-bin](https://aur.archlinux.org/packages/jotdown-rs-bin)

### WinGet Publishing (`.github/workflows/publish-winget.yml`)
- [ ] PR created to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs)
- [ ] PR merged (may take 1-2 days, requires Microsoft approval)

## Post-Release

- [ ] All workflows completed successfully
- [ ] Close this issue

## Notes

<!-- Add any release-specific notes or issues encountered -->

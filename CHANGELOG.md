# Changelog

All notable changes to this project are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - 2026-05-23

### Added
- Stdin support for multiline note creation via piping.
- Robust shell argument parsing using `shell-words` to handle quotes and spaces.

### Fixed
- Note ordering for `--last` flag now uses modification time instead of filename.
- Renaming notes now automatically updates the `title` field in frontmatter.
- Indentation for multiline task descriptions in Markdown.

### Changed
- Refactored README.md to be dev-friendly and factual.
- Improved shell command splitting for better accuracy.

## [Unreleased] - 2025-08-15

### Added
- New sunset-themed ASCII art for README and interactive shell.
- Clear exit instructions in the shell startup banner.
- AI contributing guidelines (CLAUDE.md, GEMINI.md).

### Changed
- Upgraded CI workflow to use `dtolnay/rust-toolchain`.
- Updated README with status and distribution badges.

## [Previous] - 2025-07-08

### Added
- Interactive shell (`shell` or `sh`) with history, autocompletion, and stateful notebook management.
- Enhanced template system with built-in variables (`date`, `branch`, `project_dir`, `uuid`) and custom variables (`-v`).
- Import and export commands for `.zip` and `.json` notebook formats.
- Global search across all notebooks via `find --all`.
- Task management with `task` command and `list --tasks` filter.
- Jot pinning via `pin` and `unpin` subcommands.
- Multi-notebook support with scoped commands and automated migration for legacy notes.
- Global `--notebook` flag for single-command notebook targeting.

### Changed
- Refactored codebase into modular source files.
- Improved editor detection with better fallbacks.
- Updated `info --paths` to reflect notebook structure.

### Fixed
- Race condition in test suite filename generation.
- Corrected `.gitignore` logic to track notebook files by default.
- Fixed out-of-bounds error handling for the `--last` flag.

## [0.1.0] - 2025-06-08

### Added
- Core note creation from string arguments or `$EDITOR`.
- Metadata support via tags and YAML frontmatter.
- Full-text search and tag-based filtering.
- Time-based views (`today`, `yesterday`, `week`, `on <date>`).
- Markdown compilation flag (`--compile`) for time-based views.
- Note management commands (`show`, `edit`, `delete`) with recency targeting (`--last`).
- Platform-specific storage conventions and `$JD_DIR` override support.
- Initial integration test suite and CI workflow.
# Changelog

All notable changes to this project are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- `jd new` and `jd edit` now decrypt notes to a private temp file for editing and re-encrypt afterward; previously the editor was shown raw ciphertext and saved notes were silently left unencrypted.
- The encryption identity file is now created readable only by the owner (0600) instead of world-readable.
- `jd sync` never stages `identity.txt` or `config.toml`, even when the git repository was created by hand without a `.gitignore`.
- `jd init --git` on a pre-existing repository now adds the missing `.gitignore` entries for sensitive files.
- Importing a ZIP archive now encrypts notes when encryption is enabled, and no longer creates empty files from directory entries.
- `jd export` warns that the output is plaintext when the journal is encrypted.
- `jd rename`, `jd move`, JSON imports, the `--notebook` flag, and the `JD_ACTIVE_NOTEBOOK` environment variable now reject names with path separators or traversal components, closing several ways to write files outside the notebooks directory.

## [0.2.1] - 2026-06-02

### Added
- `jd clean` command to delete all notes in the active notebook (or all notebooks with `--all`), with a double confirmation prompt.

## [0.2.0] - 2026-06-01

### Added
- Stdin support for multiline note creation via piping.
- Clear exit instructions in the interactive shell startup banner.

### Changed
- Note frontmatter now uses TOML syntax (`tags = ["rust"]`, `pinned = true`) instead of YAML. Existing YAML-format notes must be re-created or manually converted.
- `jd sync` now delegates to the system `git` binary, inheriting the user's global git config, signing keys, and hooks.

### Fixed
- Notes created within the same second no longer overwrite each other; a `-1`, `-2`, etc. suffix is appended automatically.
- `jd tag add/rm/set --last` now works without an explicit number, consistent with all other `--last` flags.
- `-t`/`--tags` flag no longer consumes the note message as a tag value; `-t rust,notes "my note"` now works as expected.
- `jd sync` no longer creates an empty commit when there are no staged changes.
- `--last` flag now selects notes by filename (creation time) rather than modification time.
- `jd prepend` no longer wraps plain-text notes in a spurious frontmatter block.
- `jd import` zip archives can no longer write files outside the target notebook directory.
- `jd notebook use` output is now single-quoted, closing a shell-injection vector for maliciously named notebooks.
- Ten subcommands (`append`, `prepend`, `move`, `rename`, `tag`, `property`) no longer panic when called without a note target.
- Renaming notes now automatically updates the `title` field in frontmatter.
- Indentation for multiline task descriptions in Markdown.

## [0.1.0] - 2025-07-08

### Added
- Interactive shell (`shell` or `sh`) with history, autocompletion, and stateful notebook management.
- Enhanced template system with built-in variables (`date`, `branch`, `project_dir`, `uuid`) and custom variables (`-v`).
- Import and export commands for `.zip` and `.json` notebook formats.
- Global search across all notebooks via `find --all`.
- Task management with `task` command and `list --tasks` filter.
- Jot pinning via `pin` and `unpin` subcommands.
- Multi-notebook support with scoped commands and automated migration for legacy notes.
- Global `--notebook` flag for single-command notebook targeting.
- Core note creation from string arguments or `$EDITOR`.
- Metadata support via tags and TOML frontmatter.
- Full-text search and tag-based filtering.
- Time-based views (`today`, `yesterday`, `week`, `on <date>`).
- Markdown compilation flag (`--compile`) for time-based views.
- Note management commands (`show`, `edit`, `delete`) with recency targeting (`--last`).
- Platform-specific storage conventions and `$JD_DIR` override support.
- Initial integration test suite and CI workflow.

### Changed
- Refactored codebase into modular source files.
- Improved editor detection with better fallbacks.

### Fixed
- Race condition in test suite filename generation.
- Corrected `.gitignore` logic to track notebook files by default.
- Fixed out-of-bounds error handling for the `--last` flag.

# jd

A minimalist command-line jotting utility written in Rust, designed for fast capture, privacy through optional encryption, and git-based versioning.

## Project overview

`jd` is a CLI-first tool that stores notes as plain Markdown files organized into notebooks. It prioritizes data ownership and portability.

### Core technologies
- **Rust**: The implementation language.
- **Clap**: CLI argument parsing (using the derive API).
- **Age**: Transparent on-disk encryption.
- **Git**: Sync integration via `std::process::Command` shell calls to the system `git` binary.
- **Rustyline**: Interactive shell with autocompletion and history.
- **Serde**: Serialization/deserialization of frontmatter (TOML) and exports (JSON/CSV).

### Architecture
- `src/main.rs`: Entry point and top-level command orchestration.
- `src/cli.rs`: CLI structure definition and subcommand parsing.
- `src/commands/`: Implementation logic for all subcommands (the "business logic"), split into one module per feature area (`capture`, `query`, `notebook`, `git`, `export_import`, `shell`, etc.) and re-exported flat from `commands/mod.rs`.
- `src/helpers.rs`: Shared utilities for path management, file I/O, encryption, and note parsing.
- `tests/cli.rs`: Comprehensive integration tests covering the entire CLI surface.

## Building and running

### Prerequisites
- Rust 1.70+
- Cargo

### Commands
- **Build**: `cargo build`
- **Run**: `cargo run -- [COMMAND] [ARGS]` (or `jd` once installed)
- **Test**: `cargo test`
- **Lint**: `cargo clippy -- -D warnings`
- **Format**: `cargo fmt`

### Branch model
`devel` is the default and integration branch; `main` tracks releases.
Never commit directly to either — branch off `devel` (`feature/...`,
`fix/...`, `chore/...`), open a PR to `devel`, and merge after CI is green.
Releases merge `devel` into `main` and tag `vX.Y.Z` there (see
`RELEASE_CHECKLIST.md`).

## Development conventions

### Coding style
- **Error handling**: Use `anyhow` for application-level errors. Provide context using `.with_context()` for file and system operations.
- **Naming**: Follow standard Rust conventions (`snake_case` for functions/variables, `PascalCase` for types).
- **CLI design**: Follow the existing `clap` patterns in `src/cli.rs`. Use descriptive docstrings for subcommands and arguments as they are used for `--help` output.

### Testing practices
- **Integration tests**: New features or bug fixes **must** include integration tests in `tests/cli.rs`.
- **Environment**: Tests should use the `setup()` helper to create isolated temporary environments and set `JD_DIR`.
- **Validation**: Use `assert_cmd` and `predicates` for verifying CLI output and side effects.

### Note format
Notes consist of an optional TOML frontmatter block (between `+++` delimiters — the Hugo/TOML-frontmatter convention, unlike `---`, which every other tool reads as a YAML signal) and a Markdown body.
```markdown
+++
tags = ["rust", "project"]
pinned = true
title = "My Note Title"
+++
Body content here.
```
Notes written by jd versions before this change use `---` instead of `+++`; jd still reads them, and transparently migrates them to `+++` the next time it rewrites the note (tag, pin, property, rename, prepend, etc.).
- Standard timed jots: `YYYY-MM-DD-HHMMSS.md` (or `YYYY-MM-DD-HHMMSS-N.md` when the same second is used more than once)
- Daily notes: `YYYY-MM-DD.md`

### Git workflow
- Use atomic commits with descriptive messages.
- Format: `<type>: <description>` (e.g., `feat: add rename command`).

### CHANGELOG
- Keep entries concise, trim, and to the point — one line per entry, no sub-bullets.
- User-facing changes only. Internal refactors, test changes, and doc fixes do not belong in the CHANGELOG.
- Add new entries under `## [Unreleased]` at the top of the file.

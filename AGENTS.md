# jd

A minimalist command-line jotting utility written in Rust, designed for fast capture, privacy through optional encryption, and git-based versioning.

## Project overview

`jd` is a CLI-first tool that stores notes as plain Markdown files organized into notebooks. It prioritizes data ownership and portability.

### Core technologies
- **Rust**: The implementation language.
- **Clap**: CLI argument parsing (using the derive API).
- **Age**: Transparent on-disk encryption.
- **Git2**: Built-in Git integration for syncing.
- **Rustyline**: Interactive shell with autocompletion and history.
- **Serde**: Serialization/deserialization of frontmatter (YAML) and exports (JSON/CSV).

### Architecture
- `src/main.rs`: Entry point and top-level command orchestration.
- `src/cli.rs`: CLI structure definition and subcommand parsing.
- `src/commands.rs`: Implementation logic for all subcommands (the "business logic").
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
- **Lint**: `cargo clippy`
- **Format**: `cargo fmt`

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
Notes consist of an optional YAML frontmatter block and a Markdown body.
```markdown
---
tags: [rust, project]
pinned: true
title: My Note Title
---
Body content here.
```
- Standard timed jots: `YYYY-MM-DD-HHMMSS.md`
- Daily notes: `YYYY-MM-DD.md`

### Git workflow
- Use atomic commits with descriptive messages.
- Format: `<type>: <description>` (e.g., `feat: add rename command`).

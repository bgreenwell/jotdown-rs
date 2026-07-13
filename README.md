# jotdown-rs <img src="assets/logo.png" align="right" width="120" />

`jotdown-rs` is a fast, minimalist command-line tool for capturing thoughts without friction. The core workflow is a single command: type `jd 'your thought'` and move on. Everything else — notebooks, encryption, git sync, templates — is available when you need it and invisible when you don't.

[![CI](https://img.shields.io/github/actions/workflow/status/bgreenwell/jotdown-rs/ci.yml?style=for-the-badge)](https://github.com/bgreenwell/jotdown-rs/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/jotdown-rs.svg?style=for-the-badge)](https://crates.io/crates/jotdown-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org)
[![Easy Install](https://img.shields.io/badge/Easy%20Install-Homebrew%20%7C%20Scoop-FBB040?style=for-the-badge)](#installation)
[![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-blue?style=for-the-badge)](https://github.com/bgreenwell/jotdown-rs/releases/latest)

## Design principles

`jd` is built for capturing thoughts at the speed of typing.

* **CLI-first**: The terminal is the primary interface for capturing and retrieving text.
* **Plain text storage**: Data is stored as standard Markdown files. Your notes remain portable and readable without `jd`.
* **Data ownership**: All notes stay local. There are no proprietary databases or mandatory sync services.
* **Progressive complexity**: The default experience is intentionally simple. Power-user features (notebooks, encryption, git sync, templates) layer on top without affecting the core workflow.

## Features

### Core — always available, no setup required

* **Instant capture**: `jd 'your thought'` creates a timestamped note and returns immediately.
* **Stdin support**: Pipe content directly into `jd` from other commands or scripts.
* **Full-text search**: Find notes by keyword across the active notebook.
* **Time-based views**: Filter notes by today, yesterday, this week, or a specific date range.
* **Note management**: Show, edit, rename, tag, pin, or delete notes by ID prefix or recency (`--last`).
* **Task tracking**: Create Markdown-formatted tasks and list all pending items at a glance.

### Power user — opt-in, zero overhead when not used

* **Multiple notebooks**: Organize notes into named collections (e.g., `work`, `personal`). The default notebook requires no configuration.
* **Templates**: Open your `$EDITOR` with pre-filled, context-aware content using built-in or custom variables.
* **Encryption**: Optional on-disk encryption via the `age` format. Notes are stored as plain text until you opt in.
* **Git integration**: Stage, commit, and push your notes to a remote with a single `jd sync` command.
* **Import / export**: Back up or transfer notebooks as ZIP archives or JSON files.
* **Interactive shell**: A persistent `jd shell` session with tab-completion and command history.

## Installation

### Homebrew (macOS / Linux)

```sh
brew install bgreenwell/tap/jotdown-rs
```

### From crates.io

Requires the [Rust toolchain](https://rustup.rs/).

```sh
cargo install jotdown-rs
```

### From source

```sh
git clone https://github.com/bgreenwell/jotdown-rs.git
cd jotdown-rs
cargo install --path .
```

-----

## Usage guide

### Shells and quotes

Your command-line shell (like Bash or Zsh) can interpret special characters like `!` or expand variables like `$USER` inside double quotes (`"`).

**The best practice is to use single quotes (`'`) for your messages to ensure the shell treats every character literally.**

```sh
# This works as expected
❯ jd 'This is a great idea!'
```

### Creating notes

Notes are created in the active notebook (default is `default`).

**1. Create a quick, timestamped note:**

```sh
❯ jd 'This is a quick thought I want to save.'
```
This creates a unique file with a full timestamp (e.g., `2026-05-23-231500.md`).

**2. Append to a daily note:**

```sh
❯ jd daily 'Worked on the readme'
```
This appends the message to a single file for the entire day (e.g., `2026-05-23.md`). Use this for running logs.

**3. Pipe multiline content from stdin:**

```sh
❯ echo "Line 1
Line 2" | jd
```
You can also pipe the output of other commands directly into `jd` to save them as notes.

**4. Create a tagged note:**

```sh
❯ jd 'A great idea for the project' -t project,rust
```

**5. Create a longer note in your editor:**

```sh
# Opens $EDITOR
❯ jd new

# Use a custom template
❯ jd new --template meeting.md
```

### Advanced templating

`jd`'s templating system can be used to create structured notes with pre-filled, context-aware information.

#### Built-in variables

You can use the following variables in any template file:

  * `{{date}}`: The current date and time in RFC 3339 format.
  * `{{uuid}}`: A unique identifier (v4 UUID) for the note.
  * `{{project_dir}}`: The name of the current directory.
  * `{{branch}}`: The current git branch name.

**Example `bug.md` template:**

```markdown
+++
tags = ["bug", "{{project_dir}}"]
+++

# Bug Report: {{uuid}}

- **Branch**: {{branch}}
- **Date**: {{date}}

## Description

(describe the bug)
```

#### Creating a new template

Creating your own template is how you can customize `jd` for your specific workflow. Here’s how:

1.  **Find your templates directory.** Run `jd info --paths` to find the location of your `jd` root directory. Your templates are stored in the `templates/` subdirectory.

2.  **Create a new file.** Create a new Markdown file in the `templates` directory. The name of the file (without the `.md` extension) is the name of your template. For example, `daily-journal.md` becomes the `daily-journal` template.

3.  **Add your content.** Open the file and add your desired content, using any of the built-in or custom variables.

Once the file is saved, you can use it immediately with the `jd new --template <template-name>` command.

#### Custom variables

You can also define your own variables from the command line using the `-v` or `--variable` flag.

**Example `dev-journal.md` template:**

```markdown
+++
tags = ["journal", "{{project_dir}}", "{{feature_name}}"]
+++
# Dev Journal: {{uuid}}

- **Ticket**: [{{ticket_id}}](https://jira.example.com/browse/{{ticket_id}})
- **Branch**: {{branch}}
- **Date**: {{date}}

## Progress

(What did I accomplish today?)
```

**Command:**

```sh
jd new \
  --template dev-journal \
  -v feature_name=user-profile \
  -v ticket_id=PROJ-123
```

### Interactive shell

`jd` provides an interactive shell for performing multiple actions without prefixing every command with `jd`.

**1. Launch the shell:**

```sh
❯ jd shell
```

**2. Manage notebooks in the shell:**
The shell maintains its own active notebook state.

```sh
jd(default)> use project-icarus
Active notebook switched to 'project-icarus'.
jd(project-icarus)>
```

**3. Autocompletion and history:**
Press `Tab` to autocomplete commands or notebook names. Use the up and down arrow keys to navigate command history.

### Working with notebooks

**1. Create a new notebook:**

```sh
❯ jd notebook new project-icarus
```

**2. List available notebooks:**
An asterisk (`*`) indicates the active notebook.

```sh
❯ jd notebook list
```

**3. Show the currently active notebook:**

```sh
❯ jd notebook status
```

**4. Switch the active notebook:**
Since a child process cannot modify the parent shell's environment, use `eval` to update the `JD_ACTIVE_NOTEBOOK` variable.

```sh
❯ eval $(jd notebook use project-icarus)
```

**5. Perform a single action in another notebook:**

```sh
❯ jd 'Remember to buy milk' --notebook personal
```

### Viewing and filtering notes

**1. List recent notes:**

```sh
❯ jd list
❯ jd list 5

# Output as JSON or CSV
❯ jd list --format json
❯ jd list --format csv
```

**2. Full-text search:**

```sh
# Search active notebook
❯ jd find 'productivity'

# Search all notebooks
❯ jd find 'database' --all

# Show surrounding context for each match
❯ jd find 'database' --context
```

**3. Filter by tags:**

```sh
❯ jd tags rust,cli
```

**4. View notes by time:**

```sh
❯ jd today
❯ jd yesterday
❯ jd week
❯ jd on 2025-05-01..2025-05-31
```

**5. Compile notes into a summary:**

```sh
❯ jd week --compile > weekly-summary.md
```

**6. Fuzzy-find and open a note interactively (macOS / Linux):**

```sh
❯ jd select
```

### Managing specific notes

**1. Show the content of a note:**

```sh
# By ID prefix
❯ jd show 2025-06-08

# By recency
❯ jd show --last
```

**2. Edit a note:**

```sh
❯ jd edit --last=3
```

**3. Rename a note:**
Renaming updates the filename and the `title` field in the note's frontmatter if it exists.

```sh
❯ jd rename 'new-title' --last
```

**4. Delete a note:**

```sh
❯ jd delete 2025-06-08
```

### Modifying note content

**1. Append or prepend text:**
Add text to the beginning or end of a note without opening an editor.

```sh
# Append to the most recent note
❯ jd append --last 'Added this to the end'

# Prepend to a specific note (below the frontmatter)
❯ jd prepend -i 2026-05-23 'Important update at the top'
```

**2. Move a note between notebooks:**

```sh
❯ jd move --last 'personal'
```

### Managing properties

Notes support arbitrary key-value pairs in their frontmatter. These can be managed using the `property` command.

**1. Set a property:**

```sh
❯ jd property set --last project "Alpha"
```

**2. Get a property value:**

```sh
❯ jd property get --last project
```

**3. Delete a property:**

```sh
❯ jd property delete --last project
```

### Managing tasks

**1. Create a task:**

```sh
❯ jd task 'Set up the new database schema'
```
This creates a note formatted as a Markdown task: `- [ ] Set up the new database schema`.

**2. View incomplete tasks:**

```sh
❯ jd list --tasks
```

### Pinning and unpinning notes

**1. Pin a note:**

```sh
❯ jd pin --last
```

**2. View pinned notes:**

```sh
❯ jd list --pinned
```

**3. Unpin a note:**

```sh
❯ jd unpin 2025-07-09-105000
```

### Managing tags

**1. Add tags:**

```sh
❯ jd tag add --last rust,idea
```

**2. Remove tags:**

```sh
❯ jd tag rm --last idea
```

**3. Overwrite tags:**

```sh
❯ jd tag set --last archived
```

### Importing and exporting notebooks

**1. Export a notebook:**

```sh
# Export to zip
jd export work --output ./work_backup.zip

# Export to JSON
jd export personal --format json --output ./personal_backup.json
```

**2. Import a notebook:**

```bash
jd import ./work_backup.zip
```

### Utility commands

```sh
# Show storage paths
❯ jd info --paths

# Show statistics for the active notebook
❯ jd info --stats

# Show statistics across all notebooks
❯ jd info --stats --all
```

### Git integration (optional)

Requires `git` to be installed and available on your `$PATH`.

**1. Initialize with Git:**

```sh
❯ jd init --git
```

**2. Link a remote:**
Navigate to the `jd` root directory (see `jd info --paths`) and add a remote.

```sh
❯ git remote add origin git@github.com:USER/repo.git
```

**3. Synchronize changes:**
`jd sync` stages, commits, and pushes changes from all notebooks.

```sh
❯ jd sync
```

### Encryption (optional)

**1. Enable encryption:**
Notes are encrypted on-disk using the `age` format.

```sh
❯ jd init --encrypt
```

**2. Decrypt all notes:**

```sh
❯ jd decrypt
```

## Configuration

### File storage location

`jd` respects the `$JD_DIR` environment variable. Default locations are:

* **macOS:** `~/Library/Application Support/jd/`
* **Linux:** `~/.config/jd/`
* **Windows:** `%AppData%\jd\`

Notes are stored in the `notebooks/` subdirectory.

### Templates

Place Markdown files in the `templates/` subdirectory of your `jd` root. Use variables like `{{date}}`, `{{branch}}`, and `{{uuid}}` for dynamic content. Custom variables can be passed via the `-v` flag.

## Contributing

Open an issue or submit a pull request on GitHub.

## License

MIT License.
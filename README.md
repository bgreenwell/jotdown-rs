# jotdown-rs

`jotdown-rs` is a minimalist command-line jotting utility written in Rust. All operations are performed using the `jd` command.

[![CI Status](https://github.com/bgreenwell/rjot/actions/workflows/rust.yml/badge.svg)](https://github.com/bgreenwell/rjot/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Platform Support](https://img.shields.io/badge/platform-linux%20%7C%20macOS%20%7C%20windows-lightgrey.svg)](https://github.com/bgreenwell/rjot)

## Design principles

`jd` is built for capturing thoughts at the speed of typing.

* **CLI-first**: The terminal is the primary interface for capturing and retrieving text.
* **Plain text storage**: Data is stored as standard Markdown files. Your notes remain portable and readable without `jd`.
* **Data ownership**: All notes stay local. There are no proprietary databases or mandatory sync services.

## Features

* **Instant capture**: Create a new jot directly from command-line arguments.
* **Stdin support**: Pipe multiline content directly into `jd` from other commands.
* **Multiple notebooks**: Organize jots into separate collections (e.g., `work`, `personal`).
* **Task management**: Create markdown-formatted tasks and view pending items across notebooks.
* **Editor integration**: Open your `$EDITOR` for longer entries with template support.
* **Full-text search**: Search across the active notebook or globally across all notebooks.
* **Time-based views**: Filter notes by date, week, or specific date ranges.
* **Note management**: Show, edit, rename, tag, or delete notes using unique ID prefixes or recency flags (`--last`).
* **Encryption**: Optional on-disk encryption using the `age` format.
* **Git integration**: Built-in support for versioning your notes.

## Installation

**Note:** Once this project gains stable releases, you will be able to install it via your system's package manager (e.g., `apt`, `brew`, etc.). Until then, you can use the methods below.

### From crates.io (recommended)

This method automatically downloads, compiles, and installs `jd` on your system.

1.  **Install the Rust toolchain**

    If you don't already have it, install Rust from the official site: [rustup.rs](https://rustup.rs/).

2.  **Install `jd`**

    ```sh
    cargo install jd
    ```

    This will place the `jd` executable in your cargo binary path (usually `~/.cargo/bin/`), making it available from anywhere in your terminal.

### From source

To build the very latest version directly from the source code:

```sh
git clone https://github.com/bgreenwell/rjot.git
cd rjot
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
❯ jd 'A great idea for the project' --tags project rust
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
---
tags:
  - bug
  - {{project_dir}}
---

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
---
tags:
  - journal
  - {{project_dir}}
  - {{feature_name}}
---
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

**3. Switch the active notebook:**
Since a child process cannot modify the parent shell's environment, use `eval` to update the `JD_ACTIVE_NOTEBOOK` variable.

```sh
❯ eval $(jd notebook use project-icarus)
```

**4. Perform a single action in another notebook:**

```sh
❯ jd 'Remember to buy milk' --notebook personal
```

### Viewing and filtering notes

**1. List recent notes:**

```sh
❯ jd list
❯ jd list 5
```

**2. Full-text search:**

```sh
# Search active notebook
❯ jd find 'productivity'

# Search all notebooks
❯ jd find 'database' --all
```

**3. Filter by tags:**

```sh
❯ jd tags rust,cli
```

**4. View notes by time:**

```sh
❯ jd today
❯ jd week
❯ jd on 2025-05-01..2025-05-31
```

**5. Compile notes into a summary:**

```sh
❯ jd week --compile > weekly-summary.md
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
❯ jd tag add --last=1 rust,idea
```

**2. Remove tags:**

```sh
❯ jd tag rm -p 2025-06-09 idea
```

**3. Overwrite tags:**

```sh
❯ jd tag set --last=2 archived
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

# Show statistics
❯ jd info --stats
```

### Git integration (optional)

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
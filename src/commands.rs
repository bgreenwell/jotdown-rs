use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use age::{secrecy::ExposeSecret, x25519, Decryptor, Identity};
use anyhow::{anyhow, bail, Result};
use chrono::{Datelike, Local, NaiveDate};
use clap::Parser;
use rand::Rng;
use rustyline::completion::Completer;
use rustyline::config::Configurer;
use rustyline::CompletionType;
use rustyline::Editor;
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};
use serde::{Deserialize, Serialize};
use std::fs::File;
use uuid::Uuid;
use zip::write::{FileOptions, ZipWriter};
use zip::ZipArchive;

// Conditionally compile everything related to skim
#[cfg(not(windows))]
use {
    crossbeam_channel::unbounded,
    skim::prelude::*,
    std::{borrow::Cow, sync::Arc},
};

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

use crate::cli::{
    ExportArgs, ImportArgs, InfoArgs, NotebookAction, NotebookArgs, PropertyAction, TagAction,
    TagArgs,
};
use crate::helpers::{
    self, get_jd_dir_root, get_note_path_for_action, get_notebooks_dir, get_templates_dir,
    parse_note_from_file, Frontmatter, TaskStats,
};

#[derive(Serialize, Deserialize, Debug)]
struct JsonExport {
    notebook_name: String,
    jots: Vec<JsonJot>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonJot {
    filename: String,
    content: String,
}

#[derive(Helper, Hinter, Highlighter, Validator)]
struct JdHelper {}

impl Completer for JdHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let mut candidates = Vec::new();
        let mut start_pos = 0;

        let parts: Vec<&str> = line[..pos].split_whitespace().collect();
        let parts_count = parts.len();

        // If the line is empty or we are still typing the first word.
        if parts.is_empty() || (parts_count == 1 && !line.ends_with(' ')) {
            let first_word = parts.first().unwrap_or(&"");
            start_pos = pos - first_word.len(); // Start replacement at the beginning of the current word.

            let all_commands = vec![
                "list", "find", "new", "task", "todo", "t", "today", "week", "tags", "notebook",
                "pin", "unpin", "edit", "show", "delete", "info", "use", "exit", "quit", "append",
                "prepend", "move", "rename", "daily",
            ];

            for cmd in all_commands {
                if cmd.starts_with(first_word) {
                    candidates.push(cmd.to_string());
                }
            }
        // If we are completing the argument for `use` or `notebook`.
        } else if parts_count > 0 {
            let command = parts[0];
            if command == "use" || command == "notebook" {
                let current_arg = parts.get(1).unwrap_or(&"");
                // The replacement should start at the beginning of the notebook name argument.
                start_pos = pos - current_arg.len();

                if let Ok(notebooks_dir) = helpers::get_notebooks_dir() {
                    if let Ok(entries) = std::fs::read_dir(notebooks_dir) {
                        for entry in entries.filter_map(Result::ok) {
                            if entry.path().is_dir() {
                                let notebook_name = entry.file_name().to_string_lossy().to_string();
                                if notebook_name.starts_with(current_arg) {
                                    candidates.push(notebook_name);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok((start_pos, candidates))
    }
}

pub fn command_notebook(args: NotebookArgs) -> Result<()> {
    match args.action {
        NotebookAction::New { name } => command_notebook_new(&name)?,
        NotebookAction::List => command_notebook_list()?,
        NotebookAction::Use { name } => command_notebook_use(&name)?,
        NotebookAction::Status => command_notebook_status()?,
    }
    Ok(())
}

fn is_valid_notebook_name(name: &str) -> bool {
    if name.is_empty() || name == "." || name == ".." {
        return false;
    }
    !name.chars().any(|c| {
        matches!(
            c,
            '/' | '\\'
                | '"'
                | '\''
                | '`'
                | '$'
                | '!'
                | '&'
                | '|'
                | ';'
                | '('
                | ')'
                | '<'
                | '>'
                | '\0'
        )
    })
}

fn command_notebook_new(name: &str) -> Result<()> {
    if !is_valid_notebook_name(name) {
        bail!(
            "Invalid notebook name: '{}'. Names cannot contain slashes, shell-special characters, or be dots.",
            name
        );
    }

    let notebooks_dir = get_notebooks_dir()?;
    let new_notebook_path = notebooks_dir.join(name);

    if new_notebook_path.exists() {
        println!("Notebook '{name}' already exists.");
    } else {
        fs::create_dir_all(&new_notebook_path)?;
        println!("Successfully created new notebook: '{name}'.");
    }
    Ok(())
}

fn command_notebook_list() -> Result<()> {
    let notebooks_dir = get_notebooks_dir()?;
    let active_notebook = env::var("JD_ACTIVE_NOTEBOOK").unwrap_or_else(|_| "default".to_string());

    println!("Available notebooks (* indicates active):");

    for entry in fs::read_dir(notebooks_dir)?.filter_map(Result::ok) {
        if entry.path().is_dir() {
            let notebook_name = entry.file_name().to_string_lossy().to_string();
            let prefix = if notebook_name == active_notebook {
                "*"
            } else {
                " "
            };
            println!("  {prefix} {notebook_name}");
        }
    }
    Ok(())
}

fn command_notebook_use(name: &str) -> Result<()> {
    if !is_valid_notebook_name(name) {
        bail!("Invalid notebook name: '{}'.", name);
    }

    let notebooks_dir = get_notebooks_dir()?;
    let target_notebook = notebooks_dir.join(name);

    if !target_notebook.exists() || !target_notebook.is_dir() {
        bail!(
            "Notebook '{}' not found. Create it with `jd notebook new {}`.",
            name,
            name
        );
    }

    // Prints a shell command for the user to evaluate. Single quotes prevent
    // any shell interpretation of the notebook name.
    println!("export JD_ACTIVE_NOTEBOOK='{name}'");
    Ok(())
}

fn command_notebook_status() -> Result<()> {
    let active_notebook = env::var("JD_ACTIVE_NOTEBOOK").unwrap_or_else(|_| "default".to_string());
    println!("Active notebook: {active_notebook}");
    Ok(())
}

pub fn command_property(entries_dir: &Path, action: PropertyAction) -> Result<()> {
    match action {
        PropertyAction::Set {
            id,
            last,
            name,
            value,
        } => {
            let note_path = get_note_path_for_action(entries_dir, id, last)?;
            let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
            let mut note = parse_note_from_file(&note_path, &notebook_name)?;

            note.frontmatter
                .fields
                .insert(name.clone(), toml::Value::String(value.clone()));

            let new_frontmatter_str = toml::to_string(&note.frontmatter)?;
            let new_content = format!("---\n{}---\n\n{}", new_frontmatter_str, note.content);
            helpers::write_note_file(&note_path, &new_content)?;
            println!(
                "Successfully set property '{}' for jot '{}'.",
                name, note.id
            );
        }
        PropertyAction::Get {
            id,
            last,
            name,
            format,
        } => {
            let note_path = get_note_path_for_action(entries_dir, id, last)?;
            let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
            let note = parse_note_from_file(&note_path, &notebook_name)?;

            if let Some(value) = note.frontmatter.fields.get(name.as_str()) {
                if format == "json" {
                    println!("{}", serde_json::to_string(value)?);
                } else {
                    match value {
                        toml::Value::String(s) => println!("{s}"),
                        toml::Value::Integer(i) => println!("{i}"),
                        toml::Value::Float(f) => println!("{f}"),
                        toml::Value::Boolean(b) => println!("{b}"),
                        toml::Value::Datetime(d) => println!("{d}"),
                        v => println!("{}", serde_json::to_string(v)?),
                    }
                }
            } else {
                bail!("Property '{}' not found for jot '{}'.", name, note.id);
            }
        }
        PropertyAction::Delete { id, last, name } => {
            let note_path = get_note_path_for_action(entries_dir, id, last)?;
            let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
            let mut note = parse_note_from_file(&note_path, &notebook_name)?;

            if note.frontmatter.fields.remove(name.as_str()).is_some() {
                let new_frontmatter_str = toml::to_string(&note.frontmatter)?;
                let new_content = format!("---\n{}---\n\n{}", new_frontmatter_str, note.content);
                helpers::write_note_file(&note_path, &new_content)?;
                println!(
                    "Successfully deleted property '{}' from jot '{}'.",
                    name, note.id
                );
            } else {
                println!("Property '{}' not found on jot '{}'.", name, note.id);
            }
        }
    }
    Ok(())
}

pub fn command_append(
    entries_dir: &Path,
    id: Option<String>,
    last: Option<usize>,
    content: &str,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id, last)?;
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
    let note = parse_note_from_file(&note_path, &notebook_name)?;

    let mut current_content = helpers::read_note_file(&note_path)?;
    if !current_content.ends_with('\n') {
        current_content.push('\n');
    }
    current_content.push_str(content);
    if !content.ends_with('\n') {
        current_content.push('\n');
    }

    helpers::write_note_file(&note_path, &current_content)?;
    println!("Successfully appended to jot '{}'.", note.id);
    Ok(())
}

pub fn command_prepend(
    entries_dir: &Path,
    id: Option<String>,
    last: Option<usize>,
    content: &str,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id, last)?;
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
    let note = parse_note_from_file(&note_path, &notebook_name)?;

    let prefix = if !content.ends_with('\n') {
        format!("{}\n", content)
    } else {
        content.to_string()
    };

    // Read the raw file and insert the new content after the frontmatter block
    // (or at the very top if there is no frontmatter), so plain-text notes are
    // not silently wrapped in a YAML header.
    let mut raw = helpers::read_note_file(&note_path)?;
    let insert_at = if raw.starts_with("---") {
        raw.get(3..)
            .and_then(|s| s.find("\n---"))
            .map(|i| 3 + i + 4) // skip past the closing ---
            .unwrap_or(0)
    } else {
        0
    };

    // Ensure there's a blank line between the frontmatter and the new content.
    let separator = if insert_at > 0 && !raw[insert_at..].starts_with("\n\n") {
        "\n\n"
    } else {
        ""
    };
    raw.insert_str(insert_at, &format!("{}{}", separator, prefix));

    helpers::write_note_file(&note_path, &raw)?;
    println!("Successfully prepended to jot '{}'.", note.id);
    Ok(())
}

pub fn command_move(
    entries_dir: &Path,
    id: Option<String>,
    last: Option<usize>,
    destination: &str,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id, last)?;
    let filename = note_path
        .file_name()
        .ok_or_else(|| anyhow!("Invalid filename"))?;

    let dest_dir = get_notebooks_dir()?.join(destination);
    if !dest_dir.exists() {
        bail!("Destination notebook '{}' does not exist.", destination);
    }

    let dest_path = dest_dir.join(filename);
    if dest_path.exists() {
        bail!(
            "A note with the same name already exists in '{}'.",
            destination
        );
    }

    fs::rename(&note_path, &dest_path)?;
    println!("Moved jot to notebook '{}'.", destination);
    Ok(())
}

pub fn command_rename(
    entries_dir: &Path,
    id: Option<String>,
    last: Option<usize>,
    new_name: &str,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id, last)?;
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
    let mut note = parse_note_from_file(&note_path, &notebook_name)?;

    let mut dest_filename = new_name.to_string();
    if !dest_filename.ends_with(".md") {
        dest_filename.push_str(".md");
    }

    let dest_path = entries_dir.join(&dest_filename);
    if dest_path.exists() {
        bail!("A file with name '{}' already exists.", dest_filename);
    }

    // If there's a title in the frontmatter, update it too.
    if note.frontmatter.fields.contains_key("title") {
        note.frontmatter.fields.insert(
            "title".to_string(),
            toml::Value::String(new_name.to_string()),
        );
        let new_frontmatter_str = toml::to_string(&note.frontmatter)?;
        let new_content = format!("---\n{}---\n\n{}", new_frontmatter_str, note.content);
        helpers::write_note_file(&note_path, &new_content)?;
    }

    fs::rename(&note_path, &dest_path)?;
    println!("Renamed jot to '{}'.", dest_filename);
    Ok(())
}

pub fn command_daily(entries_dir: &Path, message: &str) -> Result<()> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let filename = format!("{}.md", today);
    let note_path = entries_dir.join(&filename);

    if note_path.exists() {
        // Append to existing
        let mut content = helpers::read_note_file(&note_path)?;
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(message);
        content.push('\n');
        helpers::write_note_file(&note_path, &content)?;
        println!("Appended to daily note: {}", filename);
    } else {
        // Create new
        let content = format!(
            "---\ntitle = \"Daily Note - {}\"\n---\n\n{}\n",
            today, message
        );
        helpers::write_note_file(&note_path, &content)?;
        println!("Created daily note: {}", filename);
    }
    Ok(())
}

pub fn command_shell() -> Result<()> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let mut active_notebook =
        env::var("JD_ACTIVE_NOTEBOOK").unwrap_or_else(|_| "default".to_string());

    let entries_dir = helpers::get_active_entries_dir(Some(active_notebook.clone()))?;
    let (note_count, _, _) = calculate_stats_for_dir(&entries_dir).unwrap_or((
        0,
        Default::default(),
        Default::default(),
    ));

    let tips = [
        // Shell Tips
        "In the shell, type `use <name>` and press Tab to autocomplete notebook names.",
        "Use the Up/Down arrow keys in the shell to navigate your command history.",
        "You can exit the shell at any time with `exit`, `quit`, or by pressing Ctrl-D.",
        // Basic Usage Tips
        "The `t` command is a fast alias for `task`. Try `t 'My new task'`.",
        "You can use `rm` as a shorter alias for the `delete` command.",
        "Tags can be comma-separated (`-t a,b`) or space-separated (`-t a b`).",
        // Advanced Viewing & Filtering
        "Filter for a date range like this: `on 2025-01-01..2025-01-31`.",
        "Compile a full week's notes into a single file with `week --compile > summary.md`.",
        "Pin important notes with `pin <ID>` and view them with `list --pinned`.",
        "Find notes with multiple tags, like `tags rust,project`.",
        // Note Management
        "You can edit the last jot you created instantly with `edit --last`.",
        "The `--force` flag on `delete` and `decrypt` will skip confirmation prompts.",
        "Use a unique prefix of a jot's ID for any command, like `show 2025-07-21`.",
        // Configuration & Templates
        "Create custom note structures for `new` by adding files to your templates directory.",
        "Find your templates folder and other important paths with `info --paths`.",
        "Pass custom variables to your templates with the `-v` flag, like `new -t bug -v id=123`.",
        // Notebooks & Syncing
        "Run a single command in another notebook with the global `--notebook <name>` flag.",
        "Use `notebook status` to quickly check which notebook is active.",
        "After setting up a git remote, use `sync` to commit and push all changes.",
    ];
    let mut rng = rand::thread_rng();
    let tip = tips[rng.gen_range(0..tips.len())];

    // Use the new oh-my-logo generated ASCII art
    let startup_message = format!(
        "jd v{} | {} | {} jots in '{}'\nTip: {}",
        VERSION,
        chrono::Local::now().format("%Y-%m-%d"),
        note_count,
        active_notebook,
        tip
    );

    let helper = JdHelper {};
    let mut rl = Editor::new()?;
    rl.set_helper(Some(helper));
    rl.set_completion_type(CompletionType::List);

    let history_path = dirs::data_local_dir().map(|p| p.join("jd").join("history.txt"));
    if let Some(ref path) = history_path {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = rl.load_history(path);
    }

    println!("{startup_message}");

    loop {
        let prompt = format!("\x1b[1m\x1b[35mjd\x1b[0m(\x1b[33m{active_notebook}\x1b[0m)> ");
        let readline = rl.readline(&prompt);

        match readline {
            Ok(line) => {
                let _ = rl.add_history_entry(line.as_str());
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let words = match shell_words::split(line) {
                    Ok(w) => w,
                    Err(e) => {
                        eprintln!("Error parsing command: {e}");
                        continue;
                    }
                };

                let command_name = words.first().map(|s| s.as_str()).unwrap_or("");
                match command_name {
                    "exit" | "quit" => break,
                    "use" => {
                        if let Some(name) = words.get(1) {
                            let notebooks_dir = helpers::get_notebooks_dir()?;
                            if notebooks_dir.join(name).is_dir() {
                                active_notebook = name.to_string();
                                println!("Active notebook switched to '{active_notebook}'.");
                            } else {
                                eprintln!("Error: Notebook '{name}' not found.");
                            }
                        } else {
                            eprintln!("Usage: use <NOTEBOOK_NAME>");
                        }
                        continue;
                    }
                    _ => {}
                }

                let mut args = vec!["jd"];
                args.extend(words.iter().map(|s| s.as_str()));

                match crate::cli::Cli::try_parse_from(args) {
                    Ok(cli) => {
                        let notebook_override = cli
                            .notebook
                            .clone()
                            .unwrap_or_else(|| active_notebook.clone());
                        let entries_dir =
                            match crate::helpers::get_active_entries_dir(Some(notebook_override)) {
                                Ok(d) => d,
                                Err(e) => {
                                    eprintln!("Error: {e}");
                                    continue;
                                }
                            };

                        if let Some(command) = cli.command {
                            if let Err(e) = crate::run_command(command, entries_dir) {
                                eprintln!("Error: {e}");
                            }
                        } else if !cli.message.is_empty() {
                            let message = cli.message.join(" ");
                            if let Err(e) = command_down(&entries_dir, &message, cli.tags) {
                                eprintln!("Error: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        e.print().unwrap_or_default();
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("\nInterrupted (Ctrl-C). Type 'exit' or press Ctrl-D to leave.");
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                println!("Shell Error: {err:?}");
                break;
            }
        }
    }

    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }
    println!("Exiting jd shell.");
    Ok(())
}

fn git_not_found(e: std::io::Error) -> anyhow::Error {
    if e.kind() == std::io::ErrorKind::NotFound {
        anyhow!("git is not installed or not on your PATH. Install git to use this feature.")
    } else {
        e.into()
    }
}

fn run_git(dir: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(git_not_found)?;
    if !output.status.success() {
        bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

pub fn command_init(git: bool, encrypt: bool) -> Result<()> {
    let root_dir = get_jd_dir_root()?;
    println!("jd directory is at: {root_dir:?}");

    if git {
        let git_dir = root_dir.join(".git");
        if git_dir.exists() {
            println!("Git repository already exists in {root_dir:?}");
        } else {
            let output = Command::new("git")
                .current_dir(&root_dir)
                .arg("init")
                .output()
                .map_err(git_not_found)?;
            if !output.status.success() {
                bail!(
                    "Failed to initialize Git repository: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            println!("Initialized a new Git repository in {root_dir:?}");

            let gitignore_path = root_dir.join(".gitignore");
            let has_commits = Command::new("git")
                .current_dir(&root_dir)
                .args(["rev-parse", "HEAD"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if has_commits {
                println!("Git repository is not empty. Assuming it is already set up.");
            } else if !gitignore_path.exists() {
                fs::write(&gitignore_path, "identity.txt\nconfig.toml\n")?;
                println!("Created .gitignore to exclude sensitive files.");
                run_git(&root_dir, &["add", ".gitignore"])?;
                run_git(
                    &root_dir,
                    &[
                        "-c",
                        "user.name=jd",
                        "-c",
                        "user.email=jd@localhost",
                        "commit",
                        "-m",
                        "Initial commit: Add .gitignore",
                    ],
                )?;
                println!("Created initial commit to track .gitignore");
            }
        }
    }

    if encrypt {
        let identity = x25519::Identity::generate();
        let recipient = identity.to_public();
        let identity_path = root_dir.join("identity.txt");

        if identity_path.exists() {
            println!("Encryption identity already exists. Doing nothing.");
        } else {
            fs::write(&identity_path, identity.to_string().expose_secret())?;
            println!("Generated new encryption identity at: {identity_path:?}");
            println!("\nIMPORTANT: Back this file up somewhere safe!");

            let config_path = root_dir.join("config.toml");
            let config_str = format!("recipient = \"{recipient}\"");
            fs::write(config_path, config_str)?;
            println!("Saved public key to config.toml.");
            println!("\nYour public key (recipient) is: {recipient}");
        }
    }
    Ok(())
}

/// Permanently decrypts all notes in ALL notebooks. It no longer takes
/// an `entries_dir` argument as it operates globally.
pub fn command_decrypt(force: bool) -> Result<()> {
    let root_dir = get_jd_dir_root()?;
    let notebooks_dir = get_notebooks_dir()?;
    let identity_path = root_dir.join("identity.txt");

    if !identity_path.exists() {
        println!("Journal is not encrypted (no identity.txt found). Nothing to do.");
        return Ok(());
    }

    if !force {
        print!("This will permanently decrypt all notes in ALL notebooks and remove your identity file. This action cannot be undone. Continue? [y/N] ");
        io::stdout().flush()?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)?;
        if confirmation.trim().to_lowercase() != "y" {
            println!("Decryption aborted.");
            return Ok(());
        }
    }

    println!("Loading decryption key...");
    let identity_str = fs::read_to_string(&identity_path)?;
    let identity = identity_str
        .parse::<x25519::Identity>()
        .map_err(|e| anyhow!(e))?;
    let identities: Vec<Box<dyn Identity>> = vec![Box::new(identity)];

    println!("Starting decryption of all notes in all notebooks...");
    for notebook_entry in fs::read_dir(notebooks_dir)?.filter_map(Result::ok) {
        if notebook_entry.path().is_dir() {
            let entries_dir = notebook_entry.path();
            println!(
                "\nDecrypting notebook: {:?}",
                entries_dir.file_name().unwrap()
            );
            for entry in fs::read_dir(entries_dir)?.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_file() {
                    let file_bytes = fs::read(&path)?;
                    if !file_bytes.starts_with(b"age-encryption.org") {
                        println!(
                            "  Skipping non-encrypted file: {:?}",
                            path.file_name().unwrap()
                        );
                        continue;
                    }

                    let decryptor = Decryptor::new(&file_bytes as &[u8])?;
                    if let Decryptor::Recipients(reader) = decryptor {
                        let mut decrypted_bytes = vec![];
                        reader
                            .decrypt(identities.iter().map(|i| i.as_ref()))?
                            .read_to_end(&mut decrypted_bytes)?;
                        fs::write(&path, decrypted_bytes)?;
                        println!("  - Decrypted {:?}", path.file_name().unwrap());
                    }
                }
            }
        }
    }

    let config_path = root_dir.join("config.toml");
    fs::remove_file(&identity_path)?;
    if config_path.exists() {
        fs::remove_file(config_path)?;
    }
    println!("\nSuccessfully decrypted journal and removed encryption keys.");
    Ok(())
}

pub fn command_sync() -> Result<()> {
    let root_dir = get_jd_dir_root()?;

    if !root_dir.join(".git").exists() {
        bail!(
            "jd directory at {:?} is not a Git repository. Run `jd init --git` first.",
            root_dir
        );
    }

    println!("Staging all changes...");
    run_git(&root_dir, &["add", "."])?;

    let commit_message = format!("jd sync: {}", Local::now().to_rfc2822());
    let commit_output = Command::new("git")
        .current_dir(&root_dir)
        .args([
            "-c",
            "user.name=jd",
            "-c",
            "user.email=jd@localhost",
            "commit",
            "-m",
            &commit_message,
        ])
        .output()
        .map_err(git_not_found)?;

    if commit_output.status.success() {
        println!("Committed changes with message: '{commit_message}'");
    } else {
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&commit_output.stdout),
            String::from_utf8_lossy(&commit_output.stderr)
        );
        if combined.contains("nothing to commit") {
            println!("Nothing to sync — already up to date.");
            return Ok(());
        }
        bail!(
            "git commit failed: {}",
            String::from_utf8_lossy(&commit_output.stderr).trim()
        );
    }

    let branch_output = Command::new("git")
        .current_dir(&root_dir)
        .args(["branch", "--show-current"])
        .output()
        .map_err(git_not_found)?;
    if !branch_output.status.success() {
        bail!("Could not determine current branch.");
    }
    let branch_name = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();
    if branch_name.is_empty() {
        bail!("Could not get branch name. Are you in a detached HEAD state?");
    }

    let refspec = format!("refs/heads/{branch_name}:refs/heads/{branch_name}");
    println!("Pushing to remote 'origin' on branch '{branch_name}'...");
    run_git(&root_dir, &["push", "origin", &refspec])?;

    println!("Sync complete.");
    Ok(())
}

pub fn command_down(entries_dir: &Path, message: &str, tags: Option<Vec<String>>) -> Result<()> {
    let mut content = String::new();
    if let Some(tags) = tags {
        if !tags.is_empty() {
            // `..Default::default()` handles the `pinned` field, setting it to false.
            let frontmatter = Frontmatter {
                tags,
                ..Default::default()
            };
            let fm_str = toml::to_string(&frontmatter)?;
            content.push_str("---\n");
            content.push_str(&fm_str);
            content.push_str("---\n\n");
        }
    }
    content.push_str(message);
    println!("Jotting down: \"{message}\"");
    let now = Local::now();
    let filename = now.format("%Y-%m-%d-%H%M%S.md").to_string();
    let file_path = entries_dir.join(filename);
    helpers::write_note_file(&file_path, &content)?;
    println!("Successfully saved to {file_path:?}");
    Ok(())
}

pub fn command_task(entries_dir: &Path, message: &str) -> Result<()> {
    let mut task_content = String::new();
    for (i, line) in message.lines().enumerate() {
        if i == 0 {
            task_content.push_str("- [ ] ");
            task_content.push_str(line);
        } else {
            task_content.push_str("\n      ");
            task_content.push_str(line);
        }
    }
    println!("Jotting down task: \"{message}\"");
    let now = Local::now();
    let filename = now.format("%Y-%m-%d-%H%M%S.md").to_string();
    let file_path = entries_dir.join(filename);
    helpers::write_note_file(&file_path, &task_content)?;
    println!("Successfully saved to {file_path:?}");
    Ok(())
}

pub fn command_new(
    entries_dir: &Path,
    template_name: Option<String>,
    variables: Vec<(String, String)>,
) -> Result<()> {
    let editor = helpers::get_editor()?;
    let now = Local::now();
    let filename = now.format("%Y-%m-%d-%H%M%S.md").to_string();
    let file_path = entries_dir.join(filename);
    let mut tpl_name = template_name.unwrap_or_else(|| "default".to_string());
    if !tpl_name.ends_with(".md") {
        tpl_name.push_str(".md");
    }
    let templates_dir = get_templates_dir()?;
    let tpl_path = templates_dir.join(tpl_name);
    let mut initial_content = String::new();
    if tpl_path.exists() {
        let template = fs::read_to_string(tpl_path)?;

        let uuid = Uuid::new_v4().to_string();
        let project_dir = env::current_dir()?
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let branch = Command::new("git")
            .args(["branch", "--show-current"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout).ok().map(|s| {
                        let t = s.trim().to_string();
                        if t.is_empty() {
                            "detached-head".to_string()
                        } else {
                            t
                        }
                    })
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "not-a-repo".to_string());

        // Build the full substitution table up front, then apply all
        // replacements in a single pass to prevent double-substitution.
        let mut substitutions: Vec<(String, String)> = vec![
            ("{{date}}".to_string(), now.to_rfc3339()),
            ("{{uuid}}".to_string(), uuid),
            ("{{project_dir}}".to_string(), project_dir),
            ("{{branch}}".to_string(), branch),
        ];
        for (key, value) in variables {
            substitutions.push((format!("{{{{{key}}}}}"), value));
        }

        initial_content = template;
        for (placeholder, value) in &substitutions {
            initial_content = initial_content.replace(placeholder.as_str(), value.as_str());
        }

        // Warn the user if any {{...}} placeholders were not resolved.
        if let Some(start) = initial_content.find("{{") {
            if initial_content[start..].contains("}}") {
                eprintln!("Warning: template contains unreplaced placeholders. Use -v key=value to supply values.");
            }
        }
    }
    helpers::write_note_file(&file_path, &initial_content)?;
    let status = Command::new(&editor).arg(&file_path).status()?;
    if !status.success() {
        bail!("Editor exited with a non-zero status.");
    }
    let final_content = helpers::read_note_file(&file_path)?;
    if final_content.trim().is_empty() {
        fs::remove_file(&file_path)?;
        println!("Empty jot discarded.");
    } else {
        println!("Successfully saved to {file_path:?}");
    }
    Ok(())
}

pub fn command_edit(note_path: PathBuf) -> Result<()> {
    let editor = helpers::get_editor()?;
    println!(
        "Opening {:?} in {}...",
        &note_path.file_name().unwrap(),
        &editor
    );
    let status = Command::new(&editor).arg(&note_path).status()?;
    if !status.success() {
        bail!("Editor exited with a non-zero status.");
    }
    println!("Finished editing {:?}.", &note_path.file_name().unwrap());
    Ok(())
}

pub fn command_tag(entries_dir: &Path, args: TagArgs) -> Result<()> {
    let (id_prefix, last) = match &args.action {
        TagAction::Add {
            id_prefix, last, ..
        }
        | TagAction::Remove {
            id_prefix, last, ..
        }
        | TagAction::Set {
            id_prefix, last, ..
        } => (id_prefix.as_ref(), *last),
    };

    let note_path = get_note_path_for_action(entries_dir, id_prefix.cloned(), last)?;
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
    let mut note = parse_note_from_file(&note_path, &notebook_name)?;

    match args.action {
        TagAction::Add { tags, .. } => {
            for tag in tags {
                if !note.frontmatter.tags.contains(&tag) {
                    note.frontmatter.tags.push(tag);
                }
            }
            println!("Added tags to '{}'.", note.id);
        }
        TagAction::Remove { tags, .. } => {
            note.frontmatter.tags.retain(|t| !tags.contains(t));
            println!("Removed tags from '{}'.", note.id);
        }
        TagAction::Set { tags, .. } => {
            note.frontmatter.tags = tags;
            println!("Set tags for '{}'.", note.id);
        }
    }
    note.frontmatter.tags.sort();
    note.frontmatter.tags.dedup();
    let new_frontmatter_str = toml::to_string(&note.frontmatter)?;
    let new_content = format!("---\n{}---\n\n{}", new_frontmatter_str, note.content);
    helpers::write_note_file(&note.path, &new_content)?;
    Ok(())
}

// A private helper function to toggle the pinned status of a note.
// This avoids duplicating the logic for finding, parsing, modifying, and saving a note.
fn command_toggle_pin_status(
    entries_dir: &Path,
    id_prefix: Option<String>,
    last: Option<usize>,
    pin: bool,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id_prefix, last)?;
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
    let mut note = parse_note_from_file(&note_path, &notebook_name)?;

    if note.frontmatter.pinned == pin {
        println!(
            "Jot '{}' is already {}.",
            note.id,
            if pin { "pinned" } else { "unpinned" }
        );
        return Ok(());
    }

    note.frontmatter.pinned = pin;

    let new_frontmatter_str = toml::to_string(&note.frontmatter)?;
    let new_content = format!("---\n{}---\n\n{}", new_frontmatter_str, note.content);
    helpers::write_note_file(&note.path, &new_content)?;

    println!(
        "Successfully {} jot '{}'.",
        if pin { "pinned" } else { "unpinned" },
        note.id
    );

    Ok(())
}

// Public command function to pin a note.
pub fn command_pin(
    entries_dir: &Path,
    id_prefix: Option<String>,
    last: Option<usize>,
) -> Result<()> {
    command_toggle_pin_status(entries_dir, id_prefix, last, true)
}

// Public command function to unpin a note.
pub fn command_unpin(
    entries_dir: &Path,
    id_prefix: Option<String>,
    last: Option<usize>,
) -> Result<()> {
    command_toggle_pin_status(entries_dir, id_prefix, last, false)
}

pub fn command_list(
    entries_dir: &PathBuf,
    count: Option<usize>,
    pinned: bool,
    tasks: bool,
    format: &str,
) -> Result<()> {
    let num_to_list = count.unwrap_or(10);
    let entries = fs::read_dir(entries_dir)?;
    let mut notes = Vec::new();
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();

    for entry in entries.filter_map(Result::ok) {
        notes.push(parse_note_from_file(&entry.path(), &notebook_name)?);
    }

    if pinned {
        notes.retain(|note| note.frontmatter.pinned);
        if format == "human" {
            println!("Showing pinned jots:");
        }
    }

    if tasks {
        notes.retain(|note| note.tasks.iter().any(|t| !t.completed));
        notes.sort_by(|a, b| b.id.cmp(&a.id));
        notes.truncate(num_to_list);

        if format == "human" {
            if notes.is_empty() {
                println!("\nNo jots with incomplete tasks found.");
            } else {
                println!("\n{:<22} TASKS", "ID");
                println!("{:-<22} {:-<50}", "", "");
                for note in notes {
                    let pending: Vec<&str> = note
                        .tasks
                        .iter()
                        .filter(|t| !t.completed)
                        .map(|t| t.description.as_str())
                        .collect();
                    println!("{:<22} [ ] {}", note.id, pending[0]);
                    for task in &pending[1..] {
                        println!("{:<22} [ ] {}", "", task);
                    }
                }
            }
        } else {
            helpers::display_formatted_note_list(notes, format)?;
        }
        return Ok(());
    }

    notes.sort_by(|a, b| b.id.cmp(&a.id));
    notes.truncate(num_to_list);

    helpers::display_formatted_note_list(notes, format)?;
    Ok(())
}

#[cfg(not(windows))]
pub fn command_select(entries_dir: &PathBuf) -> Result<()> {
    struct NoteItem {
        id: String,
        display_text: String,
        content: String,
    }

    impl SkimItem for NoteItem {
        fn text(&self) -> Cow<'_, str> {
            Cow::Borrowed(&self.display_text)
        }

        fn output(&self) -> Cow<'_, str> {
            Cow::Borrowed(&self.id)
        }

        fn preview(&self, _context: PreviewContext) -> ItemPreview {
            ItemPreview::Text(self.content.clone())
        }
    }

    let entries = fs::read_dir(entries_dir)?;
    let mut notes = vec![];
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
    for entry in entries.filter_map(Result::ok) {
        notes.push(parse_note_from_file(&entry.path(), &notebook_name)?);
    }
    notes.sort_by(|a, b| b.id.cmp(&a.id));

    let options = SkimOptionsBuilder::default()
        .multi(false)
        .reverse(true)
        .preview(Some(""))
        .build()?;

    type SkimChannel = (Sender<Arc<dyn SkimItem>>, Receiver<Arc<dyn SkimItem>>);

    let (tx, rx): SkimChannel = unbounded();

    for note in notes {
        let display_text = format!(
            "{} | {}",
            note.id,
            note.content.lines().next().unwrap_or("").trim()
        );
        let item = NoteItem {
            id: note.id,
            display_text,
            content: note.content,
        };
        let _ = tx.send(Arc::new(item));
    }
    drop(tx);

    let skim_output = Skim::run_with(&options, Some(rx));

    if let Some(output) = skim_output {
        if output.is_abort {
            return Ok(());
        }
        for item in output.selected_items.iter() {
            println!("{}", item.output());
        }
    }

    Ok(())
}

pub fn command_find(
    entries_dir: &PathBuf,
    query: &str,
    all: bool,
    format: &str,
    context: bool,
) -> Result<()> {
    if format == "human" {
        println!("Searching for \"{query}\" in your jots...");
    }
    let mut matches = Vec::new();

    if all {
        let notebooks_dir = get_notebooks_dir()?;
        for notebook_entry in fs::read_dir(notebooks_dir)?.filter_map(Result::ok) {
            if notebook_entry.path().is_dir() {
                let notebook_path = notebook_entry.path();
                let notebook_name = notebook_path.file_name().unwrap().to_string_lossy();
                // Pass a reference to `read_dir` to avoid moving the value
                for entry in fs::read_dir(&notebook_path)?.filter_map(Result::ok) {
                    let note = parse_note_from_file(&entry.path(), &notebook_name)?;
                    if note.content.to_lowercase().contains(&query.to_lowercase()) {
                        matches.push(note);
                    }
                }
            }
        }
        if format == "human" {
            if context {
                helpers::display_search_results_with_context(matches, query);
            } else {
                display_global_find_list(matches);
            }
        } else {
            helpers::display_formatted_note_list(matches, format)?;
        }
    } else {
        let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
        for entry in fs::read_dir(entries_dir)?.filter_map(Result::ok) {
            let note = parse_note_from_file(&entry.path(), &notebook_name)?;
            if note.content.to_lowercase().contains(&query.to_lowercase()) {
                matches.push(note);
            }
        }
        if format == "human" && context {
            helpers::display_search_results_with_context(matches, query);
        } else {
            helpers::display_formatted_note_list(matches, format)?;
        }
    }
    Ok(())
}

pub fn command_tags_filter(entries_dir: &PathBuf, tags: &[String], format: &str) -> Result<()> {
    let entries = fs::read_dir(entries_dir)?;
    let mut matches = Vec::new();
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();

    for entry in entries.filter_map(Result::ok) {
        let note = parse_note_from_file(&entry.path(), &notebook_name)?;
        if tags.iter().all(|t| note.frontmatter.tags.contains(t)) {
            matches.push(note);
        }
    }
    helpers::display_formatted_note_list(matches, format)?;
    Ok(())
}

pub fn command_by_date_filter(
    entries_dir: &PathBuf,
    date: NaiveDate,
    compile: bool,
    format: &str,
) -> Result<()> {
    let date_prefix = date.format("%Y-%m-%d").to_string();
    if format == "human" {
        println!("Finding jots from {date_prefix}...");
    }
    let mut matches = Vec::new();
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();

    for entry in fs::read_dir(entries_dir)?.filter_map(Result::ok) {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(&date_prefix)
        {
            matches.push(parse_note_from_file(&entry.path(), &notebook_name)?);
        }
    }
    matches.sort_by(|a, b| a.id.cmp(&b.id));
    if compile {
        helpers::compile_notes(matches)?
    } else {
        helpers::display_formatted_note_list(matches, format)?;
    }
    Ok(())
}

pub fn command_today(entries_dir: &PathBuf, compile: bool, format: &str) -> Result<()> {
    command_by_date_filter(entries_dir, Local::now().date_naive(), compile, format)
}

pub fn command_yesterday(entries_dir: &PathBuf, compile: bool, format: &str) -> Result<()> {
    let yesterday = Local::now().date_naive() - chrono::Duration::days(1);
    command_by_date_filter(entries_dir, yesterday, compile, format)
}

pub fn command_by_week(entries_dir: &PathBuf, compile: bool, format: &str) -> Result<()> {
    let today = Local::now().date_naive();
    let week_start = today - chrono::Duration::days(today.weekday().num_days_from_sunday() as i64);
    if format == "human" {
        println!("Finding jots from this week (starting {week_start})...");
    }
    let mut matches = Vec::new();
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();

    for entry in fs::read_dir(entries_dir)?.filter_map(Result::ok) {
        let filename = entry.file_name().to_string_lossy().to_string();
        if filename.len() >= 10 {
            if let Ok(date) = NaiveDate::parse_from_str(&filename[0..10], "%Y-%m-%d") {
                if date >= week_start && date <= today {
                    matches.push(parse_note_from_file(&entry.path(), &notebook_name)?);
                }
            }
        }
    }
    matches.sort_by(|a, b| a.id.cmp(&b.id));
    if compile {
        helpers::compile_notes(matches)?
    } else {
        helpers::display_formatted_note_list(matches, format)?;
    }
    Ok(())
}

pub fn command_on(
    entries_dir: &PathBuf,
    date_spec: &str,
    compile: bool,
    format: &str,
) -> Result<()> {
    let mut matches = Vec::new();
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();

    if let Some((start_str, end_str)) = date_spec.split_once("..") {
        let start_date = NaiveDate::parse_from_str(start_str, "%Y-%m-%d")?;
        let end_date = NaiveDate::parse_from_str(end_str, "%Y-%m-%d")?;
        if format == "human" {
            println!("Finding jots from {start_date} to {end_date}...");
        }
        for entry in fs::read_dir(entries_dir)?.filter_map(Result::ok) {
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.len() >= 10 {
                if let Ok(date) = NaiveDate::parse_from_str(&filename[0..10], "%Y-%m-%d") {
                    if date >= start_date && date <= end_date {
                        matches.push(parse_note_from_file(&entry.path(), &notebook_name)?);
                    }
                }
            }
        }
    } else {
        let date = NaiveDate::parse_from_str(date_spec, "%Y-%m-%d")?;
        return command_by_date_filter(entries_dir, date, compile, format);
    }
    matches.sort_by(|a, b| a.id.cmp(&b.id));
    if compile {
        helpers::compile_notes(matches)?
    } else {
        helpers::display_formatted_note_list(matches, format)?;
    }
    Ok(())
}

pub fn command_show(note_path: PathBuf, raw: bool) -> Result<()> {
    let content = helpers::read_note_file(&note_path)?;

    if raw {
        if let Some(stripped) = content.strip_prefix("---") {
            if let Some(rel) = stripped.find("\n---") {
                print!("{}", stripped[(rel + 4)..].trim_start());
                return Ok(());
            }
        }
        print!("{content}");
    } else {
        // Try syntax highlighting for markdown
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let syntax = ps.find_syntax_by_extension("md").unwrap();
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);

        for line in LinesWithEndings::from(&content) {
            let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();
            let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
            print!("{}", escaped);
        }
        println!("\x1b[0m"); // Reset colors
    }
    Ok(())
}

pub fn command_delete(note_path: PathBuf, force: bool) -> Result<()> {
    let filename = note_path.file_name().unwrap().to_string_lossy();
    if !force {
        print!("Are you sure you want to delete '{filename}'? [y/N] ");
        io::stdout().flush()?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)?;
        if confirmation.trim().to_lowercase() != "y" {
            println!("Deletion aborted.");
            return Ok(());
        }
    }
    fs::remove_file(&note_path)?;
    println!("Successfully deleted '{filename}'.");
    Ok(())
}

pub fn command_info(entries_dir: &PathBuf, args: InfoArgs) -> Result<()> {
    if !args.paths && !args.stats {
        println!(
            "Please provide a flag to the info command, e.g., `jd info --paths` or `jd info --stats`"
        );
        println!("\nFor more information, try '--help'");
        return Ok(());
    }
    if args.paths {
        println!("--- jd paths ---");
        let active_notebook =
            env::var("JD_ACTIVE_NOTEBOOK").unwrap_or_else(|_| "default".to_string());
        println!("Root Directory:   {:?}", helpers::get_jd_dir_root()?);
        println!("Notebooks Root:   {:?}", helpers::get_notebooks_dir()?);
        println!("Active Notebook:  {active_notebook}");
        println!("Entries:          {entries_dir:?}");
        println!("Templates:        {:?}", helpers::get_templates_dir()?);
    }
    if args.stats {
        println!("\n--- jd stats ---");

        if args.all {
            // Stats for all notebooks
            let notebooks_dir = get_notebooks_dir()?;
            let mut total_notes = 0;
            let mut all_tags: HashMap<String, usize> = HashMap::new();
            let mut total_task_stats = TaskStats::default();

            for entry in fs::read_dir(notebooks_dir)?.filter_map(Result::ok) {
                if entry.path().is_dir() {
                    let notebook_path = entry.path();
                    let (note_count, tag_counts, task_stats) =
                        calculate_stats_for_dir(&notebook_path)?;
                    total_notes += note_count;
                    for (tag, count) in tag_counts {
                        *all_tags.entry(tag).or_insert(0) += count;
                    }
                    total_task_stats.completed += task_stats.completed;
                    total_task_stats.pending += task_stats.pending;
                }
            }
            println!("Stats for all notebooks combined:");
            print_stats(total_notes, all_tags, total_task_stats);
        } else {
            // Stats for the active notebook only
            let active_notebook_name = entries_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("default");
            println!("Stats for active notebook: '{active_notebook_name}'");
            let (note_count, tag_counts, task_stats) = calculate_stats_for_dir(entries_dir)?;
            print_stats(note_count, tag_counts, task_stats);
        }
    }
    Ok(())
}

pub fn display_global_find_list(notes: Vec<helpers::Note>) {
    if notes.is_empty() {
        println!("\nNo jots found.");
        return;
    }
    // Adjust spacing as needed for your desired output
    println!("\n{:<22} {:<18} FIRST LINE OF CONTENT", "ID", "NOTEBOOK");
    println!("{:-<22} {:-<18} {:-<50}", "", "", "");
    for note in notes {
        let first_line = note.content.lines().next().unwrap_or("").trim();
        println!("{:<22} {:<18} {}", note.id, note.notebook, first_line);
    }
}

fn calculate_stats_for_dir(dir: &Path) -> Result<(usize, HashMap<String, usize>, TaskStats)> {
    let mut note_count = 0;
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    let mut task_stats = TaskStats::default();
    let notebook_name = dir.file_name().unwrap().to_string_lossy();

    for entry in fs::read_dir(dir)?.filter_map(Result::ok) {
        if entry.path().is_file() {
            note_count += 1;
            let note = parse_note_from_file(&entry.path(), &notebook_name)?;
            for tag in note.frontmatter.tags {
                *tag_counts.entry(tag).or_insert(0) += 1;
            }
            for task in note.tasks {
                if task.completed {
                    task_stats.completed += 1;
                } else {
                    task_stats.pending += 1;
                }
            }
        }
    }
    Ok((note_count, tag_counts, task_stats))
}

fn print_stats(note_count: usize, tag_counts: HashMap<String, usize>, task_stats: TaskStats) {
    println!("Total jots: {note_count}");
    if !tag_counts.is_empty() {
        let mut sorted_tags: Vec<_> = tag_counts.into_iter().collect();
        sorted_tags.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        sorted_tags.truncate(5);
        println!("\nMost common tags:");
        for (tag, count) in sorted_tags {
            println!("  - {tag} ({count})");
        }
    }
    if task_stats.completed > 0 || task_stats.pending > 0 {
        println!("\nTask Summary:");
        println!("  - Completed: {}", task_stats.completed);
        println!("  - Pending:   {}", task_stats.pending);
    }
}

pub fn command_export(args: ExportArgs) -> Result<()> {
    let notebooks_dir = get_notebooks_dir()?;
    let notebook_path = notebooks_dir.join(&args.notebook_name);

    if !notebook_path.is_dir() {
        bail!("Notebook '{}' not found.", args.notebook_name);
    }

    match args.format.as_str() {
        "zip" => export_to_zip(&notebook_path, &args.output)?,
        "json" => export_to_json(&notebook_path, &args.notebook_name, &args.output)?,
        _ => bail!(
            "Unsupported format: '{}'. Please use 'zip' or 'json'.",
            args.format
        ),
    }

    println!(
        "Successfully exported notebook '{}' to {:?}",
        args.notebook_name, args.output
    );
    Ok(())
}

fn export_to_zip(notebook_path: &Path, output_path: &Path) -> Result<()> {
    let file = File::create(output_path)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Zstd);

    for entry in fs::read_dir(notebook_path)?.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap().to_string_lossy();
            zip.start_file(filename, options)?;
            let content = helpers::read_note_file(&path)?;
            zip.write_all(content.as_bytes())?;
        }
    }
    zip.finish()?;
    Ok(())
}

fn export_to_json(notebook_path: &Path, notebook_name: &str, output_path: &Path) -> Result<()> {
    let mut jots = Vec::new();
    for entry in fs::read_dir(notebook_path)?.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            jots.push(JsonJot {
                filename: path.file_name().unwrap().to_string_lossy().to_string(),
                content: helpers::read_note_file(&path)?,
            });
        }
    }

    let export_data = JsonExport {
        notebook_name: notebook_name.to_string(),
        jots,
    };

    let json_string = serde_json::to_string_pretty(&export_data)?;
    fs::write(output_path, json_string)?;
    Ok(())
}

pub fn command_import(args: ImportArgs) -> Result<()> {
    let extension = args
        .file_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    match extension {
        "zip" => import_from_zip(&args.file_path)?,
        "json" => import_from_json(&args.file_path)?,
        _ => bail!(
            "Unsupported file type: '{:?}'. Please use a '.zip' or '.json' file.",
            args.file_path
        ),
    }
    Ok(())
}

fn import_from_zip(file_path: &Path) -> Result<()> {
    let file = File::open(file_path)?;
    let mut archive = ZipArchive::new(file)?;
    let notebook_name = file_path.file_stem().unwrap().to_string_lossy().to_string();
    let notebooks_dir = get_notebooks_dir()?;
    let new_notebook_path = notebooks_dir.join(&notebook_name);

    if new_notebook_path.exists() {
        bail!("A notebook named '{}' already exists. Please rename the zip file or the existing notebook.", notebook_name);
    }
    fs::create_dir_all(&new_notebook_path)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        // Reject entries with path traversal components or absolute paths.
        let entry_name = file.name();
        let safe_name = Path::new(entry_name)
            .file_name()
            .ok_or_else(|| anyhow!("Invalid entry name in archive: '{}'", entry_name))?;
        let outpath = new_notebook_path.join(safe_name);
        let mut outfile = File::create(&outpath)?;
        io::copy(&mut file, &mut outfile)?;
    }

    println!("Successfully imported notebook '{notebook_name}' from {file_path:?}");
    Ok(())
}

fn import_from_json(file_path: &Path) -> Result<()> {
    let json_string = fs::read_to_string(file_path)?;
    let export_data: JsonExport = serde_json::from_str(&json_string)?;
    let notebooks_dir = get_notebooks_dir()?;
    let new_notebook_path = notebooks_dir.join(&export_data.notebook_name);

    if new_notebook_path.exists() {
        bail!(
            "A notebook named '{}' already exists.",
            export_data.notebook_name
        );
    }
    fs::create_dir_all(&new_notebook_path)?;

    for jot in export_data.jots {
        let jot_path = new_notebook_path.join(jot.filename);
        helpers::write_note_file(&jot_path, &jot.content)?;
    }

    println!(
        "Successfully imported notebook '{}' from {:?}",
        export_data.notebook_name, file_path
    );
    Ok(())
}

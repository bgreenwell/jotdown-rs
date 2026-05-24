use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use age::{secrecy::ExposeSecret, x25519, Decryptor, Identity};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{Datelike, Local, NaiveDate};
use clap::Parser;
use dirs;
use git2::{Cred, PushOptions, RemoteCallbacks, Repository, Signature};
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

use crate::cli::{
    ExportArgs, ImportArgs, InfoArgs, NotebookAction, NotebookArgs, TagAction, TagArgs,
};
use crate::helpers::{
    self, display_note_list, get_note_path_for_action, get_notebooks_dir, get_jd_dir_root,
    get_templates_dir, parse_note_from_file, Frontmatter, TaskStats,
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
                "pin", "unpin", "edit", "show", "delete", "info", "use", "exit", "quit",
                "append", "prepend", "move", "rename", "daily",
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

// --- Notebook Commands ---

/// Handles all notebook-related subcommands.
pub fn command_notebook(args: NotebookArgs) -> Result<()> {
    match args.action {
        NotebookAction::New { name } => command_notebook_new(&name)?,
        NotebookAction::List => command_notebook_list()?,
        NotebookAction::Use { name } => command_notebook_use(&name)?,
        NotebookAction::Status => command_notebook_status()?,
    }
    Ok(())
}

/// Creates a new notebook directory.
fn command_notebook_new(name: &str) -> Result<()> {
    // Basic sanitization to prevent directory traversal or invalid names.
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        bail!(
            "Invalid notebook name: '{}'. Names cannot contain slashes or be dots.",
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

/// Lists all available notebooks.
fn command_notebook_list() -> Result<()> {
    let notebooks_dir = get_notebooks_dir()?;
    let active_notebook =
        env::var("JD_ACTIVE_NOTEBOOK").unwrap_or_else(|_| "default".to_string());

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

/// Prints the shell command to switch the active notebook.
fn command_notebook_use(name: &str) -> Result<()> {
    let notebooks_dir = get_notebooks_dir()?;
    let target_notebook = notebooks_dir.join(name);

    if !target_notebook.exists() || !target_notebook.is_dir() {
        bail!(
            "Notebook '{}' not found. Create it with `jd notebook new {}`.",
            name,
            name
        );
    }

    // This command prints the shell command for the user to evaluate.
    // It cannot modify the parent shell's environment directly.
    println!("export JD_ACTIVE_NOTEBOOK=\"{name}\"");
    Ok(())
}

/// Shows the currently active notebook.
fn command_notebook_status() -> Result<()> {
    let active_notebook =
        env::var("JD_ACTIVE_NOTEBOOK").unwrap_or_else(|_| "default".to_string());
    println!("Active notebook: {active_notebook}");
    Ok(())
}

// --- Other Commands ---

pub fn command_append(
    entries_dir: &Path,
    id_prefix: Option<String>,
    last: Option<usize>,
    content: &str,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id_prefix, last)?;
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
    id_prefix: Option<String>,
    last: Option<usize>,
    content: &str,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id_prefix, last)?;
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
    let mut note = parse_note_from_file(&note_path, &notebook_name)?;

    // We want to prepend BELOW the frontmatter.
    let prefix = if !content.ends_with('\n') {
        format!("{}\n", content)
    } else {
        content.to_string()
    };
    note.content = format!("{}{}", prefix, note.content);

    let new_frontmatter_str = serde_yaml::to_string(&note.frontmatter)?;
    let new_content = format!("---\n{}---\n\n{}", new_frontmatter_str, note.content);
    helpers::write_note_file(&note_path, &new_content)?;
    println!("Successfully prepended to jot '{}'.", note.id);
    Ok(())
}

pub fn command_move(
    entries_dir: &Path,
    id_prefix: Option<String>,
    last: Option<usize>,
    destination: &str,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id_prefix, last)?;
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

    let mut dest_filename = new_name.to_string();
    if !dest_filename.ends_with(".md") {
        dest_filename.push_str(".md");
    }

    let dest_path = entries_dir.join(&dest_filename);
    if dest_path.exists() {
        bail!("A file with name '{}' already exists.", dest_filename);
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
            "---\ntitle: Daily Note - {}\n---\n\n{}\n",
            today, message
        );
        helpers::write_note_file(&note_path, &content)?;
        println!("Created daily note: {}", filename);
    }
    Ok(())
}

/// Enters the interactive jd shell.
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
        "\n\
        \x1b[38;5;208m      ██╗ ██████╗ \x1b[0m\n\
        \x1b[38;5;209m      ██║ ██╔══██╗\x1b[0m\n\
        \x1b[38;5;210m      ██║ ██║  ██║\x1b[0m\n\
        \x1b[38;5;211m ██   ██║ ██║  ██║\x1b[0m\n\
        \x1b[38;5;212m ╚█████╔╝ ██████╔╝\x1b[0m\n\
        \x1b[38;5;213m  ╚════╝  ╚═════╝ \x1b[0m\n\
        \n  \x1b[0;1mjd v{}\x1b[0m | Today: \x1b[32m{}\x1b[0m | Stats: \x1b[33m{} notes in '{}'\x1b[0m\n  \
        \x1b[2mTip: {}\x1b[0m\n  \
        \x1b[2mType 'exit' or 'quit' to leave the shell.\x1b[0m\n",
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

                let mut parts = line.split_whitespace();
                let command_name = parts.next().unwrap_or("");
                match command_name {
                    "exit" | "quit" => break,
                    "use" => {
                        if let Some(name) = parts.next() {
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
                args.extend(line.split_whitespace());

                match crate::cli::Cli::try_parse_from(args) {
                    Ok(cli) => {
                        let notebook_override = cli
                            .notebook
                            .clone()
                            .unwrap_or_else(|| active_notebook.clone());
                        let entries_dir =
                            crate::helpers::get_active_entries_dir(Some(notebook_override))?;

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

/// Initializes the `jd` directory, optionally with Git and/or encryption.
pub fn command_init(git: bool, encrypt: bool) -> Result<()> {
    let root_dir = get_jd_dir_root()?;
    println!("jd directory is at: {root_dir:?}");

    if git {
        match Repository::init(&root_dir) {
            Ok(repo) => {
                println!("Initialized a new Git repository in {root_dir:?}");
                let gitignore_path = root_dir.join(".gitignore");
                if !repo.is_empty()? {
                    println!("Git repository is not empty. Assuming it is already set up.");
                } else if !gitignore_path.exists() {
                    // Correctly ignore only sensitive files. Notebooks should be tracked.
                    fs::write(&gitignore_path, "identity.txt\nconfig.toml\n")?;
                    println!("Created .gitignore to exclude sensitive files.");

                    let mut index = repo.index()?;
                    index.add_path(Path::new(".gitignore"))?;
                    index.write()?;
                    let oid = index.write_tree()?;
                    let tree = repo.find_tree(oid)?;
                    let signature = Signature::now("jd", "jd@localhost")?;
                    repo.commit(
                        Some("HEAD"),
                        &signature,
                        &signature,
                        "Initial commit: Add .gitignore",
                        &tree,
                        &[],
                    )?;
                    println!("Created initial commit to track .gitignore");
                }
            }
            Err(e) if e.code() == git2::ErrorCode::Exists => {
                println!("Git repository already exists in {root_dir:?}")
            }
            Err(e) => bail!("Failed to initialize Git repository: {}", e),
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

/// Commits and pushes all changes in the jd Git repository to the `origin` remote.
pub fn command_sync() -> Result<()> {
    let root_dir = get_jd_dir_root()?;
    let repo = Repository::open(&root_dir).map_err(|_| {
        anyhow!(
            "jd directory at {:?} is not a Git repository. Run `jd init --git` first.",
            root_dir
        )
    })?;

    println!("Staging all changes...");
    let mut index = repo.index()?;
    index.add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;

    let signature = Signature::now("jd", "jd@localhost")?;
    let commit_message = format!("jd sync: {}", Local::now().to_rfc2822());

    let head = repo.head();
    let parent_commits = match head {
        Ok(head_ref) => vec![head_ref.peel_to_commit()?],
        Err(ref e) if e.code() == git2::ErrorCode::UnbornBranch => Vec::new(),
        Err(e) => return Err(e.into()),
    };

    let parents_ref: Vec<&git2::Commit> = parent_commits.iter().collect();

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &commit_message,
        &tree,
        &parents_ref,
    )?;

    println!("Committed changes with message: '{commit_message}'");

    let head = repo.head()?;
    let branch_name = head.shorthand().with_context(|| {
        "Could not get branch name from HEAD. Are you in a detached HEAD state?"
    })?;
    let refspec = format!("refs/heads/{branch_name}:refs/heads/{branch_name}");

    let mut remote = repo.find_remote("origin").map_err(|_| {
        anyhow!("Could not find remote 'origin'. Please add a remote to your git repository.")
    })?;

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_git, allowed_types| {
        let username = username_from_git.unwrap_or("git");

        if allowed_types.is_user_pass_plaintext() {
            if let Ok(token) = env::var("GITHUB_TOKEN") {
                return Cred::userpass_plaintext(username, &token);
            }
        }

        if allowed_types.is_ssh_key() {
            if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                return Ok(cred);
            }
        }

        if allowed_types.is_ssh_key() {
            if let Some(home_dir) = dirs::home_dir() {
                if let Ok(cred) =
                    Cred::ssh_key(username, None, &home_dir.join(".ssh").join("id_rsa"), None)
                {
                    return Ok(cred);
                }
            }
        }

        if allowed_types.is_user_pass_plaintext() {
            if let Ok(cred) = Cred::credential_helper(&repo.config()?, _url, Some(username)) {
                return Ok(cred);
            }
        }

        Err(git2::Error::new(
            git2::ErrorCode::Auth,
            git2::ErrorClass::Ssh,
            "failed to acquire credentials",
        ))
    });

    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);

    println!("Pushing to remote 'origin' on branch '{branch_name}'...");
    remote.push(&[&refspec], Some(&mut push_options))?;

    println!("Sync complete.");

    Ok(())
}

/// Creates a new jot instantly from command-line arguments.
pub fn command_down(entries_dir: &Path, message: &str, tags: Option<Vec<String>>) -> Result<()> {
    let mut content = String::new();
    if let Some(tags) = tags {
        if !tags.is_empty() {
            // `..Default::default()` handles the `pinned` field, setting it to false.
            let frontmatter = Frontmatter {
                tags,
                ..Default::default()
            };
            let fm_str = serde_yaml::to_string(&frontmatter)?;
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

/// Creates a new jot formatted as a Markdown task.
pub fn command_task(entries_dir: &Path, message: &str) -> Result<()> {
    let task_content = format!("- [ ] {message}");
    println!("Jotting down task: \"{message}\"");
    let now = Local::now();
    let filename = now.format("%Y-%m-%d-%H%M%S.md").to_string();
    let file_path = entries_dir.join(filename);
    helpers::write_note_file(&file_path, &task_content)?;
    println!("Successfully saved to {file_path:?}");
    Ok(())
}

/// Creates a new jot by opening the default editor.
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
        initial_content = fs::read_to_string(tpl_path)?;
        // {{date}}
        initial_content = initial_content.replace("{{date}}", &now.to_rfc3339());

        // {{uuid}}
        let uuid = Uuid::new_v4().to_string();
        initial_content = initial_content.replace("{{uuid}}", &uuid);

        // {{project_dir}}
        let project_dir = env::current_dir()?
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        initial_content = initial_content.replace("{{project_dir}}", &project_dir);

        // {{branch}}
        let branch = match Repository::discover(".") {
            Ok(repo) => {
                let head = repo.head()?;
                head.shorthand().unwrap_or("detached-head").to_string()
            }
            Err(_) => "not-a-repo".to_string(),
        };
        initial_content = initial_content.replace("{{branch}}", &branch);

        // Handle custom variables
        for (key, value) in variables {
            initial_content = initial_content.replace(&format!("{{{{{key}}}}}"), &value);
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

/// Opens an existing jot in the default editor.
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

/// Manages tags on an existing jot.
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
    let new_frontmatter_str = serde_yaml::to_string(&note.frontmatter)?;
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

    let new_frontmatter_str = serde_yaml::to_string(&note.frontmatter)?;
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

/// Lists the most recent jots.
pub fn command_list(
    entries_dir: &PathBuf,
    count: Option<usize>,
    pinned: bool,
    tasks: bool,
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
        println!("Showing pinned jots:");
    }

    if tasks {
        notes.retain(|note| note.tasks.iter().any(|t| !t.completed));
        notes.sort_by(|a, b| b.id.cmp(&a.id));
        notes.truncate(num_to_list);
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
        return Ok(());
    }

    notes.sort_by(|a, b| b.id.cmp(&a.id));
    notes.truncate(num_to_list);

    display_note_list(notes);
    Ok(())
}

/// Interactively selects a jot using a fuzzy finder.
#[cfg(not(windows))]
pub fn command_select(entries_dir: &PathBuf) -> Result<()> {
    struct NoteItem {
        id: String,
        display_text: String,
    }

    impl SkimItem for NoteItem {
        fn text(&self) -> Cow<'_, str> {
            Cow::Borrowed(&self.display_text)
        }

        fn output(&self) -> Cow<'_, str> {
            Cow::Borrowed(&self.id)
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

/// Performs a full-text search of all jots.
pub fn command_find(entries_dir: &PathBuf, query: &str, all: bool) -> Result<()> {
    println!("Searching for \"{query}\" in your jots...");
    let mut matches = Vec::new();

    if all {
        // --- GLOBAL SEARCH LOGIC ---
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
        display_global_find_list(matches);
    } else {
        // --- LOCAL SEARCH LOGIC ---
        let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();
        for entry in fs::read_dir(entries_dir)?.filter_map(Result::ok) {
            let note = parse_note_from_file(&entry.path(), &notebook_name)?;
            if note.content.to_lowercase().contains(&query.to_lowercase()) {
                matches.push(note);
            }
        }
        display_note_list(matches);
    }

    Ok(())
}

/// Filters jots by one or more tags.
pub fn command_tags_filter(entries_dir: &PathBuf, tags: &[String]) -> Result<()> {
    let entries = fs::read_dir(entries_dir)?;
    let mut matches = Vec::new();
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();

    for entry in entries.filter_map(Result::ok) {
        let note = parse_note_from_file(&entry.path(), &notebook_name)?;
        if tags.iter().all(|t| note.frontmatter.tags.contains(t)) {
            matches.push(note);
        }
    }
    display_note_list(matches);
    Ok(())
}

/// A helper function for all date-based filtering.
pub fn command_by_date_filter(entries_dir: &PathBuf, date: NaiveDate, compile: bool) -> Result<()> {
    let date_prefix = date.format("%Y-%m-%d").to_string();
    println!("Finding jots from {date_prefix}...");
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
        display_note_list(matches)
    }
    Ok(())
}

/// Lists jots created today.
pub fn command_today(entries_dir: &PathBuf, compile: bool) -> Result<()> {
    command_by_date_filter(entries_dir, Local::now().date_naive(), compile)
}

/// Lists jots created yesterday.
pub fn command_yesterday(entries_dir: &PathBuf, compile: bool) -> Result<()> {
    let yesterday = Local::now().date_naive() - chrono::Duration::days(1);
    command_by_date_filter(entries_dir, yesterday, compile)
}

/// Lists jots created in the current week.
pub fn command_by_week(entries_dir: &PathBuf, compile: bool) -> Result<()> {
    let today = Local::now().date_naive();
    let week_start = today - chrono::Duration::days(today.weekday().num_days_from_sunday() as i64);
    println!("Finding jots from this week (starting {week_start})...");
    let mut matches = Vec::new();
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();

    for entry in fs::read_dir(entries_dir)?.filter_map(Result::ok) {
        let filename = entry.file_name().to_string_lossy().to_string();
        if let Ok(date) = NaiveDate::parse_from_str(&filename[0..10], "%Y-%m-%d") {
            if date >= week_start && date <= today {
                matches.push(parse_note_from_file(&entry.path(), &notebook_name)?);
            }
        }
    }
    matches.sort_by(|a, b| a.id.cmp(&b.id));
    if compile {
        helpers::compile_notes(matches)?
    } else {
        display_note_list(matches)
    }
    Ok(())
}

/// Lists jots from a specific date or date range.
pub fn command_on(entries_dir: &PathBuf, date_spec: &str, compile: bool) -> Result<()> {
    let mut matches = Vec::new();
    let notebook_name = entries_dir.file_name().unwrap().to_string_lossy();

    if let Some((start_str, end_str)) = date_spec.split_once("..") {
        let start_date = NaiveDate::parse_from_str(start_str, "%Y-%m-%d")?;
        let end_date = NaiveDate::parse_from_str(end_str, "%Y-%m-%d")?;
        println!("Finding jots from {start_date} to {end_date}...");
        for entry in fs::read_dir(entries_dir)?.filter_map(Result::ok) {
            let filename = entry.file_name().to_string_lossy().to_string();
            if let Ok(date) = NaiveDate::parse_from_str(&filename[0..10], "%Y-%m-%d") {
                if date >= start_date && date <= end_date {
                    matches.push(parse_note_from_file(&entry.path(), &notebook_name)?);
                }
            }
        }
    } else {
        let date = NaiveDate::parse_from_str(date_spec, "%Y-%m-%d")?;
        return command_by_date_filter(entries_dir, date, compile);
    }
    matches.sort_by(|a, b| a.id.cmp(&b.id));
    if compile {
        helpers::compile_notes(matches)?
    } else {
        display_note_list(matches)
    }
    Ok(())
}

/// Displays the full content of a specific jot.
pub fn command_show(note_path: PathBuf, raw: bool) -> Result<()> {
    let content = helpers::read_note_file(&note_path)?;
    if raw && content.starts_with("---") {
        if let Some(end) = content.get(3..).and_then(|s| s.find("---")) {
            print!("{}", &content[(3 + end + 3)..].trim_start());
            return Ok(());
        }
    }
    print!("{content}");
    Ok(())
}

/// Deletes a specific jot with user confirmation.
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

/// Displays information and statistics about the journal.
/// This command is notebook-aware.
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

/// Formats and prints a list of notes from a global search.
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

/// Helper function to calculate stats for a given directory.
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

/// Helper function to print formatted stats.
fn print_stats(note_count: usize, tag_counts: HashMap<String, usize>, task_stats: TaskStats) {
    println!("Total jots: {note_count}");
    if !tag_counts.is_empty() {
        let mut sorted_tags: Vec<_> = tag_counts.into_iter().collect();
        sorted_tags.sort_by(|a, b| b.1.cmp(&a.1));
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

/// Exports a notebook to a specified file format.
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
            let filename = path.file_name().unwrap().to_str().unwrap();
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

/// Imports a notebook from a specified file.
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
        let outpath = new_notebook_path.join(file.name());
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

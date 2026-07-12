use std::env;
use std::fs;

use anyhow::Result;
use clap::Parser;
use rand::Rng;
use rustyline::completion::Completer;
use rustyline::config::Configurer;
use rustyline::CompletionType;
use rustyline::Editor;
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};

use crate::helpers;

use super::capture::command_down;
use super::info::calculate_stats_for_dir;

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

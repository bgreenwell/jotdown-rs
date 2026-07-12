mod cli;
mod commands;
mod helpers;

use anyhow::Result;
use clap::Parser;
use cli::Commands;
use std::path::PathBuf;

pub fn run_command(command: Commands, entries_dir: PathBuf) -> Result<()> {
    match command {
        Commands::Task { message } => commands::command_task(&entries_dir, &message)?,
        Commands::New {
            template,
            variables,
        } => commands::command_new(&entries_dir, template, variables)?,
        Commands::List {
            count,
            pinned,
            tasks,
            format,
        } => commands::command_list(&entries_dir, count, pinned, tasks, &format)?,
        Commands::Find {
            query,
            all,
            format,
            context,
        } => commands::command_find(&entries_dir, &query, all, &format, context)?,
        Commands::Tags { tags, format } => {
            commands::command_tags_filter(&entries_dir, &tags, &format)?
        }
        #[cfg(not(windows))]
        Commands::Select => commands::command_select(&entries_dir)?,
        Commands::Today { compile, format } => {
            commands::command_today(&entries_dir, compile, &format)?
        }
        Commands::Yesterday { compile, format } => {
            commands::command_yesterday(&entries_dir, compile, &format)?
        }
        Commands::Week { compile, format } => {
            commands::command_by_week(&entries_dir, compile, &format)?
        }
        Commands::On {
            date_spec,
            compile,
            format,
        } => commands::command_on(&entries_dir, &date_spec, compile, &format)?,
        Commands::Edit { id_prefix, last } => {
            let note_path = helpers::get_note_path_for_action(&entries_dir, id_prefix, last)?;
            commands::command_edit(note_path)?;
        }
        Commands::Show {
            id_prefix,
            last,
            raw,
        } => {
            let note_path = helpers::get_note_path_for_action(&entries_dir, id_prefix, last)?;
            commands::command_show(note_path, raw)?;
        }
        Commands::Delete {
            id_prefix,
            last,
            force,
        } => {
            let note_path = helpers::get_note_path_for_action(&entries_dir, id_prefix, last)?;
            commands::command_delete(note_path, force)?;
        }
        Commands::Pin { id_prefix, last } => commands::command_pin(&entries_dir, id_prefix, last)?,
        Commands::Unpin { id_prefix, last } => {
            commands::command_unpin(&entries_dir, id_prefix, last)?
        }
        Commands::Info(args) => commands::command_info(&entries_dir, args)?,
        Commands::Tag(args) => commands::command_tag(&entries_dir, args)?,
        Commands::Property { action } => commands::command_property(&entries_dir, action)?,
        Commands::Append { id, last, content } => {
            commands::command_append(&entries_dir, id, last, &content)?
        }
        Commands::Prepend { id, last, content } => {
            commands::command_prepend(&entries_dir, id, last, &content)?
        }
        Commands::Move {
            id,
            last,
            destination,
        } => commands::command_move(&entries_dir, id, last, &destination)?,
        Commands::Rename { id, last, new_name } => {
            commands::command_rename(&entries_dir, id, last, &new_name)?
        }
        Commands::Daily { message } => commands::command_daily(&entries_dir, &message)?,
        Commands::Notebook(args) => commands::command_notebook(args)?,
        Commands::Init { git, encrypt } => commands::command_init(git, encrypt)?,
        Commands::Sync => commands::command_sync()?,
        Commands::Decrypt { force } => commands::command_decrypt(force)?,
        Commands::Export(args) => commands::command_export(args)?,
        Commands::Import(args) => commands::command_import(args)?,
        Commands::Clean { all } => commands::command_clean(&entries_dir, all)?,

        // Only reachable when `shell`/`sh` is typed inside the interactive
        // shell; nested shells are not supported.
        Commands::Shell => {
            println!("Already in the jd shell. Type 'exit' or press Ctrl-D to leave.")
        }
    }

    Ok(())
}

/// The main entrypoint for the jd application.
fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        Some(command) => {
            if let Commands::Shell = command {
                commands::command_shell()?;
            } else {
                let entries_dir = helpers::get_active_entries_dir(cli.notebook)?;
                run_command(command, entries_dir)?;
            }
        }
        None => {
            if !cli.message.is_empty() {
                let message = cli.message.join(" ");
                let entries_dir = helpers::get_active_entries_dir(cli.notebook)?;
                commands::command_down(&entries_dir, &message, cli.tags)?;
            } else {
                // Check for piped input (stdin)
                use std::io::{self, IsTerminal, Read};
                if !io::stdin().is_terminal() {
                    let mut buffer = String::new();
                    io::stdin().read_to_string(&mut buffer)?;
                    if !buffer.trim().is_empty() {
                        let entries_dir = helpers::get_active_entries_dir(cli.notebook)?;
                        commands::command_down(&entries_dir, &buffer, cli.tags)?;
                        return Ok(());
                    }
                }
                println!("No message provided. Use 'jd <MESSAGE>' or a subcommand like 'jd list'.");
                println!("\nFor more information, try 'jd --help'");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::helpers::get_ordinal_suffix;

    #[test]
    fn test_ordinal_suffix() {
        assert_eq!(get_ordinal_suffix(1), "st");
        assert_eq!(get_ordinal_suffix(2), "nd");
        assert_eq!(get_ordinal_suffix(3), "rd");
        assert_eq!(get_ordinal_suffix(4), "th");
        assert_eq!(get_ordinal_suffix(10), "th");
        assert_eq!(get_ordinal_suffix(11), "th");
        assert_eq!(get_ordinal_suffix(12), "th");
        assert_eq!(get_ordinal_suffix(13), "th");
        assert_eq!(get_ordinal_suffix(21), "st");
        assert_eq!(get_ordinal_suffix(22), "nd");
        assert_eq!(get_ordinal_suffix(23), "rd");
        assert_eq!(get_ordinal_suffix(101), "st");
    }
}

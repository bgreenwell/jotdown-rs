// Declare the modules that make up the application.
mod cli;
mod commands;
mod helpers;

use anyhow::Result;
use clap::Parser;
use cli::Commands;
use std::path::PathBuf;

pub fn run_command(command: Commands, entries_dir: PathBuf) -> Result<()> {
    // This logic is now decoupled from where the command originates (main or shell)
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
        } => commands::command_list(&entries_dir, count, pinned, tasks)?,
        Commands::Find { query, all } => commands::command_find(&entries_dir, &query, all)?,
        Commands::Tags { tags } => commands::command_tags_filter(&entries_dir, &tags)?,
        #[cfg(not(windows))]
        Commands::Select => commands::command_select(&entries_dir)?,
        Commands::Today { compile } => commands::command_today(&entries_dir, compile)?,
        Commands::Yesterday { compile } => commands::command_yesterday(&entries_dir, compile)?,
        Commands::Week { compile } => commands::command_by_week(&entries_dir, compile)?,
        Commands::On { date_spec, compile } => {
            commands::command_on(&entries_dir, &date_spec, compile)?
        }
        Commands::Edit { id_prefix, last } => {
            let note_path = helpers::get_note_path_for_action(&entries_dir, id_prefix, last)?;
            commands::command_edit(note_path)?;
        }
        Commands::Show { id_prefix, last, raw } => {
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
        Commands::Notebook(args) => commands::command_notebook(args)?,
        Commands::Init { git, encrypt } => commands::command_init(git, encrypt)?,
        Commands::Sync => commands::command_sync()?,
        Commands::Decrypt { force } => commands::command_decrypt(force)?,
        Commands::Export(args) => commands::command_export(args)?,
        Commands::Import(args) => commands::command_import(args)?,
        // The shell command is handled in main() and will not be matched here.
        Commands::Shell => unreachable!(),
    }

    Ok(())
}

/// The main entrypoint for the jd application.
fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    // It either dispatches a command or handles the default jot action.
    match cli.command {
        Some(command) => {
            // The shell command is handled directly here before the dispatch.
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
                println!(
                    "No message provided. Use 'jd <MESSAGE>' or a subcommand like 'jd list'."
                );
                println!("\nFor more information, try 'jd --help'");
            }
        }
    }

    Ok(())
}

// Unit tests for helpers that are simple and don't require file system access.
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

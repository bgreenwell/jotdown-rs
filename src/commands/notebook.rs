use std::env;
use std::fs;

use anyhow::{bail, Result};

use crate::cli::{NotebookAction, NotebookArgs, ShellKind};
use crate::helpers::{get_notebooks_dir, is_valid_notebook_name};

pub fn command_notebook(args: NotebookArgs) -> Result<()> {
    match args.action {
        NotebookAction::New { name } => command_notebook_new(&name)?,
        NotebookAction::List => command_notebook_list()?,
        NotebookAction::Use { name, shell } => command_notebook_use(&name, shell)?,
        NotebookAction::Status => command_notebook_status()?,
    }
    Ok(())
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

fn command_notebook_use(name: &str, shell: Option<ShellKind>) -> Result<()> {
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

    // Prints a shell command for the user to evaluate. Quoting prevents any
    // shell interpretation of the notebook name (is_valid_notebook_name
    // already rejects the characters that would otherwise need escaping).
    match shell.unwrap_or(ShellKind::Bash) {
        ShellKind::Bash | ShellKind::Zsh => println!("export JD_ACTIVE_NOTEBOOK='{name}'"),
        ShellKind::Fish => println!("set -gx JD_ACTIVE_NOTEBOOK '{name}'"),
        ShellKind::Powershell => println!("$env:JD_ACTIVE_NOTEBOOK = '{name}'"),
    }
    Ok(())
}

fn command_notebook_status() -> Result<()> {
    let active_notebook = env::var("JD_ACTIVE_NOTEBOOK").unwrap_or_else(|_| "default".to_string());
    println!("Active notebook: {active_notebook}");
    Ok(())
}

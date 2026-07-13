use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use age::{secrecy::ExposeSecret, x25519, Decryptor, Identity};
use anyhow::{anyhow, bail, Result};
use chrono::Local;

use crate::helpers::{self, get_jd_dir_root, get_notebooks_dir};

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

/// Ensures the jd root's `.gitignore` excludes the sensitive files
/// (`identity.txt`, `config.toml`), appending any missing entries.
/// Returns `true` if the file was created or updated.
fn ensure_gitignore(root_dir: &Path) -> Result<bool> {
    let gitignore_path = root_dir.join(".gitignore");
    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    let mut updated = existing.clone();
    for entry in ["identity.txt", "config.toml"] {
        if !existing.lines().any(|line| line.trim() == entry) {
            if !updated.is_empty() && !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(entry);
            updated.push('\n');
        }
    }

    if updated != existing {
        fs::write(&gitignore_path, updated)?;
        return Ok(true);
    }
    Ok(false)
}

pub fn command_init(git: bool, encrypt: bool) -> Result<()> {
    let root_dir = get_jd_dir_root()?;
    println!("jd directory is at: {root_dir:?}");

    if git {
        let git_dir = root_dir.join(".git");
        if git_dir.exists() {
            println!("Git repository already exists in {root_dir:?}");
            if ensure_gitignore(&root_dir)? {
                println!("Updated .gitignore to exclude sensitive files.");
            }
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

            if ensure_gitignore(&root_dir)? {
                println!("Created .gitignore to exclude sensitive files.");
            }

            let has_commits = Command::new("git")
                .current_dir(&root_dir)
                .args(["rev-parse", "HEAD"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if !has_commits {
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
            // The private key must never be world-readable.
            helpers::write_private_file(
                &identity_path,
                identity.to_string().expose_secret().as_bytes(),
            )?;
            println!("Generated new encryption identity at: {identity_path:?}");
            println!("\nIMPORTANT: Back this file up somewhere safe!");

            let config_path = root_dir.join("config.toml");
            let config_str = format!("recipient = \"{recipient}\"");
            fs::write(config_path, config_str)?;
            println!("Saved public key to config.toml.");
            println!("\nYour public key (recipient) is: {recipient}");

            // A long-lived process (the interactive shell) must pick up the
            // newly enabled encryption on its next note operation.
            helpers::invalidate_crypto_cache();
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

    if !force
        && !helpers::confirm(
            "This will permanently decrypt all notes in ALL notebooks and remove your identity file. This action cannot be undone. Continue?",
        )?
    {
        println!("Decryption aborted.");
        return Ok(());
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
    // A long-lived process (the interactive shell) must stop encrypting on
    // its next note operation now that the identity/config are gone.
    helpers::invalidate_crypto_cache();
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

    // Never sync the private key or config, even when .gitignore is missing
    // (e.g. the repo was created by hand rather than `jd init --git`).
    run_git(
        &root_dir,
        &[
            "rm",
            "--cached",
            "--ignore-unmatch",
            "--quiet",
            "identity.txt",
            "config.toml",
        ],
    )?;

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

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::helpers::{self, get_notebooks_dir};

pub fn command_clean(entries_dir: &Path, all: bool) -> Result<()> {
    let (target_label, dirs): (String, Vec<PathBuf>) = if all {
        let notebooks_dir = get_notebooks_dir()?;
        let dirs = fs::read_dir(&notebooks_dir)?
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect();
        ("ALL notebooks".to_string(), dirs)
    } else {
        let name = entries_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        (
            format!("notebook '{name}'"),
            vec![entries_dir.to_path_buf()],
        )
    };

    if !helpers::confirm(&format!(
        "This will permanently delete all notes in {target_label}. Are you sure?"
    ))? {
        println!("Aborted.");
        return Ok(());
    }

    if !helpers::confirm("Really sure? This cannot be undone.")? {
        println!("Aborted.");
        return Ok(());
    }

    let mut total = 0usize;
    for dir in &dirs {
        for entry in fs::read_dir(dir)?.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                fs::remove_file(&path)?;
                total += 1;
            }
        }
    }

    if total == 1 {
        println!("Deleted 1 note.");
    } else {
        println!("Deleted {total} notes.");
    }
    Ok(())
}

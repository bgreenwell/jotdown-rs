use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Result};
use chrono::Local;

use crate::helpers::{
    self, get_note_path_for_action, get_notebooks_dir, is_valid_notebook_name, parse_note_from_file,
};

pub fn command_append(
    entries_dir: &Path,
    id: Option<String>,
    last: Option<usize>,
    content: &str,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id, last)?;
    let notebook_name = helpers::notebook_name(entries_dir);
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
    let notebook_name = helpers::notebook_name(entries_dir);
    let note = parse_note_from_file(&note_path, &notebook_name)?;

    let prefix = if !content.ends_with('\n') {
        format!("{}\n", content)
    } else {
        content.to_string()
    };

    // Read the raw file and insert the new content directly above the first
    // body line (past the frontmatter block, if any), so plain-text notes are
    // not silently wrapped in a frontmatter header.
    let mut raw = helpers::read_note_file(&note_path)?;
    let insert_at = match raw.strip_prefix("---").and_then(|s| s.find("\n---")) {
        Some(rel) => {
            // Past the closing `---`, its trailing newline, and any blank
            // lines, so the new text lands at the start of the body — not
            // on the delimiter line itself.
            let mut idx = 3 + rel + 4;
            while raw[idx..].starts_with('\n') {
                idx += 1;
            }
            idx
        }
        None => 0,
    };
    raw.insert_str(insert_at, &prefix);

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

    if !is_valid_notebook_name(destination) {
        bail!("Invalid destination notebook name: '{}'.", destination);
    }
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
    let notebook_name = helpers::notebook_name(entries_dir);
    let mut note = parse_note_from_file(&note_path, &notebook_name)?;

    let mut dest_filename = new_name.to_string();
    if !dest_filename.ends_with(".md") {
        dest_filename.push_str(".md");
    }

    if !helpers::is_valid_note_filename(&dest_filename) {
        bail!(
            "Invalid name: '{}'. Names cannot contain path separators or traversal components.",
            new_name
        );
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
        note.save()?;
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

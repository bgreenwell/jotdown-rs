use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Result};
use chrono::Local;

use crate::helpers::{
    self, get_note_path_for_action, get_notebooks_dir, is_valid_notebook_name, note_id_from_path,
    parse_note_from_file,
};

pub fn command_append(
    entries_dir: &Path,
    id: Option<String>,
    last: Option<usize>,
    content: &str,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id, last)?;

    let mut current_content = helpers::read_note_file(&note_path)?;
    if !current_content.ends_with('\n') {
        current_content.push('\n');
    }
    current_content.push_str(content);
    if !content.ends_with('\n') {
        current_content.push('\n');
    }

    helpers::write_note_file(&note_path, &current_content)?;
    println!(
        "Successfully appended to jot '{}'.",
        note_id_from_path(&note_path)
    );
    Ok(())
}

pub fn command_prepend(
    entries_dir: &Path,
    id: Option<String>,
    last: Option<usize>,
    content: &str,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id, last)?;

    let prefix = if !content.ends_with('\n') {
        format!("{}\n", content)
    } else {
        content.to_string()
    };

    // Read the raw file and insert the new content directly above the first
    // body line (past the frontmatter block, if any), so plain-text notes are
    // not silently wrapped in a frontmatter header.
    let mut raw = helpers::read_note_file(&note_path)?;
    let insert_at = helpers::frontmatter_body_offset(&raw).unwrap_or(0);
    raw.insert_str(insert_at, &prefix);

    helpers::write_note_file(&note_path, &raw)?;
    println!(
        "Successfully prepended to jot '{}'.",
        note_id_from_path(&note_path)
    );
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
            "{fence}\ntitle = \"Daily Note - {}\"\n{fence}\n\n{}\n",
            today,
            message,
            fence = helpers::FRONTMATTER_FENCE
        );
        helpers::write_note_file(&note_path, &content)?;
        println!("Created daily note: {}", filename);
    }
    Ok(())
}

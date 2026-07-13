use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use chrono::Local;
use uuid::Uuid;

use crate::helpers::{self, get_templates_dir, Frontmatter};

/// Builds a fresh note path for the current moment, appending a `-N` suffix
/// if a note from the same second already exists.
fn unique_note_path(entries_dir: &Path) -> PathBuf {
    let base = Local::now().format("%Y-%m-%d-%H%M%S").to_string();
    let candidate = entries_dir.join(format!("{base}.md"));
    if !candidate.exists() {
        return candidate;
    }
    let mut n = 1u32;
    loop {
        let candidate = entries_dir.join(format!("{base}-{n}.md"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
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
            content.push_str(helpers::FRONTMATTER_FENCE);
            content.push('\n');
            content.push_str(&fm_str);
            content.push_str(helpers::FRONTMATTER_FENCE);
            content.push_str("\n\n");
        }
    }
    content.push_str(message);
    println!("Jotting down: \"{message}\"");
    let file_path = unique_note_path(entries_dir);
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
    let file_path = unique_note_path(entries_dir);
    helpers::write_note_file(&file_path, &task_content)?;
    println!("Successfully saved to {file_path:?}");
    Ok(())
}

pub fn command_new(
    entries_dir: &Path,
    template_name: Option<String>,
    variables: Vec<(String, String)>,
) -> Result<()> {
    // Fail early, before creating the note file, if no editor is available.
    helpers::get_editor()?;
    let now = Local::now();
    let file_path = unique_note_path(entries_dir);
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
    helpers::edit_note_file(&file_path)?;
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
        note_path.file_name().unwrap(),
        editor
    );
    helpers::edit_note_file(&note_path)?;
    println!("Finished editing {:?}.", note_path.file_name().unwrap());
    Ok(())
}

use std::path::Path;

use anyhow::Result;

use crate::helpers::{self, get_note_path_for_action, parse_note_from_file};

/// Toggles the pinned status of a note. Shared by `command_pin` and
/// `command_unpin` to avoid duplicating the find/parse/modify/save logic.
fn command_toggle_pin_status(
    entries_dir: &Path,
    id_prefix: Option<String>,
    last: Option<usize>,
    pin: bool,
) -> Result<()> {
    let note_path = get_note_path_for_action(entries_dir, id_prefix, last)?;
    let notebook_name = helpers::notebook_name(entries_dir);
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
    note.save()?;

    println!(
        "Successfully {} jot '{}'.",
        if pin { "pinned" } else { "unpinned" },
        note.id
    );

    Ok(())
}

pub fn command_pin(
    entries_dir: &Path,
    id_prefix: Option<String>,
    last: Option<usize>,
) -> Result<()> {
    command_toggle_pin_status(entries_dir, id_prefix, last, true)
}

pub fn command_unpin(
    entries_dir: &Path,
    id_prefix: Option<String>,
    last: Option<usize>,
) -> Result<()> {
    command_toggle_pin_status(entries_dir, id_prefix, last, false)
}

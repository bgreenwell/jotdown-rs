use std::path::Path;

use anyhow::Result;

use crate::cli::{TagAction, TagArgs};
use crate::helpers::{self, get_note_path_for_action, parse_note_from_file};

pub fn command_tag(entries_dir: &Path, args: TagArgs) -> Result<()> {
    let target = match &args.action {
        TagAction::Add { target, .. }
        | TagAction::Remove { target, .. }
        | TagAction::Set { target, .. } => target,
    };

    let note_path = get_note_path_for_action(entries_dir, target.id_prefix.clone(), target.last)?;
    let notebook_name = helpers::notebook_name(entries_dir);
    let mut note = parse_note_from_file(&note_path, &notebook_name)?;

    match args.action {
        TagAction::Add { tags, .. } => {
            for tag in tags {
                if !note.frontmatter.tags.contains(&tag) {
                    note.frontmatter.tags.push(tag);
                }
            }
            println!("Added tags to '{}'.", note.id);
        }
        TagAction::Remove { tags, .. } => {
            note.frontmatter.tags.retain(|t| !tags.contains(t));
            println!("Removed tags from '{}'.", note.id);
        }
        TagAction::Set { tags, .. } => {
            note.frontmatter.tags = tags;
            println!("Set tags for '{}'.", note.id);
        }
    }
    note.frontmatter.tags.sort();
    note.frontmatter.tags.dedup();
    note.save()?;
    Ok(())
}

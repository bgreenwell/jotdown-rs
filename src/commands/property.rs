use std::path::Path;

use anyhow::{bail, Result};

use crate::cli::{OutputFormat, PropertyAction};
use crate::helpers::{self, get_note_path_for_action, parse_note_from_file};

/// Frontmatter keys owned by `Frontmatter` struct fields. Writing them
/// through the free-form `fields` table would produce a duplicate or
/// wrongly-typed key that makes the note unparseable.
fn check_reserved_property(name: &str) -> Result<()> {
    if name == "tags" || name == "pinned" {
        bail!(
            "'{}' is managed by jd and cannot be modified as a property. Use `jd tag` or `jd pin`/`jd unpin` instead.",
            name
        );
    }
    Ok(())
}

pub fn command_property(entries_dir: &Path, action: PropertyAction) -> Result<()> {
    match action {
        PropertyAction::Set {
            target,
            name,
            value,
        } => {
            check_reserved_property(&name)?;
            let note_path = get_note_path_for_action(entries_dir, target.id, target.last)?;
            let notebook_name = helpers::notebook_name(entries_dir);
            let mut note = parse_note_from_file(&note_path, &notebook_name)?;

            note.frontmatter
                .fields
                .insert(name.clone(), toml::Value::String(value.clone()));
            note.save()?;
            println!(
                "Successfully set property '{}' for jot '{}'.",
                name, note.id
            );
        }
        PropertyAction::Get {
            target,
            name,
            format,
        } => {
            let note_path = get_note_path_for_action(entries_dir, target.id, target.last)?;
            let notebook_name = helpers::notebook_name(entries_dir);
            let note = parse_note_from_file(&note_path, &notebook_name)?;

            if let Some(value) = note.frontmatter.fields.get(name.as_str()) {
                if format == OutputFormat::Json {
                    println!("{}", serde_json::to_string(value)?);
                } else {
                    match value {
                        toml::Value::String(s) => println!("{s}"),
                        toml::Value::Integer(i) => println!("{i}"),
                        toml::Value::Float(f) => println!("{f}"),
                        toml::Value::Boolean(b) => println!("{b}"),
                        toml::Value::Datetime(d) => println!("{d}"),
                        v => println!("{}", serde_json::to_string(v)?),
                    }
                }
            } else {
                bail!("Property '{}' not found for jot '{}'.", name, note.id);
            }
        }
        PropertyAction::Delete { target, name } => {
            check_reserved_property(&name)?;
            let note_path = get_note_path_for_action(entries_dir, target.id, target.last)?;
            let notebook_name = helpers::notebook_name(entries_dir);
            let mut note = parse_note_from_file(&note_path, &notebook_name)?;

            if note.frontmatter.fields.remove(name.as_str()).is_some() {
                note.save()?;
                println!(
                    "Successfully deleted property '{}' from jot '{}'.",
                    name, note.id
                );
            } else {
                println!("Property '{}' not found on jot '{}'.", name, note.id);
            }
        }
    }
    Ok(())
}

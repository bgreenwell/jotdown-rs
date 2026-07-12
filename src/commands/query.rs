use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{Datelike, Local, NaiveDate};

use crate::cli::OutputFormat;
use crate::helpers::{self, get_notebooks_dir};

// Conditionally compile everything related to skim (fuzzy finder).
#[cfg(not(windows))]
use {
    crossbeam_channel::unbounded,
    skim::prelude::*,
    std::{borrow::Cow, sync::Arc},
};

pub fn command_list(
    entries_dir: &Path,
    count: Option<usize>,
    pinned: bool,
    tasks: bool,
    format: OutputFormat,
) -> Result<()> {
    let num_to_list = count.unwrap_or(10);
    let notebook_name = helpers::notebook_name(entries_dir);
    let mut notes = helpers::parse_notes_in_dir(entries_dir, &notebook_name)?;

    if pinned {
        notes.retain(|note| note.frontmatter.pinned);
        if format == OutputFormat::Human {
            println!("Showing pinned jots:");
        }
    }

    if tasks {
        notes.retain(|note| note.tasks.iter().any(|t| !t.completed));
        // `notes` is already oldest-first (see `parse_notes_in_dir`); reverse
        // for newest-first display, keeping same-second collisions in the
        // right order.
        notes.reverse();
        notes.truncate(num_to_list);

        if format == OutputFormat::Human {
            if notes.is_empty() {
                println!("\nNo jots with incomplete tasks found.");
            } else {
                println!("\n{:<22} TASKS", "ID");
                println!("{:-<22} {:-<50}", "", "");
                for note in notes {
                    let pending: Vec<&str> = note
                        .tasks
                        .iter()
                        .filter(|t| !t.completed)
                        .map(|t| t.description.as_str())
                        .collect();
                    println!("{:<22} [ ] {}", note.id, pending[0]);
                    for task in &pending[1..] {
                        println!("{:<22} [ ] {}", "", task);
                    }
                }
            }
        } else {
            helpers::display_formatted_note_list(notes, format)?;
        }
        return Ok(());
    }

    // `notes` is already oldest-first (see `parse_notes_in_dir`); reverse for
    // newest-first display, keeping same-second collisions in the right order.
    notes.reverse();
    notes.truncate(num_to_list);

    helpers::display_formatted_note_list(notes, format)?;
    Ok(())
}

#[cfg(not(windows))]
pub fn command_select(entries_dir: &Path) -> Result<()> {
    struct NoteItem {
        id: String,
        display_text: String,
        content: String,
    }

    impl SkimItem for NoteItem {
        fn text(&self) -> Cow<'_, str> {
            Cow::Borrowed(&self.display_text)
        }

        fn output(&self) -> Cow<'_, str> {
            Cow::Borrowed(&self.id)
        }

        fn preview(&self, _context: PreviewContext) -> ItemPreview {
            ItemPreview::Text(self.content.clone())
        }
    }

    let notebook_name = helpers::notebook_name(entries_dir);
    let mut notes = helpers::parse_notes_in_dir(entries_dir, &notebook_name)?;
    notes.reverse(); // newest first

    let options = SkimOptionsBuilder::default()
        .multi(false)
        .reverse(true)
        .preview(Some(""))
        .build()?;

    type SkimChannel = (Sender<Arc<dyn SkimItem>>, Receiver<Arc<dyn SkimItem>>);

    let (tx, rx): SkimChannel = unbounded();

    for note in notes {
        let display_text = format!(
            "{} | {}",
            note.id,
            note.content.lines().next().unwrap_or("").trim()
        );
        let item = NoteItem {
            id: note.id,
            display_text,
            content: note.content,
        };
        let _ = tx.send(Arc::new(item));
    }
    drop(tx);

    let skim_output = Skim::run_with(&options, Some(rx));

    if let Some(output) = skim_output {
        if output.is_abort {
            return Ok(());
        }
        for item in output.selected_items.iter() {
            println!("{}", item.output());
        }
    }

    Ok(())
}

pub fn display_global_find_list(notes: Vec<helpers::Note>) {
    if notes.is_empty() {
        println!("\nNo jots found.");
        return;
    }
    println!("\n{:<22} {:<18} FIRST LINE OF CONTENT", "ID", "NOTEBOOK");
    println!("{:-<22} {:-<18} {:-<50}", "", "", "");
    for note in notes {
        let first_line = note.content.lines().next().unwrap_or("").trim();
        println!("{:<22} {:<18} {}", note.id, note.notebook, first_line);
    }
}

pub fn command_find(
    entries_dir: &Path,
    query: &str,
    all: bool,
    format: OutputFormat,
    context: bool,
) -> Result<()> {
    if format == OutputFormat::Human {
        println!("Searching for \"{query}\" in your jots...");
    }
    let mut matches = Vec::new();

    let query_lower = query.to_lowercase();
    if all {
        let notebooks_dir = get_notebooks_dir()?;
        for notebook_entry in fs::read_dir(notebooks_dir)?.filter_map(Result::ok) {
            if notebook_entry.path().is_dir() {
                let notebook_path = notebook_entry.path();
                let notebook_name = helpers::notebook_name(&notebook_path);
                for note in helpers::parse_notes_in_dir(&notebook_path, &notebook_name)? {
                    if note.content.to_lowercase().contains(&query_lower) {
                        matches.push(note);
                    }
                }
            }
        }
        if format == OutputFormat::Human {
            if context {
                helpers::display_search_results_with_context(matches, query);
            } else {
                display_global_find_list(matches);
            }
        } else {
            helpers::display_formatted_note_list(matches, format)?;
        }
    } else {
        let notebook_name = helpers::notebook_name(entries_dir);
        for note in helpers::parse_notes_in_dir(entries_dir, &notebook_name)? {
            if note.content.to_lowercase().contains(&query_lower) {
                matches.push(note);
            }
        }
        if format == OutputFormat::Human && context {
            helpers::display_search_results_with_context(matches, query);
        } else {
            helpers::display_formatted_note_list(matches, format)?;
        }
    }
    Ok(())
}

pub fn command_tags_filter(
    entries_dir: &Path,
    tags: &[String],
    format: OutputFormat,
) -> Result<()> {
    let notebook_name = helpers::notebook_name(entries_dir);
    let mut matches = helpers::parse_notes_in_dir(entries_dir, &notebook_name)?;
    matches.retain(|note| tags.iter().all(|t| note.frontmatter.tags.contains(t)));
    helpers::display_formatted_note_list(matches, format)?;
    Ok(())
}

pub fn command_by_date_filter(
    entries_dir: &Path,
    date: NaiveDate,
    compile: bool,
    format: OutputFormat,
) -> Result<()> {
    let date_prefix = date.format("%Y-%m-%d").to_string();
    if format == OutputFormat::Human {
        println!("Finding jots from {date_prefix}...");
    }
    let notebook_name = helpers::notebook_name(entries_dir);
    let mut matches = helpers::parse_notes_in_dir(entries_dir, &notebook_name)?;
    matches.retain(|note| note.id.starts_with(&date_prefix));
    if compile {
        helpers::compile_notes(matches)?
    } else {
        helpers::display_formatted_note_list(matches, format)?;
    }
    Ok(())
}

pub fn command_today(entries_dir: &Path, compile: bool, format: OutputFormat) -> Result<()> {
    command_by_date_filter(entries_dir, Local::now().date_naive(), compile, format)
}

pub fn command_yesterday(entries_dir: &Path, compile: bool, format: OutputFormat) -> Result<()> {
    let yesterday = Local::now().date_naive() - chrono::Duration::days(1);
    command_by_date_filter(entries_dir, yesterday, compile, format)
}

pub fn command_by_week(entries_dir: &Path, compile: bool, format: OutputFormat) -> Result<()> {
    let today = Local::now().date_naive();
    let week_start = today - chrono::Duration::days(today.weekday().num_days_from_sunday() as i64);
    if format == OutputFormat::Human {
        println!("Finding jots from this week (starting {week_start})...");
    }
    let notebook_name = helpers::notebook_name(entries_dir);
    let mut matches = helpers::parse_notes_in_dir(entries_dir, &notebook_name)?;
    matches.retain(|note| {
        note_date_from_id(&note.id)
            .map(|date| date >= week_start && date <= today)
            .unwrap_or(false)
    });
    if compile {
        helpers::compile_notes(matches)?
    } else {
        helpers::display_formatted_note_list(matches, format)?;
    }
    Ok(())
}

/// Extracts the date from a jd-generated note ID (`YYYY-MM-DD…`), if any.
fn note_date_from_id(id: &str) -> Option<NaiveDate> {
    if id.len() < 10 {
        return None;
    }
    NaiveDate::parse_from_str(&id[0..10], "%Y-%m-%d").ok()
}

pub fn command_on(
    entries_dir: &Path,
    date_spec: &str,
    compile: bool,
    format: OutputFormat,
) -> Result<()> {
    let notebook_name = helpers::notebook_name(entries_dir);

    let (start_date, end_date) = if let Some((start_str, end_str)) = date_spec.split_once("..") {
        (
            NaiveDate::parse_from_str(start_str, "%Y-%m-%d")?,
            NaiveDate::parse_from_str(end_str, "%Y-%m-%d")?,
        )
    } else {
        let date = NaiveDate::parse_from_str(date_spec, "%Y-%m-%d")?;
        return command_by_date_filter(entries_dir, date, compile, format);
    };

    if format == OutputFormat::Human {
        println!("Finding jots from {start_date} to {end_date}...");
    }
    let mut matches = helpers::parse_notes_in_dir(entries_dir, &notebook_name)?;
    matches.retain(|note| {
        note_date_from_id(&note.id)
            .map(|date| date >= start_date && date <= end_date)
            .unwrap_or(false)
    });
    if compile {
        helpers::compile_notes(matches)?
    } else {
        helpers::display_formatted_note_list(matches, format)?;
    }
    Ok(())
}

pub fn command_show(note_path: PathBuf, raw: bool) -> Result<()> {
    let content = helpers::read_note_file(&note_path)?;

    if raw {
        if let Some(stripped) = content.strip_prefix("---") {
            if let Some(rel) = stripped.find("\n---") {
                print!("{}", stripped[(rel + 4)..].trim_start());
                return Ok(());
            }
        }
        print!("{content}");
    } else {
        use syntect::easy::HighlightLines;
        use syntect::highlighting::{Style, ThemeSet};
        use syntect::parsing::SyntaxSet;
        use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

        // Try syntax highlighting for markdown
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let syntax = ps.find_syntax_by_extension("md").unwrap();
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);

        for line in LinesWithEndings::from(&content) {
            let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();
            let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
            print!("{}", escaped);
        }
        println!("\x1b[0m"); // Reset colors
    }
    Ok(())
}

pub fn command_delete(note_path: PathBuf, force: bool) -> Result<()> {
    let filename = note_path.file_name().unwrap().to_string_lossy();
    if !force && !helpers::confirm(&format!("Are you sure you want to delete '{filename}'?"))? {
        println!("Deletion aborted.");
        return Ok(());
    }
    fs::remove_file(&note_path)?;
    println!("Successfully deleted '{filename}'.");
    Ok(())
}

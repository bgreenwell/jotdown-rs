use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::cli::InfoArgs;
use crate::helpers::{self, get_notebooks_dir, parse_notes_in_dir, TaskStats};

pub fn command_info(entries_dir: &Path, args: InfoArgs) -> Result<()> {
    if !args.paths && !args.stats {
        println!(
            "Please provide a flag to the info command, e.g., `jd info --paths` or `jd info --stats`"
        );
        println!("\nFor more information, try '--help'");
        return Ok(());
    }
    if args.paths {
        println!("--- jd paths ---");
        let active_notebook =
            env::var("JD_ACTIVE_NOTEBOOK").unwrap_or_else(|_| "default".to_string());
        println!("Root Directory:   {:?}", helpers::get_jd_dir_root()?);
        println!("Notebooks Root:   {:?}", helpers::get_notebooks_dir()?);
        println!("Active Notebook:  {active_notebook}");
        println!("Entries:          {entries_dir:?}");
        println!("Templates:        {:?}", helpers::get_templates_dir()?);
    }
    if args.stats {
        println!("\n--- jd stats ---");

        if args.all {
            // Stats for all notebooks
            let notebooks_dir = get_notebooks_dir()?;
            let mut total_notes = 0;
            let mut all_tags: HashMap<String, usize> = HashMap::new();
            let mut total_task_stats = TaskStats::default();

            for entry in fs::read_dir(notebooks_dir)?.filter_map(Result::ok) {
                if entry.path().is_dir() {
                    let notebook_path = entry.path();
                    let (note_count, tag_counts, task_stats) =
                        calculate_stats_for_dir(&notebook_path)?;
                    total_notes += note_count;
                    for (tag, count) in tag_counts {
                        *all_tags.entry(tag).or_insert(0) += count;
                    }
                    total_task_stats.completed += task_stats.completed;
                    total_task_stats.pending += task_stats.pending;
                }
            }
            println!("Stats for all notebooks combined:");
            print_stats(total_notes, all_tags, total_task_stats);
        } else {
            // Stats for the active notebook only
            let active_notebook_name = entries_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("default");
            println!("Stats for active notebook: '{active_notebook_name}'");
            let (note_count, tag_counts, task_stats) = calculate_stats_for_dir(entries_dir)?;
            print_stats(note_count, tag_counts, task_stats);
        }
    }
    Ok(())
}

/// Computes note/tag/task counts for a notebook directory. Also used by the
/// interactive shell's startup banner.
pub(crate) fn calculate_stats_for_dir(
    dir: &Path,
) -> Result<(usize, HashMap<String, usize>, TaskStats)> {
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    let mut task_stats = TaskStats::default();
    let notebook_name = helpers::notebook_name(dir);

    let notes = parse_notes_in_dir(dir, &notebook_name)?;
    let note_count = notes.len();
    for note in notes {
        for tag in note.frontmatter.tags {
            *tag_counts.entry(tag).or_insert(0) += 1;
        }
        for task in note.tasks {
            if task.completed {
                task_stats.completed += 1;
            } else {
                task_stats.pending += 1;
            }
        }
    }
    Ok((note_count, tag_counts, task_stats))
}

fn print_stats(note_count: usize, tag_counts: HashMap<String, usize>, task_stats: TaskStats) {
    println!("Total jots: {note_count}");
    if !tag_counts.is_empty() {
        let mut sorted_tags: Vec<_> = tag_counts.into_iter().collect();
        sorted_tags.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        sorted_tags.truncate(5);
        println!("\nMost common tags:");
        for (tag, count) in sorted_tags {
            println!("  - {tag} ({count})");
        }
    }
    if task_stats.completed > 0 || task_stats.pending > 0 {
        println!("\nTask Summary:");
        println!("  - Completed: {}", task_stats.completed);
        println!("  - Pending:   {}", task_stats.pending);
    }
}

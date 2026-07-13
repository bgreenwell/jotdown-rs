use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "jd", version, about = "A minimalist, command-line journal.")]
pub struct Cli {
    /// The subcommand to execute. If no subcommand is provided, `jd` will
    /// treat the input as a new note for the default action.
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Tags to add to a new jot (comma-separated, e.g. -t rust,notes).
    #[arg(long, short, value_delimiter = ',')]
    pub tags: Option<Vec<String>>,

    /// Run a command in a specific notebook without switching the active one.
    #[arg(long, global = true)]
    pub notebook: Option<String>,

    /// The message for a new jot. This captures all positional arguments
    /// that are not part of a subcommand.
    pub message: Vec<String>,
}

/// Parses a key-value pair from the command line.
fn parse_key_val(s: &str) -> Result<(String, String), String> {
    s.split_once('=')
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .ok_or_else(|| format!("invalid key-value pair: {s}"))
}

/// Output format for commands that list or query notes. A `ValueEnum`
/// instead of a free `String` so an unrecognized value (e.g. a typo like
/// `--format yaml`) is a hard clap error instead of silently falling back
/// to human-readable output.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[value(rename_all = "lower")]
pub enum OutputFormat {
    Human,
    Json,
    Csv,
}

/// The shell syntax `jd notebook use` should emit its environment-variable
/// assignment in.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[value(rename_all = "lower")]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

/// Targets a note by a positional ID prefix or the `--last` flag.
/// Used by commands where the ID prefix reads naturally as the first
/// argument (`pin`, `unpin`, `edit`, `show`, `delete`).
#[derive(Args, Debug)]
pub struct PositionalTarget {
    /// The prefix of the jot ID. Must be unique.
    #[arg(group = "target", required = true)]
    pub id_prefix: Option<String>,
    /// Target the Nth most recent jot (e.g., --last=1 or just --last).
    #[arg(long, short, group = "target", num_args(0..=1), default_missing_value = "1", require_equals = true)]
    pub last: Option<usize>,
}

/// Targets a note via `--id`/`-i` or the `--last` flag. Used by commands
/// that take another required positional argument of their own
/// (`append`, `prepend`, `move`, `rename`, `property`).
#[derive(Args, Debug)]
pub struct IdFlagTarget {
    /// The prefix of the jot ID to target.
    #[arg(long, short, group = "target", required = true)]
    pub id: Option<String>,
    /// Target the Nth most recent jot.
    #[arg(long, short, group = "target", num_args(0..=1), default_missing_value = "1", require_equals = true)]
    pub last: Option<usize>,
}

/// Targets a note via `--id-prefix`/`-p` or the `--last` flag. Used by
/// `jd tag` subcommands.
#[derive(Args, Debug)]
pub struct IdPrefixFlagTarget {
    /// The ID prefix of the note to target.
    #[arg(long, short = 'p', group = "target", required = true)]
    pub id_prefix: Option<String>,
    /// Target the Nth most recent note.
    #[arg(long, short, group = "target", num_args(0..=1), default_missing_value = "1", require_equals = true)]
    pub last: Option<usize>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create a new jot using an editor, optionally with a template.
    New {
        /// The name of the template to use from the templates directory.
        #[arg(long, short)]
        template: Option<String>,

        /// Set custom variables for the template (e.g., -v key=value).
        #[arg(long, short = 'v', value_parser = parse_key_val)]
        variables: Vec<(String, String)>,
    },
    /// List the most recent jots.
    List {
        /// The number of jots to list. Defaults to 10.
        count: Option<usize>,
        /// A flag to show only pinned jots.
        #[arg(long, short)]
        pinned: bool,
        /// A flag to show only jots containing incomplete tasks.
        #[arg(long)] // Or short('t') if you prefer
        tasks: bool,
        /// The output format (human, json, csv).
        #[arg(long, short, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Pin a jot.
    Pin {
        #[command(flatten)]
        target: PositionalTarget,
    },
    /// Unpin a jot.
    Unpin {
        #[command(flatten)]
        target: PositionalTarget,
    },
    /// Create a new jot formatted as a task.
    #[command(aliases = ["t", "todo"])] // Optional aliases
    Task {
        /// The content of the task.
        #[arg(required = true)]
        message: String,
    },
    /// Find jots by searching their content.
    Find {
        /// Text to search for, case-insensitively.
        #[arg(required = true)]
        query: String,

        /// Search across all notebooks.
        #[arg(long, short)] // Or --global if you prefer
        all: bool,

        /// The output format (human, json, csv).
        #[arg(long, short, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,

        /// Show surrounding context for matches.
        #[arg(long, short)]
        context: bool,
    },
    /// Interactively select a note using a fuzzy finder.
    #[command(alias = "s")]
    #[cfg(not(windows))] // Fuzzy finder is not supported on Windows
    Select,
    /// List jots that have specific tags.
    Tags {
        /// Tags to filter by (can be comma-separated or space-separated).
        #[arg(required = true, value_delimiter = ',')]
        tags: Vec<String>,

        /// The output format (human, json, csv).
        #[arg(long, short, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Append text to an existing jot.
    Append {
        #[command(flatten)]
        target: IdFlagTarget,
        /// The text to append.
        #[arg(required = true)]
        content: String,
    },
    /// Prepend text to an existing jot (below the frontmatter).
    Prepend {
        #[command(flatten)]
        target: IdFlagTarget,
        /// The text to prepend.
        #[arg(required = true)]
        content: String,
    },
    /// Move a jot to another notebook.
    Move {
        #[command(flatten)]
        target: IdFlagTarget,
        /// The destination notebook name.
        #[arg(required = true)]
        destination: String,
    },
    /// Rename a jot (updates the filename).
    Rename {
        #[command(flatten)]
        target: IdFlagTarget,
        /// The new name for the jot (used for the filename).
        #[arg(required = true)]
        new_name: String,
    },
    /// Append text to today's daily note.
    #[command(alias = "da")]
    Daily {
        /// The text to append to the daily note.
        #[arg(required = true)]
        message: String,
    },
    /// List jots from today.
    Today {
        /// Compile all of today's jots into a single summary.
        #[arg(long, short)]
        compile: bool,
        /// The output format (human, json, csv).
        #[arg(long, short, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// List jots from yesterday.
    Yesterday {
        #[arg(long, short)]
        compile: bool,
        /// The output format (human, json, csv).
        #[arg(long, short, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// List jots from this week.
    Week {
        #[arg(long, short)]
        compile: bool,
        /// The output format (human, json, csv).
        #[arg(long, short, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// List jots from a specific date or date range.
    On {
        /// The date (YYYY-MM-DD) or range (YYYY-MM-DD..YYYY-MM-DD) to filter by.
        #[arg(required = true)]
        date_spec: String,
        #[arg(long, short)]
        compile: bool,
        /// The output format (human, json, csv).
        #[arg(long, short, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Open an existing jot in the default editor.
    Edit {
        #[command(flatten)]
        target: PositionalTarget,
    },
    /// Display the full content of a jot in the terminal.
    Show {
        #[command(flatten)]
        target: PositionalTarget,
        /// Print only the note body, stripping the frontmatter.
        #[arg(long, short)]
        raw: bool,
    },
    /// Delete a jot with confirmation.
    #[command(alias = "rm")]
    Delete {
        #[command(flatten)]
        target: PositionalTarget,
        /// Force deletion without a confirmation prompt.
        #[arg(long, short)]
        force: bool,
    },
    /// Display information about your jd setup.
    Info(InfoArgs),
    /// Manage tags on an existing jot.
    Tag(TagArgs),
    /// Manage arbitrary properties in the jot's frontmatter.
    Property {
        #[command(subcommand)]
        action: PropertyAction,
    },
    /// Manage notebooks for organizing jots.
    #[command(alias = "n")]
    Notebook(NotebookArgs),
    /// Initialize the jd directory, optionally with Git and/or encryption.
    Init {
        /// Initialize the jd directory as a Git repository.
        #[arg(long)]
        git: bool,
        /// Encrypt the jd directory with a new identity.
        #[arg(long)]
        encrypt: bool,
    },
    /// Commit and push changes to a remote Git repository.
    Sync,
    /// Permanently decrypt all notes in the jd directory.
    Decrypt {
        /// Force decryption without a confirmation prompt.
        #[arg(long, short)]
        force: bool,
    },
    /// Export a notebook to a ZIP archive or a JSON file.
    Export(ExportArgs),

    /// Import a notebook from a ZIP archive or a JSON file.
    Import(ImportArgs),

    /// Enter the interactive jd shell.
    #[command(alias = "sh")]
    Shell,

    /// Delete all notes in the active notebook (or all notebooks with --all).
    Clean {
        /// Delete all notes across every notebook, not just the active one.
        #[arg(long, short)]
        all: bool,
    },
}

#[derive(Args, Debug)]
pub struct NotebookArgs {
    /// The notebook management action to perform.
    #[command(subcommand)]
    pub action: NotebookAction,
}

#[derive(Subcommand, Debug)]
pub enum NotebookAction {
    /// Create a new, empty notebook.
    New {
        /// The name for the new notebook.
        #[arg(required = true)]
        name: String,
    },
    /// List all available notebooks.
    #[command(alias = "ls")]
    List,
    /// Print the command to switch the active notebook for the current shell session.
    ///
    /// Usage: eval $(jd notebook use <NAME>)              # bash/zsh
    ///        jd notebook use <NAME> --shell fish | source # fish
    ///        jd notebook use <NAME> --shell powershell | Invoke-Expression
    Use {
        /// The name of the notebook to switch to.
        #[arg(required = true)]
        name: String,
        /// The shell syntax to emit. Defaults to bash/zsh syntax.
        #[arg(long, value_enum)]
        shell: Option<ShellKind>,
    },
    /// Show the currently active notebook.
    Status,
}

#[derive(Args, Debug)]
pub struct InfoArgs {
    /// Display the paths used by jd for storage and templates.
    #[arg(long)]
    pub paths: bool,
    /// Display statistics about your jots, like total count and tag frequency.
    #[arg(long)]
    pub stats: bool,
    /// Show stats for all notebooks combined.
    #[arg(long, requires = "stats")]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct TagArgs {
    /// The tag management action to perform.
    #[command(subcommand)]
    pub action: TagAction,
}

#[derive(Subcommand, Debug)]
pub enum PropertyAction {
    /// Set a property value.
    Set {
        #[command(flatten)]
        target: IdFlagTarget,
        /// The name of the property.
        name: String,
        /// The value to set.
        value: String,
    },
    /// Get a property value.
    Get {
        #[command(flatten)]
        target: IdFlagTarget,
        /// The name of the property.
        name: String,
        /// The output format (human, json, csv).
        #[arg(long, short, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Delete a property.
    Delete {
        #[command(flatten)]
        target: IdFlagTarget,
        /// The name of the property.
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum TagAction {
    /// Add one or more tags to a jot.
    Add {
        #[command(flatten)]
        target: IdPrefixFlagTarget,
        /// The tags to add.
        #[arg(required = true, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// Remove one or more tags from a jot.
    #[command(alias = "rm")]
    Remove {
        #[command(flatten)]
        target: IdPrefixFlagTarget,
        /// The tags to remove.
        #[arg(required = true, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// Overwrite all existing tags on a jot.
    Set {
        #[command(flatten)]
        target: IdPrefixFlagTarget,
        /// The new set of tags.
        #[arg(required = true, value_delimiter = ',')]
        tags: Vec<String>,
    },
}

#[derive(Args, Debug)]
pub struct ExportArgs {
    /// The name of the notebook to export.
    #[arg(required = true)]
    pub notebook_name: String,

    /// The format for the export (zip or json).
    #[arg(long, short, default_value = "zip")]
    pub format: String,

    /// The path for the output file.
    #[arg(long, short, required = true)]
    pub output: PathBuf,
}

#[derive(Args, Debug)]
pub struct ImportArgs {
    /// The path to the file to import.
    #[arg(required = true)]
    pub file_path: PathBuf,
}

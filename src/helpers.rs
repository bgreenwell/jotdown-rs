use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use age::{
    x25519::{Identity, Recipient},
    Encryptor,
};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use which::which;

use crate::cli::OutputFormat;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub description: String,
    pub completed: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TaskStats {
    pub pending: usize,
    pub completed: usize,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Frontmatter {
    /// A list of tags associated with the note.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    // Represents the pinned status of a note.
    // The `skip_serializing_if` attribute is an optimization that prevents
    // `pinned: false` from being written to files, keeping them clean.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,

    /// Arbitrary additional fields in the frontmatter.
    #[serde(flatten)]
    pub fields: toml::Table,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub path: PathBuf,
    pub notebook: String,
    pub frontmatter: Frontmatter,
    pub content: String,
    pub tasks: Vec<Task>,
}

impl Note {
    /// Serializes the frontmatter and content back to this note's path,
    /// encrypting on write if encryption is enabled. Used after any
    /// in-memory edit to `frontmatter` or `content`.
    pub fn save(&self) -> Result<()> {
        let frontmatter_str = toml::to_string(&self.frontmatter)?;
        let content = format!("---\n{frontmatter_str}---\n\n{}", self.content);
        write_note_file(&self.path, &content)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct Config {
    /// The public key (`age` recipient) used for encrypting notes.
    recipient: Option<String>,
}

/// Honors the `$JD_DIR` environment variable if set, otherwise uses the platform-specific
/// user config directory. It also triggers the one-time migration for legacy installations.
pub fn get_jd_dir_root() -> Result<PathBuf> {
    let path = match env::var("JD_DIR") {
        Ok(val) => PathBuf::from(val),
        Err(_) => dirs::config_dir()
            .with_context(|| "Could not find a valid config directory.")?
            .join("jd"),
    };
    if !path.exists() {
        fs::create_dir_all(&path)?;
    }

    // Call the migration check every time the root is requested.
    // This function is cheap and will only perform the migration once.
    handle_legacy_migration(&path)?;

    Ok(path)
}

/// Handles the one-time migration from the old `entries` directory structure.
/// If it finds a legacy `entries` directory and no new `notebooks` directory,
/// it moves the old directory to `notebooks/default` to ensure backward compatibility.
fn handle_legacy_migration(root_dir: &Path) -> Result<()> {
    let legacy_entries_dir = root_dir.join("entries");
    let notebooks_dir = root_dir.join("notebooks");

    if legacy_entries_dir.exists() && !notebooks_dir.exists() {
        // Print to stderr so the one-time migration notice can never pollute
        // machine-readable output like `--format json`.
        eprintln!("jd has been updated to support notebooks!");
        eprintln!("Migrating your existing notes to the 'default' notebook...");

        fs::create_dir_all(&notebooks_dir)
            .with_context(|| "Failed to create new notebooks directory during migration.")?;

        let default_notebook_path = notebooks_dir.join("default");
        fs::rename(&legacy_entries_dir, &default_notebook_path).with_context(|| {
            format!("Failed to move notes from {legacy_entries_dir:?} to {default_notebook_path:?}")
        })?;
        eprintln!("Migration complete. Your notes are now in the 'default' notebook.");
    }
    Ok(())
}

pub fn get_notebooks_dir() -> Result<PathBuf> {
    let root_dir = get_jd_dir_root()?;
    let notebooks_dir = root_dir.join("notebooks");
    if !notebooks_dir.exists() {
        fs::create_dir_all(&notebooks_dir)?;
    }
    Ok(notebooks_dir)
}

/// Validates a notebook name: rejects empty names, dot components, and any
/// path-separator or shell-special characters, so a name can never escape the
/// notebooks directory or inject into a shell.
pub fn is_valid_notebook_name(name: &str) -> bool {
    if name.is_empty() || name == "." || name == ".." {
        return false;
    }
    !name.chars().any(|c| {
        matches!(
            c,
            '/' | '\\'
                | '"'
                | '\''
                | '`'
                | '$'
                | '!'
                | '&'
                | '|'
                | ';'
                | '('
                | ')'
                | '<'
                | '>'
                | '\0'
        )
    })
}

/// Validates a note filename: it must be a plain file name with no path
/// separators or traversal components, so it can never escape the notebook.
pub fn is_valid_note_filename(name: &str) -> bool {
    !name.is_empty() && Path::new(name).file_name().map(|f| f == name) == Some(true)
}

/// Gets the `entries` directory for the currently active notebook.
/// This is the core of the multi-notebook feature. It resolves the path based on this priority:
/// 1. The `--notebook` command-line flag (passed in as `notebook_override`).
/// 2. The `JD_ACTIVE_NOTEBOOK` environment variable.
/// 3. The "default" notebook if neither is set.
///
/// It will automatically create the notebook directory if it doesn't exist.
pub fn get_active_entries_dir(notebook_override: Option<String>) -> Result<PathBuf> {
    let notebooks_root = get_notebooks_dir()?;

    let notebook_name = if let Some(name) = notebook_override {
        name
    } else if let Ok(name) = env::var("JD_ACTIVE_NOTEBOOK") {
        name
    } else {
        "default".to_string()
    };

    if !is_valid_notebook_name(&notebook_name) {
        bail!("Invalid notebook name: '{}'.", notebook_name);
    }

    let entries_dir = notebooks_root.join(notebook_name);
    if !entries_dir.exists() {
        fs::create_dir_all(&entries_dir)?;
    }
    Ok(entries_dir)
}

pub fn get_templates_dir() -> Result<PathBuf> {
    let root_dir = get_jd_dir_root()?;
    let templates_dir = root_dir.join("templates");
    if !templates_dir.exists() {
        fs::create_dir_all(&templates_dir)?;
    }
    Ok(templates_dir)
}

/// Determines which command-line editor to use.
/// It prioritizes the `$EDITOR` environment variable, then falls back to a list
/// of common editors (`vim`, `nvim`, `nano`, `notepad.exe`).
/// # Errors
/// Returns an error if no suitable editor can be found.
pub fn get_editor() -> Result<String> {
    if let Ok(editor) = env::var("EDITOR") {
        if !editor.is_empty() {
            return Ok(editor);
        }
    }
    #[cfg(unix)]
    let fallbacks = ["vim", "nvim", "nano"];
    #[cfg(windows)]
    let fallbacks = ["notepad.exe"];
    #[cfg(not(any(unix, windows)))]
    let fallbacks: [&str; 0] = [];

    for editor in fallbacks {
        if which(editor).is_ok() {
            return Ok(editor.to_string());
        }
    }
    bail!("Could not find a default editor. Please set the $EDITOR environment variable.")
}

/// Caches `config.toml` and `identity.txt` for the lifetime of the process,
/// so bulk operations (import, list, find) don't re-stat and re-parse them
/// on every single note. The interactive shell is a single long-lived
/// process, so `init --encrypt` and `decrypt` explicitly call
/// `invalidate_crypto_cache()` after changing either file on disk.
type CryptoState = (Config, Option<String>);
static CRYPTO_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<CryptoState>>> =
    std::sync::OnceLock::new();

/// Clears the cached config/identity state. Must be called after any command
/// that creates, removes, or otherwise changes `config.toml`/`identity.txt`
/// (currently `init --encrypt` and `decrypt`), so a long-lived process like
/// the interactive shell picks up the change on its next note operation.
pub fn invalidate_crypto_cache() {
    if let Some(lock) = CRYPTO_CACHE.get() {
        *lock.lock().unwrap() = None;
    }
}

/// Returns the cached `(config, identity file contents)` pair, loading and
/// caching it on first use.
fn cached_crypto_state() -> Result<(Config, Option<String>)> {
    let lock = CRYPTO_CACHE.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = lock.lock().unwrap();
    if let Some(state) = &*guard {
        return Ok(state.clone());
    }

    let root_dir = get_jd_dir_root()?;
    let config_path = root_dir.join("config.toml");
    let config: Config = if config_path.exists() {
        toml::from_str(&fs::read_to_string(config_path)?)?
    } else {
        Config::default()
    };
    let identity_path = root_dir.join("identity.txt");
    let identity_text = if identity_path.exists() {
        Some(fs::read_to_string(identity_path)?)
    } else {
        None
    };

    let state = (config, identity_text);
    *guard = Some(state.clone());
    Ok(state)
}

/// Returns the configured encryption recipient (public key), if any.
pub fn encryption_recipient() -> Result<Option<String>> {
    Ok(cached_crypto_state()?.0.recipient)
}

/// Writes `contents` to `path` readable only by the owner (0o600 on Unix).
/// Used for the age private key and for plaintext temp files during editing.
pub fn write_private_file(path: &Path, contents: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(contents)?;
    }
    #[cfg(not(unix))]
    fs::write(path, contents)?;
    Ok(())
}

/// Opens a note in the user's editor. When encryption is enabled, the note is
/// decrypted to a private temporary file for editing and re-encrypted after
/// the editor exits — the editor never sees ciphertext, and the note is never
/// left unencrypted on disk.
pub fn edit_note_file(path: &Path) -> Result<()> {
    let editor = get_editor()?;

    if encryption_recipient()?.is_none() {
        let status = std::process::Command::new(&editor).arg(path).status()?;
        if !status.success() {
            bail!("Editor exited with a non-zero status.");
        }
        return Ok(());
    }

    let tmp_dir = get_jd_dir_root()?.join("tmp");
    fs::create_dir_all(&tmp_dir)?;
    let filename = path
        .file_name()
        .ok_or_else(|| anyhow!("Invalid note path: {path:?}"))?;
    let tmp_path = tmp_dir.join(filename);
    let content = read_note_file(path)?;
    write_private_file(&tmp_path, content.as_bytes())?;

    let result = (|| {
        let status = std::process::Command::new(&editor)
            .arg(&tmp_path)
            .status()?;
        if !status.success() {
            bail!("Editor exited with a non-zero status.");
        }
        let edited = fs::read_to_string(&tmp_path)?;
        write_note_file(path, &edited)
    })();
    // Best-effort removal of the plaintext temp file, even if editing failed.
    let _ = fs::remove_file(&tmp_path);
    result
}

/// Writes content to a note file, encrypting it if encryption is enabled.
pub fn write_note_file(path: &Path, content: &str) -> Result<()> {
    let (config, _) = cached_crypto_state()?;

    if let Some(recipient_str) = config.recipient {
        let recipient: Recipient = recipient_str
            .parse()
            .map_err(|e| anyhow!("Failed to parse recipient from config: {}", e))?;
        let encrypted_bytes = {
            let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)]);
            let mut encrypted = vec![];
            let mut writer = encryptor
                .ok_or_else(|| anyhow!("Failed to create encryptor: recipient list was empty"))?
                .wrap_output(&mut encrypted)?;
            writer.write_all(content.as_bytes())?;
            writer.finish()?;
            encrypted
        };
        fs::write(path, encrypted_bytes)?;
    } else {
        fs::write(path, content)?;
    }
    Ok(())
}

/// Reads content from a note file, decrypting it if necessary.
pub fn read_note_file(path: &Path) -> Result<String> {
    let (_, identity_text) = cached_crypto_state()?;
    let file_bytes = fs::read(path)?;

    if let Some(identity_str) =
        identity_text.filter(|_| file_bytes.starts_with(b"age-encryption.org"))
    {
        let identity: Identity = identity_str
            .parse()
            .map_err(|_| anyhow!("Failed to parse identity file."))?;
        let decryptor = age::Decryptor::new(&file_bytes as &[u8])?;
        let mut decrypted_bytes = vec![];
        if let age::Decryptor::Recipients(reader) = decryptor {
            let identities: Vec<Box<dyn age::Identity>> = vec![Box::new(identity)];
            reader
                .decrypt(identities.iter().map(|i| i.as_ref()))?
                .read_to_end(&mut decrypted_bytes)?;
        } else {
            bail!("Expected recipients-based encryption");
        }
        Ok(String::from_utf8(decrypted_bytes)?)
    } else {
        Ok(String::from_utf8(file_bytes)?)
    }
}

/// Sort key that orders same-second collision files (`…-HHMMSS-1.md`) after
/// their base file (`…-HHMMSS.md`); plain byte order compares `-` before `.`
/// and would put the *later* note first.
pub fn note_sort_key(path: &Path) -> (String, u32) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if let Some((base, suffix)) = stem.rsplit_once('-') {
        if let Ok(n) = suffix.parse::<u32>() {
            // Only treat it as a collision suffix when the base ends in the
            // 6-digit time component of jd's generated filenames.
            if base.len() >= 6 && base[base.len() - 6..].chars().all(|c| c.is_ascii_digit()) {
                return (base.to_string(), n + 1);
            }
        }
    }
    (stem.to_string(), 0)
}

/// Returns the note files (`*.md`) in a notebook directory, sorted by
/// creation order. Foreign files (`.DS_Store`, editor swap files) and
/// subdirectories are ignored so they can never break note commands.
pub fn note_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md")
        })
        .collect();
    files.sort_by_key(|path| note_sort_key(path));
    Ok(files)
}

/// Parses every note in a notebook directory in creation order, skipping
/// (with a warning on stderr) any file that cannot be parsed, so one bad
/// file cannot break a whole command.
pub fn parse_notes_in_dir(dir: &Path, notebook_name: &str) -> Result<Vec<Note>> {
    let mut notes = Vec::new();
    for path in note_files(dir)? {
        match parse_note_from_file(&path, notebook_name) {
            Ok(note) => notes.push(note),
            Err(e) => eprintln!(
                "Warning: skipping {:?}: {e:#}",
                path.file_name().unwrap_or_default()
            ),
        }
    }
    Ok(notes)
}

/// Parses only the `limit` most recent notes in a notebook directory,
/// returned newest-first. Since note filenames sort in creation order, the
/// file list can be trimmed to the requested count *before* parsing (and,
/// with encryption enabled, decrypting) each one — the difference between
/// O(limit) and O(n) decrypt operations for a large notebook.
pub fn parse_newest_notes_in_dir(
    dir: &Path,
    notebook_name: &str,
    limit: usize,
) -> Result<Vec<Note>> {
    let files = note_files(dir)?;
    let start = files.len().saturating_sub(limit);
    let mut notes = Vec::new();
    for path in files[start..].iter().rev() {
        match parse_note_from_file(path, notebook_name) {
            Ok(note) => notes.push(note),
            Err(e) => eprintln!(
                "Warning: skipping {:?}: {e:#}",
                path.file_name().unwrap_or_default()
            ),
        }
    }
    Ok(notes)
}

/// Derives a note's ID from its filename (the filename minus a single
/// trailing `.md`), without reading or decrypting the file. Used where only
/// the ID is needed, to avoid a redundant decrypt of the note's content.
pub fn note_id_from_path(path: &Path) -> String {
    let filename = path.file_name().unwrap().to_string_lossy().to_string();
    filename
        .strip_suffix(".md")
        .unwrap_or(&filename)
        .to_string()
}

/// Parses a file into a `Note` struct, separating frontmatter from content.
pub fn parse_note_from_file(path: &Path, notebook_name: &str) -> Result<Note> {
    let id = note_id_from_path(path);
    let file_content =
        read_note_file(path).with_context(|| format!("Could not read file: {path:?}"))?;

    let (frontmatter, content_str) = if let Some(after_open) = file_content.strip_prefix("---") {
        // Find the closing `---` only at the start of a line, so `---` inside
        // a TOML value (e.g. `title = "foo --- bar"`) does not terminate early.
        if let Some(rel) = after_open.find("\n---") {
            let frontmatter_str = &after_open[..rel];
            let content_part = after_open[(rel + 4)..].trim().to_string();
            let fm: Frontmatter = toml::from_str(frontmatter_str)
                .with_context(|| format!("Failed to parse TOML frontmatter in {path:?}"))?;
            (fm, content_part)
        } else {
            (Frontmatter::default(), file_content.clone())
        }
    } else {
        (Frontmatter::default(), file_content.clone())
    };

    let mut tasks = Vec::new();
    for line in content_str.lines() {
        let trimmed_line = line.trim();
        if let Some(stripped) = trimmed_line.strip_prefix("- [ ] ") {
            tasks.push(Task {
                description: stripped.to_string(),
                completed: false,
            });
        } else if let Some(stripped) = trimmed_line.strip_prefix("- [x] ") {
            tasks.push(Task {
                description: stripped.to_string(),
                completed: true,
            });
        }
    }

    Ok(Note {
        id,
        path: path.to_path_buf(),
        notebook: notebook_name.to_string(),
        frontmatter,
        content: content_str,
        tasks,
    })
}

/// Determines which note to act on based on user input (ID prefix or `--last` flag).
pub fn get_note_path_for_action(
    entries_dir: &Path,
    id_prefix: Option<String>,
    last: Option<usize>,
) -> Result<PathBuf> {
    if let Some(index) = last {
        if id_prefix.is_some() {
            bail!("Cannot use an ID prefix and the --last flag at the same time.");
        }
        find_note_by_index_from_end(entries_dir, index)
    } else if let Some(prefix) = id_prefix {
        find_unique_note_by_prefix(entries_dir, &prefix)
    } else {
        // Clap's `required = true` on the target group should prevent this.
        bail!("Provide a note ID prefix or the --last flag.");
    }
}

pub fn display_note_list(notes: Vec<Note>) {
    if notes.is_empty() {
        println!("\nNo jots found.");
        return;
    }
    println!("\n{:<22} FIRST LINE OF CONTENT", "ID");
    println!("{:-<22} {:-<50}", "", "");
    for note in notes {
        let first_line = note.content.lines().next().unwrap_or("").trim();
        println!("{:<22} {}", note.id, first_line);
    }
}

fn csv_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

pub fn display_formatted_note_list(notes: Vec<Note>, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&notes)?;
            println!("{}", json);
        }
        OutputFormat::Csv => {
            println!("id,notebook,tags,content_snippet");
            for note in notes {
                let tags = note.frontmatter.tags.join("|");
                let snippet = note
                    .content
                    .replace('\n', " ")
                    .chars()
                    .take(50)
                    .collect::<String>();
                println!(
                    "{},{},{},{}",
                    csv_quote(&note.id),
                    csv_quote(&note.notebook),
                    csv_quote(&tags),
                    csv_quote(&snippet),
                );
            }
        }
        OutputFormat::Human => {
            display_note_list(notes);
        }
    }
    Ok(())
}

pub fn display_search_results_with_context(notes: Vec<Note>, query: &str) {
    if notes.is_empty() {
        println!("\nNo matches found.");
        return;
    }
    let query_lower = query.to_lowercase();
    for note in notes {
        let mut first = true;
        for (i, line) in note.content.lines().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                if first {
                    println!("\n--- {} ({}) ---", note.id, note.notebook);
                    first = false;
                }
                println!("{:>4}: {}", i + 1, line.trim());
            }
        }
    }
}

pub fn compile_notes(notes: Vec<Note>) -> Result<()> {
    for note in notes {
        println!("---\n\n# {}\n\n{}", note.id, note.content);
    }
    Ok(())
}

pub fn find_unique_note_by_prefix(entries_dir: &Path, prefix: &str) -> Result<PathBuf> {
    let mut matches = Vec::new();
    for path in note_files(entries_dir)? {
        if path
            .file_name()
            .map(|name| name.to_string_lossy().starts_with(prefix))
            .unwrap_or(false)
        {
            matches.push(path);
        }
    }
    if matches.is_empty() {
        bail!("No jot found with the prefix '{}'", prefix);
    } else if matches.len() > 1 {
        bail!(
            "Prefix '{}' is not unique. Multiple jots found:\n{}",
            prefix,
            matches
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n")
        );
    } else {
        Ok(matches.into_iter().next().unwrap())
    }
}

/// Returns a notebook directory's name, used as the display "notebook"
/// field on its notes.
pub fn notebook_name(dir: &Path) -> std::borrow::Cow<'_, str> {
    dir.file_name().unwrap_or_default().to_string_lossy()
}

/// Prompts the user with `[y/N]` and returns whether they answered `y`
/// (case-insensitive). Any other input, including empty input, is "no".
pub fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt} [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().eq_ignore_ascii_case("y"))
}

pub fn get_ordinal_suffix(n: usize) -> &'static str {
    if (11..=13).contains(&(n % 100)) {
        "th"
    } else {
        match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    }
}

pub fn find_note_by_index_from_end(entries_dir: &Path, index: usize) -> Result<PathBuf> {
    if index == 0 {
        bail!("--last index must be 1 or greater.");
    }
    let entries = note_files(entries_dir)?;
    let total_jots = entries.len();
    if total_jots == 0 {
        bail!("No jots exist to act upon.");
    }
    if index > total_jots {
        bail!(
            "Index out of bounds. You asked for the {}{} last jot, but only {} exist.",
            index,
            get_ordinal_suffix(index),
            total_jots
        );
    }
    let target_index = total_jots - index;
    entries
        .get(target_index)
        .cloned()
        .with_context(|| "Failed to get entry at calculated index.")
}

#[cfg(test)]
mod tests {
    use super::get_ordinal_suffix;

    #[test]
    fn test_ordinal_suffix() {
        assert_eq!(get_ordinal_suffix(1), "st");
        assert_eq!(get_ordinal_suffix(2), "nd");
        assert_eq!(get_ordinal_suffix(3), "rd");
        assert_eq!(get_ordinal_suffix(4), "th");
        assert_eq!(get_ordinal_suffix(10), "th");
        assert_eq!(get_ordinal_suffix(11), "th");
        assert_eq!(get_ordinal_suffix(12), "th");
        assert_eq!(get_ordinal_suffix(13), "th");
        assert_eq!(get_ordinal_suffix(21), "st");
        assert_eq!(get_ordinal_suffix(22), "nd");
        assert_eq!(get_ordinal_suffix(23), "rd");
        assert_eq!(get_ordinal_suffix(101), "st");
    }
}

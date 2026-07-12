use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use zip::write::{FileOptions, ZipWriter};
use zip::ZipArchive;

use crate::cli::{ExportArgs, ImportArgs};
use crate::helpers::{self, get_notebooks_dir, is_valid_notebook_name};

#[derive(Serialize, Deserialize, Debug)]
struct JsonExport {
    notebook_name: String,
    jots: Vec<JsonJot>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonJot {
    filename: String,
    content: String,
}

pub fn command_export(args: ExportArgs) -> Result<()> {
    let notebooks_dir = get_notebooks_dir()?;
    let notebook_path = notebooks_dir.join(&args.notebook_name);

    if !notebook_path.is_dir() {
        bail!("Notebook '{}' not found.", args.notebook_name);
    }

    if helpers::encryption_recipient()?.is_some() {
        eprintln!(
            "Warning: exports are written as PLAINTEXT. Your encrypted notes will be decrypted in the output file."
        );
    }

    match args.format.as_str() {
        "zip" => export_to_zip(&notebook_path, &args.output)?,
        "json" => export_to_json(&notebook_path, &args.notebook_name, &args.output)?,
        _ => bail!(
            "Unsupported format: '{}'. Please use 'zip' or 'json'.",
            args.format
        ),
    }

    println!(
        "Successfully exported notebook '{}' to {:?}",
        args.notebook_name, args.output
    );
    Ok(())
}

fn export_to_zip(notebook_path: &Path, output_path: &Path) -> Result<()> {
    let file = File::create(output_path)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Zstd);

    for path in helpers::note_files(notebook_path)? {
        let filename = path.file_name().unwrap().to_string_lossy();
        zip.start_file(filename, options)?;
        let content = helpers::read_note_file(&path)?;
        zip.write_all(content.as_bytes())?;
    }
    zip.finish()?;
    Ok(())
}

fn export_to_json(notebook_path: &Path, notebook_name: &str, output_path: &Path) -> Result<()> {
    let mut jots = Vec::new();
    for path in helpers::note_files(notebook_path)? {
        jots.push(JsonJot {
            filename: path.file_name().unwrap().to_string_lossy().to_string(),
            content: helpers::read_note_file(&path)?,
        });
    }

    let export_data = JsonExport {
        notebook_name: notebook_name.to_string(),
        jots,
    };

    let json_string = serde_json::to_string_pretty(&export_data)?;
    fs::write(output_path, json_string)?;
    Ok(())
}

pub fn command_import(args: ImportArgs) -> Result<()> {
    let extension = args
        .file_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    match extension {
        "zip" => import_from_zip(&args.file_path)?,
        "json" => import_from_json(&args.file_path)?,
        _ => bail!(
            "Unsupported file type: '{:?}'. Please use a '.zip' or '.json' file.",
            args.file_path
        ),
    }
    Ok(())
}

fn import_from_zip(file_path: &Path) -> Result<()> {
    let file = File::open(file_path)?;
    let mut archive = ZipArchive::new(file)?;
    let notebook_name = file_path.file_stem().unwrap().to_string_lossy().to_string();
    let notebooks_dir = get_notebooks_dir()?;
    let new_notebook_path = notebooks_dir.join(&notebook_name);

    if new_notebook_path.exists() {
        bail!("A notebook named '{}' already exists. Please rename the zip file or the existing notebook.", notebook_name);
    }
    fs::create_dir_all(&new_notebook_path)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        // Directory entries would otherwise be created as empty files.
        if file.is_dir() {
            continue;
        }
        // Reject entries with path traversal components or absolute paths.
        let entry_name = file.name().to_string();
        let safe_name = Path::new(&entry_name)
            .file_name()
            .ok_or_else(|| anyhow!("Invalid entry name in archive: '{}'", entry_name))?
            .to_owned();
        let outpath = new_notebook_path.join(safe_name);
        let mut content = String::new();
        file.read_to_string(&mut content)
            .with_context(|| format!("Archive entry '{entry_name}' is not valid UTF-8 text"))?;
        // Route through write_note_file so notes are encrypted when enabled.
        helpers::write_note_file(&outpath, &content)?;
    }

    println!("Successfully imported notebook '{notebook_name}' from {file_path:?}");
    Ok(())
}

fn import_from_json(file_path: &Path) -> Result<()> {
    let json_string = fs::read_to_string(file_path)?;
    let export_data: JsonExport = serde_json::from_str(&json_string)?;
    if !is_valid_notebook_name(&export_data.notebook_name) {
        bail!(
            "Invalid notebook name in import file: '{}'.",
            export_data.notebook_name
        );
    }
    let notebooks_dir = get_notebooks_dir()?;
    let new_notebook_path = notebooks_dir.join(&export_data.notebook_name);

    if new_notebook_path.exists() {
        bail!(
            "A notebook named '{}' already exists.",
            export_data.notebook_name
        );
    }
    fs::create_dir_all(&new_notebook_path)?;

    for jot in export_data.jots {
        // Reject filenames with path traversal components or absolute paths.
        if !helpers::is_valid_note_filename(&jot.filename) {
            bail!("Invalid filename in import file: '{}'.", jot.filename);
        }
        let jot_path = new_notebook_path.join(jot.filename);
        helpers::write_note_file(&jot_path, &jot.content)?;
    }

    println!(
        "Successfully imported notebook '{}' from {:?}",
        export_data.notebook_name, file_path
    );
    Ok(())
}

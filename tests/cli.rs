use assert_cmd::Command;
use chrono::Local;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use tempfile::{tempdir, TempDir};

type TestResult = Result<(), Box<dyn std::error::Error>>;

// Helper function to set up a test environment
// This creates the `notebooks/default` structure to ensure all
// existing tests run in the default notebook context.
fn setup() -> (TempDir, PathBuf) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let jd_dir = temp_dir.path().to_path_buf();
    // All tests will now run inside the 'default' notebook by default.
    fs::create_dir_all(jd_dir.join("notebooks").join("default"))
        .expect("Failed to create default notebook dir");
    (temp_dir, jd_dir)
}

#[test]
fn test_default_jot_creation() -> TestResult {
    let (_temp_dir, jd_dir) = setup();

    let mut cmd = Command::cargo_bin("jd")?;
    cmd.arg("a default note")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Jotting down:"));

    // Verify the note was created in the `default` notebook.
    let entries_dir = jd_dir.join("notebooks").join("default");
    assert_eq!(fs::read_dir(entries_dir)?.count(), 1);
    Ok(())
}

#[test]
fn test_misspelled_command_is_a_note() -> TestResult {
    let (_temp_dir, jd_dir) = setup();

    let mut cmd = Command::cargo_bin("jd")?;
    cmd.arg("lisy")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Jotting down: \"lisy\""));

    let entries_dir = jd_dir.join("notebooks").join("default");
    assert_eq!(
        fs::read_dir(entries_dir)?.count(),
        1,
        "Expected a note to be created from the typo"
    );

    Ok(())
}

#[test]
fn test_tagged_jot_creation() -> TestResult {
    let (_temp_dir, jd_dir) = setup();

    let mut cmd = Command::cargo_bin("jd")?;
    cmd.arg("a tagged note")
        .args(["--tags", "rust,project"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    let entries_dir = jd_dir.join("notebooks").join("default");
    let entry_path = fs::read_dir(entries_dir)?.next().unwrap()?.path();
    let content = fs::read_to_string(entry_path)?;

    assert!(content.contains("tags ="));
    assert!(content.contains("\"rust\""));
    assert!(content.contains("\"project\""));
    Ok(())
}

#[test]
fn test_list_and_find() -> TestResult {
    let (_temp_dir, jd_dir) = setup();

    Command::cargo_bin("jd")?
        .arg("note about a unique_keyword")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    Command::cargo_bin("jd")?
        .arg("list")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("unique_keyword"));

    Command::cargo_bin("jd")?
        .arg("find")
        .arg("unique_keyword")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("unique_keyword"));

    Ok(())
}

#[test]
fn test_show_edit_delete() -> TestResult {
    let (temp_dir, jd_dir) = setup();

    // Create notes and get the first one's ID
    Command::cargo_bin("jd")?
        .arg("first note")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_secs(1));
    Command::cargo_bin("jd")?
        .arg("second note")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    let mut entries: Vec<_> = fs::read_dir(jd_dir.join("notebooks").join("default"))?
        .map(|r| r.unwrap().path())
        .collect();
    entries.sort();
    let first_note_id = entries[0].file_stem().unwrap().to_str().unwrap();

    // Test edit with ID prefix
    let script_path;
    #[cfg(unix)]
    {
        script_path = temp_dir.path().join("editor.sh");
        fs::write(&script_path, "#!/bin/sh\necho 'edited content' > \"$1\"")?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(windows)]
    {
        script_path = temp_dir.path().join("editor.bat");
        fs::write(&script_path, "@echo edited content > %1")?;
    }

    Command::cargo_bin("jd")?
        .arg("edit")
        .arg(first_note_id)
        .env("JD_DIR", &jd_dir)
        .env("EDITOR", &script_path)
        .assert()
        .success();

    // Verify edit
    let first_note_content = fs::read_to_string(&entries[0])?;
    assert!(first_note_content.contains("edited content"));

    // Test delete with --last
    Command::cargo_bin("jd")?
        .arg("delete")
        .arg("--last")
        .arg("--force")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    assert_eq!(
        fs::read_dir(jd_dir.join("notebooks").join("default"))?.count(),
        1,
        "Expected one jot to remain."
    );

    Ok(())
}

#[test]
fn test_info_command() -> TestResult {
    let (_temp_dir, jd_dir) = setup();

    Command::cargo_bin("jd")?
        .arg("info")
        .arg("--paths")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Active Notebook:  default"))
        .stdout(predicate::str::contains("Entries:"));

    Command::cargo_bin("jd")?
        .arg("info")
        .arg("--stats")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Stats for active notebook: 'default'",
        ))
        .stdout(predicate::str::contains("Total jots: 0"));

    Ok(())
}

#[test]
fn test_tag_management() -> TestResult {
    let (_temp_dir, jd_dir) = setup();
    let entries_dir = jd_dir.join("notebooks").join("default");

    Command::cargo_bin("jd")?
        .arg("note for tags")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    // add — bare --last (no =1) must work the same as --last=1
    Command::cargo_bin("jd")?
        .args(["tag", "add", "--last", "rust,testing"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    let note_path = fs::read_dir(&entries_dir)?.next().unwrap()?.path();
    let content = fs::read_to_string(&note_path)?;
    assert!(content.contains("\"rust\""), "rust tag should be present");
    assert!(
        content.contains("\"testing\""),
        "testing tag should be present"
    );

    // rm — bare --last removes one tag
    Command::cargo_bin("jd")?
        .args(["tag", "rm", "--last", "rust"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    let content = fs::read_to_string(&note_path)?;
    assert!(!content.contains("\"rust\""), "rust tag should be removed");
    assert!(content.contains("\"testing\""), "testing tag should remain");

    // set — bare --last overwrites all tags
    Command::cargo_bin("jd")?
        .args(["tag", "set", "--last", "archived"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    let content = fs::read_to_string(&note_path)?;
    assert!(
        !content.contains("\"testing\""),
        "testing tag should be gone after set"
    );
    assert!(
        content.contains("\"archived\""),
        "archived tag should be set"
    );

    Ok(())
}

#[test]
fn test_time_based_commands_and_compile() -> TestResult {
    let (_temp_dir, jd_dir) = setup();
    let entries_dir = jd_dir.join("notebooks").join("default");

    // Create a note for today
    let today = Local::now().date_naive();
    fs::write(
        entries_dir.join(format!("{}-120000.md", today.format("%Y-%m-%d"))),
        "note for today",
    )?;

    // Test `today`
    Command::cargo_bin("jd")?
        .arg("today")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("note for today"));

    Ok(())
}

#[test]
fn test_git_init_and_sync() -> TestResult {
    let (_temp_dir, jd_dir) = setup();

    // 1. Init with git
    Command::cargo_bin("jd")?
        .args(["init", "--git"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized a new Git repository"));

    assert!(jd_dir.join(".git").exists());
    assert!(jd_dir.join(".gitignore").exists());

    Ok(())
}

// Test module for notebooks
#[cfg(test)]
mod notebooks {
    use super::*;

    #[test]
    fn test_notebook_creation_and_list() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // Create a new notebook
        Command::cargo_bin("jd")?
            .args(["notebook", "new", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Successfully created new notebook: 'work'",
            ));

        assert!(jd_dir.join("notebooks").join("work").exists());

        // List notebooks
        Command::cargo_bin("jd")?
            .args(["notebook", "list"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("* default"))
            .stdout(predicate::str::contains("  work"));

        Ok(())
    }

    #[test]
    fn test_notebook_status_and_use() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // Default status
        Command::cargo_bin("jd")?
            .args(["notebook", "status"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("Active notebook: default"));

        // Status with env var
        Command::cargo_bin("jd")?
            .args(["notebook", "status"])
            .env("JD_DIR", &jd_dir)
            .env("JD_ACTIVE_NOTEBOOK", "personal")
            .assert()
            .success()
            .stdout(predicate::str::contains("Active notebook: personal"));

        // `use` command should print the export command
        Command::cargo_bin("jd")?
            .args(["notebook", "new", "project-x"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        Command::cargo_bin("jd")?
            .args(["notebook", "use", "project-x"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "export JD_ACTIVE_NOTEBOOK='project-x'",
            ));

        Ok(())
    }

    #[test]
    fn test_jotting_in_different_notebooks() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        Command::cargo_bin("jd")?
            .args(["notebook", "new", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // Jot in default
        Command::cargo_bin("jd")?
            .arg("a personal note")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // Added sleep to prevent filename collision due to second-level timestamp resolution.
        std::thread::sleep(std::time::Duration::from_millis(1200));

        // Jot in work notebook using --notebook flag
        Command::cargo_bin("jd")?
            .arg("a work note")
            .args(["--notebook", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        std::thread::sleep(std::time::Duration::from_millis(1200));

        // Jot in work notebook using env var
        Command::cargo_bin("jd")?
            .arg("another work note")
            .env("JD_DIR", &jd_dir)
            .env("JD_ACTIVE_NOTEBOOK", "work")
            .assert()
            .success();

        // Verify counts
        assert_eq!(
            fs::read_dir(jd_dir.join("notebooks").join("default"))?.count(),
            1
        );
        assert_eq!(
            fs::read_dir(jd_dir.join("notebooks").join("work"))?.count(),
            2
        );

        // Verify `list` is scoped correctly
        Command::cargo_bin("jd")?
            .arg("list")
            .env("JD_DIR", &jd_dir)
            .env("JD_ACTIVE_NOTEBOOK", "work")
            .assert()
            .success()
            .stdout(predicate::str::contains("a work note"))
            .stdout(predicate::str::contains("another work note"))
            .stdout(predicate::str::contains("a personal note").not());

        Ok(())
    }

    #[test]
    fn test_legacy_migration() -> TestResult {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let jd_dir = temp_dir.path().to_path_buf();

        // 1. Create the legacy `entries` directory structure
        let legacy_entries = jd_dir.join("entries");
        fs::create_dir_all(&legacy_entries)?;
        fs::write(legacy_entries.join("legacy_note.md"), "old note")?;

        // 2. Run any jd command, which should trigger the migration
        Command::cargo_bin("jd")?
            .arg("info")
            .arg("--paths")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("Migrating your existing notes"));

        // 3. Verify the new structure
        assert!(
            !jd_dir.join("entries").exists(),
            "Legacy entries dir should be gone"
        );
        let default_notebook = jd_dir.join("notebooks").join("default");
        assert!(
            default_notebook.exists(),
            "Default notebook should be created"
        );
        assert!(
            default_notebook.join("legacy_note.md").exists(),
            "Legacy note should be moved"
        );

        Ok(())
    }

    #[test]
    fn test_info_stats_all_notebooks() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        Command::cargo_bin("jd")?
            .args(["notebook", "new", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // Add one note to default, two to work
        Command::cargo_bin("jd")?
            .arg("note 1")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        std::thread::sleep(std::time::Duration::from_millis(1200));
        Command::cargo_bin("jd")?
            .arg("note 2")
            .args(["--notebook", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        std::thread::sleep(std::time::Duration::from_millis(1200));
        Command::cargo_bin("jd")?
            .arg("note 3")
            .args(["--notebook", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // Check stats for all
        Command::cargo_bin("jd")?
            .args(["info", "--stats", "--all"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("Stats for all notebooks combined"))
            .stdout(predicate::str::contains("Total jots: 3"));

        Ok(())
    }

    #[test]
    fn test_tags_filter_is_scoped() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        Command::cargo_bin("jd")?
            .args(["notebook", "new", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // Create two notes with the same tag in different notebooks
        Command::cargo_bin("jd")?
            .arg("personal task")
            .args(["--tags", "todo"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        std::thread::sleep(std::time::Duration::from_millis(1200));
        Command::cargo_bin("jd")?
            .arg("work task")
            .args(["--tags", "todo", "--notebook", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // Filter in the 'work' notebook
        Command::cargo_bin("jd")?
            .args(["tags", "todo", "--notebook", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("work task"))
            .stdout(predicate::str::contains("personal task").not());

        Ok(())
    }

    #[test]
    fn test_new_with_template_in_notebook() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        let templates_dir = jd_dir.join("templates");
        fs::create_dir(&templates_dir)?;
        fs::write(templates_dir.join("meeting.md"), "## Meeting Notes")?;
        Command::cargo_bin("jd")?
            .args(["notebook", "new", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        Command::cargo_bin("jd")?
            .args(["new", "--template", "meeting.md", "--notebook", "work"])
            .env("JD_DIR", &jd_dir)
            .env("EDITOR", "true") // Use `true` as a no-op editor
            .assert()
            .success();

        let work_notebook = jd_dir.join("notebooks").join("work");
        let entry_path = fs::read_dir(work_notebook)?.next().unwrap()?.path();
        let content = fs::read_to_string(entry_path)?;
        assert!(content.contains("## Meeting Notes"));

        Ok(())
    }

    #[test]
    fn test_find_is_scoped_and_global_search_works() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Create a couple of new notebooks
        Command::cargo_bin("jd")?
            .args(["notebook", "new", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        Command::cargo_bin("jd")?
            .args(["notebook", "new", "personal"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 2. Create notes with a shared keyword in different notebooks
        // Note in 'default'
        Command::cargo_bin("jd")?
            .arg("A note about a database_migration in default.")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        std::thread::sleep(std::time::Duration::from_millis(1200));

        // Note in 'work'
        Command::cargo_bin("jd")?
            .arg("Work note on the database_migration plan.")
            .args(["--notebook", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        std::thread::sleep(std::time::Duration::from_millis(1200));

        // A note without the keyword
        Command::cargo_bin("jd")?
            .arg("A personal note about something else.")
            .args(["--notebook", "personal"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 3. Test that a normal find is scoped to the active notebook ('default')
        Command::cargo_bin("jd")?
            .args(["find", "database_migration"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("in default"))
            .stdout(predicate::str::contains("in work").not());

        // 4. Test that `find --all` searches across all notebooks
        Command::cargo_bin("jd")?
            .args(["find", "database_migration", "--all"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("NOTEBOOK")) // Check for new header
            .stdout(predicate::str::contains("default"))
            .stdout(predicate::str::contains("work"))
            .stdout(predicate::str::contains("something else").not());

        Ok(())
    }
}

// Test module for error handling
#[cfg(test)]
mod error_handling {
    use super::*;

    #[test]
    fn test_fails_on_invalid_notebook_name() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        Command::cargo_bin("jd")?
            .args(["notebook", "new", "invalid/name"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .failure()
            .stderr(predicate::str::contains("Invalid notebook name"));
        Ok(())
    }

    #[test]
    fn test_fails_on_nonexistent_notebook_use() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        Command::cargo_bin("jd")?
            .args(["notebook", "use", "fake-notebook"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "Notebook 'fake-notebook' not found",
            ));
        Ok(())
    }

    #[test]
    fn test_fails_on_ambiguous_prefix() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        let entries = jd_dir.join("notebooks").join("default");
        fs::write(entries.join("2025-01-01-100000.md"), "note 1")?;
        fs::write(entries.join("2025-01-01-200000.md"), "note 2")?;

        Command::cargo_bin("jd")?
            .args(["show", "2025-01-01"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "Prefix '2025-01-01' is not unique",
            ));
        Ok(())
    }

    #[test]
    fn test_fails_on_out_of_bounds_last() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        Command::cargo_bin("jd")?
            .arg("a single note")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        Command::cargo_bin("jd")?
            .args(["show", "--last=5"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .failure()
            .stderr(predicate::str::contains("Index out of bounds"));
        Ok(())
    }
}

// Test for full encryption feature
#[test]
fn test_full_encryption_and_decryption_lifecycle_across_notebooks() -> TestResult {
    let (_temp_dir, jd_dir) = setup();

    // 1. Init with encryption
    Command::cargo_bin("jd")?
        .args(["init", "--encrypt"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    // 2. Create a second notebook
    Command::cargo_bin("jd")?
        .args(["notebook", "new", "secrets"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    // 3. Create encrypted notes in both notebooks
    Command::cargo_bin("jd")?
        .arg("default secret")
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_millis(1200));
    Command::cargo_bin("jd")?
        .arg("special secret")
        .args(["--notebook", "secrets"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    // 4. Verify both files on disk are encrypted
    let default_note_path = fs::read_dir(jd_dir.join("notebooks").join("default"))?
        .next()
        .unwrap()?
        .path();
    let secret_note_path = fs::read_dir(jd_dir.join("notebooks").join("secrets"))?
        .next()
        .unwrap()?
        .path();
    assert!(fs::read(&default_note_path)?.starts_with(b"age-encryption.org"));
    assert!(fs::read(&secret_note_path)?.starts_with(b"age-encryption.org"));

    // 5. Verify jd can read them transparently
    Command::cargo_bin("jd")?
        .args(["show", "--last"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("default secret"));
    Command::cargo_bin("jd")?
        .args(["show", "--last", "--notebook", "secrets"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("special secret"));

    // 6. Decrypt the entire journal
    Command::cargo_bin("jd")?
        .args(["decrypt", "--force"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    // 7. Verify both files are now plaintext
    assert_eq!(fs::read_to_string(default_note_path)?, "default secret");
    assert_eq!(fs::read_to_string(secret_note_path)?, "special secret");

    // 8. Verify identity/config files are gone
    assert!(!jd_dir.join("identity.txt").exists());
    assert!(!jd_dir.join("config.toml").exists());

    Ok(())
}

// Test module for pinning feature.
#[cfg(test)]
mod pinning {
    use super::*;

    /// Tests the complete lifecycle of pinning and unpinning a note
    /// using the `--last` flag and by its ID prefix.
    #[test]
    fn test_pin_and_unpin_lifecycle() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Create a couple of notes to work with.
        Command::cargo_bin("jd")?
            .arg("an unimportant note")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        std::thread::sleep(std::time::Duration::from_millis(1200));

        Command::cargo_bin("jd")?
            .arg("a very important note")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 2. Pin the last note created.
        Command::cargo_bin("jd")?
            .args(["pin", "--last"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("Successfully pinned jot"));

        // 3. Verify the `pinned: true` attribute exists in the file.
        let entries_dir = jd_dir.join("notebooks").join("default");
        let mut entries: Vec<_> = fs::read_dir(entries_dir)?
            .map(|r| r.unwrap().path())
            .collect();
        entries.sort(); // Sort to get the most recent note last.
        let last_note_path = entries.last().unwrap();
        let last_note_content = fs::read_to_string(last_note_path)?;
        assert!(
            last_note_content.contains("pinned = true"),
            "The note should contain 'pinned = true' after pinning."
        );

        // 4. Unpin the same note using its ID prefix.
        let note_id = last_note_path.file_stem().unwrap().to_str().unwrap();
        Command::cargo_bin("jd")?
            .args(["unpin", note_id])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("Successfully unpinned jot"));

        // 5. Verify the `pinned` attribute is now gone from the file.
        let unpinned_content = fs::read_to_string(last_note_path)?;
        assert!(
            !unpinned_content.contains("pinned ="),
            "The 'pinned' key should be removed after unpinning."
        );

        Ok(())
    }

    /// Tests the `list --pinned` command.
    #[test]
    fn test_list_pinned_jots() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Create a mix of pinned and unpinned notes.
        Command::cargo_bin("jd")?
            .arg("unpinned note 1")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        std::thread::sleep(std::time::Duration::from_millis(1200));
        Command::cargo_bin("jd")?
            .arg("pinned note 1")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        Command::cargo_bin("jd")?
            .args(["pin", "--last"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        std::thread::sleep(std::time::Duration::from_millis(1200));
        Command::cargo_bin("jd")?
            .arg("unpinned note 2")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        std::thread::sleep(std::time::Duration::from_millis(1200));
        Command::cargo_bin("jd")?
            .arg("pinned note 2")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        Command::cargo_bin("jd")?
            .args(["pin", "--last"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 2. Run `list --pinned` and verify the output.
        Command::cargo_bin("jd")?
            .args(["list", "--pinned"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("pinned note 1"))
            .stdout(predicate::str::contains("pinned note 2"))
            .stdout(predicate::str::contains("unpinned note").not());

        Ok(())
    }

    /// Tests that re-pinning an already pinned jot doesn't cause an error.
    #[test]
    fn test_pinning_is_idempotent() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        Command::cargo_bin("jd")?
            .arg("a note to be pinned repeatedly")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // Pin it once.
        Command::cargo_bin("jd")?
            .args(["pin", "--last"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // **MODIFICATION**: The assertion now correctly checks for the unique part of the confirmation message.
        // Pin it again. Should report that it's already pinned.
        Command::cargo_bin("jd")?
            .args(["pin", "--last"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("is already pinned."));

        Ok(())
    }
}

// Test module for the task feature.
#[cfg(test)]
mod tasks {
    use super::*;

    /// Tests that the `task`, `todo`, and `t` subcommands all create a
    /// correctly formatted task jot.
    #[test]
    fn test_task_creation_and_aliases() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Test the main `task` command
        Command::cargo_bin("jd")?
            .args(["task", "this is the main command"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        std::thread::sleep(std::time::Duration::from_millis(1200));

        // 2. Test the `todo` alias
        Command::cargo_bin("jd")?
            .args(["todo", "this is the todo alias"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        std::thread::sleep(std::time::Duration::from_millis(1200));

        // 3. Test the `t` alias
        Command::cargo_bin("jd")?
            .args(["t", "this is the t alias"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 4. Verify the contents of the created files
        let entries_dir = jd_dir.join("notebooks").join("default");
        let mut entries: Vec<_> = fs::read_dir(entries_dir)?
            .map(|r| r.unwrap().path())
            .collect();
        entries.sort();

        assert_eq!(entries.len(), 3, "Expected three task jots to be created");

        let content1 = fs::read_to_string(&entries[0])?;
        let content2 = fs::read_to_string(&entries[1])?;
        let content3 = fs::read_to_string(&entries[2])?;

        assert!(content1.contains("- [ ] this is the main command"));
        assert!(content2.contains("- [ ] this is the todo alias"));
        assert!(content3.contains("- [ ] this is the t alias"));

        Ok(())
    }

    /// Tests the `list --tasks` command to ensure it only shows jots
    /// with incomplete tasks.
    #[test]
    fn test_list_tasks() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Create a jot with no tasks.
        Command::cargo_bin("jd")?
            .arg("just a regular note")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        std::thread::sleep(std::time::Duration::from_millis(1200));

        // 2. Create a jot with only completed tasks.
        let completed_task_path = jd_dir
            .join("notebooks")
            .join("default")
            .join("2025-01-01-100000.md");
        fs::write(completed_task_path, "- [x] This task is done")?;
        std::thread::sleep(std::time::Duration::from_millis(1200));

        // 3. Create a jot with an incomplete task.
        Command::cargo_bin("jd")?
            .args(["task", "this task is pending"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 4. Run `list --tasks` and verify the output.
        Command::cargo_bin("jd")?
            .args(["list", "--tasks"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("this task is pending"))
            .stdout(predicate::str::contains("regular note").not())
            .stdout(predicate::str::contains("This task is done").not());

        Ok(())
    }

    /// Tests the `info --stats` command to verify task summary output.
    #[test]
    fn test_info_stats_with_tasks() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Create notes with a mix of pending and completed tasks.
        let entries_dir = jd_dir.join("notebooks").join("default");
        fs::write(
            entries_dir.join("tasks1.md"),
            "- [ ] pending 1\n- [x] done 1",
        )?;
        fs::write(
            entries_dir.join("tasks2.md"),
            "- [ ] pending 2\n- [ ] pending 3",
        )?;
        fs::write(entries_dir.join("tasks3.md"), "- [x] done 2")?;

        // 2. Run `info --stats` and check the summary.
        Command::cargo_bin("jd")?
            .args(["info", "--stats"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("Task Summary:"))
            .stdout(predicate::str::contains("Completed: 2"))
            .stdout(predicate::str::contains("Pending:   3"));

        Ok(())
    }
}

/// Test module for import/export feature
#[cfg(test)]
mod import_export {
    use super::*;

    #[test]
    fn test_export_and_import_zip() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        let notebook_name = "zip-test-notebook";
        let output_zip = jd_dir.join("export.zip");

        // 1. Create a notebook and a note
        Command::cargo_bin("jd")?
            .args(["notebook", "new", notebook_name])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        Command::cargo_bin("jd")?
            .arg("a note for zip export")
            .args(["--notebook", notebook_name])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 2. Export the notebook to a zip file
        Command::cargo_bin("jd")?
            .args([
                "export",
                notebook_name,
                "--output",
                output_zip.to_str().unwrap(),
            ])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        assert!(output_zip.exists());

        // 3. Import the notebook from the zip file
        Command::cargo_bin("jd")?
            .args(["import", output_zip.to_str().unwrap()])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 4. Verify the imported notebook and its content
        let imported_notebook_path = jd_dir.join("notebooks").join("export"); // "export" from file stem
        assert!(imported_notebook_path.exists());
        assert_eq!(fs::read_dir(&imported_notebook_path)?.count(), 1);

        Ok(())
    }

    #[test]
    fn test_export_and_import_json() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        let notebook_name = "json-test-notebook";
        let notebook_path = jd_dir.join("notebooks").join(notebook_name); // Path to the original notebook
        let output_json = jd_dir.join("export.json");

        // 1. Create a notebook and a note
        Command::cargo_bin("jd")?
            .args(["notebook", "new", notebook_name])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        Command::cargo_bin("jd")?
            .arg("a note for json export")
            .args(["--notebook", notebook_name])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 2. Export the notebook to a json file
        Command::cargo_bin("jd")?
            .args([
                "export",
                notebook_name,
                "--format",
                "json",
                "--output",
                output_json.to_str().unwrap(),
            ])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        assert!(output_json.exists());

        // 3. ✅ REMOVE the original notebook to simulate a restore
        fs::remove_dir_all(&notebook_path)?;
        assert!(
            !notebook_path.exists(),
            "Original notebook should be deleted before import"
        );

        // 4. Import the notebook from the json file. This should now succeed.
        Command::cargo_bin("jd")?
            .args(["import", output_json.to_str().unwrap()])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 5. Verify the imported notebook
        let imported_notebook_path = jd_dir.join("notebooks").join(notebook_name);
        assert!(imported_notebook_path.exists());
        assert_eq!(fs::read_dir(&imported_notebook_path)?.count(), 1);

        Ok(())
    }
}

// Test module for the templating feature.
#[cfg(test)]
mod templating {
    use super::*;
    use predicates::str::is_match;

    /// Tests the replacement of built-in variables like `{{date}}`,
    /// `{{branch}}`, `{{project_dir}}`, and `{{uuid}}`.
    #[test]
    fn test_built_in_template_variables() -> TestResult {
        let (temp_dir, jd_dir) = setup();
        let templates_dir = jd_dir.join("templates");
        fs::create_dir(&templates_dir)?;

        // 1. Initialize a git repo on branch "main" to test the {{branch}} variable.
        std::process::Command::new("git")
            .current_dir(temp_dir.path())
            .args(["init"])
            .output()?;
        std::process::Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "--allow-empty", "-m", "init"])
            .env("GIT_AUTHOR_NAME", "jd-test")
            .env("GIT_AUTHOR_EMAIL", "test@jd.com")
            .env("GIT_COMMITTER_NAME", "jd-test")
            .env("GIT_COMMITTER_EMAIL", "test@jd.com")
            .output()?;
        std::process::Command::new("git")
            .current_dir(temp_dir.path())
            .args(["branch", "-m", "main"])
            .output()?;

        // 2. Create a template file with all the built-in variables.
        let template_content =
            "Date: {{date}}\nBranch: {{branch}}\nProject: {{project_dir}}\nID: {{uuid}}";
        fs::write(templates_dir.join("built-in.md"), template_content)?;

        // 3. Run the `new` command with the template.
        Command::cargo_bin("jd")?
            .current_dir(&temp_dir) // Run from inside the temp dir to get project_dir
            .args(["new", "--template", "built-in.md"])
            .env("JD_DIR", &jd_dir)
            .env("EDITOR", "true") // No-op editor
            .assert()
            .success();

        // 4. Verify the output file.
        let entries_dir = jd_dir.join("notebooks").join("default");
        let entry_path = fs::read_dir(entries_dir)?.next().unwrap()?.path();
        let content = fs::read_to_string(entry_path)?;

        assert!(content.contains("Branch: main"));
        assert!(content.contains(&format!(
            "Project: {}",
            temp_dir.path().file_name().unwrap().to_str().unwrap()
        )));
        assert!(is_match(r"ID: [0-9a-f]{8}-([0-9a-f]{4}-){3}[0-9a-f]{12}")
            .unwrap()
            .eval(&content));
        assert!(!content.contains("{{date}}")); // Just check it was replaced

        Ok(())
    }

    /// Tests the replacement of custom variables passed via the `-v` flag.
    #[test]
    fn test_custom_template_variables() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        let templates_dir = jd_dir.join("templates");
        fs::create_dir(&templates_dir)?;
        fs::write(
            templates_dir.join("custom.md"),
            "Ticket: {{ticket_id}}\nFeature: {{feature}}",
        )?;

        Command::cargo_bin("jd")?
            .args([
                "new",
                "--template",
                "custom.md",
                "-v",
                "ticket_id=PROJ-456",
                "-v",
                "feature=templating",
            ])
            .env("JD_DIR", &jd_dir)
            .env("EDITOR", "true")
            .assert()
            .success();

        let entries_dir = jd_dir.join("notebooks").join("default");
        let entry_path = fs::read_dir(entries_dir)?.next().unwrap()?.path();
        let content = fs::read_to_string(entry_path)?;

        assert!(content.contains("Ticket: PROJ-456"));
        assert!(content.contains("Feature: templating"));

        Ok(())
    }
}

// A module for testing the interactive shell.
#[cfg(test)]
mod shell {
    use super::*;
    use std::io::Write;
    // Use the standard library's process::Command for interactive tests.
    use std::process::{Command, Stdio};

    /// Tests the basic lifecycle of the interactive shell.
    #[test]
    fn test_shell_lifecycle_and_commands() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // Use std::process::Command to get a handle to stdin.
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_jd"));
        cmd.arg("shell")
            .env("JD_DIR", &jd_dir)
            .stdin(Stdio::piped()) // Pipe stdin so we can write to it.
            .stdout(Stdio::piped()); // Pipe stdout so we can read it.

        let mut process = cmd.spawn()?;

        {
            let stdin = process.stdin.as_mut().expect("Failed to open stdin");

            // 1. Create a jot to ensure the list command has output.
            stdin.write_all(b"a note for the shell test\n")?;

            // 2. Run the `list` command and verify its output.
            stdin.write_all(b"list\n")?;

            // 3. Exit the shell.
            stdin.write_all(b"exit\n")?;
        }

        // Wait for the process to exit and capture its output.
        let output = process.wait_with_output()?;

        // Assert that the entire process was successful.
        assert!(output.status.success());

        // Verify the output contains expected strings from the shell lifecycle.
        let stdout = String::from_utf8(output.stdout)?;

        // Check that the output from the `list` command is present.
        assert!(stdout.contains("a note for the shell test"));
        assert!(stdout.contains("Exiting jd shell."));

        Ok(())
    }
}

#[cfg(test)]
mod manipulation {
    use super::*;

    #[test]
    fn test_append_and_prepend() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Create a note
        Command::cargo_bin("jd")?
            .arg("Initial content")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 2. Append text
        Command::cargo_bin("jd")?
            .args(["append", "--last=1", "Appended line"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 3. Prepend text
        Command::cargo_bin("jd")?
            .args(["prepend", "--last=1", "Prepended line"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 4. Verify content
        Command::cargo_bin("jd")?
            .args(["show", "--last", "--raw"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Prepended line\nInitial content\nAppended line",
            ));

        Ok(())
    }

    #[test]
    fn test_move_jot() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Create a note and a new notebook
        Command::cargo_bin("jd")?
            .arg("Moving this note")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        Command::cargo_bin("jd")?
            .args(["notebook", "new", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 2. Move the note
        Command::cargo_bin("jd")?
            .args(["move", "--last=1", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 3. Verify it's in the new notebook
        Command::cargo_bin("jd")?
            .args(["list", "--notebook", "work"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("Moving this note"));

        // 4. Verify it's gone from the default notebook
        Command::cargo_bin("jd")?
            .arg("list")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("Moving this note").not());

        Ok(())
    }

    #[test]
    fn test_rename_jot() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Create a note
        Command::cargo_bin("jd")?
            .arg("Original")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 2. Rename it
        Command::cargo_bin("jd")?
            .args(["rename", "renamed-note", "--last"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 3. Verify the file exists with the new name
        assert!(jd_dir
            .join("notebooks")
            .join("default")
            .join("renamed-note.md")
            .exists());

        Ok(())
    }

    #[test]
    fn test_rename_daily_note_updates_title() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Create a daily note
        Command::cargo_bin("jd")?
            .arg("daily")
            .arg("Daily message")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let old_path = jd_dir
            .join("notebooks")
            .join("default")
            .join(format!("{}.md", today));
        assert!(old_path.exists());

        // 2. Rename it
        Command::cargo_bin("jd")?
            .args(["rename", "new-daily-title", "--last"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 3. Verify the file exists with the new name
        let new_path = jd_dir
            .join("notebooks")
            .join("default")
            .join("new-daily-title.md");
        assert!(new_path.exists());

        // 4. Verify the title was updated in the frontmatter
        let content = std::fs::read_to_string(new_path)?;
        assert!(content.contains("title = \"new-daily-title\""));

        Ok(())
    }

    #[test]
    fn test_stdin_piping() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // Use the cargo_bin helper from assert_cmd to get the path to the jd binary
        use assert_cmd::cargo::cargo_bin;
        let mut cmd = std::process::Command::new(cargo_bin("jd"));
        cmd.env("JD_DIR", &jd_dir)
            .stdin(std::process::Stdio::piped());

        let mut child = cmd.spawn()?;
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        std::io::Write::write_all(&mut stdin, b"line 1\nline 2")?;
        drop(stdin);

        let output = child.wait_with_output()?;
        assert!(output.status.success());

        // Verify a note was created with the multiline content
        let entries = std::fs::read_dir(jd_dir.join("notebooks").join("default"))?;
        let mut found = false;
        for entry in entries {
            let content = std::fs::read_to_string(entry?.path())?;
            if content.contains("line 1\nline 2") {
                found = true;
                break;
            }
        }
        assert!(found);

        Ok(())
    }

    #[test]
    fn test_multiline_task_indentation() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Create a multiline task
        Command::cargo_bin("jd")?
            .arg("task")
            .arg("line 1\nline 2")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 2. Verify the task was indented correctly
        let entries = std::fs::read_dir(jd_dir.join("notebooks").join("default"))?;
        let mut found = false;
        for entry in entries {
            let content = std::fs::read_to_string(entry?.path())?;
            if content.contains("- [ ] line 1\n      line 2") {
                found = true;
                break;
            }
        }
        assert!(found);

        Ok(())
    }

    #[test]
    fn test_property_management() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        // 1. Create a note
        Command::cargo_bin("jd")?
            .arg("Property test")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 2. Set a property
        Command::cargo_bin("jd")?
            .args(["property", "set", "--last=1", "status", "in-progress"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 3. Get the property
        Command::cargo_bin("jd")?
            .args(["property", "get", "--last=1", "status"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("in-progress"));

        // 4. Delete the property
        Command::cargo_bin("jd")?
            .args(["property", "delete", "--last=1", "status"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 5. Verify it's gone
        Command::cargo_bin("jd")?
            .args(["property", "get", "--last=1", "status"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .failure();

        Ok(())
    }

    #[test]
    fn test_json_output() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        Command::cargo_bin("jd")?
            .arg("JSON test note")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        Command::cargo_bin("jd")?
            .args(["list", "1", "--format", "json"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("\"content\": \"JSON test note\""));

        Ok(())
    }

    #[test]
    fn test_csv_output() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        Command::cargo_bin("jd")?
            .arg("CSV test note")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        Command::cargo_bin("jd")?
            .args(["list", "1", "--format", "csv"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("CSV test note"));

        Ok(())
    }

    #[test]
    fn test_find_with_context() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        Command::cargo_bin("jd")?
            .arg("This is a line with a keyword.\nThis line is different.")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        Command::cargo_bin("jd")?
            .args(["find", "keyword", "--context"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "1: This is a line with a keyword.",
            ))
            .stdout(predicate::str::contains("This line is different.").not());

        Ok(())
    }
}

#[cfg(test)]
mod daily {
    use super::*;

    #[test]
    fn test_daily_note_lifecycle() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let filename = format!("{}.md", today);

        // 1. Create daily note
        Command::cargo_bin("jd")?
            .args(["daily", "First entry"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 2. Append to daily note
        Command::cargo_bin("jd")?
            .args(["daily", "Second entry"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // 3. Verify file exists and has content
        let daily_path = jd_dir.join("notebooks").join("default").join(&filename);
        assert!(daily_path.exists());

        let content = std::fs::read_to_string(daily_path)?;
        assert!(content.contains("First entry"));
        assert!(content.contains("Second entry"));

        Ok(())
    }
}

#[test]
fn test_same_second_no_collision() -> TestResult {
    let (_temp_dir, jd_dir) = setup();
    let entries_dir = jd_dir.join("notebooks").join("default");

    // Create multiple notes without sleeping — all land in the same second.
    for i in 0..5 {
        Command::cargo_bin("jd")?
            .arg(format!("rapid note {i}"))
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
    }

    let count = fs::read_dir(&entries_dir)?.count();
    assert_eq!(count, 5, "expected 5 distinct note files, got {count}");

    // All 5 messages must be findable.
    for i in 0..5 {
        Command::cargo_bin("jd")?
            .args(["find", &format!("rapid note {i}")])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains(format!("rapid note {i}")));
    }

    Ok(())
}

#[test]
fn test_clean_aborts_on_no() -> TestResult {
    let (_temp_dir, jd_dir) = setup();
    let entries_dir = jd_dir.join("notebooks").join("default");

    Command::cargo_bin("jd")?
        .args(["note one"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    Command::cargo_bin("jd")?
        .arg("clean")
        .env("JD_DIR", &jd_dir)
        .write_stdin("n\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Aborted"));

    assert_eq!(
        fs::read_dir(&entries_dir)?.count(),
        1,
        "note should still exist after abort"
    );
    Ok(())
}

#[test]
fn test_clean_deletes_on_double_confirm() -> TestResult {
    let (_temp_dir, jd_dir) = setup();
    let entries_dir = jd_dir.join("notebooks").join("default");

    for msg in ["note one", "note two", "note three"] {
        Command::cargo_bin("jd")?
            .args([msg])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
    }
    assert_eq!(fs::read_dir(&entries_dir)?.count(), 3);

    Command::cargo_bin("jd")?
        .arg("clean")
        .env("JD_DIR", &jd_dir)
        .write_stdin("y\ny\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted 3 notes"));

    assert_eq!(
        fs::read_dir(&entries_dir)?.count(),
        0,
        "all notes should be gone"
    );
    Ok(())
}

#[test]
fn test_clean_all_clears_every_notebook() -> TestResult {
    let (_temp_dir, jd_dir) = setup();

    // Create notes in two notebooks
    Command::cargo_bin("jd")?
        .args(["note in default"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    fs::create_dir_all(jd_dir.join("notebooks").join("work"))?;
    Command::cargo_bin("jd")?
        .args(["note in work", "--notebook", "work"])
        .env("JD_DIR", &jd_dir)
        .assert()
        .success();

    Command::cargo_bin("jd")?
        .args(["clean", "--all"])
        .env("JD_DIR", &jd_dir)
        .write_stdin("y\ny\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted 2 notes"));

    assert_eq!(
        fs::read_dir(jd_dir.join("notebooks").join("default"))?.count(),
        0
    );
    assert_eq!(
        fs::read_dir(jd_dir.join("notebooks").join("work"))?.count(),
        0
    );
    Ok(())
}

// Tests for the encryption hardening work (key perms, edit cycle, sync guard,
// import/export behavior with encryption enabled).
#[cfg(test)]
mod encryption_hardening {
    use super::*;

    fn init_encrypted(jd_dir: &PathBuf) -> TestResult {
        Command::cargo_bin("jd")?
            .args(["init", "--encrypt"])
            .env("JD_DIR", jd_dir)
            .assert()
            .success();
        Ok(())
    }

    /// Writes an executable fake-editor script and returns its path (Unix only).
    #[cfg(unix)]
    fn fake_editor(dir: &std::path::Path, script_body: &str) -> TestResult2<PathBuf> {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fake_editor.sh");
        fs::write(&path, format!("#!/bin/sh\n{script_body}\n"))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        Ok(path)
    }

    #[cfg(unix)]
    type TestResult2<T> = Result<T, Box<dyn std::error::Error>>;

    #[test]
    #[cfg(unix)]
    fn test_identity_file_is_owner_only() -> TestResult {
        use std::os::unix::fs::PermissionsExt;
        let (_temp_dir, jd_dir) = setup();
        init_encrypted(&jd_dir)?;

        let mode = fs::metadata(jd_dir.join("identity.txt"))?
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "identity.txt must be 0600");
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_edit_encrypted_note_shows_plaintext_and_reencrypts() -> TestResult {
        let (temp_dir, jd_dir) = setup();
        init_encrypted(&jd_dir)?;

        Command::cargo_bin("jd")?
            .arg("original secret")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        // The fake editor records what it was shown, then replaces the content.
        let seen_path = temp_dir.path().join("seen.txt");
        let editor = fake_editor(
            temp_dir.path(),
            &format!(
                "cp \"$1\" {seen}\nprintf 'edited secret' > \"$1\"",
                seen = seen_path.display()
            ),
        )?;

        Command::cargo_bin("jd")?
            .args(["edit", "--last"])
            .env("JD_DIR", &jd_dir)
            .env("EDITOR", &editor)
            .assert()
            .success();

        // The editor must have been shown plaintext, not age ciphertext.
        let seen = fs::read_to_string(&seen_path)?;
        assert!(seen.contains("original secret"));
        assert!(!seen.contains("age-encryption.org"));

        // The note on disk must be re-encrypted with the edited content readable.
        let note_path = fs::read_dir(jd_dir.join("notebooks").join("default"))?
            .next()
            .unwrap()?
            .path();
        assert!(fs::read(&note_path)?.starts_with(b"age-encryption.org"));
        Command::cargo_bin("jd")?
            .args(["show", "--last"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("edited secret"));

        // No plaintext temp file may be left behind.
        let tmp_dir = jd_dir.join("tmp");
        if tmp_dir.exists() {
            assert_eq!(fs::read_dir(&tmp_dir)?.count(), 0);
        }
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_new_encrypted_note_editor_sees_template_plaintext() -> TestResult {
        let (temp_dir, jd_dir) = setup();
        init_encrypted(&jd_dir)?;

        let templates_dir = jd_dir.join("templates");
        fs::create_dir_all(&templates_dir)?;
        fs::write(templates_dir.join("default.md"), "TEMPLATE_MARKER")?;

        let seen_path = temp_dir.path().join("seen.txt");
        let editor = fake_editor(
            temp_dir.path(),
            &format!(
                "cp \"$1\" {seen}\nprintf 'typed in editor' > \"$1\"",
                seen = seen_path.display()
            ),
        )?;

        Command::cargo_bin("jd")?
            .arg("new")
            .env("JD_DIR", &jd_dir)
            .env("EDITOR", &editor)
            .assert()
            .success();

        let seen = fs::read_to_string(&seen_path)?;
        assert!(seen.contains("TEMPLATE_MARKER"));
        assert!(!seen.contains("age-encryption.org"));

        let note_path = fs::read_dir(jd_dir.join("notebooks").join("default"))?
            .next()
            .unwrap()?
            .path();
        assert!(
            fs::read(&note_path)?.starts_with(b"age-encryption.org"),
            "note saved from the editor must be encrypted on disk"
        );
        Command::cargo_bin("jd")?
            .args(["show", "--last"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("typed in editor"));
        Ok(())
    }

    #[test]
    fn test_zip_import_respects_encryption() -> TestResult {
        let (temp_dir, jd_dir) = setup();

        // Build a plaintext zip via export before enabling encryption.
        Command::cargo_bin("jd")?
            .arg("portable note")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();
        let zip_path = temp_dir.path().join("imported.zip");
        Command::cargo_bin("jd")?
            .args(["export", "default", "--output"])
            .arg(&zip_path)
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        init_encrypted(&jd_dir)?;

        Command::cargo_bin("jd")?
            .arg("import")
            .arg(&zip_path)
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        let imported_dir = jd_dir.join("notebooks").join("imported");
        let note_path = fs::read_dir(&imported_dir)?.next().unwrap()?.path();
        assert!(
            fs::read(&note_path)?.starts_with(b"age-encryption.org"),
            "zip import must encrypt notes when encryption is enabled"
        );
        Command::cargo_bin("jd")?
            .args(["show", "--last", "--notebook", "imported"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("portable note"));
        Ok(())
    }

    #[test]
    fn test_export_warns_about_plaintext_when_encrypted() -> TestResult {
        let (temp_dir, jd_dir) = setup();
        init_encrypted(&jd_dir)?;

        Command::cargo_bin("jd")?
            .arg("a secret")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        Command::cargo_bin("jd")?
            .args(["export", "default", "--output"])
            .arg(temp_dir.path().join("out.zip"))
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stderr(predicate::str::contains("PLAINTEXT"));
        Ok(())
    }

    #[test]
    fn test_sync_never_stages_identity_or_config() -> TestResult {
        let (temp_dir, jd_dir) = setup();

        // A pre-existing repo (user ran `git init` themselves): no .gitignore.
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .current_dir(&jd_dir)
                .args(args)
                .output()
                .expect("git failed")
        };
        assert!(git(&["init"]).status.success());
        let remote = temp_dir.path().join("remote.git");
        assert!(std::process::Command::new("git")
            .args(["init", "--bare"])
            .arg(&remote)
            .output()?
            .status
            .success());
        assert!(git(&["remote", "add", "origin", remote.to_str().unwrap()])
            .status
            .success());

        init_encrypted(&jd_dir)?;
        Command::cargo_bin("jd")?
            .arg("a synced note")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        Command::cargo_bin("jd")?
            .arg("sync")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        let tracked = String::from_utf8(git(&["ls-files"]).stdout)?;
        assert!(
            !tracked.contains("identity.txt"),
            "identity.txt must never be committed by sync; tracked files:\n{tracked}"
        );
        assert!(
            !tracked.contains("config.toml"),
            "config.toml must never be committed by sync; tracked files:\n{tracked}"
        );
        assert!(tracked.contains("notebooks/"));
        Ok(())
    }

    #[test]
    fn test_init_git_on_existing_repo_adds_gitignore() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        assert!(std::process::Command::new("git")
            .current_dir(&jd_dir)
            .arg("init")
            .output()?
            .status
            .success());

        Command::cargo_bin("jd")?
            .args(["init", "--git"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("Updated .gitignore"));

        let gitignore = fs::read_to_string(jd_dir.join(".gitignore"))?;
        assert!(gitignore.contains("identity.txt"));
        assert!(gitignore.contains("config.toml"));
        Ok(())
    }
}

// Tests that no user-supplied name can escape the notebooks directory.
#[cfg(test)]
mod path_validation {
    use super::*;

    fn create_note(jd_dir: &PathBuf, message: &str) -> TestResult {
        Command::cargo_bin("jd")?
            .arg(message)
            .env("JD_DIR", jd_dir)
            .assert()
            .success();
        Ok(())
    }

    #[test]
    fn test_rename_cannot_escape_notebook() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        create_note(&jd_dir, "escape attempt")?;

        Command::cargo_bin("jd")?
            .args(["rename", "--last", "../escaped-note"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .failure()
            .stderr(predicate::str::contains("Invalid name"));

        assert!(!jd_dir.join("notebooks").join("escaped-note.md").exists());
        // The original note is untouched.
        assert_eq!(
            fs::read_dir(jd_dir.join("notebooks").join("default"))?.count(),
            1
        );
        Ok(())
    }

    #[test]
    fn test_move_cannot_escape_notebooks_dir() -> TestResult {
        let (_temp_dir, jd_dir) = setup();
        create_note(&jd_dir, "escape attempt")?;

        Command::cargo_bin("jd")?
            .args(["move", "--last", "../.."])
            .env("JD_DIR", &jd_dir)
            .assert()
            .failure()
            .stderr(predicate::str::contains("Invalid destination"));

        assert_eq!(
            fs::read_dir(jd_dir.join("notebooks").join("default"))?.count(),
            1
        );
        Ok(())
    }

    #[test]
    fn test_active_notebook_env_var_is_validated() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        Command::cargo_bin("jd")?
            .arg("list")
            .env("JD_DIR", &jd_dir)
            .env("JD_ACTIVE_NOTEBOOK", "../..")
            .assert()
            .failure()
            .stderr(predicate::str::contains("Invalid notebook name"));
        Ok(())
    }

    #[test]
    fn test_notebook_flag_is_validated() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        Command::cargo_bin("jd")?
            .args(["list", "--notebook", "../evil"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .failure()
            .stderr(predicate::str::contains("Invalid notebook name"));
        Ok(())
    }

    #[test]
    fn test_json_import_rejects_traversal_filenames() -> TestResult {
        let (temp_dir, jd_dir) = setup();

        let payload = r#"{"notebook_name":"evil","jots":[{"filename":"../../../pwned.md","content":"escaped!"}]}"#;
        let payload_path = temp_dir.path().join("payload.json");
        fs::write(&payload_path, payload)?;

        Command::cargo_bin("jd")?
            .arg("import")
            .arg(&payload_path)
            .env("JD_DIR", &jd_dir)
            .assert()
            .failure()
            .stderr(predicate::str::contains("Invalid filename"));

        assert!(!jd_dir.join("pwned.md").exists());
        assert!(!temp_dir.path().join("pwned.md").exists());
        Ok(())
    }

    #[test]
    fn test_json_import_rejects_traversal_notebook_name() -> TestResult {
        let (temp_dir, jd_dir) = setup();

        let payload =
            r#"{"notebook_name":"../outside","jots":[{"filename":"a.md","content":"x"}]}"#;
        let payload_path = temp_dir.path().join("payload.json");
        fs::write(&payload_path, payload)?;

        Command::cargo_bin("jd")?
            .arg("import")
            .arg(&payload_path)
            .env("JD_DIR", &jd_dir)
            .assert()
            .failure()
            .stderr(predicate::str::contains("Invalid notebook name"));

        assert!(!jd_dir.join("outside").exists());
        Ok(())
    }
}

// Regression tests for data-corruption bugs.
#[cfg(test)]
mod corruption_regressions {
    use super::*;

    /// Prepending to a note WITH frontmatter must not place text on the
    /// closing `---` delimiter line (regression: `---new` corruption).
    #[test]
    fn test_prepend_preserves_frontmatter_block() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        Command::cargo_bin("jd")?
            .arg("hello body")
            .args(["--tags", "rust"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        Command::cargo_bin("jd")?
            .args(["prepend", "--last=1", "NEWLINE"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        let note_path = fs::read_dir(jd_dir.join("notebooks").join("default"))?
            .next()
            .unwrap()?
            .path();
        let raw = fs::read_to_string(&note_path)?;

        // The closing delimiter must still sit alone on its own line.
        assert!(
            raw.contains("\n---\n"),
            "closing frontmatter delimiter was corrupted:\n{raw}"
        );
        assert!(
            !raw.contains("---NEWLINE"),
            "text landed on the delimiter:\n{raw}"
        );

        // The prepended text must be the first body line.
        Command::cargo_bin("jd")?
            .args(["show", "--last", "--raw"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::starts_with("NEWLINE\nhello body"));

        // And the note must still parse with its tags intact.
        Command::cargo_bin("jd")?
            .args(["tags", "rust"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("NEWLINE"));
        Ok(())
    }

    /// `property set`/`delete` must refuse the struct-owned frontmatter keys;
    /// previously `property set tags x` made the note (and `jd list`)
    /// unparseable.
    #[test]
    fn test_property_set_rejects_reserved_keys() -> TestResult {
        let (_temp_dir, jd_dir) = setup();

        Command::cargo_bin("jd")?
            .arg("prop test")
            .args(["--tags", "rust"])
            .env("JD_DIR", &jd_dir)
            .assert()
            .success();

        for key in ["tags", "pinned"] {
            Command::cargo_bin("jd")?
                .args(["property", "set", "--last=1", key, "oops"])
                .env("JD_DIR", &jd_dir)
                .assert()
                .failure()
                .stderr(predicate::str::contains("managed by jd"));

            Command::cargo_bin("jd")?
                .args(["property", "delete", "--last=1", key])
                .env("JD_DIR", &jd_dir)
                .assert()
                .failure()
                .stderr(predicate::str::contains("managed by jd"));
        }

        // The note must still list fine with its tags intact.
        Command::cargo_bin("jd")?
            .arg("list")
            .env("JD_DIR", &jd_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("prop test"));
        Ok(())
    }
}

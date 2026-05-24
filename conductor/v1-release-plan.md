# jotdown-rs (jd) v1.0 Release Plan

## 1. Objective & Audit Summary
Prepare `jotdown-rs` (binary `jd`) for its v1.0 major release by bridging functional gaps and incorporating developer-centric features inspired by the Obsidian CLI. 

**Code Quality Audit Results:**
*   **Architecture & Stability**: The codebase is remarkably clean. An audit for "AI slop" (e.g., haphazard `unwrap()`, `expect()`, `panic!()`, or lingering `TODO`s) turned up clean. The project robustly uses the `anyhow` crate for error propagation.
*   **Philosophy Differentiation**: While Obsidian CLI is a "remote control" for a running GUI instance, `jd` remains a pure, standalone, lightning-fast CLI tool. It does not require a daemon or heavy application to be running, adhering strictly to its "plain text is sacred" mandate.

## 2. Key Initiatives for v1.0

### A. Advanced File & Data Manipulation
We will expand the tool's ability to manipulate existing jots without requiring the user to open an external `$EDITOR`.
*   **`append` / `prepend`**: Quickly add text to the end or beginning (just below the frontmatter) of an existing note.
*   **`move`**: Transfer a jot between different notebooks (e.g., `jd move <id> project-x`).
*   **`rename`**: Rename the underlying file or update a specific title field safely.

### B. Scripting & Structured Output
To make `jd` a better Unix citizen, we will allow it to be chained with other tools (like `jq` or `awk`).
*   **`--format=json|csv`**: Add this flag to listing and querying commands (`list`, `find`, `today`, `tags`). Instead of human-readable terminal output, it will emit parseable data containing note IDs, paths, frontmatter, and content snippets.

### C. Generalized Property Management
Currently, `jd` only supports appending or removing items from a `tags` array in the note's YAML frontmatter. We will generalize this into a **Property Management** system.
*   **`property set/get/delete`**: Commands like `jd property set <id> status "in-progress"`. 
*   **Why?**: This allows users to treat their notes like a flexible database, adding custom metadata (e.g., `priority: high`, `due: 2026-06-01`, `ticket: PROJ-123`) directly from the CLI without opening an editor.

### D. Daily Notes Pipeline
A streamlined workflow specifically designed for daily journaling and logging.
*   **`daily:append`**: A dedicated command that automatically creates (if it doesn't exist) or appends to a specific "daily note" (e.g., `2026-05-23.md`). This prevents cluttering the notebook with hundreds of tiny individual jots if the user prefers a single, running log for the day.

### E. Enhanced Finding, Searching, and Viewing
Improving the day-to-day usability of locating information.
*   **Contextual Search (`find --context`)**: Inspired by `grep`, when searching for text, `jd` will return not just the note ID, but the specific line number and surrounding text where the match occurred.
*   **Richer Viewing (`show`)**: Add capabilities to format or syntax-highlight the output of `jd show` in the terminal for better readability.
*   **Fuzzy Finding Polish**: Ensure the interactive `Select` (fuzzy finder via `skim`) provides a rich preview pane showing note frontmatter and content.

## 3. Implementation Phasing
*   **Phase 0**: Project Rename (`rjot` -> `jotdown-rs`, command `jd`). Update config, references, and environment variables.
*   **Phase 1**: Core Data Manipulation (`append`, `prepend`, `move`) and Daily Notes.
*   **Phase 2**: Generalized Property management (refactoring the frontmatter parsing).
*   **Phase 3**: Structured output (`--format`) and Enhanced Search/View tools.
*   **Phase 4**: Documentation updates, final code health check, and v1.0 release tagging.
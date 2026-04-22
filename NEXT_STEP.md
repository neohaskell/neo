# Next Step

## Goal
Implement the `neo new` interactive scaffolding interview and automated initialization.

## Action Items
1. **Interactive Prompt State**:
   - Create `NewProjectState` implementing the `State` trait for the TEA loop.
   - Implement sequential prompts for `name`, `version`, `description`, `author`, and `license` using `ratatui` widgets and `crossterm` events.
2. **Project Scaffolding**:
   - Implement `network::fetch_starter_template()` using `reqwest` to download the tarball.
   - Implement extraction logic using `flate2` and `tar`.
   - Write the collected answers into the generated `neo.json` using `serde_json`.
3. **System Initialization**:
   - Initialize a git repository in the new directory.
   - Call the lock hook installation logic.
4. **CI Mode Support**:
   - Handle the `--ci` flag by skipping prompts, taking `project_name` from args, and using defaults for other fields.

//! Core logic for `neo skills setup`.
//!
//! Discovers skills in a cloned `neohaskell/skills` checkout and plans/installs
//! them into the per-tool folders of a project root. This module is
//! deliberately free of terminal and network concerns so it is fully
//! unit-testable without a TTY or a real clone:
//!
//!   - the clone lives in [`crate::network::fetch_skills_repo`];
//!   - the interactive picker + orchestration live in [`crate::commands::skills`].
//!
//! A source skill is a directory `skills/<name>/SKILL.md` in the
//! [agentskills.io](https://agentskills.io) format: a `---`-fenced frontmatter
//! header with at least `name:` and `description:`, followed by the skill body.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::errors::NeoError;

// ---------------------------------------------------------------------------
// Supported tools + install strategies
// ---------------------------------------------------------------------------

/// How a discovered skill is materialized for a given tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Copy the whole `<name>/` skill folder verbatim into `<dir>/<name>/`.
    /// Used by tools that natively read the agentskills.io `SKILL.md` format
    /// (bundled scripts/references are preserved).
    FolderCopy { dir: &'static str },
    /// Render each skill to a single `<dir>/<name>.mdc` file with Cursor
    /// frontmatter. Bundled scripts cannot travel into a single file (warned).
    CursorRule { dir: &'static str },
    /// Inline every selected skill's body into a managed block in a single
    /// root file (e.g. `AGENTS.md`).
    ManagedFile { path: &'static str },
}

/// A supported AI coding agent and where its skills live in the project root.
#[derive(Debug, Clone, Copy)]
pub struct Tool {
    pub id: &'static str,
    pub display: &'static str,
    pub strategy: Strategy,
}

impl Tool {
    /// The project-root-relative folder or file this tool installs into, for
    /// display in the picker and the summary line.
    pub fn dest_hint(&self) -> &'static str {
        match self.strategy {
            Strategy::FolderCopy { dir } => dir,
            Strategy::CursorRule { dir } => dir,
            Strategy::ManagedFile { path } => path,
        }
    }
}

/// The v1 tool set: the three tools that natively read the `SKILL.md` format
/// (folder-copied verbatim), plus Cursor and the universal `AGENTS.md`.
///
/// The exact destination strings carry sharp collision traps — see the tests:
///   - Codex installs to `.agents/skills` (plural), NOT `.codex/skills`.
///   - `.agents/skills` (Codex) is one char from `.agent/rules` (Antigravity,
///     not yet supported) — do not "fix" it.
pub const SUPPORTED_TOOLS: &[Tool] = &[
    Tool { id: "claude", display: "Claude Code", strategy: Strategy::FolderCopy { dir: ".claude/skills" } },
    Tool { id: "codex", display: "OpenAI Codex CLI", strategy: Strategy::FolderCopy { dir: ".agents/skills" } },
    Tool { id: "kiro", display: "Kiro (AWS)", strategy: Strategy::FolderCopy { dir: ".kiro/skills" } },
    Tool { id: "cursor", display: "Cursor", strategy: Strategy::CursorRule { dir: ".cursor/rules" } },
    Tool { id: "agents", display: "AGENTS.md (universal)", strategy: Strategy::ManagedFile { path: "AGENTS.md" } },
];

/// Comma-separated list of valid `--tool` ids, for error help text.
pub fn valid_tool_ids() -> String {
    SUPPORTED_TOOLS.iter().map(|t| t.id).collect::<Vec<_>>().join(", ")
}

fn tool_by_id(id: &str) -> Option<&'static Tool> {
    SUPPORTED_TOOLS.iter().find(|t| t.id == id)
}

/// Resolve a list of `--tool` id strings to [`Tool`] refs, de-duplicating and
/// erroring actionably on any unknown id.
pub fn resolve_tools(ids: &[String]) -> miette::Result<Vec<&'static Tool>> {
    let mut out: Vec<&'static Tool> = Vec::new();
    for id in ids {
        match tool_by_id(id) {
            Some(t) => {
                if !out.iter().any(|x| x.id == t.id) {
                    out.push(t);
                }
            }
            None => {
                return Err(miette::miette!(
                    help = format!(
                        "Valid --tool ids are: {}. Re-run e.g. `neo skills setup --tool claude --tool cursor`, or `--all-tools` to install for every supported tool.",
                        valid_tool_ids()
                    ),
                    "selecting tools for `neo skills setup`: `{}` is not a supported tool id.",
                    id,
                ));
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Frontmatter parsing
// ---------------------------------------------------------------------------

/// Parse a leading `---`-fenced frontmatter block into a flat map of
/// single-line `key: value` pairs, returning `(frontmatter, body)`.
///
/// Only flat single-line scalar values are supported (sufficient for the
/// agentskills.io `name` + `description` fields). Surrounding single or double
/// quotes are stripped; keys and values are trimmed. A leading UTF-8 BOM and
/// `\r\n` line endings are tolerated. A document without a leading `---` fence,
/// or without a matching closing `---`, yields an empty map and the whole input
/// as the body.
pub fn parse_front_matter(input: &str) -> (BTreeMap<String, String>, String) {
    let stripped = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut lines = stripped.lines();
    match lines.next() {
        Some(l) if l.trim_end() == "---" => {}
        _ => return (BTreeMap::new(), input.to_string()),
    }

    let mut fm = BTreeMap::new();
    let mut closed = false;
    let mut body_lines: Vec<&str> = Vec::new();
    for line in lines.by_ref() {
        if !closed {
            if line.trim_end() == "---" {
                closed = true;
                continue;
            }
            if let Some((k, v)) = line.split_once(':') {
                let key = k.trim();
                if !key.is_empty() {
                    fm.insert(key.to_string(), strip_quotes(v.trim()).to_string());
                }
            }
        } else {
            body_lines.push(line);
        }
    }

    if !closed {
        return (BTreeMap::new(), input.to_string());
    }

    let mut body = body_lines.join("\n");
    if body.starts_with('\n') {
        body.remove(0);
    }
    (fm, body)
}

fn strip_quotes(v: &str) -> &str {
    let bytes = v.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &v[1..v.len() - 1];
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Skill discovery
// ---------------------------------------------------------------------------

/// A skill discovered in a `neohaskell/skills` checkout.
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// Absolute path to the skill's source directory (contains `SKILL.md`).
    pub dir: PathBuf,
    /// The `SKILL.md` body (everything after the frontmatter fence).
    pub body: String,
    /// True when the skill folder bundles files other than `SKILL.md`
    /// (scripts/references) — preserved by FolderCopy, dropped by CursorRule.
    pub has_bundled_files: bool,
}

/// Discover all skills under `<checkout>/skills/`.
///
/// Returns an empty vec (not an error) when the `skills/` directory is absent —
/// that is the correct "nothing to install yet" signal against an empty
/// upstream repo. Each skill folder must contain a `SKILL.md` whose frontmatter
/// declares `name` (matching the folder) and `description`; a violation is an
/// actionable error naming the offending path.
pub fn discover_skills(checkout: &Path) -> miette::Result<Vec<Skill>> {
    let skills_root = checkout.join("skills");
    if !skills_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();
    let entries = std::fs::read_dir(&skills_root)
        .map_err(|e| NeoError::io_at("listing the skills directory at", &skills_root, e))?;
    for entry in entries {
        let entry = entry
            .map_err(|e| NeoError::io_at("reading an entry of the skills directory at", &skills_root, e))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| NeoError::io_at("reading the file type of", &path, e))?;
        if !file_type.is_dir() {
            // Ignore stray top-level files (README.md, LICENSE, …).
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let skill_md = path.join("SKILL.md");
        if !skill_md.is_file() {
            return Err(miette::miette!(
                help = format!(
                    "Every skill in `neohaskell/skills` is a folder containing a `SKILL.md`. Add `skills/{dir}/SKILL.md` (agentskills.io format: a `---`-fenced `name:`/`description:` header, then the skill body), or remove the empty `skills/{dir}/` folder from the source repo.",
                    dir = dir_name
                ),
                "discovering skills: the skill folder `skills/{dir}/` has no `SKILL.md` (expected `skills/{dir}/SKILL.md`).",
                dir = dir_name,
            ));
        }

        let content = std::fs::read_to_string(&skill_md)
            .map_err(|e| NeoError::io_at("reading the skill manifest at", &skill_md, e))?;
        let (fm, body) = parse_front_matter(&content);
        let name = fm.get("name").map(String::as_str).unwrap_or("");
        let description = fm.get("description").map(String::as_str).unwrap_or("");

        if name.is_empty() {
            return Err(miette::miette!(
                help = format!(
                    "Add a `name:` line to the frontmatter of `{path}`. The file must start with a `---` fence, e.g.\n  ---\n  name: {dir}\n  description: Use when ...\n  ---\n  <skill body>",
                    path = skill_md.display(),
                    dir = dir_name
                ),
                "discovering skills: `{path}` is missing the required `name` frontmatter field (got name=\"\").",
                path = skill_md.display(),
            ));
        }
        if description.is_empty() {
            return Err(miette::miette!(
                help = format!(
                    "Add a `description:` line to the frontmatter of `{path}` — one line summarizing when the skill applies, e.g. `description: Use when adding a new event slice`.",
                    path = skill_md.display()
                ),
                "discovering skills: `{path}` is missing the required `description` frontmatter field (got description=\"\").",
                path = skill_md.display(),
            ));
        }
        if name != dir_name {
            return Err(miette::miette!(
                help = format!(
                    "Rename the folder to `skills/{name}/`, or change the frontmatter `name` to `{dir}`, so they match (agentskills.io requires the skill name to equal its folder name).",
                    name = name,
                    dir = dir_name
                ),
                "discovering skills: skill folder `skills/{dir}/` declares `name: {name}` — the folder name and frontmatter name must match.",
                dir = dir_name,
                name = name,
            ));
        }

        let has_bundled_files = dir_has_extra_files(&path, "SKILL.md")?;
        skills.push(Skill {
            name: name.to_string(),
            description: description.to_string(),
            dir: path,
            body,
            has_bundled_files,
        });
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

fn dir_has_extra_files(dir: &Path, ignore: &str) -> miette::Result<bool> {
    for entry in std::fs::read_dir(dir).map_err(|e| NeoError::io_at("listing the skill folder at", dir, e))? {
        let entry = entry.map_err(|e| NeoError::io_at("reading an entry of the skill folder at", dir, e))?;
        if entry.file_name() != ignore {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Filter the discovered skills down to the `--skill`-requested subset.
/// An empty `wanted` keeps all. An unknown requested name is an actionable
/// error listing the available skills.
pub fn filter_skills(all: Vec<Skill>, wanted: &[String]) -> miette::Result<Vec<Skill>> {
    if wanted.is_empty() {
        return Ok(all);
    }
    let available: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
    for w in wanted {
        if !available.iter().any(|n| n == w) {
            return Err(miette::miette!(
                help = format!(
                    "Available skills: {}. Omit `--skill` to install all of them.",
                    if available.is_empty() { "(none)".to_string() } else { available.join(", ") }
                ),
                "selecting skills for `neo skills setup`: no skill named `{}` exists in `neohaskell/skills`.",
                w,
            ));
        }
    }
    Ok(all.into_iter().filter(|s| wanted.iter().any(|w| w == &s.name)).collect())
}

// ---------------------------------------------------------------------------
// Renderers (CursorRule + ManagedFile)
// ---------------------------------------------------------------------------

/// Render a skill as a Cursor `.mdc` rule file: Cursor frontmatter
/// (`description`/`globs`/`alwaysApply`) followed by the skill body.
pub fn render_cursor_mdc(skill: &Skill) -> String {
    format!(
        "---\ndescription: {desc}\nglobs:\nalwaysApply: false\n---\n\n{body}\n",
        desc = skill.description,
        body = skill.body.trim_end(),
    )
}

/// Sentinel delimiting the auto-managed region inside `AGENTS.md`. Content
/// outside these markers is never touched.
pub const AGENTS_BEGIN: &str =
    "<!-- BEGIN neo skills (managed by `neo skills setup` — do not edit inside this block) -->";
pub const AGENTS_END: &str = "<!-- END neo skills -->";

/// Render the managed `AGENTS.md` block (markers included) that inlines every
/// selected skill's description and body.
pub fn render_agents_block(skills: &[Skill]) -> String {
    let mut out = String::new();
    out.push_str(AGENTS_BEGIN);
    out.push('\n');
    out.push_str("## NeoHaskell skills\n\n");
    for skill in skills {
        out.push_str(&format!("### {}\n\n", skill.name));
        out.push_str(skill.description.trim());
        out.push_str("\n\n");
        let body = skill.body.trim();
        if !body.is_empty() {
            out.push_str(body);
            out.push_str("\n\n");
        }
    }
    out.push_str(AGENTS_END);
    out.push('\n');
    out
}

/// Insert or replace the managed skills block in an `AGENTS.md` document,
/// preserving all content outside the markers. Idempotent: applying it to its
/// own output yields the same string.
pub fn upsert_managed_block(existing: &str, block: &str) -> String {
    let block = block.trim_end_matches('\n');
    if let (Some(start), Some(end_idx)) = (existing.find(AGENTS_BEGIN), existing.find(AGENTS_END)) {
        let end = end_idx + AGENTS_END.len();
        let before = &existing[..start];
        let after = &existing[end..];
        let mut out = String::new();
        out.push_str(before);
        out.push_str(block);
        out.push_str(after);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    } else {
        let mut out = String::new();
        out.push_str(existing);
        if !existing.is_empty() {
            if !existing.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str(block);
        out.push('\n');
        out
    }
}

// ---------------------------------------------------------------------------
// Install plan
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Create,
    Overwrite,
    Skip,
}

/// One unit of installation work: a (tool, skill) destination and what would
/// happen to it. `ManagedFile` tools contribute a single aggregate item
/// (`skill_name == "(all)"`) covering every skill.
#[derive(Debug, Clone)]
pub struct PlanItem {
    pub tool_id: &'static str,
    pub skill_name: String,
    pub dest: PathBuf,
    pub action: Action,
    /// Pre-rendered content for single-file strategies (CursorRule/ManagedFile);
    /// `None` for FolderCopy (the source dir is copied at write time).
    pub rendered: Option<String>,
    /// Source skill dir for FolderCopy; `None` otherwise.
    pub src_dir: Option<PathBuf>,
    /// True when bundled scripts will be dropped (CursorRule of a bundled skill).
    pub warn_bundled: bool,
}

/// Compute the install plan for the chosen tools and skills against
/// `project_root`. Pure: reads existing destinations to classify each item as
/// Create/Overwrite/Skip but writes nothing.
pub fn build_plan(
    project_root: &Path,
    tools: &[&Tool],
    skills: &[Skill],
) -> miette::Result<Vec<PlanItem>> {
    let mut plan = Vec::new();
    for tool in tools {
        match tool.strategy {
            Strategy::FolderCopy { dir } => {
                for skill in skills {
                    let dest = project_root.join(dir).join(&skill.name);
                    let action = folder_action(&skill.dir, &dest)?;
                    plan.push(PlanItem {
                        tool_id: tool.id,
                        skill_name: skill.name.clone(),
                        dest,
                        action,
                        rendered: None,
                        src_dir: Some(skill.dir.clone()),
                        warn_bundled: false,
                    });
                }
            }
            Strategy::CursorRule { dir } => {
                for skill in skills {
                    let dest = project_root.join(dir).join(format!("{}.mdc", skill.name));
                    let rendered = render_cursor_mdc(skill);
                    let action = file_action(&dest, &rendered)?;
                    plan.push(PlanItem {
                        tool_id: tool.id,
                        skill_name: skill.name.clone(),
                        dest,
                        action,
                        rendered: Some(rendered),
                        src_dir: None,
                        warn_bundled: skill.has_bundled_files,
                    });
                }
            }
            Strategy::ManagedFile { path } => {
                let dest = project_root.join(path);
                let existing = read_to_string_opt(&dest)?;
                let block = render_agents_block(skills);
                let new_content = upsert_managed_block(existing.as_deref().unwrap_or(""), &block);
                let action = match &existing {
                    None => Action::Create,
                    Some(e) if *e == new_content => Action::Skip,
                    Some(_) => Action::Overwrite,
                };
                plan.push(PlanItem {
                    tool_id: tool.id,
                    skill_name: "(all)".to_string(),
                    dest,
                    action,
                    rendered: Some(new_content),
                    src_dir: None,
                    warn_bundled: false,
                });
            }
        }
    }
    Ok(plan)
}

fn folder_action(src: &Path, dest: &Path) -> miette::Result<Action> {
    if !dest.exists() {
        return Ok(Action::Create);
    }
    if dirs_identical(src, dest)? {
        Ok(Action::Skip)
    } else {
        Ok(Action::Overwrite)
    }
}

fn file_action(dest: &Path, rendered: &str) -> miette::Result<Action> {
    match read_to_string_opt(dest)? {
        None => Ok(Action::Create),
        Some(existing) if existing == rendered => Ok(Action::Skip),
        Some(_) => Ok(Action::Overwrite),
    }
}

fn read_to_string_opt(path: &Path) -> miette::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(NeoError::io_at("reading the existing file at", path, e).into()),
    }
}

fn relative_file_set(root: &Path) -> miette::Result<BTreeSet<PathBuf>> {
    let mut set = BTreeSet::new();
    for entry in walkdir::WalkDir::new(root).min_depth(1) {
        let entry = entry.map_err(|e| walkdir_io(e, root))?;
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(root).unwrap().to_path_buf();
            set.insert(rel);
        }
    }
    Ok(set)
}

fn dirs_identical(a: &Path, b: &Path) -> miette::Result<bool> {
    let a_files = relative_file_set(a)?;
    let b_files = relative_file_set(b)?;
    if a_files != b_files {
        return Ok(false);
    }
    for rel in &a_files {
        let ca = std::fs::read(a.join(rel))
            .map_err(|e| NeoError::io_at("reading a source skill file at", &a.join(rel), e))?;
        let cb = std::fs::read(b.join(rel))
            .map_err(|e| NeoError::io_at("reading an installed skill file at", &b.join(rel), e))?;
        if ca != cb {
            return Ok(false);
        }
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Writers
// ---------------------------------------------------------------------------

/// Recursively copy `src` into `dst`, creating `dst` and parents. Does NOT
/// remove `dst` first — callers wanting a clean replace must do so themselves.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> miette::Result<()> {
    std::fs::create_dir_all(dst)
        .map_err(|e| NeoError::io_at("creating the destination skill directory at", dst, e))?;
    for entry in walkdir::WalkDir::new(src).min_depth(1) {
        let entry = entry.map_err(|e| walkdir_io(e, src))?;
        let rel = entry.path().strip_prefix(src).expect("walkdir entry is under src");
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)
                .map_err(|e| NeoError::io_at("creating a skill subdirectory at", &target, e))?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| NeoError::io_at("creating a skill parent directory at", parent, e))?;
            }
            std::fs::copy(entry.path(), &target)
                .map_err(|e| NeoError::io_at("copying a skill file to", &target, e))?;
        }
    }
    Ok(())
}

/// Apply one plan item to disk. Writes for Create/Overwrite, no-op for Skip.
/// FolderCopy overwrites are a clean replace (remove then copy).
pub fn apply_item(item: &PlanItem) -> miette::Result<()> {
    if item.action == Action::Skip {
        return Ok(());
    }
    match (&item.src_dir, &item.rendered) {
        (Some(src), None) => {
            if item.dest.exists() {
                std::fs::remove_dir_all(&item.dest)
                    .map_err(|e| NeoError::io_at("removing the existing skill directory at", &item.dest, e))?;
            }
            copy_dir_recursive(src, &item.dest)?;
        }
        (None, Some(content)) => {
            if let Some(parent) = item.dest.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| NeoError::io_at("creating the destination directory at", parent, e))?;
            }
            std::fs::write(&item.dest, content)
                .map_err(|e| NeoError::io_at("writing the rendered file to", &item.dest, e))?;
        }
        _ => unreachable!("a plan item is either a folder copy or a rendered file"),
    }
    Ok(())
}

fn walkdir_io(e: walkdir::Error, root: &Path) -> NeoError {
    let io = e
        .into_io_error()
        .unwrap_or_else(|| std::io::Error::other("walkdir traversal error"));
    NeoError::io_at("walking a skill directory at", root, io)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    /// Build a checkout with one skill `<name>` (optionally with a bundled
    /// script) and return the checkout dir.
    fn checkout_with(skill: &str, desc: &str, bundled: bool) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            &format!("skills/{skill}/SKILL.md"),
            &format!("---\nname: {skill}\ndescription: {desc}\n---\n\n# {skill}\n\nBody text.\n"),
        );
        if bundled {
            write(dir.path(), &format!("skills/{skill}/scripts/run.sh"), "#!/bin/sh\necho hi\n");
        }
        dir
    }

    // ---- frontmatter ----

    #[test]
    fn parse_front_matter_valid() {
        let (fm, body) = parse_front_matter("---\nname: foo\ndescription: a bar\n---\n\n# Title\nbody\n");
        assert_eq!(fm.get("name").unwrap(), "foo");
        assert_eq!(fm.get("description").unwrap(), "a bar");
        assert_eq!(body, "# Title\nbody");
    }

    #[test]
    fn parse_front_matter_no_fence_is_all_body() {
        let (fm, body) = parse_front_matter("# Just markdown\nno frontmatter\n");
        assert!(fm.is_empty());
        assert_eq!(body, "# Just markdown\nno frontmatter\n");
    }

    #[test]
    fn parse_front_matter_unterminated_fence_is_all_body() {
        let input = "---\nname: foo\n(no closing fence)\n";
        let (fm, body) = parse_front_matter(input);
        assert!(fm.is_empty());
        assert_eq!(body, input);
    }

    #[test]
    fn parse_front_matter_handles_crlf_and_bom_and_quotes() {
        let (fm, body) =
            parse_front_matter("\u{feff}---\r\nname: \"foo\"\r\ndescription: 'a bar'\r\n---\r\nbody\r\n");
        assert_eq!(fm.get("name").unwrap(), "foo");
        assert_eq!(fm.get("description").unwrap(), "a bar");
        assert_eq!(body, "body");
    }

    #[test]
    fn parse_front_matter_empty_value_is_empty_string() {
        let (fm, _) = parse_front_matter("---\nname: foo\ndescription:\n---\n");
        assert_eq!(fm.get("name").unwrap(), "foo");
        assert_eq!(fm.get("description").unwrap(), "");
    }

    // ---- tool table ----

    #[test]
    fn tool_table_dest_paths_exact() {
        let by = |id: &str| SUPPORTED_TOOLS.iter().find(|t| t.id == id).unwrap();
        assert_eq!(by("claude").dest_hint(), ".claude/skills");
        // Codex is `.agents/skills` (plural) — NOT `.codex/skills`.
        assert_eq!(by("codex").dest_hint(), ".agents/skills");
        assert_ne!(by("codex").dest_hint(), ".codex/skills");
        // …and NOT `.agent/rules` (Antigravity, singular).
        assert_ne!(by("codex").dest_hint(), ".agent/rules");
        assert_eq!(by("kiro").dest_hint(), ".kiro/skills");
        assert_eq!(by("cursor").dest_hint(), ".cursor/rules");
        assert_eq!(by("agents").dest_hint(), "AGENTS.md");
        assert_eq!(SUPPORTED_TOOLS.len(), 5);
    }

    #[test]
    fn resolve_tools_unknown_id_errors_and_lists_valid() {
        let err = resolve_tools(&["bogus".to_string()]).unwrap_err();
        let rendered = format!("{err:?}");
        assert!(rendered.contains("bogus"), "should quote the bad id: {rendered}");
        assert!(rendered.contains("claude"), "should list valid ids: {rendered}");
    }

    #[test]
    fn resolve_tools_dedupes() {
        let tools = resolve_tools(&["claude".to_string(), "claude".to_string()]).unwrap();
        assert_eq!(tools.len(), 1);
    }

    // ---- discovery ----

    #[test]
    fn discover_skills_missing_dir_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(discover_skills(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn discover_skills_happy_sorted() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "skills/bbb/SKILL.md", "---\nname: bbb\ndescription: B\n---\nbody b\n");
        write(dir.path(), "skills/aaa/SKILL.md", "---\nname: aaa\ndescription: A\n---\nbody a\n");
        write(dir.path(), "skills/README.md", "ignored stray file");
        let skills = discover_skills(dir.path()).unwrap();
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["aaa", "bbb"]);
    }

    #[test]
    fn discover_skills_missing_skill_md_errors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("skills/broken")).unwrap();
        let err = discover_skills(dir.path()).unwrap_err();
        let rendered = format!("{err:?}");
        assert!(rendered.contains("skills/broken/SKILL.md"), "names expected path: {rendered}");
    }

    #[test]
    fn discover_skills_name_dir_mismatch_errors() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "skills/foo/SKILL.md", "---\nname: bar\ndescription: x\n---\n");
        let err = discover_skills(dir.path()).unwrap_err();
        let rendered = format!("{err:?}");
        assert!(rendered.contains("foo"), "{rendered}");
        assert!(rendered.contains("bar"), "{rendered}");
    }

    #[test]
    fn discover_skills_missing_description_errors() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "skills/foo/SKILL.md", "---\nname: foo\n---\n");
        let err = discover_skills(dir.path()).unwrap_err();
        assert!(format!("{err:?}").contains("description"));
    }

    #[test]
    fn discover_skills_flags_bundled_files() {
        let c = checkout_with("withscripts", "has a script", true);
        let skills = discover_skills(c.path()).unwrap();
        assert!(skills[0].has_bundled_files);
    }

    // ---- filter ----

    #[test]
    fn filter_skills_unknown_name_errors() {
        let c = checkout_with("foo", "d", false);
        let all = discover_skills(c.path()).unwrap();
        let err = filter_skills(all, &["nope".to_string()]).unwrap_err();
        assert!(format!("{err:?}").contains("nope"));
    }

    #[test]
    fn filter_skills_subset() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "skills/foo/SKILL.md", "---\nname: foo\ndescription: d\n---\n");
        write(dir.path(), "skills/bar/SKILL.md", "---\nname: bar\ndescription: d\n---\n");
        let all = discover_skills(dir.path()).unwrap();
        let got = filter_skills(all, &["foo".to_string()]).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "foo");
    }

    // ---- renderers ----

    #[test]
    fn render_cursor_mdc_shape() {
        let skill = Skill {
            name: "foo".into(),
            description: "Use when X".into(),
            dir: PathBuf::from("/x"),
            body: "# Foo\n\ndo the thing".into(),
            has_bundled_files: false,
        };
        let out = render_cursor_mdc(&skill);
        assert!(out.starts_with("---\ndescription: Use when X\nglobs:\nalwaysApply: false\n---\n"));
        assert!(out.contains("do the thing"));
    }

    #[test]
    fn agents_block_inlines_bodies_and_upsert_is_idempotent() {
        let skills = vec![Skill {
            name: "foo".into(),
            description: "Use when X".into(),
            dir: PathBuf::from("/x"),
            body: "Step one.".into(),
            has_bundled_files: false,
        }];
        let block = render_agents_block(&skills);
        assert!(block.contains("### foo"));
        assert!(block.contains("Use when X"));
        assert!(block.contains("Step one."));

        let existing = "# My AGENTS\n\nProject rules.\n";
        let once = upsert_managed_block(existing, &block);
        assert!(once.contains("# My AGENTS"));
        assert!(once.contains("Project rules."));
        assert!(once.contains(AGENTS_BEGIN));
        // Idempotent: re-applying to its own output is a fixed point.
        let twice = upsert_managed_block(&once, &block);
        assert_eq!(once, twice);
    }

    #[test]
    fn upsert_replaces_block_and_preserves_surrounding_content() {
        let block_v1 = render_agents_block(&[Skill {
            name: "a".into(),
            description: "first".into(),
            dir: PathBuf::from("/"),
            body: "one".into(),
            has_bundled_files: false,
        }]);
        let block_v2 = render_agents_block(&[Skill {
            name: "b".into(),
            description: "second".into(),
            dir: PathBuf::from("/"),
            body: "two".into(),
            has_bundled_files: false,
        }]);
        let base = "TOP\n";
        let v1 = upsert_managed_block(base, &block_v1);
        let tail = format!("{v1}\nBOTTOM\n");
        let v2 = upsert_managed_block(&tail, &block_v2);
        assert!(v2.starts_with("TOP\n"));
        assert!(v2.contains("BOTTOM"));
        assert!(v2.contains("### b"));
        assert!(!v2.contains("### a"), "old block must be replaced: {v2}");
    }

    // ---- plan + write ----

    #[test]
    fn build_plan_create_then_skip_then_overwrite() {
        let root = tempfile::tempdir().unwrap();
        let c = checkout_with("foo", "d", false);
        let skills = discover_skills(c.path()).unwrap();
        let tools = resolve_tools(&["claude".into(), "cursor".into(), "agents".into()]).unwrap();

        // First run: everything is Create.
        let plan = build_plan(root.path(), &tools, &skills).unwrap();
        assert!(plan.iter().all(|p| p.action == Action::Create));
        for item in &plan {
            apply_item(item).unwrap();
        }
        assert!(root.path().join(".claude/skills/foo/SKILL.md").exists());
        assert!(root.path().join(".cursor/rules/foo.mdc").exists());
        assert!(root.path().join("AGENTS.md").exists());

        // Second run with identical inputs: everything is Skip (idempotent).
        let plan2 = build_plan(root.path(), &tools, &skills).unwrap();
        assert!(plan2.iter().all(|p| p.action == Action::Skip), "expected all Skip: {plan2:?}");

        // Mutate the installed claude SKILL.md → that item becomes Overwrite.
        std::fs::write(root.path().join(".claude/skills/foo/SKILL.md"), "tampered").unwrap();
        let plan3 = build_plan(root.path(), &tools, &skills).unwrap();
        let claude_item = plan3.iter().find(|p| p.tool_id == "claude").unwrap();
        assert_eq!(claude_item.action, Action::Overwrite);
    }

    #[test]
    fn folder_copy_overwrite_is_clean_replace() {
        let root = tempfile::tempdir().unwrap();
        let c = checkout_with("foo", "d", false);
        let skills = discover_skills(c.path()).unwrap();
        let tools = resolve_tools(&["claude".into()]).unwrap();
        for item in build_plan(root.path(), &tools, &skills).unwrap() {
            apply_item(&item).unwrap();
        }
        // Add a stale extra file under the installed dir, then re-install.
        let stale = root.path().join(".claude/skills/foo/STALE.txt");
        std::fs::write(&stale, "old").unwrap();
        std::fs::write(root.path().join(".claude/skills/foo/SKILL.md"), "tampered").unwrap();
        for item in build_plan(root.path(), &tools, &skills).unwrap() {
            apply_item(&item).unwrap();
        }
        // Clean replace: stale file gone, SKILL.md restored from source.
        assert!(!stale.exists(), "overwrite must remove stale files");
        let restored = std::fs::read_to_string(root.path().join(".claude/skills/foo/SKILL.md")).unwrap();
        assert!(restored.contains("name: foo"));
    }

    #[test]
    fn cursor_rule_flags_bundled_and_copy_preserves_them() {
        let root = tempfile::tempdir().unwrap();
        let c = checkout_with("foo", "d", true);
        let skills = discover_skills(c.path()).unwrap();
        let tools = resolve_tools(&["claude".into(), "cursor".into()]).unwrap();
        let plan = build_plan(root.path(), &tools, &skills).unwrap();
        let cursor_item = plan.iter().find(|p| p.tool_id == "cursor").unwrap();
        assert!(cursor_item.warn_bundled, "cursor item must flag dropped bundled scripts");
        for item in &plan {
            apply_item(item).unwrap();
        }
        // FolderCopy preserves the bundled script; CursorRule cannot.
        assert!(root.path().join(".claude/skills/foo/scripts/run.sh").exists());
        assert!(root.path().join(".cursor/rules/foo.mdc").exists());
    }
}

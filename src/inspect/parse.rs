//! Dumb-string-search parser for a single NeoHaskell domain directory.
//!
//! We treat `.hs` files as text. The NeoHaskell convention puts each
//! command/query/integration in its own file with a known function name
//! (`decide`, `handleEvent`) — we extract a "function body" by finding
//! the function header and slurping until the next top-level definition
//! (next line starting at column 0 with a non-comment identifier or
//! a module-level keyword). Inside that body, constructor matching is
//! a plain-substring scan filtered against the known event/command name
//! set, so we don't need a real Haskell lexer.
//!
//! False positives ARE possible (e.g. `ItemAdded` appearing in a doc
//! comment) but our scope is "give the heal prompt a 90%-accurate
//! cross-reference table" — not a verifier. The agent can still spot-
//! check anything that looks off.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use regex::Regex;

use super::{CommandInfo, EventInfo, IntegrationInfo, IntegrationKind, QueryInfo};

/// Parse `<dir>/Core.hs` and `<dir>/Event.hs` (whichever exists) for the
/// event sum constructors. Returns them in source order so the heal
/// prompt can list events the way the developer arranged them.
pub fn events_in_domain(dir: &Path) -> Vec<EventInfo> {
    let mut out = Vec::new();
    // Payload-module names referenced by the event sum type's arms. In the
    // CIOS convention an arm reads `ProposalPdfTranscribed PdfTranscribed.Event`
    // — `ProposalPdfTranscribed` is the canonical event constructor and
    // `PdfTranscribed` is merely the payload module living at
    // `Events/PdfTranscribed.hs`. We collect those payload names so the
    // `Events/` directory scan below does NOT mint them as phantom events
    // (which used to double every such event: once as the constructor, once
    // as the bare file stem, with one copy left dangling as a dead-end leaf).
    let mut payload_modules: BTreeSet<String> = BTreeSet::new();

    for fname in ["Core.hs", "Event.hs"] {
        let path = dir.join(fname);
        if !path.is_file() {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        for name in extract_event_constructors(&body) {
            // Dedup across Core.hs + Event.hs without disturbing source order.
            if !out.iter().any(|e: &EventInfo| e.name == name) {
                out.push(EventInfo {
                    name,
                    file: path.clone(),
                });
            }
        }
        payload_modules.extend(extract_event_payload_modules(&body));
    }

    let constructor_set: BTreeSet<String> = out.iter().map(|e| e.name.clone()).collect();

    // Also scan one-event-per-file layout: Events/<Name>.hs. A file stem is
    // only a standalone event when it is NOT a payload module of an existing
    // constructor. When `constructor == payload module` (e.g.
    // `EvaluationTriggered EvaluationTriggered.Event`) the stem matches a
    // constructor and is kept (the exact-name dedup keeps it single). When
    // there is no sum type at all (older one-event-per-file projects),
    // `payload_modules` is empty and every stem is kept — preserving the
    // original behaviour.
    let events_dir = dir.join("Events");
    if events_dir.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&events_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        // Deterministic order regardless of filesystem enumeration order.
        entries.sort();
        for path in entries {
            if path.extension().and_then(|s| s.to_str()) != Some("hs") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if payload_modules.contains(stem) && !constructor_set.contains(stem) {
                // It's the payload of an entity-prefixed constructor we
                // already recorded — not its own event.
                continue;
            }
            if !out.iter().any(|e| e.name == stem) {
                out.push(EventInfo {
                    name: stem.to_string(),
                    file: path,
                });
            }
        }
    }

    out
}

pub fn commands_in_domain(dir: &Path, known_events: &[String]) -> Vec<CommandInfo> {
    list_dot_hs(dir.join("Commands").as_path())
        .into_iter()
        .filter_map(|path| parse_command_file(&path, known_events))
        .collect()
}

pub fn queries_in_domain(dir: &Path, known_events: &[String]) -> Vec<QueryInfo> {
    list_dot_hs(dir.join("Queries").as_path())
        .into_iter()
        .filter_map(|path| parse_query_file(&path, known_events))
        .collect()
}

pub fn integrations_in_domain(dir: &Path, known_events: &[String]) -> Vec<IntegrationInfo> {
    // Two passes; results UNIONed by integration name:
    //
    // PASS A — Per-file `handleEvent`. Each `Integrations/<Name>.hs` file
    // that defines its OWN `handleEvent :: ... -> Integration.Outbound`
    // (Payment-style in CIOS, also the testbed convention) is parsed
    // standalone — strict arm-by-arm scan of the `case event of` block,
    // arms that resolve to `Integration.none` are not counted.
    //
    // PASS B — Domain dispatcher `<Domain>/Integrations.hs`. When this
    // module exists (Proposal / ProposalMetricEvaluation style in CIOS),
    // it imports each handler function from its sub-module and routes
    // events to handlers via a single `case event of` block. We map the
    // import statements (`fnName` → `IntgName`) and walk the dispatcher's
    // case arms to figure out which integration handles which event,
    // without depending on the dispatcher function's name.
    let mut by_name: std::collections::BTreeMap<String, IntegrationInfo> =
        std::collections::BTreeMap::new();
    for path in list_dot_hs(dir.join("Integrations").as_path()) {
        if let Some(info) = parse_integration_file(&path, known_events) {
            by_name.insert(info.name.clone(), info);
        }
    }
    augment_from_dispatcher(dir, known_events, &mut by_name);
    // Drop plumbing-only modules. An `Integrations/<Name>.hs` that handles
    // NO event AND emits NO command is a pure helper (HTTP client, JSON
    // codec, request builder — e.g. CIOS Payment's `BankHttp`/`EvocaBank`),
    // not an event-model integration. Emitting a node for it shows up as a
    // dead orphan in the healer's graph. We keep an integration the moment
    // EITHER list is non-empty: an outbound integration that HANDLES an
    // event but emits no command (e.g. a Brevo email call triggered by an
    // event) has non-empty `handles_events` and MUST be kept.
    by_name.retain(|_, info| !info.handles_events.is_empty() || !info.emits_commands.is_empty());
    by_name.into_values().collect()
}

/// Read `<dir>/Integrations.hs` if present; merge its event-to-integration
/// mapping into `by_name`. Integrations that ONLY appear in the dispatcher
/// (no `Integrations/<Name>.hs` file) get an entry synthesised from the
/// dispatcher import — kind defaults to `Outbound`, emits derived from
/// the file body if it exists.
fn augment_from_dispatcher(
    dir: &Path,
    known_events: &[String],
    by_name: &mut std::collections::BTreeMap<String, IntegrationInfo>,
) {
    let dispatcher_path = dir.join("Integrations.hs");
    let Ok(body) = std::fs::read_to_string(&dispatcher_path) else {
        return;
    };
    let import_map = extract_integration_import_map(&body);
    if import_map.is_empty() {
        return;
    }
    let arm_map = extract_dispatcher_arms(&body, known_events, &import_map);
    for (intg_name, events) in arm_map {
        let entry = by_name.entry(intg_name.clone()).or_insert_with(|| {
            let file = dir.join(format!("Integrations/{intg_name}.hs"));
            let emits = if file.is_file() {
                std::fs::read_to_string(&file)
                    .map(|b| extract_emitted_commands(&b))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let kind = if emits.is_empty() {
                IntegrationKind::Outbound
            } else {
                IntegrationKind::Reactive
            };
            IntegrationInfo {
                name: intg_name.clone(),
                file,
                kind,
                handles_events: Vec::new(),
                emits_commands: emits,
            }
        });
        for e in events {
            if !entry.handles_events.contains(&e) {
                entry.handles_events.push(e);
            }
        }
    }
}

/// Parse `import …Integrations.<IntgName> (fn1, fn2)` lines from the
/// dispatcher module, returning `fnName → IntgName`. Qualified imports
/// (`qualified as Alias`) are skipped — the dispatcher arms reference
/// the bare function name in CIOS convention, not an alias.
fn extract_integration_import_map(body: &str) -> std::collections::BTreeMap<String, String> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            // `import <…>.Integrations.<IntgName> ( fn1, fn2, ... )`
            r"(?m)^\s*import\s+(?:[A-Z]\w*\.)*Integrations\.([A-Z]\w*)\s*\(([^)]*)\)",
        )
        .unwrap()
    });
    let mut out = std::collections::BTreeMap::new();
    for cap in re.captures_iter(body) {
        let Some(intg) = cap.get(1) else { continue };
        let intg_name = intg.as_str().to_string();
        let imports = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        for raw in imports.split(',') {
            let trimmed = raw.trim();
            // Function names start with a lowercase letter; data types
            // and constructors (uppercase) live elsewhere.
            let fn_name: String = trimmed
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if let Some(first) = fn_name.chars().next() {
                if first.is_ascii_lowercase() {
                    out.insert(fn_name, intg_name.clone());
                }
            }
        }
    }
    out
}

/// Walk the dispatcher body looking for `case <var> of …` arms whose
/// head is a known event constructor. For each such arm, scan its RHS
/// for handler-function-name tokens that appear in `import_map`. Records
/// `IntgName → [EventCtor]` for every match. Arms whose RHS contains
/// only `Integration.none` (or no handler-fn match at all) contribute
/// nothing — that's how Payment-style "don't react to this event"
/// declarations are correctly excluded.
fn extract_dispatcher_arms(
    body: &str,
    known_events: &[String],
    import_map: &std::collections::BTreeMap<String, String>,
) -> std::collections::BTreeMap<String, Vec<String>> {
    let candidate_set: std::collections::BTreeSet<&str> =
        known_events.iter().map(String::as_str).collect();
    let lines: Vec<&str> = body.lines().collect();

    let is_arm_start = |line: &str| -> bool {
        let trimmed = line.trim_start();
        let first = match trimmed.chars().next() {
            Some(c) => c,
            None => return false,
        };
        if first == '_' {
            let next = trimmed.as_bytes().get(1).copied();
            return matches!(next, None | Some(b' ') | Some(b'\t'));
        }
        if !first.is_ascii_uppercase() {
            return false;
        }
        let end = trimmed
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(trimmed.len());
        candidate_set.contains(&trimmed[..end])
    };

    let mut result: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for i in 0..lines.len() {
        let trimmed = lines[i].trim_start();
        let first_word_end = trimmed
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(trimmed.len());
        let first_word = &trimmed[..first_word_end];
        if !candidate_set.contains(first_word) {
            continue;
        }

        // Gather this arm's body: from this line to (exclusive) the next arm-start.
        let mut arm = String::from(lines[i]);
        for j in (i + 1)..lines.len() {
            if is_arm_start(lines[j]) {
                break;
            }
            arm.push('\n');
            arm.push_str(lines[j]);
        }

        for (fn_name, intg_name) in import_map {
            if contains_word(&arm, fn_name) {
                let bucket = result.entry(intg_name.clone()).or_default();
                if !bucket.contains(&first_word.to_string()) {
                    bucket.push(first_word.to_string());
                }
            }
        }
    }
    result
}

fn list_dot_hs(dir: &Path) -> Vec<PathBuf> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    walk_hs(dir, &mut out);
    out.sort();
    out
}

fn walk_hs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk_hs(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("hs") {
            out.push(p);
        }
    }
}

fn parse_command_file(path: &Path, known_events: &[String]) -> Option<CommandInfo> {
    let body = std::fs::read_to_string(path).ok()?;
    let name = path.file_stem()?.to_str()?.to_string();
    let decide_body = extract_function_body(&body, "decide").unwrap_or_default();
    let produces = filter_present(&decide_body, known_events);
    let via_web_transport = body.contains("WebTransport");
    Some(CommandInfo {
        name,
        file: path.to_path_buf(),
        produces,
        via_web_transport,
    })
}

fn parse_query_file(path: &Path, known_events: &[String]) -> Option<QueryInfo> {
    let body = std::fs::read_to_string(path).ok()?;
    let name = path.file_stem()?.to_str()?.to_string();
    let subscribes_to = filter_present(&body, known_events);
    Some(QueryInfo {
        name,
        file: path.to_path_buf(),
        subscribes_to,
    })
}

fn parse_integration_file(path: &Path, known_events: &[String]) -> Option<IntegrationInfo> {
    let body = std::fs::read_to_string(path).ok()?;
    let name = path.file_stem()?.to_str()?.to_string();
    // STRICT per-file scan: look for a `handleEvent` function and walk
    // its `case event of` block, counting only arms whose RHS does any
    // `Integration.<verb>` other than `Integration.none` or any
    // `Command.Emit`. False positives from imports of event-types-for-
    // context are correctly excluded.
    //
    // For Pattern-A integrations (CIOS Proposal / ProposalMetricEvaluation)
    // whose handler function isn't named `handleEvent`, this scan returns
    // empty — that's fine; the dispatcher pass in `augment_from_dispatcher`
    // fills in the events afterwards.
    let handle_body = extract_function_body(&body, "handleEvent").unwrap_or_default();
    let handles_events = active_handles_in_case_body(&handle_body, known_events);
    // Emission can happen either DIRECTLY inside the handler body
    // (testbed-style: `Command.Emit { command = X { … } }`) or via a
    // sibling `ToAction` instance elsewhere in the same file
    // (CIOS-style: `Integration.emitCommand X { … }`). Scan the whole
    // file so the kind classifier finds both.
    let emits_commands = extract_emitted_commands(&body);
    let kind = if emits_commands.is_empty() {
        IntegrationKind::Outbound
    } else {
        IntegrationKind::Reactive
    };
    Some(IntegrationInfo {
        name,
        file: path.to_path_buf(),
        kind,
        handles_events,
        emits_commands,
    })
}

/// Walk a `case <evt> of …` body and return only the event constructors
/// whose arm body actually does something — i.e. whose RHS has any
/// `Integration.<word>` other than `Integration.none`, or any
/// `Command.Emit`. Wildcard arms (`_ -> …`) and arms whose only RHS is
/// `Integration.none` MUST NOT count as "handled".
fn active_handles_in_case_body(body: &str, candidates: &[String]) -> Vec<String> {
    let candidate_set: std::collections::BTreeSet<&str> =
        candidates.iter().map(String::as_str).collect();

    let is_arm_start = |line: &str| -> bool {
        let trimmed = line.trim_start();
        let first = match trimmed.chars().next() {
            Some(c) => c,
            None => return false,
        };
        if first == '_' {
            let next = trimmed.as_bytes().get(1).copied();
            return matches!(next, None | Some(b' ') | Some(b'\t'));
        }
        if !first.is_ascii_uppercase() {
            return false;
        }
        let end = trimmed
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(trimmed.len());
        candidate_set.contains(&trimmed[..end])
    };

    let lines: Vec<&str> = body.lines().collect();
    let mut active = std::collections::BTreeSet::new();
    for i in 0..lines.len() {
        let trimmed = lines[i].trim_start();
        let first_word_end = trimmed
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(trimmed.len());
        let first_word = &trimmed[..first_word_end];
        if !candidate_set.contains(first_word) {
            continue;
        }
        let mut arm = String::from(lines[i]);
        for j in (i + 1)..lines.len() {
            if is_arm_start(lines[j]) {
                break;
            }
            arm.push('\n');
            arm.push_str(lines[j]);
        }
        if arm_is_active(&arm) {
            active.insert(first_word.to_string());
        }
    }
    candidates
        .iter()
        .filter(|c| active.contains(c.as_str()))
        .cloned()
        .collect()
}

/// True iff the arm body has at least one non-`none` Integration verb or
/// any `Command.Emit`.
fn arm_is_active(arm: &str) -> bool {
    if arm.contains("Command.Emit") {
        return true;
    }
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\bIntegration\.([A-Za-z_]\w*)\b").unwrap());
    for cap in re.captures_iter(arm) {
        if cap.get(1).map(|m| m.as_str()) != Some("none") {
            return true;
        }
    }
    false
}

/// Extract constructor names from the event sum:
///
///     data CartEvent
///       = CartCreated { ... }
///       | ItemAdded { ... }
///       | ...
///
/// Limited to the FIRST `data X ... =` block that mentions `Event` in
/// the type name — that's the convention in NeoHaskell domains.
fn extract_event_constructors(src: &str) -> Vec<String> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        // `(?m)` so `^` matches at line starts; `\z` (not multiline `$`)
        // so the non-greedy body keeps going past blank lines until
        // either the next top-level non-indented line or the end of input.
        Regex::new(r"(?m)^data\s+([A-Z]\w*Event)\b[^=]*=([\s\S]*?)(?:\n\S|\z)").unwrap()
    });
    let Some(cap) = re.captures(src) else {
        return Vec::new();
    };
    let block = cap.get(2).map(|m| m.as_str()).unwrap_or("");
    // Constructor names are tokens starting with uppercase letters,
    // appearing right after `=` or `|`.
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for raw in block.split('|') {
        let cleaned = raw.trim_start();
        let cleaned = cleaned.trim_start_matches(|c: char| c == '|' || c.is_whitespace());
        let ident: String = cleaned
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if let Some(first) = ident.chars().next() {
            if first.is_ascii_uppercase() && seen.insert(ident.clone()) {
                out.push(ident);
            }
        }
    }
    out
}

/// Extract the payload-module names referenced by an event sum type's
/// arms. For
///
///     data ProposalEvent
///       = ProposalPdfUploaded    PdfUploaded.Event
///       | ProposalPdfTranscribed PdfTranscribed.Event
///       | EvaluationTriggered    EvaluationTriggered.Event
///
/// this returns `{PdfUploaded, PdfTranscribed, EvaluationTriggered}` — the
/// module qualifier in front of the `.Event` payload type on each arm. These
/// are the file stems under `Events/` that must NOT be minted as their own
/// events (they are payloads of the constructors on the left). Arms written
/// with an inline record (`CartCreated { entityId :: Uuid }`) contribute
/// nothing — there is no separate payload module to suppress.
fn extract_event_payload_modules(src: &str) -> BTreeSet<String> {
    static BLOCK_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static PAYLOAD_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let block_re = BLOCK_RE.get_or_init(|| {
        Regex::new(r"(?m)^data\s+([A-Z]\w*Event)\b[^=]*=([\s\S]*?)(?:\n\S|\z)").unwrap()
    });
    // A payload reference: an upper-case module identifier immediately
    // followed by `.Event` (the NeoHaskell payload-type convention).
    let payload_re = PAYLOAD_RE.get_or_init(|| Regex::new(r"\b([A-Z]\w*)\.Event\b").unwrap());
    let mut out = BTreeSet::new();
    let Some(cap) = block_re.captures(src) else {
        return out;
    };
    let block = cap.get(2).map(|m| m.as_str()).unwrap_or("");
    for pc in payload_re.captures_iter(block) {
        if let Some(m) = pc.get(1) {
            out.insert(m.as_str().to_string());
        }
    }
    out
}

/// Slurp the body of a top-level Haskell function definition. Finds the
/// line that starts with `<name>` at column 0 (the function clause) and
/// returns everything from there until the next column-0 declaration.
fn extract_function_body(src: &str, name: &str) -> Option<String> {
    let mut found_at: Option<usize> = None;
    let mut end_at = src.len();
    let mut byte_cursor = 0usize;
    let mut iter = src.lines();

    while let Some(line) = iter.next() {
        let line_start = byte_cursor;
        byte_cursor += line.len() + 1; // +1 for the '\n' (works for typical \n; if \r\n, harmless)

        if found_at.is_none() {
            // Looking for the function clause: a line starting with `<name>` at column 0
            // followed by a space, `(`, or `=`.
            if line.starts_with(name) {
                let rest = &line[name.len()..];
                if rest
                    .chars()
                    .next()
                    .map(|c| c.is_whitespace() || c == '(' || c == ':' || c == '=')
                    .unwrap_or(false)
                {
                    found_at = Some(line_start);
                }
            }
        } else if !line.is_empty() && !line.starts_with(char::is_whitespace) {
            // Comments and pragmas don't end the body.
            if line.starts_with("--") || line.starts_with("{-") {
                continue;
            }
            // Continuation clauses of the SAME function — pattern-match
            // arms like `decide ... = ...` and `decide _ _ = ...` — also
            // don't end the body. Identify by the leading word.
            let first_word_end = line
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(line.len());
            let first_word = &line[..first_word_end];
            if first_word == name {
                continue;
            }
            end_at = line_start;
            break;
        }
    }

    found_at.map(|start| src[start..end_at].to_string())
}

/// Return the elements of `candidates` (in their original order) that
/// appear as a whole-word token inside `haystack`.
fn filter_present(haystack: &str, candidates: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for c in candidates {
        if contains_word(haystack, c) && seen.insert(c.clone()) {
            out.push(c.clone());
        }
    }
    out
}

/// Whole-word match. We can't rely on regex word boundaries because
/// constructor names like `OrderPlaced_v2` should NOT match `OrderPlaced`
/// — the trailing char must not be alphanumeric/underscore.
fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut start = 0;
    while let Some(idx) = haystack[start..].find(needle) {
        let abs = start + idx;
        let before_ok = abs == 0
            || !haystack
                .as_bytes()
                .get(abs - 1)
                .map(|b| (*b as char).is_alphanumeric() || *b == b'_')
                .unwrap_or(false);
        let end = abs + needle.len();
        let after_ok = end >= haystack.len()
            || !haystack
                .as_bytes()
                .get(end)
                .map(|b| (*b as char).is_alphanumeric() || *b == b'_')
                .unwrap_or(false);
        if before_ok && after_ok {
            return true;
        }
        start = abs + 1;
    }
    false
}

/// Find every emitted command and return the constructor names. Two
/// idioms in the wild:
///
///   * Testbed (`Service.Command.Core`): `Command.Emit { command = X { … } }`
///   * App-specific helpers (CIOS, etc.): `Integration.emitCommand X { … }`
///     or `Integration.emitCommand\n  X { … }` (constructor on next line).
fn extract_emitted_commands(src: &str) -> Vec<String> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?:Command\.Emit\s*\{\s*command\s*=|Integration\.emitCommand)\s*([A-Z]\w*)",
        )
        .unwrap()
    });
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for cap in re.captures_iter(src) {
        if let Some(m) = cap.get(1) {
            let s = m.as_str().to_string();
            if seen.insert(s.clone()) {
                out.push(s);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_event_constructors_from_data_block() {
        let src = r#"
module Foo where
data CartEvent
  = CartCreated { entityId :: Uuid }
  | ItemAdded { stockId :: Uuid }
  | ItemRemoved { stockId :: Uuid }
  deriving (Generic)
"#;
        assert_eq!(
            extract_event_constructors(src),
            vec!["CartCreated", "ItemAdded", "ItemRemoved"]
        );
    }

    #[test]
    fn extract_event_constructors_returns_empty_when_no_event_sum() {
        let src = "module Foo where\nbar = 1\n";
        assert!(extract_event_constructors(src).is_empty());
    }

    #[test]
    fn extracts_decide_body_until_next_top_level() {
        let src = r#"
module Foo where
decide :: Cmd -> Decision Event
decide cmd _ = case x of
  Just _ -> Decider.acceptExisting [Foo {}]
  _ -> Decider.reject "no"
type instance EntityOf Cmd = Bar
"#;
        let body = extract_function_body(src, "decide").expect("body");
        assert!(body.contains("Decider.acceptExisting"));
        assert!(body.contains("Foo {}"));
        assert!(!body.contains("type instance"));
    }

    #[test]
    fn extract_function_body_returns_none_when_function_missing() {
        let src = "module X where\nfoo = 1\n";
        assert!(extract_function_body(src, "decide").is_none());
    }

    #[test]
    fn contains_word_respects_token_boundaries() {
        assert!(contains_word("emit ItemAdded {}", "ItemAdded"));
        assert!(!contains_word("ItemAddedFoo", "ItemAdded"));
        assert!(!contains_word("FooItemAdded", "ItemAdded"));
        assert!(contains_word("[ItemAdded]", "ItemAdded"));
        assert!(contains_word("ItemAdded", "ItemAdded"));
    }

    #[test]
    fn filter_present_returns_only_real_matches_in_order() {
        let candidates = vec![
            "CartCreated".to_string(),
            "ItemAdded".to_string(),
            "ItemRemoved".to_string(),
        ];
        let body = "do Decider.acceptExisting [ItemAdded {entityId = i}]";
        assert_eq!(filter_present(body, &candidates), vec!["ItemAdded".to_string()]);
    }

    #[test]
    fn extract_emitted_commands_finds_command_dot_emit() {
        let src = r#"
do
  Integration.outbound
    Command.Emit
      { command = ReserveStock { quantity = q }
      }
  Integration.outbound
    Command.Emit { command = NotifyCustomer { id = c } }
"#;
        assert_eq!(
            extract_emitted_commands(src),
            vec!["ReserveStock".to_string(), "NotifyCustomer".to_string()]
        );
    }

    #[test]
    fn extract_emitted_commands_recognises_integration_emit_command_idiom() {
        // CIOS-style: a helper that wraps emission so the surface API is
        // `Integration.emitCommand <Ctor> { … }` rather than the
        // `Command.Emit { command = … }` style used in the testbed.
        let src = r#"
toAction req = Integration.action \_ctx ->
  Integration.emitCommand
    SendThankYouEmail
      { paymentId = req.paymentId
      , payerEmail = req.payerEmail
      }
"#;
        assert_eq!(
            extract_emitted_commands(src),
            vec!["SendThankYouEmail".to_string()]
        );
    }

    #[test]
    fn extract_emitted_commands_dedups() {
        let src = r#"
Command.Emit { command = Foo {} }
Command.Emit { command = Foo {} }
"#;
        assert_eq!(extract_emitted_commands(src), vec!["Foo".to_string()]);
    }

    #[test]
    fn extract_event_payload_modules_pulls_module_qualifiers() {
        let src = r#"
data ProposalEvent
  = ProposalPdfUploaded PdfUploaded.Event
  | ProposalPdfTranscribed PdfTranscribed.Event
  | EvaluationTriggered EvaluationTriggered.Event
  deriving (Generic, Show)
"#;
        let mods = extract_event_payload_modules(src);
        assert!(mods.contains("PdfUploaded"), "got {mods:?}");
        assert!(mods.contains("PdfTranscribed"), "got {mods:?}");
        assert!(mods.contains("EvaluationTriggered"), "got {mods:?}");
    }

    #[test]
    fn extract_event_payload_modules_empty_for_inline_record_arms() {
        let src = r#"
data CartEvent
  = CartCreated { entityId :: Uuid }
  | ItemAdded { stockId :: Uuid }
  deriving (Generic)
"#;
        assert!(extract_event_payload_modules(src).is_empty());
    }

    #[test]
    fn events_in_domain_skips_payload_modules_and_keeps_constructors() {
        // Regression: an event sum whose constructor is entity-prefixed
        // (`ProposalPdfTranscribed`) but whose payload module is bare
        // (`Events/PdfTranscribed.hs`) must yield ONE event named after the
        // constructor — not two (constructor + dangling payload stem).
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("Events")).unwrap();
        std::fs::write(
            root.join("Event.hs"),
            "module Datalake.Proposal.Event where\n\
             data ProposalEvent\n  \
               = ProposalPdfTranscribed PdfTranscribed.Event\n  \
               | EvaluationTriggered EvaluationTriggered.Event\n  \
               deriving (Generic, Show)\n",
        )
        .unwrap();
        std::fs::write(root.join("Events/PdfTranscribed.hs"), "module X where\ndata Event = Event {}\n").unwrap();
        std::fs::write(
            root.join("Events/EvaluationTriggered.hs"),
            "module X where\ndata Event = Event {}\n",
        )
        .unwrap();
        let events: Vec<String> = events_in_domain(root).into_iter().map(|e| e.name).collect();
        assert_eq!(
            events,
            vec!["ProposalPdfTranscribed".to_string(), "EvaluationTriggered".to_string()],
            "payload stem PdfTranscribed must be suppressed; EvaluationTriggered (ctor==module) kept once",
        );
    }

    #[test]
    fn events_in_domain_keeps_bare_file_events_when_no_sum_type() {
        // Older one-event-per-file projects have no `data XEvent` block, so
        // the Events/ stems ARE the events — preserve that.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("Events")).unwrap();
        std::fs::write(root.join("Core.hs"), "module X.Core where\n").unwrap();
        std::fs::write(root.join("Events/ThingHappened.hs"), "module X where\ndata Event = Event {}\n").unwrap();
        let events: Vec<String> = events_in_domain(root).into_iter().map(|e| e.name).collect();
        assert_eq!(events, vec!["ThingHappened".to_string()]);
    }
}

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
    }

    // Also scan one-event-per-file layout: Events/<Name>.hs
    let events_dir = dir.join("Events");
    if events_dir.is_dir() {
        for entry in std::fs::read_dir(&events_dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("hs") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            // Each file's name is the constructor name by convention.
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
    list_dot_hs(dir.join("Integrations").as_path())
        .into_iter()
        .filter_map(|path| parse_integration_file(&path, known_events))
        .collect()
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
    let handle_body = extract_function_body(&body, "handleEvent").unwrap_or_default();
    let handles_events = active_handles_in_case_body(&handle_body, known_events);
    // Emission can happen either DIRECTLY inside the handleEvent body
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

/// Walk a `handleEvent` `case <evt> of …` body and return only the event
/// constructors whose arm body actually does something — i.e. whose body
/// has any `Integration.<word>` other than `Integration.none`, or any
/// `Command.Emit`. Wildcard arms (`_ -> …`) and arms whose only RHS is
/// `Integration.none` MUST NOT count as "handled" — otherwise every
/// integration looks like it listens to every event in the sum.
fn active_handles_in_case_body(body: &str, candidates: &[String]) -> Vec<String> {
    let candidate_set: std::collections::BTreeSet<&str> =
        candidates.iter().map(String::as_str).collect();

    // An "arm start" is a line whose first non-whitespace token is either
    // a known event constructor or the wildcard `_`. We use these as
    // arm boundaries when slicing the body.
    let is_arm_start = |line: &str| -> bool {
        let trimmed = line.trim_start();
        let first = match trimmed.chars().next() {
            Some(c) => c,
            None => return false,
        };
        if first == '_' {
            // Distinguish `_` arm from a variable name like `_event`.
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

        // Collect this arm's body: starting line through (exclusive)
        // the next arm-start line.
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
/// any `Command.Emit`. We don't require a specific verb name; in practice
/// `Integration.batch`, `Integration.outbound`, `Integration.send`, and
/// project-specific helpers all signal an active arm.
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
    fn active_handles_skips_integration_none_arms() {
        // A real-world handleEvent: only PaymentRequested is actively
        // handled; every other event in the sum just falls through to
        // `Integration.none`. The bug pre-fix had us reporting EVERY
        // event as "handled" because the constructor name appeared in
        // the body — even though that arm explicitly skipped.
        let body = r#"
handleEvent _entity event =
  case event of
    PaymentRequested {} ->
      Integration.batch
        [ Integration.outbound BankFormRequest {} ]
    PaymentFormPrepared _ -> Integration.none
    PaymentFormPreparationFailed _ -> Integration.none
    PayerReturnedFromPaymentForm _ -> Integration.none
    PaymentApproved _ -> Integration.none
"#;
        let candidates = vec![
            "PaymentRequested".to_string(),
            "PaymentFormPrepared".to_string(),
            "PaymentFormPreparationFailed".to_string(),
            "PayerReturnedFromPaymentForm".to_string(),
            "PaymentApproved".to_string(),
        ];
        let out = active_handles_in_case_body(body, &candidates);
        assert_eq!(out, vec!["PaymentRequested".to_string()]);
    }

    #[test]
    fn active_handles_counts_reactive_command_emit_arms() {
        let body = r#"
handleEvent _ event = case event of
  ItemAdded {} -> Integration.outbound Command.Emit { command = NotifyStock {} }
  CartCleared _ -> Integration.none
"#;
        let candidates = vec!["ItemAdded".to_string(), "CartCleared".to_string()];
        let out = active_handles_in_case_body(body, &candidates);
        assert_eq!(out, vec!["ItemAdded".to_string()]);
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
}

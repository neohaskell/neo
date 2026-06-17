//! `workspace/healEventModel` — invoke `claude -p` to repair a malformed
//! `event-model.json` in-place.
//!
//! Contract:
//!   1. Read `<workspace_root>/event-model.json` and re-validate.
//!   2. If already valid, return `Healed` (no-op).
//!   3. Otherwise spawn `claude -p` with the validation errors + schema
//!      inlined into the prompt and the workspace scoped via `--add-dir`.
//!   4. After the subprocess exits, re-read and re-validate.
//!   5. Return `Healed` if now valid, `StillInvalid { errors }` if not.
//!
//! Errors (surface as JSON-RPC `RpcError`, not as part of the success outcome):
//!   - `NeoError::HealingClaudeMissing` — `claude` not on PATH
//!   - `NeoError::HealingFailed` — subprocess exited non-zero, hit timeout,
//!     or otherwise could not run to completion
//!   - `NeoError::IoErrorAt` — file disappeared or became unreadable

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;

use crate::errors::NeoError;
use crate::ide::methods::read_event_model::EVENT_MODEL_FILENAME;
use crate::ide::session::Session;
use crate::ide::validate::{self, ErrorKind, ValidationError, ValidationOutcome, SCHEMA_JSON};

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HealEventModelParams {
    /// How aggressively to invoke the agent.
    ///
    /// - `Validate` (default, used by the auto-triggered modal): only spawn
    ///   `claude` if the file actually fails validation. If it's already
    ///   valid, return `Healed` immediately as a no-op.
    /// - `Improve` (used by the manual "Heal with AI" button): always spawn
    ///   `claude` regardless of validation state. Lets the user ask the
    ///   agent to fix layout / add inferred edges on a passing file.
    #[serde(default)]
    pub mode: HealMode,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum HealMode {
    #[default]
    Validate,
    Improve,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "status")]
pub enum HealOutcome {
    Healed,
    StillInvalid { errors: Vec<ValidationError> },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealEventModelResult {
    pub outcome: HealOutcome,
}

/// Knobs that production callers don't touch but tests override. Production
/// goes through `handle(...)` which uses `HealConfig::default()` — `claude`
/// on PATH, 5-minute timeout.
#[derive(Debug, Clone)]
pub struct HealConfig {
    /// Path to the `claude` binary. Default: `"claude"` (resolved via PATH).
    pub claude_binary: PathBuf,
    /// Hard timeout for the subprocess.
    pub timeout: Duration,
}

impl Default for HealConfig {
    fn default() -> Self {
        Self {
            claude_binary: PathBuf::from("claude"),
            timeout: Duration::from_secs(300),
        }
    }
}

pub async fn handle(
    session: Session,
    params: HealEventModelParams,
) -> Result<HealEventModelResult, NeoError> {
    handle_with_config(session, params.mode, HealConfig::default()).await
}

pub(crate) async fn handle_with_config(
    session: Session,
    mode: HealMode,
    config: HealConfig,
) -> Result<HealEventModelResult, NeoError> {
    let path = session.workspace.root.join(EVENT_MODEL_FILENAME);
    tracing::info!(path = %path.display(), ?mode, "heal: starting");

    let content = std::fs::read_to_string(&path).map_err(|e| {
        NeoError::io_at(
            "reading `event-model.json` to start healing",
            path.clone(),
            e,
        )
    })?;

    let initial_errors = match validate::validate_event_model(&content) {
        ValidationOutcome::Valid => {
            if mode == HealMode::Validate {
                tracing::info!("heal: file already valid, no-op (mode=validate)");
                return Ok(HealEventModelResult {
                    outcome: HealOutcome::Healed,
                });
            }
            tracing::info!(
                "heal: file already valid but mode=improve — running claude anyway to refine layout/edges",
            );
            Vec::new()
        }
        ValidationOutcome::Invalid { errors } => {
            tracing::info!(
                error_count = errors.len(),
                first_pointer = %errors.first().map(|e| e.pointer.as_str()).unwrap_or(""),
                "heal: schema/referential errors detected",
            );
            errors
        }
        ValidationOutcome::MalformedJson { parse_error } => {
            tracing::info!(parse_error = %parse_error, "heal: file is malformed JSON");
            vec![ValidationError {
                pointer: String::new(),
                message: format!(
                    "file is not valid JSON: {parse_error}. The whole document must be parseable JSON before any other rule applies."
                ),
                kind: ErrorKind::Schema,
            }]
        }
        ValidationOutcome::NotFound => {
            // Unreachable: `read_to_string` above would have returned NotFound.
            return Err(NeoError::io_at(
                "reading `event-model.json` to start healing",
                path,
                std::io::Error::from(std::io::ErrorKind::NotFound),
            ));
        }
    };

    let prompt = build_prompt(&path, &initial_errors);

    // Flag list. `--max-turns` is NOT a valid claude flag (caused immediate
    // exit-1 in earlier iterations); use `--verbose` to make claude chatty
    // on stderr so the streaming log shows progress.
    let workspace_root = session.workspace.root.clone();
    let args: Vec<String> = vec![
        "-p".to_string(),
        "--add-dir".to_string(),
        workspace_root.display().to_string(),
        "--allowed-tools".to_string(),
        "Read,Edit,Write".to_string(),
        "--verbose".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--include-partial-messages".to_string(),
        prompt.clone(),
    ];
    let args_for_log: Vec<String> = args
        .iter()
        .take(args.len() - 1)
        .cloned()
        .chain(std::iter::once(format!("<prompt {} bytes>", prompt.len())))
        .collect();
    tracing::info!(
        binary = %config.claude_binary.display(),
        cwd = %workspace_root.display(),
        timeout_secs = config.timeout.as_secs(),
        prompt_bytes = prompt.len(),
        args = ?args_for_log,
        "heal: spawning claude -p",
    );

    let spawn_result = Command::new(&config.claude_binary)
        .args(&args)
        .current_dir(&workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn();

    let mut child = match spawn_result {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::error!(
                binary = %config.claude_binary.display(),
                "heal: claude binary not found on PATH",
            );
            return Err(NeoError::HealingClaudeMissing);
        }
        Err(e) => {
            tracing::error!(error = %e, "heal: failed to spawn claude");
            return Err(NeoError::HealingFailed {
                reason: format!(
                    "failed to spawn `{}`: {e}",
                    config.claude_binary.display()
                ),
                stderr_tail: String::new(),
            });
        }
    };

    tracing::info!(pid = ?child.id(), "heal: claude subprocess spawned, streaming output");

    // Take the piped handles BEFORE waiting so we can stream them line-by-
    // line. Without this, the user sees no progress during the (potentially
    // multi-minute) heal — the whole point of the logging exercise.
    let stdout = child
        .stdout
        .take()
        .expect("stdout was piped at spawn");
    let stderr = child
        .stderr
        .take()
        .expect("stderr was piped at spawn");

    let stdout_buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let stdout_task = {
        let buf = Arc::clone(&stdout_buf);
        tokio::spawn(stream_lines(stdout, "stdout", buf))
    };
    let stderr_task = {
        let buf = Arc::clone(&stderr_buf);
        tokio::spawn(stream_lines(stderr, "stderr", buf))
    };

    let start = Instant::now();
    let status_result = tokio::time::timeout(config.timeout, child.wait()).await;
    let elapsed = start.elapsed();

    let status = match status_result {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            tracing::error!(error = %e, "heal: wait error on claude subprocess");
            // Best-effort drain so we don't leak the tasks.
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(NeoError::HealingFailed {
                reason: format!("waiting for `claude -p` to exit: {e}"),
                stderr_tail: String::new(),
            });
        }
        Err(_) => {
            tracing::warn!(
                timeout_secs = config.timeout.as_secs(),
                "heal: claude timed out, killing",
            );
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            let stdout_dump = collect_all(&stdout_buf);
            let stderr_dump = collect_all(&stderr_buf);
            tracing::error!(
                timeout_secs = config.timeout.as_secs(),
                "heal: claude killed after timeout — output captured before kill:\n\
                 --- captured stdout ---\n\
                 {stdout_block}\n\
                 --- captured stderr ---\n\
                 {stderr_block}\n\
                 --- end claude output ---",
                stdout_block = if stdout_dump.is_empty() {
                    "(empty)"
                } else {
                    stdout_dump.as_str()
                },
                stderr_block = if stderr_dump.is_empty() {
                    "(empty)"
                } else {
                    stderr_dump.as_str()
                },
            );
            let tail = collect_tail(&stderr_buf, 20);
            return Err(NeoError::HealingFailed {
                reason: format!("timed out after {} seconds", config.timeout.as_secs()),
                stderr_tail: tail,
            });
        }
    };

    // Drain streaming tasks (process has exited; tasks reach EOF shortly).
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    let stdout_line_count = stdout_buf.lock().map(|g| g.len()).unwrap_or(0);
    let stderr_line_count = stderr_buf.lock().map(|g| g.len()).unwrap_or(0);
    tracing::info!(
        elapsed_secs = elapsed.as_secs(),
        exit_code = ?status.code(),
        stdout_lines = stdout_line_count,
        stderr_lines = stderr_line_count,
        "heal: claude exited",
    );

    if !status.success() {
        let stdout_dump = collect_all(&stdout_buf);
        let stderr_dump = collect_all(&stderr_buf);
        let code = status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "killed by signal".to_string());
        // Print the full captured output as ONE multi-line error so the user
        // doesn't have to scroll through interleaved per-line streams. Empty
        // sections are called out explicitly.
        tracing::error!(
            exit_code = %code,
            "heal: claude failed\n\
             --- captured stdout ({stdout_lines} lines) ---\n\
             {stdout_block}\n\
             --- captured stderr ({stderr_lines} lines) ---\n\
             {stderr_block}\n\
             --- end claude output ---",
            stdout_lines = stdout_line_count,
            stderr_lines = stderr_line_count,
            stdout_block = if stdout_dump.is_empty() {
                "(empty — claude wrote nothing to stdout)"
            } else {
                stdout_dump.as_str()
            },
            stderr_block = if stderr_dump.is_empty() {
                "(empty — claude wrote nothing to stderr)"
            } else {
                stderr_dump.as_str()
            },
        );
        let tail = collect_tail(&stderr_buf, 20);
        return Err(NeoError::HealingFailed {
            reason: format!("exit code {code}"),
            stderr_tail: if tail.is_empty() {
                "(stderr empty)".to_string()
            } else {
                tail
            },
        });
    }

    let new_content = std::fs::read_to_string(&path).map_err(|e| {
        NeoError::io_at(
            "re-reading `event-model.json` after healing",
            path.clone(),
            e,
        )
    })?;

    let outcome = match validate::validate_event_model(&new_content) {
        ValidationOutcome::Valid => {
            tracing::info!("heal: file is now valid — healed");
            HealOutcome::Healed
        }
        ValidationOutcome::Invalid { errors } => {
            tracing::warn!(
                remaining_errors = errors.len(),
                "heal: file still has validation errors after claude exit",
            );
            HealOutcome::StillInvalid { errors }
        }
        ValidationOutcome::MalformedJson { parse_error } => {
            tracing::warn!(parse_error = %parse_error, "heal: file still malformed JSON after claude exit");
            HealOutcome::StillInvalid {
                errors: vec![ValidationError {
                    pointer: String::new(),
                    message: format!(
                        "file is still not valid JSON after healing: {parse_error}. The agent left a malformed document on disk — open it and inspect manually, or click Heal again."
                    ),
                    kind: ErrorKind::Schema,
                }],
            }
        }
        ValidationOutcome::NotFound => {
            return Err(NeoError::io_at(
                "re-reading `event-model.json` after healing",
                path,
                std::io::Error::from(std::io::ErrorKind::NotFound),
            ));
        }
    };

    Ok(HealEventModelResult { outcome })
}

/// Read `reader` line-by-line. Emit each line as a `tracing::info!` event
/// keyed on `stream` ("stdout" / "stderr") under the
/// `neo::ide::heal::claude` target so users can `tail -f` (or simply watch
/// `neo ide`'s stderr) and see claude's progress in real time. Each line
/// is also appended to `buf` so the caller can still build a stderr-tail
/// for the failure-path error message.
async fn stream_lines<R>(reader: R, stream: &'static str, buf: Arc<Mutex<Vec<String>>>)
where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                tracing::info!(target: "neo::ide::heal::claude", stream, "{line}");
                if let Ok(mut guard) = buf.lock() {
                    guard.push(line);
                }
            }
            Ok(None) => break,
            Err(e) => {
                tracing::warn!(
                    target: "neo::ide::heal::claude",
                    stream,
                    "read error: {e}",
                );
                break;
            }
        }
    }
}

fn collect_tail(buf: &Arc<Mutex<Vec<String>>>, n: usize) -> String {
    let Ok(guard) = buf.lock() else {
        return String::new();
    };
    let start = guard.len().saturating_sub(n);
    guard[start..].join("\n")
}

fn collect_all(buf: &Arc<Mutex<Vec<String>>>) -> String {
    let Ok(guard) = buf.lock() else {
        return String::new();
    };
    guard.join("\n")
}

fn build_prompt(path: &Path, errors: &[ValidationError]) -> String {
    let errors_text = if errors.is_empty() {
        "  (no schema or referential errors — the file already validates. \
         Your job is to IMPROVE it: add missing edges between nodes whose \
         names suggest a connection, and refine layout positions per the \
         conventions below. Do not invent new nodes.)".to_string()
    } else {
        errors
            .iter()
            .map(|e| {
                let pointer = if e.pointer.is_empty() {
                    "(whole document)"
                } else {
                    e.pointer.as_str()
                };
                format!("  - {pointer}: {}", e.message)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "You are healing a NeoHaskell event-model file used by the `neo ide` visual editor.\n\
\n\
File: {file_path}\n\
\n\
== JSON Schema (draft 2020-12) — the file MUST satisfy this exactly ==\n\
{schema}\n\
\n\
== Validation errors to address (each one needs a concrete fix) ==\n\
{errors}\n\
\n\
== Event-modeling primer (use this to decide how to fix the file) ==\n\
\n\
An event model is a chronological diagram of a system. Five node kinds:\n  \
  - command  — user intent, present-tense verb (e.g. \"PlaceOrder\").\n  \
  - event    — a fact that happened, past tense (e.g. \"OrderPlaced\").\n  \
  - query    — a read model (noun-phrase, e.g. \"OrderSummary\").\n  \
  - integration — connection to another system; kind=inbound means we receive a call, kind=outbound means we send one.\n  \
  - uiPlaceholder — a screen/form a user interacts with (e.g. \"CheckoutForm\").\n\
\n\
Two structural groupings:\n  \
  - chapters — large arcs across many slices (e.g. \"Ordering\", \"Fulfilment\").\n  \
  - slices   — one user-visible feature/use-case (e.g. \"Place Order\"). Slices belong to chapters.\n  \
  - entities — domain aggregates owning events (e.g. \"Order\", \"Inventory\"). Each event lives in exactly one entity's swim lane.\n\
\n\
The six allowed edge types form a directed graph between specific node kinds:\n  \
  - commandProducesEvent       (command   → event)         — command succeeded; event is the consequence\n  \
  - eventFeedsQuery            (event     → query)         — event updates a read model\n  \
  - eventTriggersIntegration   (event     → integration)   — outbound: tell another system\n  \
  - integrationTriggersCommand (integration → command)     — inbound: external trigger raises a command\n  \
  - commandFromUI              (uiPlaceholder → command)   — user submits a form\n  \
  - queryToUI                  (query     → uiPlaceholder) — UI reads from a query\n\
\n\
Idiomatic chains within a slice:\n  \
  UI → command → event → query → UI       (typical user-driven flow)\n  \
  event → outbound integration            (notify external system)\n  \
  inbound integration → command → event   (external trigger)\n\
\n\
== Repair guidance — DO ==\n  \
  1. Fix every schema error listed above. The file MUST validate against the schema.\n  \
  2. WIRE EVERY NODE. Walk the node list and add the missing edges the model needs to make sense. These rules are MANDATORY, not optional — a node missing the connection below is a bug to repair, not a stylistic preference:\n     \
     2a. EVERY command MUST have at least one outgoing `commandProducesEvent` edge to an event. If the command is named in present tense (e.g. `PlaceOrder`, `RegisterPayerReturn`, `SendThankYouEmail`), find the event whose name is the past-tense form (`OrderPlaced`, `PayerReturnedFromPaymentForm`, `ThankYouEmailSent`) and add a `commandProducesEvent` edge from the command to that event. If no matching event exists in the same slice and the command is clearly meant to produce one, the event is missing — but DO NOT fabricate it; instead leave the command's outgoing edge undone and report it in your final message.\n     \
     2b. EVERY event that materially updates a read model MUST have an outgoing `eventFeedsQuery` edge to the relevant query. Walk the queries: for each query, identify every event whose semantics affect what the query returns (e.g. a `PaymentApproved` event updates an `AwaitingConfirmationStatus` query, and so does `PaymentDeclined`, `PaymentExpired`, etc.). Add a `eventFeedsQuery` edge from EACH such event to the query. A query with no incoming `eventFeedsQuery` edges is broken — pick the events that semantically feed it.\n     \
     2c. EVERY inbound integration MUST have at least one outgoing `integrationTriggersCommand` edge to the command it triggers. An inbound integration is named after what it receives (e.g. `BankReturnRedirect`, `StripeWebhook`); the command it triggers is what the system DOES with that input (e.g. `RegisterPayerReturn`, `ApplyPayment`). Add the `integrationTriggersCommand` edge from the inbound integration to that command.\n     \
     2d. EVERY outbound integration MUST have at least one incoming `eventTriggersIntegration` edge from the event that causes it. An outbound integration is named after what it calls (e.g. `BankPaymentStatusAPI`, `EmailService`); the event that triggers it is the fact that made the call necessary (e.g. `PaymentApproved` → `EmailService`, `PaymentRequested` → `BankPaymentFormAPI`). Add the `eventTriggersIntegration` edge from that event to the outbound integration.\n     \
     2e. EVERY uiPlaceholder paired with a command in the same slice MUST have an outgoing `commandFromUI` edge to that command. EVERY uiPlaceholder paired with a query in the same slice MUST have an incoming `queryToUI` edge from that query. UI placeholders without these edges are orphaned and break the flow.\n  \
  3. Use `crypto.randomUUID()`-shape ids for any new edges (e.g. `edge-<short-random>`), and pick a `type` from the six allowed edge types — never invent a new type. Set `sourceHandle` and `targetHandle` on every new edge: use `\"bottom\"`/`\"top\"` for vertical connections (UI↔command, command↔event, query↔UI), and `\"right\"`/`\"left\"` for horizontal connections (event↔integration, integration↔command across slices).\n  \
  4. Fill in missing layout positions in `layout.nodePositions`. Each node needs `{{ x: number, y: number }}`. Suggested coordinates if you have to invent them (slice index N = 0, 1, 2…; entity index M = 0, 1, 2…):\n     \
     - slice column x base = N * 400 + 40\n     \
     - uiPlaceholder              y = -60         (above the slice header)\n     \
     - command, query, integration y = 120        (SAME band — above events, below UI)\n     \
     - event                       y = 340 + M * 200 + 60   (inside its entity swim lane)\n     \
     If multiple nodes of the same kind land in the same slice, stack them by adding 80 to y for each.\n  \
  5. CORRECT EXISTING BAD POSITIONS. Integrations are NOT below events — they sit at the command/query level (y around 120-280). If you see an integration positioned at y > 300 (inside or below the entity lane), MOVE IT to y ≈ 120 so it visually sits with the commands and queries it logically pairs with. Same for UI placeholders dropped into the events band: lift them up to y ≈ -60.\n\
\n\
== Repair guidance — DO NOT ==\n  \
  - Do NOT rename or renumber existing ids unless the schema forces it (breaks links).\n  \
  - Do NOT delete entities, slices, chapters, or nodes the user clearly intended (only drop entries that are unfixable garbage).\n  \
  - Do NOT add edges between node kinds the schema forbids (the six edge types above are exhaustive).\n  \
  - Do NOT add fields the schema doesn't define — `additionalProperties: false` rejects unknown keys at every level.\n  \
  - Do NOT create new files; edit the file at the path above in place.\n\
\n\
When done, the file at the path above must (a) parse as JSON, (b) satisfy the schema, (c) have every node connected via the edge patterns above unless it's genuinely standalone. Exit when finished.",
        file_path = path.display(),
        schema = SCHEMA_JSON,
        errors = errors_text,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ide::workspace::Workspace;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;

    const VALID_MODEL: &str = r#"{
  "id": "m1",
  "name": "demo",
  "chapters": [],
  "entities": [],
  "slices": [],
  "nodes": [],
  "edges": [],
  "layout": { "nodePositions": {}, "viewport": { "x": 0, "y": 0, "zoom": 1 } }
}"#;

    const INVALID_MODEL: &str = r#"{
  "name": "missing id",
  "chapters": [],
  "entities": [],
  "slices": [],
  "nodes": [],
  "edges": [],
  "layout": { "nodePositions": {}, "viewport": { "x": 0, "y": 0, "zoom": 1 } }
}"#;

    fn fixture_session(dir: &std::path::Path) -> Session {
        let ws = Workspace::from_root(dir).unwrap();
        Session::new(Arc::new(ws))
    }

    /// Write a bash script at `path` with `body` as its shell content. The
    /// script becomes a stub `claude` that tests point `HealConfig` at via
    /// its absolute path.
    fn write_stub(path: &std::path::Path, body: &str) {
        let script = format!("#!/usr/bin/env bash\n{body}\n");
        std::fs::write(path, script).unwrap();
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }

    fn quick_config(claude_binary: PathBuf, timeout_ms: u64) -> HealConfig {
        HealConfig {
            claude_binary,
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    #[tokio::test]
    async fn heal_returns_healed_when_stub_fixes_file() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        let model_path = workspace.join("event-model.json");
        std::fs::write(&model_path, INVALID_MODEL).unwrap();

        let stub_dir = tempfile::tempdir().unwrap();
        let stub_path = stub_dir.path().join("claude");
        // Stub overwrites the file with a valid model.
        write_stub(
            &stub_path,
            &format!(
                "cat > '{}' <<'EOF'\n{}\nEOF\nexit 0",
                model_path.display(),
                VALID_MODEL
            ),
        );

        let session = fixture_session(workspace);
        let result = handle_with_config(session, HealMode::Validate, quick_config(stub_path, 10_000))
            .await
            .expect("heal should succeed");
        assert_eq!(result.outcome, HealOutcome::Healed);
        // File on disk should now be valid.
        let after = std::fs::read_to_string(&model_path).unwrap();
        assert!(after.contains("\"id\": \"m1\""));
    }

    #[tokio::test]
    async fn heal_returns_still_invalid_when_stub_leaves_errors() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        std::fs::write(workspace.join("event-model.json"), INVALID_MODEL).unwrap();

        let stub_dir = tempfile::tempdir().unwrap();
        let stub_path = stub_dir.path().join("claude");
        // Stub does nothing — file remains invalid.
        write_stub(&stub_path, "exit 0");

        let session = fixture_session(workspace);
        let result = handle_with_config(session, HealMode::Validate, quick_config(stub_path, 10_000))
            .await
            .expect("heal should return Ok with StillInvalid");
        match result.outcome {
            HealOutcome::StillInvalid { errors } => {
                assert!(!errors.is_empty(), "expected at least one remaining error");
            }
            other => panic!("expected StillInvalid, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn heal_returns_claude_missing_when_binary_absent() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        std::fs::write(workspace.join("event-model.json"), INVALID_MODEL).unwrap();

        // Point at a path that definitely doesn't exist.
        let bogus = std::path::PathBuf::from("/nonexistent/path/to/claude-does-not-exist-12345");
        let session = fixture_session(workspace);
        let result = handle_with_config(session, HealMode::Validate, quick_config(bogus, 10_000)).await;
        assert!(
            matches!(result, Err(NeoError::HealingClaudeMissing)),
            "expected HealingClaudeMissing, got {result:?}"
        );
    }

    #[tokio::test]
    async fn heal_returns_failed_when_subprocess_nonzero_exit() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        std::fs::write(workspace.join("event-model.json"), INVALID_MODEL).unwrap();

        let stub_dir = tempfile::tempdir().unwrap();
        let stub_path = stub_dir.path().join("claude");
        write_stub(&stub_path, "echo 'bang' 1>&2\nexit 1");

        let session = fixture_session(workspace);
        let result = handle_with_config(session, HealMode::Validate, quick_config(stub_path, 10_000)).await;
        match result {
            Err(NeoError::HealingFailed { reason, stderr_tail }) => {
                assert!(reason.contains("exit code 1"), "reason: {reason}");
                assert!(stderr_tail.contains("bang"), "stderr_tail: {stderr_tail}");
            }
            other => panic!("expected HealingFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn heal_returns_failed_on_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        std::fs::write(workspace.join("event-model.json"), INVALID_MODEL).unwrap();

        let stub_dir = tempfile::tempdir().unwrap();
        let stub_path = stub_dir.path().join("claude");
        // Sleep longer than the timeout.
        write_stub(&stub_path, "sleep 10\nexit 0");

        let session = fixture_session(workspace);
        let result = handle_with_config(session, HealMode::Validate, quick_config(stub_path, 200)).await;
        match result {
            Err(NeoError::HealingFailed { reason, .. }) => {
                assert!(reason.contains("timed out"), "reason should mention timeout: {reason}");
            }
            other => panic!("expected HealingFailed (timeout), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn heal_errors_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        // Note: no event-model.json on disk.
        let stub_dir = tempfile::tempdir().unwrap();
        let stub_path = stub_dir.path().join("claude");
        write_stub(&stub_path, "exit 0");

        let session = fixture_session(workspace);
        let result = handle_with_config(session, HealMode::Validate, quick_config(stub_path, 10_000)).await;
        match result {
            Err(NeoError::IoErrorAt { operation, path, .. }) => {
                assert!(operation.contains("event-model.json"), "op: {operation}");
                assert!(path.contains("event-model.json"), "path: {path}");
            }
            other => panic!("expected IoErrorAt, got {other:?}"),
        }
    }

    /// Helper: spawn the stub claude with a script that captures argv +
    /// stdin-prompt to a file, then assert on it.
    async fn run_with_argv_capture(
        workspace: &std::path::Path,
        capture: &std::path::Path,
    ) -> Result<HealEventModelResult, NeoError> {
        let stub_dir = tempfile::tempdir().unwrap();
        let stub_path = stub_dir.path().join("claude");
        // Write argv (one per line) + pwd to the capture file, then exit 0.
        // The model file is left untouched so we get StillInvalid back; the
        // caller only cares about side-effects on the capture file.
        write_stub(
            &stub_path,
            &format!(
                "printf '%s\\n' \"$@\" > '{cap}'\nprintf 'PWD=%s\\n' \"$PWD\" >> '{cap}'\nexit 0",
                cap = capture.display()
            ),
        );
        let session = fixture_session(workspace);
        // Need to keep stub_dir alive past the call — leak it by moving into a static-ish
        // location. Simpler: write a copy of the stub to a path we own.
        let owned_stub = workspace.join(".test-claude-stub.sh");
        std::fs::copy(&stub_path, &owned_stub).unwrap();
        let mut perms = std::fs::metadata(&owned_stub).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&owned_stub, perms).unwrap();
        handle_with_config(session, HealMode::Validate, quick_config(owned_stub, 10_000)).await
    }

    #[tokio::test]
    async fn heal_prompt_contains_validation_errors() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        std::fs::write(workspace.join("event-model.json"), INVALID_MODEL).unwrap();
        let capture = workspace.join("argv.log");
        let _ = run_with_argv_capture(workspace, &capture).await;
        let logged = std::fs::read_to_string(&capture).unwrap();
        // The prompt is the last positional arg. It should contain the literal
        // string "id" (from the validation error about the missing required field).
        assert!(
            logged.contains("Validation errors"),
            "argv should include the prompt header, got: {logged}"
        );
        assert!(logged.contains("id"), "prompt should name the missing `id` field");
    }

    #[tokio::test]
    async fn heal_prompt_contains_file_path() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        std::fs::write(workspace.join("event-model.json"), INVALID_MODEL).unwrap();
        let capture = workspace.join("argv.log");
        let _ = run_with_argv_capture(workspace, &capture).await;
        let logged = std::fs::read_to_string(&capture).unwrap();
        assert!(
            logged.contains("event-model.json"),
            "prompt should include the file path, got: {logged}"
        );
    }

    #[tokio::test]
    async fn heal_prompt_contains_schema() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        std::fs::write(workspace.join("event-model.json"), INVALID_MODEL).unwrap();
        let capture = workspace.join("argv.log");
        let _ = run_with_argv_capture(workspace, &capture).await;
        let logged = std::fs::read_to_string(&capture).unwrap();
        assert!(
            logged.contains("$schema"),
            "prompt should include the JSON Schema header, got first 200 chars: {}",
            &logged.chars().take(200).collect::<String>()
        );
    }

    #[tokio::test]
    async fn heal_prompt_teaches_event_modeling_semantics() {
        // The prompt must include enough guidance that claude can ADD
        // missing edges and FILL missing layout positions, not just
        // patch schema violations. Asserts on substrings of the primer
        // + DO/DON'T sections.
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        std::fs::write(workspace.join("event-model.json"), INVALID_MODEL).unwrap();
        let capture = workspace.join("argv.log");
        let _ = run_with_argv_capture(workspace, &capture).await;
        let logged = std::fs::read_to_string(&capture).unwrap();

        // All six edge type names must appear so claude knows which links
        // it can add.
        for edge_type in &[
            "commandProducesEvent",
            "eventFeedsQuery",
            "eventTriggersIntegration",
            "integrationTriggersCommand",
            "commandFromUI",
            "queryToUI",
        ] {
            assert!(
                logged.contains(edge_type),
                "prompt should mention edge type `{edge_type}`",
            );
        }

        // Layout guidance must include both coordinate hints and the
        // `layout.nodePositions` target.
        assert!(
            logged.contains("layout.nodePositions"),
            "prompt should mention layout.nodePositions",
        );
        assert!(
            logged.contains("slice index"),
            "prompt should mention slice index for x-coordinate calculation",
        );

        // DO instruction to add missing edges.
        assert!(
            logged.to_lowercase().contains("add the missing edge")
                || logged.to_lowercase().contains("add the edge"),
            "prompt should explicitly instruct claude to add missing edges",
        );

        // Mandatory wiring rules — each must appear, keyed on the edge-type
        // it requires. Regression-guards the user-reported request:
        // commands must reach events, events must feed queries, integrations
        // must be wired to commands.
        let wiring_rules = [
            (
                "command → event wiring",
                "every command",
                "commandProducesEvent",
            ),
            (
                "event → query wiring",
                "every event",
                "eventFeedsQuery",
            ),
            (
                "inbound integration → command wiring",
                "every inbound integration",
                "integrationTriggersCommand",
            ),
            (
                "event → outbound integration wiring",
                "every outbound integration",
                "eventTriggersIntegration",
            ),
        ];
        let logged_lower = logged.to_lowercase();
        for (label, must_phrase, edge_type) in wiring_rules {
            assert!(
                logged_lower.contains(must_phrase),
                "prompt missing the `{must_phrase}` MUST clause for {label}",
            );
            assert!(
                logged.contains(edge_type),
                "prompt missing edge type `{edge_type}` referenced in {label} rule",
            );
        }
    }

    #[tokio::test]
    async fn heal_uses_workspace_root_as_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        std::fs::write(workspace.join("event-model.json"), INVALID_MODEL).unwrap();
        let capture = workspace.join("argv.log");
        let _ = run_with_argv_capture(workspace, &capture).await;
        let logged = std::fs::read_to_string(&capture).unwrap();
        // PWD=<workspace canonical path>
        let canonical = workspace.canonicalize().unwrap();
        assert!(
            logged.contains(&format!("PWD={}", canonical.display())),
            "stub PWD should match workspace root; got: {logged}"
        );
    }

    #[tokio::test]
    async fn heal_passes_allowed_tools_flag() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        std::fs::write(workspace.join("event-model.json"), INVALID_MODEL).unwrap();
        let capture = workspace.join("argv.log");
        let _ = run_with_argv_capture(workspace, &capture).await;
        let logged = std::fs::read_to_string(&capture).unwrap();
        assert!(
            logged.contains("--allowed-tools"),
            "argv should include --allowed-tools, got: {logged}"
        );
        assert!(
            logged.contains("Read,Edit,Write"),
            "argv should pass `Read,Edit,Write` as the allowed-tools value, got: {logged}"
        );
    }

    #[tokio::test]
    async fn heal_validate_mode_skips_subprocess_on_valid_file() {
        // Default mode (Validate) on a valid file must short-circuit before
        // spawning claude — saves API tokens on the auto-triggered path.
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        std::fs::write(workspace.join("event-model.json"), VALID_MODEL).unwrap();
        let stub_dir = tempfile::tempdir().unwrap();
        let stub_path = stub_dir.path().join("claude");
        // Stub would corrupt the file if invoked — we assert it isn't.
        write_stub(&stub_path, "echo 'STUB RAN — should not have been invoked'\nexit 1");

        let session = fixture_session(workspace);
        let result = handle_with_config(session, HealMode::Validate, quick_config(stub_path, 10_000))
            .await
            .expect("validate mode on valid file should succeed");
        assert_eq!(result.outcome, HealOutcome::Healed);
        // File untouched.
        let after = std::fs::read_to_string(workspace.join("event-model.json")).unwrap();
        assert_eq!(after, VALID_MODEL);
    }

    #[tokio::test]
    async fn heal_improve_mode_runs_subprocess_even_on_valid_file() {
        // The "Heal with AI" manual button uses mode=Improve, which must
        // invoke claude even when the file already validates — that's how
        // the user asks the agent to refine layout / add inferred edges.
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        let model_path = workspace.join("event-model.json");
        std::fs::write(&model_path, VALID_MODEL).unwrap();

        let stub_dir = tempfile::tempdir().unwrap();
        let stub_path = stub_dir.path().join("claude");
        let marker = workspace.join("STUB_RAN");
        // Touch a marker file so the test can prove the stub ran.
        write_stub(
            &stub_path,
            &format!("touch '{}'\nexit 0", marker.display()),
        );

        let session = fixture_session(workspace);
        let result = handle_with_config(session, HealMode::Improve, quick_config(stub_path, 10_000))
            .await
            .expect("improve mode should succeed");
        // Outcome is Healed because the file is still valid after the no-op stub.
        assert_eq!(result.outcome, HealOutcome::Healed);
        // And the stub WAS invoked.
        assert!(
            marker.exists(),
            "improve mode must invoke claude even on a valid file (marker missing)",
        );
    }
}

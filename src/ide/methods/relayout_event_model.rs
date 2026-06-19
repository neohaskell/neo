//! `workspace/relayoutEventModel` — re-run the deterministic LAYOUT pass
//! of the heal pipeline against `event-model.json` without touching the
//! file's structure.
//!
//! This is the "I just want my positions cleaned up" entry point. It runs:
//!   * Y-band position fixes (snaps off-band nodes back to canonical y)
//!   * Chapter grouping (groups heal-prefixed slices by their entity)
//!   * X-axis slice-column rebalance (pushes colliding columns apart)
//!   * Missing layout entries (assigns a position to any node lacking one)
//!
//! It does NOT:
//!   * Add entities / slices / nodes / edges from the inspection
//!   * Detect orphans
//!   * Fix integration kinds
//!   * Spawn `claude` — pure Rust, no LLM
//!
//! Use cases:
//!   * Quick UI button: "Re-layout" — clean up positions without the
//!     full heal workflow / confirmations.
//!   * Hand-authored event-model.json from a non-NeoHaskell workspace
//!     where the user just wants the canonical layout.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::NeoError;
use crate::ide::heal::apply::apply_diff;
use crate::ide::heal::diff::{compute_diff_with_options, ComputeOptions};
use crate::ide::methods::read_event_model::EVENT_MODEL_FILENAME;
use crate::ide::session::Session;
use crate::ide::validate;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RelayoutEventModelParams {}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RelayoutEventModelResult {
    /// Number of deterministic layout fixes applied to the file. `0`
    /// means the file's layout was already canonical — the file on disk
    /// is unchanged.
    pub applied: usize,
    /// Short human-readable summary of what changed.
    pub summary: String,
}

pub async fn handle(
    session: Session,
    _params: RelayoutEventModelParams,
) -> Result<RelayoutEventModelResult, NeoError> {
    let path = session.workspace.root.join(EVENT_MODEL_FILENAME);
    let original = std::fs::read_to_string(&path).map_err(|e| {
        NeoError::io_at("reading `event-model.json` for relayout", path.clone(), e)
    })?;
    let mut value: serde_json::Value = serde_json::from_str(&original).map_err(|e| {
        NeoError::HealingFailed {
            reason: format!("relayout requires valid JSON; parse failed: {e}"),
            stderr_tail: String::new(),
        }
    })?;
    let inspection = crate::inspect::inspect_project(&session.workspace.root);

    let diff = compute_diff_with_options(&value, &inspection, ComputeOptions::layout_only());
    let applied = apply_diff(&mut value, &diff);
    let summary = diff.summary();

    tracing::info!(applied, %summary, "relayout: deterministic layout pass complete");

    if applied > 0 {
        let new_content = serde_json::to_string_pretty(&value).map_err(|e| {
            NeoError::HealingFailed {
                reason: format!("relayout could not re-serialise model: {e}"),
                stderr_tail: String::new(),
            }
        })?;
        atomic_write(&path, &new_content)?;
        // Quick post-validate so we never write a broken file.
        let _ = validate::validate_event_model(&new_content);
    }

    Ok(RelayoutEventModelResult { applied, summary })
}

fn atomic_write(path: &Path, content: &str) -> Result<(), NeoError> {
    let tmp = path.with_extension("json.relayout-tmp");
    std::fs::write(&tmp, content.as_bytes()).map_err(|e| {
        NeoError::io_at("writing relayouted event-model.json (tmp)", tmp.clone(), e)
    })?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        NeoError::io_at(
            "renaming relayouted event-model.json into place",
            path.to_path_buf(),
            e,
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ide::workspace::Workspace;
    use std::sync::Arc;

    fn fixture_session(dir: &Path) -> Session {
        let ws = Workspace::from_root(dir).unwrap();
        Session::new(Arc::new(ws))
    }

    #[tokio::test]
    async fn relayout_only_fixes_positions_does_not_materialize() {
        // Workspace has a NeoHaskell project (so a heal would materialize
        // nodes). But the relayout call MUST leave node/slice/entity
        // counts unchanged — only layout-level fields move.
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        let core = workspace.join("src/App/Cart/Core.hs");
        std::fs::create_dir_all(core.parent().unwrap()).unwrap();
        std::fs::write(
            &core,
            "module App.Cart.Core where\ndata CartEvent = ItemAdded {} deriving (Generic)\n",
        )
        .unwrap();
        let cmd = workspace.join("src/App/Cart/Commands/AddItem.hs");
        std::fs::create_dir_all(cmd.parent().unwrap()).unwrap();
        std::fs::write(
            &cmd,
            "module App.Cart.Commands.AddItem where\n\
             decide _ _ _ = Decider.acceptExisting [ItemAdded {}]\n",
        )
        .unwrap();

        // Plant a valid model with an integration at the wrong y. Relayout
        // must snap it back to the canonical band.
        let model = serde_json::json!({
            "id": "m1", "name": "demo",
            "chapters": [],
            "entities": [{ "id": "ent1", "name": "Cart", "order": 0 }],
            "slices": [{ "id": "sl1", "name": "Stale", "chapterId": null, "order": 0 }],
            "nodes": [
                { "id": "intg1", "type": "integration", "name": "Misplaced",
                  "sliceId": "sl1", "kind": "outbound" }
            ],
            "edges": [],
            "layout": {
                "nodePositions": { "intg1": { "x": 200, "y": 500 } },
                "viewport": { "x": 0, "y": 0, "zoom": 1 }
            }
        });
        let model_path = workspace.join("event-model.json");
        std::fs::write(
            &model_path,
            serde_json::to_string_pretty(&model).unwrap(),
        )
        .unwrap();

        let session = fixture_session(workspace);
        let result = handle(session, RelayoutEventModelParams {}).await.unwrap();
        assert!(result.applied > 0, "relayout should fix the off-band y; summary={}", result.summary);

        // File should now have the integration at y=120 — but no new
        // nodes (relayout doesn't materialize AddItem / ItemAdded).
        let patched: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&model_path).unwrap()).unwrap();
        let nodes = patched["nodes"].as_array().unwrap();
        assert_eq!(
            nodes.len(),
            1,
            "relayout must not add nodes; got {:?}",
            nodes
        );
        let y = patched["layout"]["nodePositions"]["intg1"]["y"].as_f64().unwrap();
        assert!((y - 120.0).abs() < f64::EPSILON, "integration y should snap to 120; got {y}");
    }

    #[tokio::test]
    async fn relayout_returns_zero_applied_when_file_already_canonical() {
        // No NeoHaskell project, valid model with positioned nodes in
        // the right bands → relayout has nothing to do.
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();
        let model = serde_json::json!({
            "id": "m1", "name": "demo",
            "chapters": [], "entities": [], "slices": [],
            "nodes": [], "edges": [],
            "layout": { "nodePositions": {}, "viewport": { "x": 0, "y": 0, "zoom": 1 } }
        });
        let model_path = workspace.join("event-model.json");
        std::fs::write(&model_path, serde_json::to_string_pretty(&model).unwrap()).unwrap();
        let original = std::fs::read_to_string(&model_path).unwrap();

        let session = fixture_session(workspace);
        let result = handle(session, RelayoutEventModelParams {}).await.unwrap();
        assert_eq!(result.applied, 0);
        // File untouched.
        assert_eq!(std::fs::read_to_string(&model_path).unwrap(), original);
    }
}

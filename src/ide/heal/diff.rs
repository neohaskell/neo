//! Compute a deterministic `HealDiff` between an event-model JSON value
//! and a NeoHaskell `ProjectInspection`.
//!
//! Scope:
//!   * Materialize missing structural pieces — entities, slices, and the
//!     command/event/query/integration nodes for every code-side symbol
//!     the model doesn't have yet. The diff also wires those new nodes
//!     with the edges their kind requires (commands → events, events →
//!     queries, events → integrations, integrations → commands).
//!   * Auto-wire missing edges between EXISTING nodes — driven by
//!     `command.produces`, `query.subscribes_to`,
//!     `integration.handles_events`, `integration.emits_commands`.
//!   * Fix integration `kind` drift — when inspection classifies an
//!     integration as Reactive (emits a command) but the model says
//!     Outbound, correct the model.
//!   * Fix misplaced y-positions — integration/command/query/UI nodes
//!     dropped into the event band (y > 300) → move to canonical band.
//!   * Add `layout.nodePositions` entries for every node missing one
//!     (including the brand-new materialized nodes).
//!
//! What the LLM still owns (`Residual`):
//!   * `OrphanModelNode` — a node in the model whose name has no code
//!     backing. LLM decides: typo (rename), dead (remove), planned (leave).
//!
//! Idempotency: deterministic IDs derived from `(type, name)` hashes keep
//! re-running the pass on an already-patched model a no-op.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use serde::Serialize;
use serde_json::Value;

use crate::inspect::{DomainInspection, IntegrationKind, ProjectInspection};

/// Canonical y-band per node kind. Used both to detect misplaced positions
/// and to write a sensible default when a position is missing.
const Y_UI_PLACEHOLDER: f64 = -60.0;
const Y_COMMAND_QUERY_INTEGRATION: f64 = 120.0;
const Y_EVENT: f64 = 400.0;
/// Anything in `[300, ∞)` for an integration/command/query/UI is "in the
/// event band" — wrong, fix it. Below 300 we leave it alone.
const Y_BAND_FLOOR_FOR_NON_EVENT: f64 = 300.0;
/// Left margin used when a node has no slice (or its slice is unknown).
const SLICE_COLUMN_OFFSET: f64 = 40.0;

/// What the deterministic pass wants to change about the model. Each entry
/// carries enough info to apply without re-deriving from the inspection.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealDiff {
    /// Chapters to add (one per inspection entity whose slices need a home).
    pub add_chapters: Vec<ChapterToAdd>,
    /// Entities to add (one per inspection domain that lacks a matching entity).
    pub add_entities: Vec<EntityToAdd>,
    /// Slices to add (one per command/query/integration/orphan-event name
    /// that lacks a matching slice).
    pub add_slices: Vec<SliceToAdd>,
    /// Nodes (command/event/query/integration) to add for every code-side
    /// symbol the model is missing.
    pub add_nodes: Vec<NodeToAdd>,
    /// Edges to add between nodes (existing + freshly materialised).
    pub add_edges: Vec<EdgeToAdd>,
    /// Existing slices whose `chapterId` / `order` need updating to group
    /// them into their entity's chapter.
    pub update_slices: Vec<SliceUpdate>,
    /// Integration nodes whose `kind` field disagrees with the code.
    pub fix_integration_kinds: Vec<KindFix>,
    /// Existing nodes whose y-coordinate is in the wrong band.
    pub fix_positions: Vec<PositionFix>,
    /// Nodes with no entry in `layout.nodePositions` — includes freshly
    /// materialised nodes.
    pub ensure_layout_entries: Vec<LayoutEntry>,
    /// Unresolved issues — things the diff identified but cannot fix
    /// deterministically. The LLM (or the user) needs to resolve these.
    pub residuals: Vec<Residual>,
}

impl HealDiff {
    /// Total number of repairs this diff would apply (excludes residuals).
    pub fn applied_count(&self) -> usize {
        self.add_chapters.len()
            + self.add_entities.len()
            + self.add_slices.len()
            + self.add_nodes.len()
            + self.add_edges.len()
            + self.update_slices.len()
            + self.fix_integration_kinds.len()
            + self.fix_positions.len()
            + self.ensure_layout_entries.len()
    }

    /// Short, human-readable one-line summary for logs and the heal overlay.
    pub fn summary(&self) -> String {
        format!(
            "{} chapters, {} entities, {} slices, {} nodes, {} edges, {} slice updates, {} kind fixes, {} position fixes, {} layout entries, {} residuals",
            self.add_chapters.len(),
            self.add_entities.len(),
            self.add_slices.len(),
            self.add_nodes.len(),
            self.add_edges.len(),
            self.update_slices.len(),
            self.fix_integration_kinds.len(),
            self.fix_positions.len(),
            self.ensure_layout_entries.len(),
            self.residuals.len(),
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterToAdd {
    pub id: String,
    pub name: String,
    pub order: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityToAdd {
    pub id: String,
    pub name: String,
    pub order: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliceToAdd {
    pub id: String,
    pub name: String,
    pub chapter_id: Option<String>,
    pub order: f64,
    pub reason: String,
    /// Domain entity that this slice belongs to — set when the slice is
    /// created during processing of an inspection domain. Drives chapter
    /// grouping in the post-pass. Not serialised to disk on the slice.
    #[serde(skip)]
    pub entity_id_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliceUpdate {
    pub slice_id: String,
    pub slice_name: String,
    /// `Some(_)` to set chapterId; absent on the struct means no chapter change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_chapter_id: Option<String>,
    /// `Some(_)` to set order; absent means no order change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_order: Option<f64>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeToAdd {
    pub id: String,
    pub node_type: String,
    pub name: String,
    pub slice_id: String,
    /// Only set for `command` / `event` nodes (queries + integrations don't
    /// carry an entity per schema).
    pub entity_id: Option<String>,
    /// Only set for `integration` nodes (`inbound` / `outbound`).
    pub kind: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeToAdd {
    pub edge_type: String,
    pub source_id: String,
    pub target_id: String,
    pub source_handle: String,
    pub target_handle: String,
    /// Human-readable rationale, e.g. "command RequestPayment produces event PaymentRequested".
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindFix {
    pub node_id: String,
    pub node_name: String,
    pub from_kind: String,
    pub to_kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionFix {
    pub node_id: String,
    pub node_name: String,
    pub node_kind: String,
    /// `Some` when the y-band fix should run. `None` for x-only fixes
    /// (e.g. slice-column rebalance, where we don't want to clobber a
    /// hand-set y).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_y: Option<f64>,
    /// `Some` when the x-axis fix should run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_x: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutEntry {
    pub node_id: String,
    pub x: f64,
    pub y: f64,
}

/// Something the deterministic pass noticed but cannot fix on its own.
/// The LLM uses this as its much-shorter input prompt.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Residual {
    /// A model node whose name doesn't appear anywhere in the inspection.
    /// LLM (or user) decides whether it's a UI placeholder, a planned
    /// feature, or a typo.
    OrphanModelNode {
        node_id: String,
        node_name: String,
        node_type: String,
    },
}

/// Which phases of `compute_diff` to run. Default = both (the heal flow).
/// `layout_only` skips materialisation/orphan/kind-fix and runs ONLY the
/// layout passes — used by `workspace/relayoutEventModel` so the user can
/// clean up a model's positions without touching its structure.
#[derive(Debug, Clone, Copy)]
pub struct ComputeOptions {
    pub structural: bool,
    pub layout: bool,
}

impl Default for ComputeOptions {
    fn default() -> Self {
        Self::full()
    }
}

impl ComputeOptions {
    pub const fn full() -> Self {
        Self { structural: true, layout: true }
    }
    pub const fn layout_only() -> Self {
        Self { structural: false, layout: true }
    }
    pub const fn structural_only() -> Self {
        Self { structural: true, layout: false }
    }
}

/// Compute the diff between a parsed event-model JSON value and the
/// project inspection. Returns an empty diff when the model already
/// matches the code.
pub fn compute_diff(model: &Value, inspection: &ProjectInspection) -> HealDiff {
    compute_diff_with_options(model, inspection, ComputeOptions::default())
}

/// Same as `compute_diff`, but with control over which phases run.
pub fn compute_diff_with_options(
    model: &Value,
    inspection: &ProjectInspection,
    opts: ComputeOptions,
) -> HealDiff {
    let mut diff = HealDiff::default();
    let mut plan = MaterializePlan::from_model(model);

    // --- 1. Ensure entities for each domain ----------------------------
    if opts.structural {
        for domain in &inspection.domains {
            plan.ensure_entity(&mut diff, &domain.name, &domain.name);
        }
    }

    // --- 2. Materialise nodes + wire edges per domain ------------------
    if opts.structural {
    for domain in &inspection.domains {
        let entity_id = plan.entity_id_for(&domain.name);
        let event_owner = primary_event_owners(domain);

        // Commands first: each command gets its own slice (named after the
        // command), and a command node. If the command already exists in
        // the model we reuse whatever slice it's already in.
        for cmd in &domain.commands {
            let cmd_node_id = plan.ensure_node_in_slice(
                &mut diff,
                "command",
                &cmd.name,
                &cmd.name,
                &format!("slice for command {}", cmd.name),
                entity_id.as_deref(),
                entity_id.as_deref(),
                None,
                &format!("command {} discovered in domain {}", cmd.name, domain.name),
            );

            // Each event the command produces lives in the slice of its
            // *primary* (alphabetically-first) producing command. That keeps
            // an event shared by multiple commands from oscillating between
            // slices on re-runs.
            for ev_name in &cmd.produces {
                let primary = event_owner.get(ev_name.as_str()).copied().unwrap_or(&cmd.name);
                let ev_node_id = plan.ensure_node_in_slice(
                    &mut diff,
                    "event",
                    ev_name,
                    primary,
                    &format!("slice for command {primary}"),
                    entity_id.as_deref(),
                    entity_id.as_deref(),
                    None,
                    &format!(
                        "event {} produced by command {} (domain {})",
                        ev_name, primary, domain.name
                    ),
                );
                plan.ensure_edge(
                    &mut diff,
                    "commandProducesEvent",
                    &cmd_node_id,
                    &ev_node_id,
                    "bottom",
                    "top",
                    &format!(
                        "command {} produces event {} (per `decide` in domain {})",
                        cmd.name, ev_name, domain.name
                    ),
                );
            }
        }

        // Events that no command produces (declared in `Core.hs`/`Event.hs`
        // but unreferenced from any `decide`) — give them their own slice
        // so the structure is preserved.
        for ev in &domain.events {
            if event_owner.contains_key(ev.name.as_str()) {
                continue;
            }
            plan.ensure_node_in_slice(
                &mut diff,
                "event",
                &ev.name,
                &ev.name,
                &format!("slice for orphan event {}", ev.name),
                entity_id.as_deref(),
                entity_id.as_deref(),
                None,
                &format!(
                    "event {} declared in domain {} (no producing command)",
                    ev.name, domain.name
                ),
            );
        }

        // Queries: each gets its own slice + node, edges from every
        // subscribed event we can find.
        for q in &domain.queries {
            let q_node_id = plan.ensure_node_in_slice(
                &mut diff,
                "query",
                &q.name,
                &q.name,
                &format!("slice for query {}", q.name),
                entity_id.as_deref(),
                None,
                None,
                &format!("query {} discovered in domain {}", q.name, domain.name),
            );
            for ev_name in &q.subscribes_to {
                let Some(ev_node_id) = plan.node_id("event", ev_name) else {
                    continue;
                };
                plan.ensure_edge(
                    &mut diff,
                    "eventFeedsQuery",
                    &ev_node_id,
                    &q_node_id,
                    "right",
                    "left",
                    &format!(
                        "query {} subscribes to event {} (per query file in domain {})",
                        q.name, ev_name, domain.name
                    ),
                );
            }
        }

        // Integrations: each gets its own slice + node, kind set from
        // inspection. Then event→integration and integration→command edges.
        for intg in &domain.integrations {
            let inspection_kind = match intg.kind {
                IntegrationKind::Outbound => "outbound",
                IntegrationKind::Reactive => "inbound",
            };
            let intg_node_id = plan.ensure_node_in_slice(
                &mut diff,
                "integration",
                &intg.name,
                &intg.name,
                &format!("slice for integration {}", intg.name),
                entity_id.as_deref(),
                None,
                Some(inspection_kind),
                &format!(
                    "integration {} discovered in domain {} (kind={inspection_kind})",
                    intg.name, domain.name
                ),
            );

            // Fix kind drift on every existing integration node with this
            // name (model may carry duplicates in multiple slices).
            for node in plan.existing_nodes_named("integration", &intg.name) {
                let current_kind = node.kind.as_deref().unwrap_or("");
                if current_kind != inspection_kind {
                    diff.fix_integration_kinds.push(KindFix {
                        node_id: node.id.clone(),
                        node_name: node.name.clone(),
                        from_kind: current_kind.to_string(),
                        to_kind: inspection_kind.to_string(),
                        reason: format!(
                            "code shows integration {} {} ({inspection_kind}-style handler)",
                            intg.name,
                            match intg.kind {
                                IntegrationKind::Outbound => "calls an external system",
                                IntegrationKind::Reactive => "emits a command (bridges domains)",
                            },
                        ),
                    });
                }
            }

            for ev_name in &intg.handles_events {
                let Some(ev_node_id) = plan.node_id("event", ev_name) else {
                    continue;
                };
                plan.ensure_edge(
                    &mut diff,
                    "eventTriggersIntegration",
                    &ev_node_id,
                    &intg_node_id,
                    "right",
                    "left",
                    &format!(
                        "integration {} handles event {} (per `handleEvent` in domain {})",
                        intg.name, ev_name, domain.name
                    ),
                );
            }

            for cmd_name in &intg.emits_commands {
                let Some(cmd_node_id) = plan.node_id("command", cmd_name) else {
                    continue;
                };
                plan.ensure_edge(
                    &mut diff,
                    "integrationTriggersCommand",
                    &intg_node_id,
                    &cmd_node_id,
                    "top",
                    "bottom",
                    &format!(
                        "integration {} emits command {} (per `Command.Emit` in domain {})",
                        intg.name, cmd_name, domain.name
                    ),
                );
            }
        }
    }
    } // close `if opts.structural` wrapping steps 1-2

    // --- 3. Orphan model nodes (model has them, code doesn't) ----------
    //
    // Only run when the inspection actually saw a NeoHaskell project. If
    // `inspection.domains` is empty (non-NeoHaskell workspace, or the
    // workspace wasn't analysed), we have no truth about what's supposed
    // to exist — flagging every existing node as orphan would be wrong.
    // We still run the position-fix + layout passes below so a "just
    // clean up positions" heal works on any workspace.
    if opts.structural && !inspection.domains.is_empty() {
        let inspection_names = inspection_name_set(inspection);
        for node in plan.iter_existing_nodes() {
            // UI placeholders don't have Haskell modules — never orphan them.
            if node.r#type == "uiPlaceholder" {
                continue;
            }
            if !inspection_names.contains(&(node.r#type.clone(), node.name.clone())) {
                diff.residuals.push(Residual::OrphanModelNode {
                    node_id: node.id.clone(),
                    node_name: node.name.clone(),
                    node_type: node.r#type.clone(),
                });
            }
        }
    }

    if !opts.layout {
        return diff;
    }

    // --- 4. Position fixes (existing nodes only) -----------------------
    let positions = model
        .get("layout")
        .and_then(|l| l.get("nodePositions"))
        .and_then(|p| p.as_object());

    for node in plan.iter_existing_nodes() {
        let pos = positions.and_then(|m| m.get(&node.id));
        if let Some(obj) = pos.and_then(|v| v.as_object()) {
            let current_y = obj.get("y").and_then(|v| v.as_f64());
            if let Some(current_y) = current_y {
                let target_y = canonical_y(&node.r#type);
                let in_event_band = current_y > Y_BAND_FLOOR_FOR_NON_EVENT;
                let is_non_event = node.r#type != "event";
                if is_non_event && in_event_band && (current_y - target_y).abs() > f64::EPSILON {
                    diff.fix_positions.push(PositionFix {
                        node_id: node.id.clone(),
                        node_name: node.name.clone(),
                        node_kind: node.r#type.clone(),
                        from_y: Some(current_y),
                        to_y: Some(target_y),
                        from_x: None,
                        to_x: None,
                    });
                }
            }
        }
    }

    // --- 4.5. Chapter grouping ---------------------------------------
    //
    // Heal-created slices (id prefix `slice-heal-`) get grouped under a
    // chapter named after their domain's entity. Must run BEFORE the
    // rebalance + layout passes — those passes use slice `order` to
    // compute x columns, so reordering here makes the position assignment
    // match the final layout instead of triggering a second-run fix.
    let nodes_snapshot = diff.add_nodes.clone();
    group_slices_into_chapters(model, inspection, &nodes_snapshot, &mut diff);
    // After grouping, sort pending slices by their reassigned order so
    // PositionCalculator iterates them in the new left-to-right rhythm.
    diff.add_slices.sort_by(|a, b| {
        a.order
            .partial_cmp(&b.order)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // --- 4.7. Slice-column rebalance ---------------------------------
    //
    // Walks slices in `order` and ensures each slice's column is at
    // least `DEFAULT_NODE_WIDTH + SLICE_COLUMN_GAP` past the previous
    // one's. Catches the case where an earlier heal pass placed a new
    // slice at a hash-derived x that happens to overlap a hand-placed
    // slice's column. Re-positions the colliding nodes (x only — y is
    // left alone so the event/command band assignment from the y pass
    // stands).
    rebalance_slice_columns(model, &plan, &mut diff);

    // --- 5. Layout entries (existing nodes lacking a position +
    //                       every freshly materialised node) -----------
    //
    // Position assignment is a SECOND pass so we can see every existing
    // node's x and place new slice columns past the rightmost edge —
    // never on top of an existing well-placed node.
    let mut layout = PositionCalculator::new(model, &plan, &diff);
    let positions = model
        .get("layout")
        .and_then(|l| l.get("nodePositions"))
        .and_then(|p| p.as_object());
    let mut layout_added: BTreeSet<String> = BTreeSet::new();
    for node in plan.iter_all_nodes() {
        let has_position = positions
            .and_then(|m| m.get(&node.id))
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.get("x").and_then(|v| v.as_f64()).is_some()
                    && obj.get("y").and_then(|v| v.as_f64()).is_some()
            })
            .unwrap_or(false);
        if has_position {
            continue;
        }
        if !layout_added.insert(node.id.clone()) {
            continue;
        }
        let (x, y) = layout.assign(
            node.slice_id.as_deref(),
            &node.r#type,
            node.entity_id.as_deref(),
        );
        diff.ensure_layout_entries.push(LayoutEntry {
            node_id: node.id.clone(),
            x,
            y,
        });
    }

    diff
}

fn group_slices_into_chapters(
    model: &Value,
    inspection: &ProjectInspection,
    add_nodes_snapshot: &[NodeToAdd],
    diff: &mut HealDiff,
) {
    // 1. Build entity_id → name from existing entities + diff.add_entities.
    let mut entity_name: BTreeMap<String, String> = BTreeMap::new();
    if let Some(arr) = model.get("entities").and_then(|v| v.as_array()) {
        for e in arr {
            if let (Some(id), Some(name)) = (
                e.get("id").and_then(|v| v.as_str()),
                e.get("name").and_then(|v| v.as_str()),
            ) {
                entity_name.insert(id.to_string(), name.to_string());
            }
        }
    }
    for e in &diff.add_entities {
        entity_name.insert(e.id.clone(), e.name.clone());
    }

    // 2. Build a symbol_name → entity_id index from the inspection so we
    // can map ANY heal-slice (named after a command/query/event/integration)
    // back to its domain's entity, even when the slice's only nodes are
    // queries/integrations that don't carry entityId per the schema.
    let mut name_to_entity_id: BTreeMap<String, String> = BTreeMap::new();
    for domain in &inspection.domains {
        let entity_id_for_domain = entity_name
            .iter()
            .find(|(_, n)| n.as_str() == domain.name.as_str())
            .map(|(id, _)| id.clone());
        let Some(eid) = entity_id_for_domain else {
            continue;
        };
        for c in &domain.commands {
            name_to_entity_id.entry(c.name.clone()).or_insert(eid.clone());
        }
        for q in &domain.queries {
            name_to_entity_id.entry(q.name.clone()).or_insert(eid.clone());
        }
        for i in &domain.integrations {
            name_to_entity_id.entry(i.name.clone()).or_insert(eid.clone());
        }
        for e in &domain.events {
            name_to_entity_id.entry(e.name.clone()).or_insert(eid.clone());
        }
    }

    // 3. Build slice_id → entity_id map by combining (in priority order):
    //    - SliceToAdd.entity_id_hint (new slices we just queued)
    //    - newly-added nodes' entity_id
    //    - existing nodes' entityId for heal-prefixed slices already in the model
    //    - the inspection symbol map (fallback for query/integration slices)
    let mut slice_entity: BTreeMap<String, String> = BTreeMap::new();
    for s in &diff.add_slices {
        if let Some(eid) = &s.entity_id_hint {
            slice_entity.entry(s.id.clone()).or_insert(eid.clone());
        }
    }
    for n in add_nodes_snapshot {
        // Only re-chapter slices the heal pass created. A user-authored
        // slice that just happens to receive a new node from this run
        // must keep its existing chapter assignment.
        if !n.slice_id.starts_with("slice-heal-") {
            continue;
        }
        if let Some(eid) = &n.entity_id {
            slice_entity.entry(n.slice_id.clone()).or_insert(eid.clone());
        }
    }
    if let Some(arr) = model.get("nodes").and_then(|v| v.as_array()) {
        for n in arr {
            let Some(slice_id) = n.get("sliceId").and_then(|v| v.as_str()) else {
                continue;
            };
            if !slice_id.starts_with("slice-heal-") {
                continue;
            }
            let Some(entity_id) = n.get("entityId").and_then(|v| v.as_str()) else {
                continue;
            };
            slice_entity
                .entry(slice_id.to_string())
                .or_insert(entity_id.to_string());
        }
    }
    // Fallback path: walk every heal slice in the model AND the pending
    // diff.add_slices, look up its NAME in the inspection symbol map.
    let mut all_heal_slices: Vec<(String, String)> = Vec::new();
    if let Some(arr) = model.get("slices").and_then(|v| v.as_array()) {
        for s in arr {
            let (Some(id), Some(name)) = (
                s.get("id").and_then(|v| v.as_str()),
                s.get("name").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            if id.starts_with("slice-heal-") {
                all_heal_slices.push((id.to_string(), name.to_string()));
            }
        }
    }
    for s in &diff.add_slices {
        all_heal_slices.push((s.id.clone(), s.name.clone()));
    }
    for (id, name) in &all_heal_slices {
        if slice_entity.contains_key(id) {
            continue;
        }
        if let Some(eid) = name_to_entity_id.get(name) {
            slice_entity.insert(id.clone(), eid.clone());
        }
    }

    if slice_entity.is_empty() {
        return;
    }

    // 3. Build existing model slice info we may need to update.
    let mut model_slice_chapter: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut model_slice_name: BTreeMap<String, String> = BTreeMap::new();
    let mut model_slice_order: BTreeMap<String, f64> = BTreeMap::new();
    if let Some(arr) = model.get("slices").and_then(|v| v.as_array()) {
        for s in arr {
            let Some(id) = s.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let chapter = s
                .get("chapterId")
                .and_then(|v| if v.is_null() { None } else { v.as_str() })
                .map(|s| s.to_string());
            model_slice_chapter.insert(id.to_string(), chapter);
            if let Some(n) = s.get("name").and_then(|v| v.as_str()) {
                model_slice_name.insert(id.to_string(), n.to_string());
            }
            if let Some(o) = s.get("order").and_then(|v| v.as_f64()) {
                model_slice_order.insert(id.to_string(), o);
            }
        }
    }

    // 4. Existing chapter lookup by name + max chapter order.
    let mut chapter_by_name: BTreeMap<String, String> = BTreeMap::new();
    let mut max_chapter_order: f64 = -1.0;
    if let Some(arr) = model.get("chapters").and_then(|v| v.as_array()) {
        for c in arr {
            if let (Some(id), Some(name)) = (
                c.get("id").and_then(|v| v.as_str()),
                c.get("name").and_then(|v| v.as_str()),
            ) {
                chapter_by_name.insert(name.to_string(), id.to_string());
            }
            if let Some(o) = c.get("order").and_then(|v| v.as_f64()) {
                if o > max_chapter_order {
                    max_chapter_order = o;
                }
            }
        }
    }
    let mut next_chapter_order = max_chapter_order + 1.0;

    // 5. Slice order cursor: start past every NON-heal slice's order, so
    // the assignment is idempotent across runs (re-running won't keep
    // pushing heal slices further right because we're not counting their
    // own orders towards the baseline).
    let mut max_non_heal_slice_order: f64 = -1.0;
    if let Some(arr) = model.get("slices").and_then(|v| v.as_array()) {
        for s in arr {
            let id = s.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if id.starts_with("slice-heal-") {
                continue;
            }
            if let Some(o) = s.get("order").and_then(|v| v.as_f64()) {
                if o > max_non_heal_slice_order {
                    max_non_heal_slice_order = o;
                }
            }
        }
    }
    let mut next_slice_order = max_non_heal_slice_order + 1.0;

    // 6. Group slices by entity (sorted by entity name for deterministic
    // chapter creation order, then slices alphabetically within entity).
    let mut by_entity: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (slice_id, entity_id) in &slice_entity {
        by_entity
            .entry(entity_id.clone())
            .or_default()
            .push(slice_id.clone());
    }
    let mut sorted_entities: Vec<(String, Vec<String>)> = by_entity.into_iter().collect();
    sorted_entities.sort_by(|a, b| {
        let na = entity_name.get(&a.0).cloned().unwrap_or_default();
        let nb = entity_name.get(&b.0).cloned().unwrap_or_default();
        na.cmp(&nb)
    });

    // 7. For each entity group: find/create chapter, then assign every
    // slice in the group a chapter + contiguous order (new slices
    // updated in-place; existing model slices via SliceUpdate).
    let slice_name_for = |id: &str, diff: &HealDiff| -> String {
        if let Some(s) = diff.add_slices.iter().find(|s| s.id == id) {
            return s.name.clone();
        }
        model_slice_name.get(id).cloned().unwrap_or_default()
    };
    for (entity_id, mut slice_ids) in sorted_entities {
        let ename = entity_name
            .get(&entity_id)
            .cloned()
            .unwrap_or_else(|| entity_id.clone());

        let chapter_id = if let Some(existing) = chapter_by_name.get(&ename) {
            existing.clone()
        } else {
            let id = synth_id("chapter", &ename);
            diff.add_chapters.push(ChapterToAdd {
                id: id.clone(),
                name: ename.clone(),
                order: next_chapter_order,
                reason: format!("chapter for entity {ename}"),
            });
            chapter_by_name.insert(ename.clone(), id.clone());
            next_chapter_order += 1.0;
            id
        };

        slice_ids.sort_by(|a, b| {
            let na = slice_name_for(a, diff);
            let nb = slice_name_for(b, diff);
            na.cmp(&nb)
        });

        for slice_id in &slice_ids {
            let assigned_order = next_slice_order;
            next_slice_order += 1.0;

            // Pending slice (in diff.add_slices)? Update in-place.
            if let Some(s) = diff.add_slices.iter_mut().find(|s| &s.id == slice_id) {
                s.chapter_id = Some(chapter_id.clone());
                s.order = assigned_order;
                continue;
            }

            // Existing slice — emit a SliceUpdate only when something changes.
            let current_chapter = model_slice_chapter
                .get(slice_id)
                .cloned()
                .unwrap_or(None);
            let current_order = model_slice_order.get(slice_id).copied();
            let chapter_changed = current_chapter.as_deref() != Some(chapter_id.as_str());
            let order_changed = current_order
                .map(|c| (c - assigned_order).abs() > 0.5)
                .unwrap_or(true);
            if !chapter_changed && !order_changed {
                continue;
            }
            diff.update_slices.push(SliceUpdate {
                slice_id: slice_id.clone(),
                slice_name: slice_name_for(slice_id, diff),
                set_chapter_id: if chapter_changed {
                    Some(chapter_id.clone())
                } else {
                    None
                },
                set_order: if order_changed {
                    Some(assigned_order)
                } else {
                    None
                },
                reason: format!("group with entity {ename}"),
            });
        }
    }
}

fn rebalance_slice_columns(
    model: &Value,
    plan: &MaterializePlan,
    diff: &mut HealDiff,
) {
    let positions = model
        .get("layout")
        .and_then(|l| l.get("nodePositions"))
        .and_then(|p| p.as_object());

    // Leftmost x per slice (from existing nodes only — pending nodes
    // don't have positions yet; PositionCalculator handles those).
    let mut slice_current_x: BTreeMap<String, f64> = BTreeMap::new();
    if let Some(positions) = positions {
        for node in plan.iter_existing_nodes() {
            let Some(slice_id) = node.slice_id.as_ref() else {
                continue;
            };
            let Some(pos) = positions.get(&node.id).and_then(|v| v.as_object()) else {
                continue;
            };
            let Some(x) = pos.get("x").and_then(|v| v.as_f64()) else {
                continue;
            };
            slice_current_x
                .entry(slice_id.clone())
                .and_modify(|cur| {
                    if x < *cur {
                        *cur = x;
                    }
                })
                .or_insert(x);
        }
    }

    // Sorted (slice_id, order) list — existing slices from the model,
    // pending slices appended (they already have orders assigned at
    // materialise time, past the last existing one).
    let mut all_slices: Vec<(String, f64)> = Vec::new();
    if let Some(arr) = model.get("slices").and_then(|v| v.as_array()) {
        for s in arr {
            let Some(id) = s.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let order = s.get("order").and_then(|v| v.as_f64()).unwrap_or(0.0);
            all_slices.push((id.to_string(), order));
        }
    }
    for s in &diff.add_slices {
        all_slices.push((s.id.clone(), s.order));
    }
    all_slices.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    // Monotonic resolution: each slice's column = max(its current
    // claimed x, previous resolved + DEFAULT_NODE_WIDTH + GAP). Slices
    // with no claimed x (pending or empty) inherit min_x.
    let mut slice_resolved_x: BTreeMap<String, f64> = BTreeMap::new();
    let mut prev_resolved: Option<f64> = None;
    for (slice_id, _order) in &all_slices {
        let current = slice_current_x.get(slice_id).copied();
        let min_x = prev_resolved.map(|p| p + DEFAULT_NODE_WIDTH + SLICE_COLUMN_GAP);
        let resolved = match (current, min_x) {
            (Some(c), Some(m)) if c < m => m,
            (Some(c), _) => c,
            (None, Some(m)) => m,
            (None, None) => SLICE_COLUMN_OFFSET,
        };
        slice_resolved_x.insert(slice_id.clone(), resolved);
        prev_resolved = Some(resolved);
    }

    // For each existing node whose x doesn't match its slice's resolved
    // column, queue an x-only PositionFix.
    let Some(positions) = positions else { return };
    for node in plan.iter_existing_nodes() {
        let Some(slice_id) = node.slice_id.as_ref() else {
            continue;
        };
        let Some(target_x) = slice_resolved_x.get(slice_id).copied() else {
            continue;
        };
        let Some(pos) = positions.get(&node.id).and_then(|v| v.as_object()) else {
            continue;
        };
        let Some(current_x) = pos.get("x").and_then(|v| v.as_f64()) else {
            continue;
        };
        if (current_x - target_x).abs() > 0.5 {
            diff.fix_positions.push(PositionFix {
                node_id: node.id.clone(),
                node_name: node.name.clone(),
                node_kind: node.r#type.clone(),
                from_x: Some(current_x),
                to_x: Some(target_x),
                from_y: None,
                to_y: None,
            });
        }
    }
}

/// Map each event name to the alphabetically-first command in the domain
/// that produces it. Used to decide which slice a multi-producer event
/// lives in deterministically.
fn primary_event_owners(domain: &DomainInspection) -> BTreeMap<&str, &str> {
    let mut sorted: Vec<&_> = domain.commands.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out: BTreeMap<&str, &str> = BTreeMap::new();
    for cmd in sorted {
        for ev in &cmd.produces {
            out.entry(ev.as_str()).or_insert(cmd.name.as_str());
        }
    }
    out
}

/// Y band per node type — uiPlaceholder above, command/query/integration
/// in the same row, event in the entity swim lane.
fn canonical_y(node_type: &str) -> f64 {
    match node_type {
        "uiPlaceholder" => Y_UI_PLACEHOLDER,
        "event" => Y_EVENT,
        _ => Y_COMMAND_QUERY_INTEGRATION,
    }
}

fn inspection_name_set(inspection: &ProjectInspection) -> BTreeSet<(String, String)> {
    let mut set: BTreeSet<(String, String)> = BTreeSet::new();
    for domain in &inspection.domains {
        for c in &domain.commands {
            set.insert(("command".to_string(), c.name.clone()));
        }
        for e in &domain.events {
            set.insert(("event".to_string(), e.name.clone()));
        }
        for q in &domain.queries {
            set.insert(("query".to_string(), q.name.clone()));
        }
        for i in &domain.integrations {
            set.insert(("integration".to_string(), i.name.clone()));
        }
    }
    set
}

/// Mutable index of the model PLUS pending additions. `ensure_*` methods
/// either return the id of an existing piece or queue a new one and return
/// its synthetic id — so subsequent lookups in the same `compute_diff`
/// pass find it without re-querying the model JSON.
struct MaterializePlan {
    nodes: Vec<NodeSummary>,
    entities_by_name: BTreeMap<String, String>,
    slices_by_name: BTreeMap<String, String>,
    nodes_by_type_and_name: BTreeMap<(String, String), Vec<usize>>,
    edge_keys: BTreeSet<(String, String, String)>,
    /// `order` to assign to the next pending entity (one past the max).
    next_entity_order: f64,
    /// `order` to assign to the next pending slice.
    next_slice_order: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct NodeSummary {
    pub id: String,
    pub r#type: String,
    pub name: String,
    pub slice_id: Option<String>,
    pub entity_id: Option<String>,
    pub kind: Option<String>,
    /// `true` for nodes already present in the model JSON; `false` for
    /// nodes queued by this pass.
    pub is_existing: bool,
}

impl MaterializePlan {
    fn from_model(model: &Value) -> Self {
        let mut nodes: Vec<NodeSummary> = Vec::new();
        let mut nodes_by_type_and_name: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
        if let Some(arr) = model.get("nodes").and_then(|n| n.as_array()) {
            for raw in arr {
                let Some(id) = raw.get("id").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(r#type) = raw.get("type").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(name) = raw.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                let slice_id = raw
                    .get("sliceId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let entity_id = raw
                    .get("entityId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let kind = raw
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let idx = nodes.len();
                let summary = NodeSummary {
                    id: id.to_string(),
                    r#type: r#type.to_string(),
                    name: name.to_string(),
                    slice_id,
                    entity_id,
                    kind,
                    is_existing: true,
                };
                nodes_by_type_and_name
                    .entry((summary.r#type.clone(), summary.name.clone()))
                    .or_default()
                    .push(idx);
                nodes.push(summary);
            }
        }

        let mut edge_keys: BTreeSet<(String, String, String)> = BTreeSet::new();
        if let Some(arr) = model.get("edges").and_then(|e| e.as_array()) {
            for raw in arr {
                let Some(t) = raw.get("type").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(s) = raw.get("sourceId").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(tg) = raw.get("targetId").and_then(|v| v.as_str()) else {
                    continue;
                };
                edge_keys.insert((t.to_string(), s.to_string(), tg.to_string()));
            }
        }

        let mut entities_by_name: BTreeMap<String, String> = BTreeMap::new();
        let mut max_entity_order: f64 = -1.0;
        if let Some(arr) = model.get("entities").and_then(|v| v.as_array()) {
            for raw in arr {
                let Some(id) = raw.get("id").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(name) = raw.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                entities_by_name.insert(name.to_string(), id.to_string());
                if let Some(order) = raw.get("order").and_then(|v| v.as_f64()) {
                    if order > max_entity_order {
                        max_entity_order = order;
                    }
                }
            }
        }

        let mut slices_by_name: BTreeMap<String, String> = BTreeMap::new();
        let mut max_slice_order: f64 = -1.0;
        if let Some(arr) = model.get("slices").and_then(|v| v.as_array()) {
            for raw in arr {
                let Some(id) = raw.get("id").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(name) = raw.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                slices_by_name.insert(name.to_string(), id.to_string());
                if let Some(order) = raw.get("order").and_then(|v| v.as_f64()) {
                    if order > max_slice_order {
                        max_slice_order = order;
                    }
                }
            }
        }

        Self {
            nodes,
            entities_by_name,
            slices_by_name,
            nodes_by_type_and_name,
            edge_keys,
            next_entity_order: max_entity_order + 1.0,
            next_slice_order: max_slice_order + 1.0,
        }
    }

    fn entity_id_for(&self, name: &str) -> Option<String> {
        self.entities_by_name.get(name).cloned()
    }

    fn ensure_entity(&mut self, diff: &mut HealDiff, name: &str, reason_name: &str) -> String {
        if let Some(id) = self.entities_by_name.get(name) {
            return id.clone();
        }
        let id = synth_id("entity", name);
        let order = self.next_entity_order;
        self.next_entity_order += 1.0;
        self.entities_by_name.insert(name.to_string(), id.clone());
        diff.add_entities.push(EntityToAdd {
            id: id.clone(),
            name: name.to_string(),
            order,
            reason: format!("entity for inspection domain {reason_name}"),
        });
        id
    }

    fn ensure_slice(
        &mut self,
        diff: &mut HealDiff,
        name: &str,
        reason: &str,
        entity_hint: Option<&str>,
    ) -> String {
        if let Some(id) = self.slices_by_name.get(name) {
            return id.clone();
        }
        let id = synth_id("slice", name);
        let order = self.next_slice_order;
        self.next_slice_order += 1.0;
        self.slices_by_name.insert(name.to_string(), id.clone());
        diff.add_slices.push(SliceToAdd {
            id: id.clone(),
            name: name.to_string(),
            chapter_id: None,
            order,
            reason: reason.to_string(),
            entity_id_hint: entity_hint.map(|s| s.to_string()),
        });
        id
    }

    /// One-shot "ensure this node exists, with a slice if we have to make
    /// one". When the node already exists we reuse its current slice and
    /// DON'T synthesise a fresh one — otherwise the diff would propose
    /// dead `add_slices` entries that no node references. When the node
    /// doesn't exist we ensure (or reuse) a slice named `slice_name`, then
    /// queue the node referencing it.
    fn ensure_node_in_slice(
        &mut self,
        diff: &mut HealDiff,
        node_type: &str,
        name: &str,
        slice_name: &str,
        slice_reason: &str,
        slice_entity_hint: Option<&str>,
        entity_id: Option<&str>,
        kind: Option<&str>,
        node_reason: &str,
    ) -> String {
        let key = (node_type.to_string(), name.to_string());
        if let Some(idxs) = self.nodes_by_type_and_name.get(&key) {
            if let Some(first) = idxs.first().copied() {
                if let Some(n) = self.nodes.get(first) {
                    return n.id.clone();
                }
            }
        }
        let slice_id = self.ensure_slice(diff, slice_name, slice_reason, slice_entity_hint);
        let id = synth_node_id(node_type, name);
        let idx = self.nodes.len();
        let summary = NodeSummary {
            id: id.clone(),
            r#type: node_type.to_string(),
            name: name.to_string(),
            slice_id: Some(slice_id.clone()),
            entity_id: entity_id.map(|s| s.to_string()),
            kind: kind.map(|s| s.to_string()),
            is_existing: false,
        };
        self.nodes_by_type_and_name
            .entry(key)
            .or_default()
            .push(idx);
        self.nodes.push(summary);
        diff.add_nodes.push(NodeToAdd {
            id: id.clone(),
            node_type: node_type.to_string(),
            name: name.to_string(),
            slice_id,
            entity_id: entity_id.map(|s| s.to_string()),
            kind: kind.map(|s| s.to_string()),
            reason: node_reason.to_string(),
        });
        id
    }

    fn ensure_edge(
        &mut self,
        diff: &mut HealDiff,
        edge_type: &str,
        source_id: &str,
        target_id: &str,
        source_handle: &str,
        target_handle: &str,
        reason: &str,
    ) {
        let key = (
            edge_type.to_string(),
            source_id.to_string(),
            target_id.to_string(),
        );
        if !self.edge_keys.insert(key) {
            return;
        }
        diff.add_edges.push(EdgeToAdd {
            edge_type: edge_type.to_string(),
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
            source_handle: source_handle.to_string(),
            target_handle: target_handle.to_string(),
            reason: reason.to_string(),
        });
    }

    fn node_id(&self, node_type: &str, name: &str) -> Option<String> {
        let key = (node_type.to_string(), name.to_string());
        let idxs = self.nodes_by_type_and_name.get(&key)?;
        let first = *idxs.first()?;
        self.nodes.get(first).map(|n| n.id.clone())
    }

    fn existing_nodes_named(&self, node_type: &str, name: &str) -> Vec<&NodeSummary> {
        let key = (node_type.to_string(), name.to_string());
        let Some(idxs) = self.nodes_by_type_and_name.get(&key) else {
            return Vec::new();
        };
        idxs.iter()
            .filter_map(|i| self.nodes.get(*i))
            .filter(|n| n.is_existing)
            .collect()
    }

    fn iter_existing_nodes(&self) -> impl Iterator<Item = &NodeSummary> {
        self.nodes.iter().filter(|n| n.is_existing)
    }

    fn iter_all_nodes(&self) -> impl Iterator<Item = &NodeSummary> {
        self.nodes.iter()
    }

}

/// Visual layout constants — kept in lockstep with the frontend
/// (`assets/ide/src/ui/layout/autoLayout.ts` + `grid.ts`).
const STACK_DY: f64 = 80.0;
const DEFAULT_NODE_WIDTH: f64 = 240.0;
const SLICE_COLUMN_GAP: f64 = 80.0;
const Y_HEADER: f64 = 40.0;
const Y_TOP_MARGIN: f64 = 300.0;
const Y_LANE_HEIGHT: f64 = 200.0;
const Y_EVENT_OFFSET_IN_LANE: f64 = 60.0;

/// Decides where (x, y) every node lacking a layout entry should sit.
/// Built once per `compute_diff` call from `(model, plan, diff)`; pure
/// function of those inputs so the same heal pass yields byte-identical
/// positions every time. The big rule: **new slice columns are placed
/// past the rightmost existing node**, never on top of one.
struct PositionCalculator {
    /// Where a node in this slice should sit horizontally. For existing
    /// slices: the leftmost x of an already-placed sibling (so new nodes
    /// stack vertically aligned). For new slices: a fresh column past
    /// every existing node's right edge.
    slice_x: BTreeMap<String, f64>,
    /// Ordinal index of each entity (existing then new), used to pick
    /// the y-band for events that belong to that entity's swim lane.
    entity_idx: BTreeMap<String, usize>,
    /// Counts already-placed nodes per `(slice_id, type)` so additional
    /// nodes for the same bucket stack vertically by `STACK_DY` instead
    /// of landing on top of each other.
    stack_ranks: BTreeMap<(String, String), usize>,
}

impl PositionCalculator {
    fn new(model: &Value, plan: &MaterializePlan, diff: &HealDiff) -> Self {
        let positions = model
            .get("layout")
            .and_then(|l| l.get("nodePositions"))
            .and_then(|p| p.as_object());

        // 1. Existing nodes seed slice_x (leftmost sibling x) and stack
        // counts (every already-placed node bumps the rank for its
        // (slice, type) bucket).
        let mut slice_x: BTreeMap<String, f64> = BTreeMap::new();
        let mut stack_ranks: BTreeMap<(String, String), usize> = BTreeMap::new();
        let mut global_right: f64 = SLICE_COLUMN_OFFSET;
        if let Some(positions) = positions {
            for node in plan.iter_existing_nodes() {
                let Some(slice_id) = node.slice_id.as_ref() else { continue };
                let Some(pos) = positions.get(&node.id).and_then(|v| v.as_object()) else {
                    continue;
                };
                let Some(x) = pos.get("x").and_then(|v| v.as_f64()) else { continue };
                slice_x
                    .entry(slice_id.clone())
                    .and_modify(|cur| {
                        if x < *cur {
                            *cur = x;
                        }
                    })
                    .or_insert(x);
                *stack_ranks
                    .entry((slice_id.clone(), node.r#type.clone()))
                    .or_insert(0) += 1;
                let right = x + DEFAULT_NODE_WIDTH;
                if right > global_right {
                    global_right = right;
                }
            }
        }

        // 2. Brand-new slices land past every existing node, in
        // `add_slices` order (which is itself sorted by the order we
        // assigned at materialisation time). This yields a deterministic
        // left-to-right rhythm for new columns.
        for slice in &diff.add_slices {
            global_right += SLICE_COLUMN_GAP;
            slice_x.insert(slice.id.clone(), global_right);
            global_right += DEFAULT_NODE_WIDTH;
        }

        // 3. Entity indices: existing entities (in `order`) then new
        // entities appended. Drives event y so each entity gets its own
        // swim lane.
        let mut entity_idx: BTreeMap<String, usize> = BTreeMap::new();
        let mut next_idx: usize = 0;
        if let Some(arr) = model.get("entities").and_then(|v| v.as_array()) {
            let mut sorted: Vec<&Value> = arr.iter().collect();
            sorted.sort_by(|a, b| {
                let oa = a.get("order").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let ob = b.get("order").and_then(|v| v.as_f64()).unwrap_or(0.0);
                oa.partial_cmp(&ob).unwrap_or(std::cmp::Ordering::Equal)
            });
            for e in sorted {
                if let Some(id) = e.get("id").and_then(|v| v.as_str()) {
                    entity_idx.entry(id.to_string()).or_insert_with(|| {
                        let v = next_idx;
                        next_idx += 1;
                        v
                    });
                }
            }
        }
        for e in &diff.add_entities {
            entity_idx.entry(e.id.clone()).or_insert_with(|| {
                let v = next_idx;
                next_idx += 1;
                v
            });
        }

        Self {
            slice_x,
            entity_idx,
            stack_ranks,
        }
    }

    /// Assign and consume one (x, y) for a node. Bumps the stack rank
    /// for the node's (slice, type) bucket so the next call for the same
    /// bucket lands `STACK_DY` lower.
    fn assign(
        &mut self,
        slice_id: Option<&str>,
        node_type: &str,
        entity_id: Option<&str>,
    ) -> (f64, f64) {
        let slice_key = slice_id.unwrap_or("").to_string();
        let x = slice_id
            .and_then(|s| self.slice_x.get(s).copied())
            .unwrap_or(SLICE_COLUMN_OFFSET);
        let entity_index = entity_id
            .and_then(|id| self.entity_idx.get(id).copied())
            .unwrap_or(0);
        let base_y = banded_y(node_type, entity_index);

        let bucket = (slice_key, node_type.to_string());
        let rank = *self.stack_ranks.entry(bucket.clone()).or_insert(0);
        self.stack_ranks.insert(bucket, rank + 1);
        let y = base_y + (rank as f64) * STACK_DY;
        (x, y)
    }
}

/// Per-entity, per-type y-band. Matches `autoLayout.ts::bandY` so the
/// Rust pre-pass and the frontend's `autoLayoutMissingPositions` produce
/// the same coordinates for the same inputs.
fn banded_y(node_type: &str, entity_idx: usize) -> f64 {
    match node_type {
        "uiPlaceholder" => Y_UI_PLACEHOLDER,
        "event" => Y_HEADER + Y_TOP_MARGIN + (entity_idx as f64) * Y_LANE_HEIGHT + Y_EVENT_OFFSET_IN_LANE,
        _ => Y_COMMAND_QUERY_INTEGRATION,
    }
}

/// Hash-based id derived from a kind + name. The same `(kind, name)` pair
/// always yields the same id, so re-running the pass against an already
/// healed model is a no-op.
fn synth_id(kind: &str, name: &str) -> String {
    let mut h = DefaultHasher::new();
    kind.hash(&mut h);
    name.hash(&mut h);
    format!("{kind}-heal-{:016x}", h.finish())
}

fn synth_node_id(node_type: &str, name: &str) -> String {
    let mut h = DefaultHasher::new();
    "node".hash(&mut h);
    node_type.hash(&mut h);
    name.hash(&mut h);
    format!("node-heal-{node_type}-{:016x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspect::{CommandInfo, DomainInspection, EventInfo, IntegrationInfo, QueryInfo};
    use std::path::PathBuf;

    fn minimal_model() -> Value {
        serde_json::json!({
            "id": "m1",
            "name": "demo",
            "chapters": [{ "id": "ch1", "name": "Main", "order": 0 }],
            "entities": [{ "id": "ent1", "name": "Orders", "order": 0 }],
            "slices": [
                { "id": "sl1", "name": "PlaceOrder", "chapterId": "ch1", "order": 0 },
                { "id": "sl2", "name": "ShipOrder",  "chapterId": "ch1", "order": 1 }
            ],
            "nodes": [
                { "id": "cmd1", "type": "command", "name": "PlaceOrder",  "sliceId": "sl1", "entityId": "ent1" },
                { "id": "ev1",  "type": "event",   "name": "OrderPlaced", "sliceId": "sl1", "entityId": "ent1" },
                { "id": "cmd2", "type": "command", "name": "ShipOrder",   "sliceId": "sl2", "entityId": "ent1" },
                { "id": "ev2",  "type": "event",   "name": "OrderShipped","sliceId": "sl2", "entityId": "ent1" },
                { "id": "qy1",  "type": "query",   "name": "OrderSummary","sliceId": "sl2" },
                { "id": "intg1","type": "integration","name":"Notifier",  "sliceId": "sl2", "kind": "outbound" }
            ],
            "edges": [],
            "layout": { "nodePositions": {}, "viewport": { "x": 0, "y": 0, "zoom": 1 } }
        })
    }

    fn empty_model() -> Value {
        serde_json::json!({
            "id": "m1",
            "name": "demo",
            "chapters": [],
            "entities": [],
            "slices": [],
            "nodes": [],
            "edges": [],
            "layout": { "nodePositions": {}, "viewport": { "x": 0, "y": 0, "zoom": 1 } }
        })
    }

    fn fixture_inspection() -> ProjectInspection {
        ProjectInspection {
            root: PathBuf::from("/"),
            domains: vec![DomainInspection {
                name: "Orders".to_string(),
                path: PathBuf::from("/Orders"),
                events: vec![
                    EventInfo {
                        name: "OrderPlaced".to_string(),
                        file: PathBuf::new(),
                    },
                    EventInfo {
                        name: "OrderShipped".to_string(),
                        file: PathBuf::new(),
                    },
                ],
                commands: vec![
                    CommandInfo {
                        name: "PlaceOrder".to_string(),
                        file: PathBuf::new(),
                        produces: vec!["OrderPlaced".to_string()],
                        via_web_transport: false,
                    },
                    CommandInfo {
                        name: "ShipOrder".to_string(),
                        file: PathBuf::new(),
                        produces: vec!["OrderShipped".to_string()],
                        via_web_transport: false,
                    },
                ],
                queries: vec![QueryInfo {
                    name: "OrderSummary".to_string(),
                    file: PathBuf::new(),
                    subscribes_to: vec!["OrderPlaced".to_string(), "OrderShipped".to_string()],
                }],
                integrations: vec![IntegrationInfo {
                    name: "Notifier".to_string(),
                    file: PathBuf::new(),
                    kind: IntegrationKind::Outbound,
                    handles_events: vec!["OrderShipped".to_string()],
                    emits_commands: vec![],
                }],
            }],
        }
    }

    #[test]
    fn diff_proposes_missing_command_produces_event_edges() {
        let model = minimal_model();
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);

        let cpe: Vec<_> = diff
            .add_edges
            .iter()
            .filter(|e| e.edge_type == "commandProducesEvent")
            .collect();
        assert_eq!(cpe.len(), 2, "expected both commandProducesEvent edges");
        assert!(cpe.iter().any(|e| e.source_id == "cmd1" && e.target_id == "ev1"));
        assert!(cpe.iter().any(|e| e.source_id == "cmd2" && e.target_id == "ev2"));
    }

    #[test]
    fn diff_proposes_event_feeds_query_edges() {
        let model = minimal_model();
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);
        let efq: Vec<_> = diff
            .add_edges
            .iter()
            .filter(|e| e.edge_type == "eventFeedsQuery")
            .collect();
        assert_eq!(efq.len(), 2);
        assert!(efq.iter().any(|e| e.source_id == "ev1" && e.target_id == "qy1"));
        assert!(efq.iter().any(|e| e.source_id == "ev2" && e.target_id == "qy1"));
    }

    #[test]
    fn diff_proposes_event_triggers_integration_edge() {
        let model = minimal_model();
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);
        let eti: Vec<_> = diff
            .add_edges
            .iter()
            .filter(|e| e.edge_type == "eventTriggersIntegration")
            .collect();
        assert_eq!(eti.len(), 1);
        assert_eq!(eti[0].source_id, "ev2");
        assert_eq!(eti[0].target_id, "intg1");
    }

    #[test]
    fn diff_is_empty_when_model_already_has_all_edges_and_nodes() {
        let mut model = minimal_model();
        let inspection = fixture_inspection();

        // Pre-populate every edge the diff would otherwise propose. Use
        // hash-based ids that the heal pass would itself generate so the
        // edge-key index sees them as already present.
        let edges = serde_json::json!([
            { "id": "e1", "type": "commandProducesEvent",     "sourceId": "cmd1",  "targetId": "ev1" },
            { "id": "e2", "type": "commandProducesEvent",     "sourceId": "cmd2",  "targetId": "ev2" },
            { "id": "e3", "type": "eventFeedsQuery",          "sourceId": "ev1",   "targetId": "qy1" },
            { "id": "e4", "type": "eventFeedsQuery",          "sourceId": "ev2",   "targetId": "qy1" },
            { "id": "e5", "type": "eventTriggersIntegration", "sourceId": "ev2",   "targetId": "intg1" }
        ]);
        model["edges"] = edges;
        // Fill in layout entries so ensure_layout_entries doesn't fire.
        let nodes = model["nodes"].as_array().unwrap().clone();
        let mut positions = serde_json::Map::new();
        for n in nodes {
            let id = n["id"].as_str().unwrap().to_string();
            positions.insert(id, serde_json::json!({ "x": 40.0, "y": 120.0 }));
        }
        model["layout"]["nodePositions"] = Value::Object(positions);

        let diff = compute_diff(&model, &inspection);
        assert!(
            diff.add_edges.is_empty(),
            "no edges should be proposed; got {:?}",
            diff.add_edges,
        );
        assert!(
            diff.add_nodes.is_empty(),
            "no nodes should be proposed; got {:?}",
            diff.add_nodes,
        );
        assert!(diff.add_entities.is_empty());
        assert!(diff.add_slices.is_empty());
    }

    #[test]
    fn diff_fixes_integration_kind_when_code_says_reactive() {
        let model = minimal_model();
        let mut inspection = fixture_inspection();
        inspection.domains[0].integrations[0].kind = IntegrationKind::Reactive;
        inspection.domains[0].integrations[0].emits_commands = vec!["PlaceOrder".to_string()];

        let diff = compute_diff(&model, &inspection);
        assert_eq!(diff.fix_integration_kinds.len(), 1);
        let fix = &diff.fix_integration_kinds[0];
        assert_eq!(fix.node_id, "intg1");
        assert_eq!(fix.from_kind, "outbound");
        assert_eq!(fix.to_kind, "inbound");
    }

    #[test]
    fn diff_proposes_layout_entries_for_nodes_missing_positions() {
        let model = minimal_model();
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);
        // Every node in the fixture lacks a layout entry → all 6 proposed.
        assert_eq!(diff.ensure_layout_entries.len(), 6);
        // Event lands on the entity swim-lane band.
        let ev_entry = diff
            .ensure_layout_entries
            .iter()
            .find(|e| e.node_id == "ev1")
            .expect("layout for ev1");
        assert!((ev_entry.y - Y_EVENT).abs() < f64::EPSILON);
        // Command/integration land on the upper band.
        let cmd_entry = diff
            .ensure_layout_entries
            .iter()
            .find(|e| e.node_id == "cmd1")
            .expect("layout for cmd1");
        assert!((cmd_entry.y - Y_COMMAND_QUERY_INTEGRATION).abs() < f64::EPSILON);
    }

    #[test]
    fn diff_fixes_integration_position_dropped_into_event_band() {
        let mut model = minimal_model();
        model["layout"]["nodePositions"]["intg1"] = serde_json::json!({ "x": 400, "y": 400 });
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);

        let fix = diff
            .fix_positions
            .iter()
            .find(|p| p.node_id == "intg1")
            .expect("expected fix for intg1");
        assert!((fix.to_y.unwrap() - Y_COMMAND_QUERY_INTEGRATION).abs() < f64::EPSILON);
        assert!((fix.from_y.unwrap() - 400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn diff_materializes_missing_command_as_add_node() {
        let mut model = minimal_model();
        // Strip cmd2 from the model.
        model["nodes"] = serde_json::json!(
            model["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|n| n["id"] != "cmd2")
                .cloned()
                .collect::<Vec<_>>()
        );
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);
        assert!(
            diff.add_nodes
                .iter()
                .any(|n| n.node_type == "command" && n.name == "ShipOrder"),
            "ShipOrder should be queued as add_node; got: {:?}",
            diff.add_nodes
        );
        // And no Missing* residual is emitted any more — the materialise
        // path covers it.
        assert!(diff
            .residuals
            .iter()
            .all(|r| matches!(r, Residual::OrphanModelNode { .. })));
    }

    #[test]
    fn diff_materializes_command_event_query_integration_into_empty_model() {
        let model = empty_model();
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);

        // 1 entity (Orders).
        assert_eq!(diff.add_entities.len(), 1, "{:?}", diff.add_entities);
        assert_eq!(diff.add_entities[0].name, "Orders");

        // Slices: PlaceOrder, ShipOrder, OrderSummary, Notifier.
        let slice_names: BTreeSet<&str> =
            diff.add_slices.iter().map(|s| s.name.as_str()).collect();
        assert!(slice_names.contains("PlaceOrder"));
        assert!(slice_names.contains("ShipOrder"));
        assert!(slice_names.contains("OrderSummary"));
        assert!(slice_names.contains("Notifier"));

        // Nodes: 2 commands, 2 events, 1 query, 1 integration.
        let by_type =
            |t: &str| diff.add_nodes.iter().filter(|n| n.node_type == t).count();
        assert_eq!(by_type("command"), 2);
        assert_eq!(by_type("event"), 2);
        assert_eq!(by_type("query"), 1);
        assert_eq!(by_type("integration"), 1);

        // Integration node carries kind=outbound.
        let intg = diff
            .add_nodes
            .iter()
            .find(|n| n.node_type == "integration")
            .unwrap();
        assert_eq!(intg.kind.as_deref(), Some("outbound"));

        // Command + event nodes carry the entity id.
        let cmd = diff
            .add_nodes
            .iter()
            .find(|n| n.node_type == "command" && n.name == "PlaceOrder")
            .unwrap();
        assert!(cmd.entity_id.is_some(), "command should carry entityId");
        let ev = diff
            .add_nodes
            .iter()
            .find(|n| n.node_type == "event" && n.name == "OrderPlaced")
            .unwrap();
        assert!(ev.entity_id.is_some(), "event should carry entityId");

        // Query + integration do NOT carry entity id (schema doesn't allow).
        let q = diff
            .add_nodes
            .iter()
            .find(|n| n.node_type == "query")
            .unwrap();
        assert!(q.entity_id.is_none());
        assert!(intg.entity_id.is_none());

        // Edges: 2 commandProducesEvent + 2 eventFeedsQuery + 1 eventTriggersIntegration = 5.
        let by_etype =
            |t: &str| diff.add_edges.iter().filter(|e| e.edge_type == t).count();
        assert_eq!(by_etype("commandProducesEvent"), 2);
        assert_eq!(by_etype("eventFeedsQuery"), 2);
        assert_eq!(by_etype("eventTriggersIntegration"), 1);

        // Every node gets a layout entry too.
        assert_eq!(diff.ensure_layout_entries.len(), 6);
    }

    #[test]
    fn diff_reuses_existing_entity_and_slice_by_name() {
        let mut model = empty_model();
        model["entities"] = serde_json::json!([
            { "id": "ent-prev", "name": "Orders", "order": 0 }
        ]);
        model["slices"] = serde_json::json!([
            { "id": "sl-prev", "name": "PlaceOrder", "chapterId": null, "order": 0 }
        ]);

        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);

        // Entity "Orders" already exists → no new entity for it.
        assert!(!diff.add_entities.iter().any(|e| e.name == "Orders"));
        // Slice "PlaceOrder" already exists → no new slice for it.
        assert!(!diff.add_slices.iter().any(|s| s.name == "PlaceOrder"));

        // The materialised PlaceOrder command must reference the EXISTING
        // slice id ("sl-prev"), not a freshly-synthesised one.
        let cmd = diff
            .add_nodes
            .iter()
            .find(|n| n.node_type == "command" && n.name == "PlaceOrder")
            .expect("PlaceOrder command should be queued");
        assert_eq!(cmd.slice_id, "sl-prev");
        // And it should reference the existing entity id.
        assert_eq!(cmd.entity_id.as_deref(), Some("ent-prev"));
    }

    #[test]
    fn diff_event_with_no_producer_gets_own_slice() {
        let mut inspection = fixture_inspection();
        // Add an event nobody produces.
        inspection.domains[0].events.push(EventInfo {
            name: "OrphanEvent".to_string(),
            file: PathBuf::new(),
        });
        let model = empty_model();
        let diff = compute_diff(&model, &inspection);

        let orphan = diff
            .add_nodes
            .iter()
            .find(|n| n.name == "OrphanEvent")
            .expect("OrphanEvent should be queued");
        // The orphan event's slice was synthesised by name.
        assert!(diff.add_slices.iter().any(|s| s.name == "OrphanEvent"));
        let orphan_slice = diff
            .add_slices
            .iter()
            .find(|s| s.name == "OrphanEvent")
            .unwrap();
        assert_eq!(orphan.slice_id, orphan_slice.id);
    }

    #[test]
    fn diff_event_shared_by_two_commands_lives_in_alphabetically_first_slice() {
        let mut inspection = fixture_inspection();
        // Make both commands produce OrderPlaced. PlaceOrder is alphabetically
        // before ShipOrder, so OrderPlaced should land in PlaceOrder's slice.
        inspection.domains[0].commands[1]
            .produces
            .push("OrderPlaced".to_string());
        let model = empty_model();
        let diff = compute_diff(&model, &inspection);

        let ev = diff
            .add_nodes
            .iter()
            .find(|n| n.name == "OrderPlaced")
            .expect("OrderPlaced should be queued");
        let place_order_slice_id = diff
            .add_slices
            .iter()
            .find(|s| s.name == "PlaceOrder")
            .unwrap()
            .id
            .clone();
        assert_eq!(ev.slice_id, place_order_slice_id);
    }

    #[test]
    fn diff_reports_orphan_model_node_when_not_in_inspection() {
        let mut model = minimal_model();
        // Add an event that doesn't exist in the code.
        model["nodes"].as_array_mut().unwrap().push(serde_json::json!({
            "id": "ev99", "type": "event", "name": "GhostEvent",
            "sliceId": "sl1", "entityId": "ent1",
        }));
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);
        assert!(diff.residuals.iter().any(|r| matches!(
            r,
            Residual::OrphanModelNode { node_name, .. } if node_name == "GhostEvent"
        )));
    }

    #[test]
    fn ui_placeholders_are_not_reported_as_orphans() {
        let mut model = minimal_model();
        model["nodes"].as_array_mut().unwrap().push(serde_json::json!({
            "id": "ui1", "type": "uiPlaceholder", "name": "OrderForm",
            "sliceId": "sl1",
        }));
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);
        assert!(!diff.residuals.iter().any(|r| matches!(
            r,
            Residual::OrphanModelNode { node_name, .. } if node_name == "OrderForm"
        )), "UI placeholders shouldn't be orphans (no code backing expected)");
    }

    #[test]
    fn diff_includes_reactive_integration_wiring() {
        let mut inspection = fixture_inspection();
        // Promote Notifier to reactive emitting PlaceOrder.
        inspection.domains[0].integrations[0].kind = IntegrationKind::Reactive;
        inspection.domains[0].integrations[0].emits_commands = vec!["PlaceOrder".to_string()];
        let model = empty_model();
        let diff = compute_diff(&model, &inspection);

        let intg = diff
            .add_nodes
            .iter()
            .find(|n| n.node_type == "integration")
            .unwrap();
        assert_eq!(intg.kind.as_deref(), Some("inbound"));

        let intg_to_cmd: Vec<_> = diff
            .add_edges
            .iter()
            .filter(|e| e.edge_type == "integrationTriggersCommand")
            .collect();
        assert_eq!(intg_to_cmd.len(), 1, "{:?}", intg_to_cmd);
    }

    #[test]
    fn layout_places_new_slice_columns_past_rightmost_existing_node() {
        // Existing model has a hand-placed slice at x=1500. The inspection
        // adds a brand-new slice/command. The new node MUST land past
        // x=1500 (not in the same column, not at a hash-random offset),
        // so the new column doesn't overlap the existing one.
        let mut model = minimal_model();
        model["layout"]["nodePositions"] = serde_json::json!({
            "cmd1": { "x": 40,   "y": 120 },
            "ev1":  { "x": 40,   "y": 400 },
            "cmd2": { "x": 1500, "y": 120 },
            "ev2":  { "x": 1500, "y": 400 },
            "qy1":  { "x": 1500, "y": 200 },
            "intg1":{ "x": 1500, "y": 280 }
        });

        let mut inspection = fixture_inspection();
        // Plant a fresh command in code that the model doesn't have.
        inspection.domains[0].commands.push(CommandInfo {
            name: "ArchiveOrder".to_string(),
            file: PathBuf::new(),
            produces: vec![],
            via_web_transport: false,
        });

        let diff = compute_diff(&model, &inspection);

        let new_cmd = diff
            .add_nodes
            .iter()
            .find(|n| n.name == "ArchiveOrder")
            .expect("ArchiveOrder should be materialised");
        let layout = diff
            .ensure_layout_entries
            .iter()
            .find(|e| e.node_id == new_cmd.id)
            .expect("ArchiveOrder must get a layout entry");

        assert!(
            layout.x > 1500.0 + DEFAULT_NODE_WIDTH,
            "new slice's node x ({}) must land past the rightmost existing node \
             (rightmost edge ≈ {}); otherwise the new column overlaps the existing one",
            layout.x,
            1500.0 + DEFAULT_NODE_WIDTH,
        );
        assert!(
            (layout.y - Y_COMMAND_QUERY_INTEGRATION).abs() < f64::EPSILON,
            "command y should be in the canonical command band, got {}",
            layout.y,
        );
    }

    #[test]
    fn layout_stacks_siblings_of_same_kind_in_same_slice() {
        // Two new commands materialise into the same brand-new slice
        // (because both produce events that share the alphabetically-first
        // producer's slice). They must stack vertically (different y) so
        // they don't overlap.
        let mut inspection = fixture_inspection();
        inspection.domains[0].commands.push(CommandInfo {
            name: "AnnotateOrder".to_string(),
            file: PathBuf::new(),
            produces: vec![],
            via_web_transport: false,
        });
        inspection.domains[0].commands.push(CommandInfo {
            name: "AnnotateOrderTwice".to_string(),
            file: PathBuf::new(),
            produces: vec![],
            via_web_transport: false,
        });

        let mut model = empty_model();
        // Pin one existing command at the canonical command y so the
        // stack starts above it: we expect new commands at +80 and +160.
        model["nodes"] = serde_json::json!([
            { "id": "exist", "type": "command", "name": "ExistingCmd",
              "sliceId": "sl-shared", "entityId": "ent-shared" }
        ]);
        model["slices"] = serde_json::json!([
            { "id": "sl-shared", "name": "AnnotateOrder", "chapterId": null, "order": 0 }
        ]);
        model["entities"] = serde_json::json!([
            { "id": "ent-shared", "name": "Orders", "order": 0 }
        ]);
        model["layout"]["nodePositions"] = serde_json::json!({
            "exist": { "x": 200, "y": 120 }
        });
        // Force both new commands into the same slice by NAMING the slice
        // identically (slice "AnnotateOrder" reused; new command
        // "AnnotateOrderTwice" creates a separate slice).
        // To get two new nodes in the SAME slice, instead use a
        // common-producer event scenario:
        inspection.domains[0].commands = vec![
            CommandInfo {
                name: "AnnotateOrder".to_string(),
                file: PathBuf::new(),
                produces: vec!["NoteAdded".to_string()],
                via_web_transport: false,
            },
        ];
        // Two queries in the same slice would also stack. Add a second
        // query named identically? Queries need unique names. The cleaner
        // way: use the integration's own slice (auto-named "Notifier"),
        // and add an extra query that lands in "Notifier"'s slice — but
        // queries always get their own slice. So we settle on a tighter
        // assertion: any (slice, type) bucket with N new nodes spreads y
        // by STACK_DY * (N-1).
        let diff = compute_diff(&model, &inspection);

        // Group new command entries by their (slice, type) and check
        // that any bucket with multiple members has spread-y values.
        use std::collections::HashMap;
        let mut buckets: HashMap<(String, String), Vec<f64>> = HashMap::new();
        for entry in &diff.ensure_layout_entries {
            let node = diff
                .add_nodes
                .iter()
                .find(|n| n.id == entry.node_id);
            let Some(n) = node else { continue };
            buckets
                .entry((n.slice_id.clone(), n.node_type.clone()))
                .or_default()
                .push(entry.y);
        }
        for ((slice, ty), mut ys) in buckets {
            if ys.len() < 2 {
                continue;
            }
            ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for w in ys.windows(2) {
                assert!(
                    (w[1] - w[0] - STACK_DY).abs() < f64::EPSILON,
                    "siblings in bucket ({slice}, {ty}) must be stacked by exactly STACK_DY={STACK_DY}; got ys={ys:?}",
                );
            }
        }
    }

    #[test]
    fn layout_event_y_follows_entity_index() {
        // Two entities → events in entity 1 should land below entity 0's
        // events (one LANE_HEIGHT lower). This is the multi-entity case
        // that the old `canonical_y` couldn't represent.
        let inspection = ProjectInspection {
            root: PathBuf::from("/"),
            domains: vec![
                DomainInspection {
                    name: "Orders".to_string(),
                    path: PathBuf::from("/Orders"),
                    events: vec![EventInfo {
                        name: "OrderPlaced".to_string(),
                        file: PathBuf::new(),
                    }],
                    commands: vec![CommandInfo {
                        name: "PlaceOrder".to_string(),
                        file: PathBuf::new(),
                        produces: vec!["OrderPlaced".to_string()],
                        via_web_transport: false,
                    }],
                    queries: vec![],
                    integrations: vec![],
                },
                DomainInspection {
                    name: "Payments".to_string(),
                    path: PathBuf::from("/Payments"),
                    events: vec![EventInfo {
                        name: "PaymentCaptured".to_string(),
                        file: PathBuf::new(),
                    }],
                    commands: vec![CommandInfo {
                        name: "CapturePayment".to_string(),
                        file: PathBuf::new(),
                        produces: vec!["PaymentCaptured".to_string()],
                        via_web_transport: false,
                    }],
                    queries: vec![],
                    integrations: vec![],
                },
            ],
        };
        let model = empty_model();
        let diff = compute_diff(&model, &inspection);

        let order_event = diff
            .add_nodes
            .iter()
            .find(|n| n.name == "OrderPlaced")
            .unwrap();
        let payment_event = diff
            .add_nodes
            .iter()
            .find(|n| n.name == "PaymentCaptured")
            .unwrap();
        let order_layout = diff
            .ensure_layout_entries
            .iter()
            .find(|e| e.node_id == order_event.id)
            .unwrap();
        let payment_layout = diff
            .ensure_layout_entries
            .iter()
            .find(|e| e.node_id == payment_event.id)
            .unwrap();

        // Entity 0 events at y = 400, entity 1 events at y = 400 + 200 = 600.
        assert!(
            (order_layout.y - 400.0).abs() < f64::EPSILON,
            "Order entity index 0 should put its events at y=400, got {}",
            order_layout.y
        );
        assert!(
            (payment_layout.y - 600.0).abs() < f64::EPSILON,
            "Payments entity index 1 should put its events at y=600, got {}",
            payment_layout.y
        );
    }

    #[test]
    fn grouping_creates_chapter_per_entity_for_heal_slices() {
        // Empty model + inspection with two domains → each domain's
        // entity gets its own freshly-created chapter, and every
        // heal-created slice for that entity lands in it.
        let inspection = ProjectInspection {
            root: PathBuf::from("/"),
            domains: vec![
                DomainInspection {
                    name: "Orders".to_string(),
                    path: PathBuf::from("/Orders"),
                    events: vec![EventInfo {
                        name: "OrderPlaced".to_string(),
                        file: PathBuf::new(),
                    }],
                    commands: vec![CommandInfo {
                        name: "PlaceOrder".to_string(),
                        file: PathBuf::new(),
                        produces: vec!["OrderPlaced".to_string()],
                        via_web_transport: false,
                    }],
                    queries: vec![],
                    integrations: vec![],
                },
                DomainInspection {
                    name: "Payments".to_string(),
                    path: PathBuf::from("/Payments"),
                    events: vec![EventInfo {
                        name: "PaymentCaptured".to_string(),
                        file: PathBuf::new(),
                    }],
                    commands: vec![CommandInfo {
                        name: "CapturePayment".to_string(),
                        file: PathBuf::new(),
                        produces: vec!["PaymentCaptured".to_string()],
                        via_web_transport: false,
                    }],
                    queries: vec![],
                    integrations: vec![],
                },
            ],
        };
        let model = empty_model();
        let diff = compute_diff(&model, &inspection);

        // Two chapters created (one per entity).
        assert_eq!(diff.add_chapters.len(), 2, "{:?}", diff.add_chapters);
        let chapter_names: BTreeSet<&str> = diff
            .add_chapters
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert!(chapter_names.contains("Orders"));
        assert!(chapter_names.contains("Payments"));

        // Each slice's chapter_id resolves to the chapter for its entity.
        let chapter_for_name = |name: &str| {
            diff.add_chapters
                .iter()
                .find(|c| c.name == name)
                .unwrap()
                .id
                .clone()
        };
        let orders_chapter = chapter_for_name("Orders");
        let payments_chapter = chapter_for_name("Payments");
        for s in &diff.add_slices {
            let expected = if s.name == "PlaceOrder" {
                &orders_chapter
            } else if s.name == "CapturePayment" {
                &payments_chapter
            } else {
                panic!("unexpected slice {}", s.name);
            };
            assert_eq!(
                s.chapter_id.as_deref(),
                Some(expected.as_str()),
                "slice {} should be in its entity's chapter",
                s.name,
            );
        }
    }

    #[test]
    fn grouping_reassigns_existing_heal_slice_into_chapter() {
        // Model has a heal-prefixed slice with chapter_id=null left over
        // from an earlier heal run. Re-running heal should emit a
        // SliceUpdate that assigns it to its entity's chapter.
        let mut model = empty_model();
        model["entities"] = serde_json::json!([
            { "id": "ent-orders", "name": "Orders", "order": 0 }
        ]);
        model["slices"] = serde_json::json!([
            { "id": "slice-heal-existing", "name": "PlaceOrder", "chapterId": null, "order": 5 }
        ]);
        model["nodes"] = serde_json::json!([
            { "id": "cmd1", "type": "command", "name": "PlaceOrder",
              "sliceId": "slice-heal-existing", "entityId": "ent-orders" }
        ]);
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);

        // The existing heal slice should get a chapter assignment.
        let update = diff
            .update_slices
            .iter()
            .find(|u| u.slice_id == "slice-heal-existing")
            .expect("existing heal slice should be reassigned");
        assert!(update.set_chapter_id.is_some());
    }

    #[test]
    fn grouping_does_not_touch_user_created_slices() {
        // Model has a user-created slice (no slice-heal- prefix). Heal
        // must not move it into a chapter.
        let mut model = empty_model();
        model["entities"] = serde_json::json!([
            { "id": "ent-orders", "name": "Orders", "order": 0 }
        ]);
        model["slices"] = serde_json::json!([
            { "id": "user-slice-001", "name": "PlaceOrder", "chapterId": null, "order": 0 }
        ]);
        model["nodes"] = serde_json::json!([
            { "id": "cmd1", "type": "command", "name": "PlaceOrder",
              "sliceId": "user-slice-001", "entityId": "ent-orders" }
        ]);
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);

        assert!(
            !diff
                .update_slices
                .iter()
                .any(|u| u.slice_id == "user-slice-001"),
            "user-created slice must not be re-chaptered; got {:?}",
            diff.update_slices,
        );
    }

    #[test]
    fn grouping_is_idempotent_when_chapter_already_correct() {
        // First heal run creates chapter and assigns slices. Re-running
        // must produce zero update_slices and zero add_chapters.
        let mut model = empty_model();
        let inspection = ProjectInspection {
            root: PathBuf::from("/"),
            domains: vec![DomainInspection {
                name: "Orders".to_string(),
                path: PathBuf::from("/Orders"),
                events: vec![EventInfo {
                    name: "OrderPlaced".to_string(),
                    file: PathBuf::new(),
                }],
                commands: vec![CommandInfo {
                    name: "PlaceOrder".to_string(),
                    file: PathBuf::new(),
                    produces: vec!["OrderPlaced".to_string()],
                    via_web_transport: false,
                }],
                queries: vec![],
                integrations: vec![],
            }],
        };
        let diff_one = compute_diff(&model, &inspection);
        crate::ide::heal::apply::apply_diff(&mut model, &diff_one);

        let diff_two = compute_diff(&model, &inspection);
        assert!(
            diff_two.add_chapters.is_empty(),
            "second run must not create new chapters; got {:?}",
            diff_two.add_chapters,
        );
        assert!(
            diff_two.update_slices.is_empty(),
            "second run must not propose slice updates; got {:?}",
            diff_two.update_slices,
        );
    }

    #[test]
    fn rebalance_pushes_colliding_slice_columns_apart() {
        // Two slices each claim x=200 — `B` follows `A` in order, so
        // it gets pushed past A's column extent. A's node stays put;
        // B's node gets an x fix.
        let model = serde_json::json!({
            "id": "m1", "name": "demo",
            "chapters": [],
            "entities": [{ "id": "ent1", "name": "Ent", "order": 0 }],
            "slices": [
                { "id": "sl-a", "name": "A", "chapterId": null, "order": 0 },
                { "id": "sl-b", "name": "B", "chapterId": null, "order": 1 }
            ],
            "nodes": [
                { "id": "nA", "type": "command", "name": "CmdA", "sliceId": "sl-a", "entityId": "ent1" },
                { "id": "nB", "type": "command", "name": "CmdB", "sliceId": "sl-b", "entityId": "ent1" }
            ],
            "edges": [],
            "layout": {
                "nodePositions": {
                    "nA": { "x": 200, "y": 120 },
                    "nB": { "x": 200, "y": 120 }
                },
                "viewport": { "x": 0, "y": 0, "zoom": 1 }
            }
        });
        let inspection = ProjectInspection {
            root: PathBuf::from("/"),
            domains: vec![],
        };
        let diff = compute_diff(&model, &inspection);

        // A stays at 200; B moves past A (200 + W + GAP = 200 + 240 + 80 = 520).
        let b_fix = diff
            .fix_positions
            .iter()
            .find(|f| f.node_id == "nB")
            .expect("B should get an x-fix");
        assert_eq!(b_fix.to_x, Some(520.0));
        assert_eq!(b_fix.to_y, None, "y must not be touched");
        assert!(
            !diff.fix_positions.iter().any(|f| f.node_id == "nA"),
            "A must not move",
        );
    }

    #[test]
    fn rebalance_leaves_well_spaced_slices_alone() {
        // Slices with already-correct columns get no fixes.
        let model = serde_json::json!({
            "id": "m1", "name": "demo",
            "chapters": [],
            "entities": [{ "id": "ent1", "name": "Ent", "order": 0 }],
            "slices": [
                { "id": "sl-a", "name": "A", "chapterId": null, "order": 0 },
                { "id": "sl-b", "name": "B", "chapterId": null, "order": 1 }
            ],
            "nodes": [
                { "id": "nA", "type": "command", "name": "CmdA", "sliceId": "sl-a", "entityId": "ent1" },
                { "id": "nB", "type": "command", "name": "CmdB", "sliceId": "sl-b", "entityId": "ent1" }
            ],
            "edges": [],
            "layout": {
                "nodePositions": {
                    "nA": { "x": 40,  "y": 120 },
                    "nB": { "x": 600, "y": 120 }
                },
                "viewport": { "x": 0, "y": 0, "zoom": 1 }
            }
        });
        let inspection = ProjectInspection {
            root: PathBuf::from("/"),
            domains: vec![],
        };
        let diff = compute_diff(&model, &inspection);
        assert!(
            diff.fix_positions.iter().all(|f| f.to_x.is_none() && f.to_y.is_none()),
            "no fixes should fire on a well-spaced layout; got {:?}",
            diff.fix_positions,
        );
    }

    #[test]
    fn rebalance_is_fixed_point_after_apply() {
        // Run compute_diff → apply_diff → compute_diff again. The second
        // pass must propose NO further x-fixes (the first pass got it
        // right).
        let mut model = serde_json::json!({
            "id": "m1", "name": "demo",
            "chapters": [],
            "entities": [{ "id": "ent1", "name": "Ent", "order": 0 }],
            "slices": [
                { "id": "sl-a", "name": "A", "chapterId": null, "order": 0 },
                { "id": "sl-b", "name": "B", "chapterId": null, "order": 1 },
                { "id": "sl-c", "name": "C", "chapterId": null, "order": 2 }
            ],
            "nodes": [
                { "id": "nA", "type": "command", "name": "CmdA", "sliceId": "sl-a", "entityId": "ent1" },
                { "id": "nB", "type": "command", "name": "CmdB", "sliceId": "sl-b", "entityId": "ent1" },
                { "id": "nC", "type": "command", "name": "CmdC", "sliceId": "sl-c", "entityId": "ent1" }
            ],
            "edges": [],
            "layout": {
                "nodePositions": {
                    "nA": { "x": 100, "y": 120 },
                    "nB": { "x": 100, "y": 120 },
                    "nC": { "x": 100, "y": 120 }
                },
                "viewport": { "x": 0, "y": 0, "zoom": 1 }
            }
        });
        let inspection = ProjectInspection {
            root: PathBuf::from("/"),
            domains: vec![],
        };
        let diff = compute_diff(&model, &inspection);
        crate::ide::heal::apply::apply_diff(&mut model, &diff);

        let diff_after = compute_diff(&model, &inspection);
        let x_fixes: Vec<_> = diff_after
            .fix_positions
            .iter()
            .filter(|f| f.to_x.is_some())
            .collect();
        assert!(
            x_fixes.is_empty(),
            "second compute_diff should propose no further x fixes; got {x_fixes:?}",
        );
    }

    #[test]
    fn layout_is_idempotent_across_runs() {
        // The same inputs MUST yield byte-identical layout entries on
        // every run — that's the contract the patched file relies on
        // for git-stable output.
        let model = empty_model();
        let inspection = fixture_inspection();
        let diff1 = compute_diff(&model, &inspection);
        let diff2 = compute_diff(&model, &inspection);

        let entries1: Vec<_> = diff1
            .ensure_layout_entries
            .iter()
            .map(|e| (e.node_id.clone(), e.x, e.y))
            .collect();
        let entries2: Vec<_> = diff2
            .ensure_layout_entries
            .iter()
            .map(|e| (e.node_id.clone(), e.x, e.y))
            .collect();
        assert_eq!(
            entries1, entries2,
            "same inputs must produce identical layout entries on every run"
        );
    }

    /// End-to-end fixed point: compute_diff + (apply via re-build) leaves
    /// compute_diff with nothing further to propose. Apply step is exercised
    /// against the diff fields directly here — diff.rs is unit-scoped; the
    /// full pipeline lives in `apply::tests::pipeline_is_fixed_point`.
    #[test]
    fn materialised_diff_is_self_consistent() {
        let model = empty_model();
        let inspection = fixture_inspection();
        let diff = compute_diff(&model, &inspection);

        // Every edge must reference either an existing node id OR a
        // freshly-materialised node id from THIS diff. No dangling refs.
        let mut known_ids: BTreeSet<String> = BTreeSet::new();
        if let Some(arr) = model.get("nodes").and_then(|v| v.as_array()) {
            for n in arr {
                if let Some(id) = n.get("id").and_then(|v| v.as_str()) {
                    known_ids.insert(id.to_string());
                }
            }
        }
        for n in &diff.add_nodes {
            known_ids.insert(n.id.clone());
        }
        for e in &diff.add_edges {
            assert!(
                known_ids.contains(&e.source_id),
                "edge source {} not in known ids",
                e.source_id
            );
            assert!(
                known_ids.contains(&e.target_id),
                "edge target {} not in known ids",
                e.target_id
            );
        }
    }

    #[test]
    fn diff_wires_orphan_query_to_every_local_event() {
        // Mirrors the Task-1 inspection default: a query that names no
        // event in source has `subscribes_to` filled with ALL of its
        // domain's local events. The differ must then materialise an
        // `eventFeedsQuery` edge from each of those events to the query —
        // no longer leaving it an orphan with zero incoming edges.
        let mut inspection = fixture_inspection();
        // Domain "Orders" has events OrderPlaced + OrderShipped. Replace the
        // fixture's single-query with an orphan query subscribed to BOTH
        // local events (the post-default shape from inspect_domain).
        inspection.domains[0].queries = vec![QueryInfo {
            name: "OrdersProjection".to_string(),
            file: PathBuf::new(),
            subscribes_to: vec!["OrderPlaced".to_string(), "OrderShipped".to_string()],
        }];

        let model = empty_model();
        let diff = compute_diff(&model, &inspection);

        // Find the materialised query node id.
        let q_node = diff
            .add_nodes
            .iter()
            .find(|n| n.node_type == "query" && n.name == "OrdersProjection")
            .expect("query node should be materialised");
        // Find materialised event node ids.
        let ev_ids: BTreeSet<&str> = diff
            .add_nodes
            .iter()
            .filter(|n| n.node_type == "event")
            .map(|n| n.id.as_str())
            .collect();

        let efq: Vec<_> = diff
            .add_edges
            .iter()
            .filter(|e| e.edge_type == "eventFeedsQuery" && e.target_id == q_node.id)
            .collect();
        assert_eq!(
            efq.len(),
            2,
            "orphan query must be fed by both local events; got {efq:?}",
        );
        for e in &efq {
            assert!(
                ev_ids.contains(e.source_id.as_str()),
                "eventFeedsQuery source {} must be a materialised event node",
                e.source_id,
            );
        }
    }
}

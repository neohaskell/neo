//! Cheap, dumb-parser introspection of a NeoHaskell project layout.
//!
//! The conventions encoded here mirror what the heal-event-model prompt
//! used to ask the agent to grep for at runtime — but doing the grepping
//! in Rust here means:
//!
//!   * the heal prompt can ship with the answers PRE-COMPUTED, so the
//!     agent stops burning tool calls on file discovery and we can demote
//!     from opus to sonnet;
//!   * humans can run `neo inspect` to see the same view themselves;
//!   * the parser is grep-level brittle on purpose — a NeoHaskell project
//!     that follows the testbed convention parses cleanly; one that
//!     doesn't is signalling a deeper problem.
//!
//! What we extract from each domain (`src/<App>/<Domain>/`):
//!
//!   * Commands (`Commands/<Name>.hs`) — their name, the event constructors
//!     they produce (found by scanning the `decide` body), whether they're
//!     reachable from a `WebTransport` (i.e. behind an HTTP route).
//!   * Events — constructor names from `<Domain>Event` data declaration in
//!     `Core.hs` or `Event.hs`.
//!   * Queries (`Queries/<Name>.hs`) — name + the event constructors that
//!     appear in the file (best-guess subscriber set).
//!   * Integrations (`Integrations/<Name>.hs`) — name, kind (outbound vs
//!     reactive), events handled in `handleEvent`, downstream commands
//!     emitted via `Command.Emit`.
//!
//! Output is `serde_json`-serialisable so the heal prompt can splice it
//! in directly without re-formatting.

pub mod parse;

use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

/// Top-level inspection result for a NeoHaskell project root.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInspection {
    pub root: PathBuf,
    pub domains: Vec<DomainInspection>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainInspection {
    pub name: String,
    pub path: PathBuf,
    pub events: Vec<EventInfo>,
    pub commands: Vec<CommandInfo>,
    pub queries: Vec<QueryInfo>,
    pub integrations: Vec<IntegrationInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventInfo {
    /// Constructor name (e.g. `OrderPlaced`).
    pub name: String,
    /// File where the constructor was found — usually `Core.hs` or `Event.hs`.
    pub file: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandInfo {
    pub name: String,
    pub file: PathBuf,
    /// Event constructor names that appear in the `decide` body.
    /// Cross-referenced against the domain's known event set, so noise
    /// like `Decider`, `Maybe`, etc. is filtered out.
    pub produces: Vec<String>,
    /// `true` if the command file has a `TransportsOf <Cmd> = '[WebTransport ...]`
    /// declaration — i.e. it can be invoked over HTTP.
    pub via_web_transport: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryInfo {
    pub name: String,
    pub file: PathBuf,
    /// Event constructors referenced in the file body — best-guess subscriber list.
    pub subscribes_to: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationInfo {
    pub name: String,
    pub file: PathBuf,
    pub kind: IntegrationKind,
    /// Event constructors matched in the `handleEvent` case arms.
    pub handles_events: Vec<String>,
    /// Command names emitted via `Command.Emit { command = X { ... } }`.
    /// Empty for plain outbound integrations.
    pub emits_commands: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationKind {
    /// Pure side-effecting integration (HTTP call to an external system, etc.).
    /// `emits_commands` is empty.
    Outbound,
    /// Bridges domains: listens to an event in this domain and raises a
    /// command in another. `emits_commands` is non-empty.
    Reactive,
}

/// Discover and parse every domain under `<root>/src/`. Returns a
/// `ProjectInspection` even when nothing is found (empty domains) so
/// callers can render a stable shape.
pub fn inspect_project(root: &Path) -> ProjectInspection {
    let src = root.join("src");
    let domains = if src.is_dir() {
        discover_domains(&src)
            .into_iter()
            .map(|d| inspect_domain(&d))
            .collect()
    } else {
        Vec::new()
    };
    ProjectInspection {
        root: root.to_path_buf(),
        domains,
    }
}

/// A "domain" is any directory that contains `Core.hs` AND at least one of
/// `Commands/`, `Events/`, `Queries/`, `Integrations/`. We walk `src/`
/// depth-first looking for such directories. Nested domains (e.g.
/// `Datalake/Payment/`, `Datalake/Proposal/`) are returned as siblings.
fn discover_domains(src: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in WalkDir::new(src).max_depth(6).into_iter().flatten() {
        if !entry.file_type().is_dir() {
            continue;
        }
        let p = entry.path();
        if !p.join("Core.hs").is_file() && !p.join("Event.hs").is_file() {
            continue;
        }
        let has_subdir = ["Commands", "Events", "Queries", "Integrations"]
            .iter()
            .any(|s| p.join(s).is_dir());
        if has_subdir {
            out.push(p.to_path_buf());
        }
    }
    out.sort();
    out
}

fn inspect_domain(dir: &Path) -> DomainInspection {
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?")
        .to_string();

    // Events first — the parser uses the resulting set to cross-reference
    // command-produces and integration-handles.
    let events = parse::events_in_domain(dir);
    let event_names: Vec<String> = events.iter().map(|e| e.name.clone()).collect();

    let mut commands = parse::commands_in_domain(dir, &event_names);
    let mut queries = parse::queries_in_domain(dir, &event_names);
    let mut integrations = parse::integrations_in_domain(dir, &event_names);

    commands.sort_by(|a, b| a.name.cmp(&b.name));
    queries.sort_by(|a, b| a.name.cmp(&b.name));
    integrations.sort_by(|a, b| a.name.cmp(&b.name));

    DomainInspection {
        name,
        path: dir.to_path_buf(),
        events,
        commands,
        queries,
        integrations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, body: &str) {
        let full = root.join(rel);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, body).unwrap();
    }

    #[test]
    fn inspect_project_returns_empty_when_no_src() {
        let dir = tempfile::tempdir().unwrap();
        let out = inspect_project(dir.path());
        assert!(out.domains.is_empty());
    }

    #[test]
    fn inspect_discovers_a_domain_with_core_hs_and_commands_dir() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/App/Cart/Core.hs", CORE_HS);
        write(dir.path(), "src/App/Cart/Commands/AddItem.hs", CMD_ADD_ITEM);
        let out = inspect_project(dir.path());
        assert_eq!(out.domains.len(), 1, "expected one domain, got {out:?}");
        let cart = &out.domains[0];
        assert_eq!(cart.name, "Cart");
        assert_eq!(cart.events.iter().map(|e| &e.name).collect::<Vec<_>>(), ["CartCreated", "ItemAdded"]);
        assert_eq!(cart.commands.len(), 1);
        assert_eq!(cart.commands[0].name, "AddItem");
        assert_eq!(cart.commands[0].produces, vec!["ItemAdded".to_string()]);
    }

    #[test]
    fn inspect_classifies_reactive_integration_when_emits_command() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/App/Cart/Core.hs", CORE_HS);
        write(
            dir.path(),
            "src/App/Cart/Integrations/ReserveStock.hs",
            INTEGRATION_REACTIVE,
        );
        let out = inspect_project(dir.path());
        let cart = &out.domains[0];
        assert_eq!(cart.integrations.len(), 1);
        let intg = &cart.integrations[0];
        assert_eq!(intg.name, "ReserveStock");
        assert_eq!(intg.kind, IntegrationKind::Reactive);
        assert_eq!(intg.handles_events, vec!["ItemAdded".to_string()]);
        assert_eq!(intg.emits_commands, vec!["ReserveStockOnAdded".to_string()]);
    }

    #[test]
    fn inspect_classifies_outbound_integration_when_no_command_emit() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/App/Cart/Core.hs", CORE_HS);
        write(
            dir.path(),
            "src/App/Cart/Integrations/EmailCart.hs",
            INTEGRATION_OUTBOUND,
        );
        let out = inspect_project(dir.path());
        let intg = &out.domains[0].integrations[0];
        assert_eq!(intg.kind, IntegrationKind::Outbound);
        assert!(intg.emits_commands.is_empty());
        assert_eq!(intg.handles_events, vec!["ItemAdded".to_string()]);
    }

    const CORE_HS: &str = r#"
module App.Cart.Core where
data CartEvent
  = CartCreated { entityId :: Uuid, ownerId :: Text }
  | ItemAdded { entityId :: Uuid, stockId :: Uuid, quantity :: Int }
  deriving (Generic)
"#;

    const CMD_ADD_ITEM: &str = r#"
module App.Cart.Commands.AddItem where
data AddItem = AddItem { cartId :: Uuid, stockId :: Uuid, quantity :: Int }
decide :: AddItem -> Maybe CartEntity -> RequestContext -> Decision CartEvent
decide cmd entity _ctx = case entity of
  Nothing -> Decider.reject "Cart not found!"
  Just cart -> Decider.acceptExisting
    [ ItemAdded
        { entityId = cart.cartId
        , stockId = cmd.stockId
        , quantity = cmd.quantity
        }
    ]
type instance TransportsOf AddItem = '[WebTransport]
command ''AddItem
"#;

    const INTEGRATION_REACTIVE: &str = r#"
module App.Cart.Integrations.ReserveStock where
handleEvent :: CartEntity -> CartEvent -> Integration.Outbound
handleEvent cart event = case event of
  ItemAdded { stockId, quantity } ->
    Integration.batch
      [ Integration.outbound
          Command.Emit
            { command = ReserveStockOnAdded { stockId = stockId } }
      ]
  _ -> Integration.none
outboundIntegration ''ReserveStock
"#;

    const INTEGRATION_OUTBOUND: &str = r#"
module App.Cart.Integrations.EmailCart where
handleEvent :: CartEntity -> CartEvent -> Integration.Outbound
handleEvent _cart event = case event of
  ItemAdded { stockId } -> Integration.send (postJson "/email" stockId)
  _ -> Integration.none
"#;
}

//! The `neo ide` API surface: a JSON-RPC 2.0 server mounted on the same
//! axum router that serves the bundled Vite IDE.
//!
//! Design panel: see `/Users/nick/.claude/plans/since-i-got-a-cryptic-planet.md`
//! for the rationale (LSP/Tauri/Replit convergence on JSON-RPC over one WS,
//! typed handlers, transport seam for cloud-eventual).
//!
//! Public surface is `AppState` (the axum router state) and the `methods` /
//! `server` entry points wired in by `commands/ide.rs`.

pub mod heal;
pub mod methods;
pub mod registry;
pub mod rpc;
pub mod server;
pub mod session;
pub mod transport;
pub mod validate;
pub mod workspace;

use std::sync::Arc;

use crate::ide::registry::MethodRegistry;
use crate::ide::transport::LocalTransport;

/// Axum router state shared across `/ws` upgrades. Each WebSocket connection
/// gets a shared reference to the registry (method handlers) and the transport
/// (which mints the per-connection `Session` and resolves `Workspace`).
#[derive(Clone)]
pub struct AppState {
    pub registry: MethodRegistry,
    pub transport: Arc<LocalTransport>,
}

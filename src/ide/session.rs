//! Per-WebSocket-connection session.
//!
//! Minted at WS upgrade by the `Transport`. Carries the `Workspace` directly
//! (resolved by the transport at accept time) so handlers don't need
//! task-local globals or a separate `Ctx` argument to find the workspace
//! they're operating on.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::ide::workspace::Workspace;


/// Opaque per-session identifier. We deliberately avoid pulling in `uuid` as
/// a dep — a monotonic counter + the boot timestamp gives an id that is
/// unique within a server lifetime (enough for the local case) and grep-able
/// in logs (`session_42@1747838291`). The cloud transport can substitute a
/// real UUID/JWT-derived id without changing the wire shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn mint() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self(format!("session_{n}@{secs}"))
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub workspace: Arc<Workspace>,
}

impl Session {
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            id: SessionId::mint(),
            workspace,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_workspace() -> Arc<Workspace> {
        let dir = tempfile::tempdir().unwrap();
        Arc::new(Workspace::from_root(dir.path()).unwrap())
    }

    #[test]
    fn session_id_is_uniquely_minted() {
        let a = SessionId::mint();
        let b = SessionId::mint();
        assert_ne!(a, b, "consecutive mints must differ");
    }

    #[test]
    fn session_id_displays_grep_able() {
        let id = SessionId::mint();
        let s = id.to_string();
        assert!(s.starts_with("session_"), "id should start with `session_`: {s}");
        assert!(s.contains('@'), "id should embed timestamp: {s}");
    }

    #[test]
    fn session_attaches_workspace() {
        let ws = fixture_workspace();
        let ws_id = ws.id.clone();
        let s = Session::new(ws);
        assert_eq!(s.workspace.id, ws_id);
    }
}

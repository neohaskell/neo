//! `workspace/readEventModel` — read `<workspace_root>/event-model.json`.
//!
//! File-not-found is success-with-null, not an error, so the frontend can
//! treat "no file yet" as a fresh project without branching on the JSON-RPC
//! error code. Any other IO failure (permissions, disk error) surfaces as
//! `NeoError::IoErrorAt` with the full path + operation context.

use serde::{Deserialize, Serialize};

use crate::errors::NeoError;
use crate::ide::session::Session;

pub const EVENT_MODEL_FILENAME: &str = "event-model.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadEventModelParams {
    // No params in v1. Open shape for future filters / variants.
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadEventModelResult {
    /// Raw file contents (JSON string). `None` when the file does not exist.
    pub content: Option<String>,
}

pub async fn handle(
    session: Session,
    _params: ReadEventModelParams,
) -> Result<ReadEventModelResult, NeoError> {
    let path = session.workspace.root.join(EVENT_MODEL_FILENAME);
    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(ReadEventModelResult { content: Some(content) }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(ReadEventModelResult { content: None })
        }
        Err(e) => Err(NeoError::io_at("reading `event-model.json`", path, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ide::workspace::Workspace;
    use std::sync::Arc;

    fn fixture_session(dir: &std::path::Path) -> Session {
        let ws = Workspace::from_root(dir).unwrap();
        Session::new(Arc::new(ws))
    }

    #[tokio::test]
    async fn read_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let session = fixture_session(dir.path());
        let result = handle(session, ReadEventModelParams {}).await.unwrap();
        assert!(result.content.is_none(), "missing file → None content");
    }

    #[tokio::test]
    async fn read_returns_content_when_file_present() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"{"name":"demo","slices":[]}"#;
        std::fs::write(dir.path().join("event-model.json"), content).unwrap();
        let session = fixture_session(dir.path());
        let result = handle(session, ReadEventModelParams {}).await.unwrap();
        assert_eq!(result.content.as_deref(), Some(content));
    }

    #[tokio::test]
    async fn read_preserves_byte_for_byte() {
        // Whitespace, indentation, unicode all round-trip — the Rust handler
        // is a pass-through, not a re-serialiser.
        let dir = tempfile::tempdir().unwrap();
        let content = "{\n  \"name\": \"日本語\",\n  \"slices\": []\n}\n";
        std::fs::write(dir.path().join("event-model.json"), content).unwrap();
        let session = fixture_session(dir.path());
        let result = handle(session, ReadEventModelParams {}).await.unwrap();
        assert_eq!(result.content.as_deref(), Some(content));
    }

    #[tokio::test]
    async fn read_serialized_uses_camel_case() {
        let dir = tempfile::tempdir().unwrap();
        let session = fixture_session(dir.path());
        let result = handle(session, ReadEventModelParams {}).await.unwrap();
        let s = serde_json::to_string(&result).unwrap();
        // `content` is already camel-case-compatible (single word). Verify the
        // null case serialises as JSON null, not omitted.
        assert!(s.contains("\"content\":null"), "missing content:null: {s}");
    }
}

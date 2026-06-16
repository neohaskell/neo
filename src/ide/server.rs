//! The axum WebSocket upgrade handler and the per-connection JSON-RPC loop.
//!
//! Lifecycle per connection:
//! 1. axum extracts `WebSocketUpgrade` + `State<AppState>`.
//! 2. We call `transport.accept()` to mint a `Session` (which already carries
//!    the workspace it's bound to).
//! 3. We hand off to the on-upgrade callback which runs the read/dispatch
//!    loop until the client closes or the socket dies.
//!
//! Per the panel's "cancellation reserved, not implemented" stance, the
//! per-connection state is intentionally minimal: just the registry and the
//! session. Cancellable methods land later and will add a CancellationToken
//! map keyed by request id.

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};

use crate::ide::rpc::{parse_incoming, Incoming, Response};
use crate::ide::transport::Transport;
use crate::ide::AppState;

pub async fn ws_upgrade(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        // The transport mints a session — accept failure (cloud auth, etc.)
        // closes the socket before any frame is exchanged. In local mode
        // accept never fails.
        let session = match state.transport.accept() {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(?err.code, %err.message, "transport rejected WS upgrade");
                return;
            }
        };
        tracing::debug!(session_id = %session.id, workspace_id = %session.workspace.id, "ws connection accepted");
        connection_loop(socket, session, state).await;
    })
}

async fn connection_loop(socket: WebSocket, session: crate::ide::session::Session, state: AppState) {
    let (mut sink, mut stream) = socket.split();

    while let Some(msg) = stream.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!(error = %e, session_id = %session.id, "ws read error, closing");
                break;
            }
        };

        let text = match msg {
            Message::Text(t) => t,
            Message::Binary(_) => {
                // We don't speak binary; ignore silently — the spec says the
                // server MAY accept binary frames, but it's safer to drop.
                continue;
            }
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Close(_) => {
                tracing::debug!(session_id = %session.id, "client closed ws");
                break;
            }
        };

        let response = handle_text_frame(&text, &session, &state).await;
        // None == no response (notification). Skip the send.
        let Some(response) = response else { continue };

        let payload = match serde_json::to_string(&response) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "failed to serialise response — closing connection");
                break;
            }
        };
        if let Err(e) = sink.send(Message::Text(payload)).await {
            tracing::debug!(error = %e, session_id = %session.id, "ws send failed, closing");
            break;
        }
    }
}

/// Parse + dispatch one text frame. Returns `Some(Response)` for requests,
/// `None` for notifications (which per spec must not be responded to). Parse
/// errors and Invalid Request errors yield `Some(Response { id: None, .. })`
/// per JSON-RPC 2.0 spec.
async fn handle_text_frame(
    text: &str,
    session: &crate::ide::session::Session,
    state: &AppState,
) -> Option<Response> {
    match parse_incoming(text) {
        Ok(Incoming::Request(req)) => {
            let id = req.id;
            let result = state.registry.dispatch(&req.method, session, req.params).await;
            Some(match result {
                Ok(value) => Response::success(id, value),
                Err(err) => Response::failure(id, err),
            })
        }
        Ok(Incoming::Notification(notif)) => {
            // No response — but log so future cancellation / progress
            // notifications are debuggable when they land.
            tracing::debug!(method = %notif.method, "notification received (no response, no handler in v1)");
            None
        }
        Err(rpc_err) => Some(Response::failure(None, rpc_err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ide::methods::register_all;
    use crate::ide::registry::MethodRegistry;
    use crate::ide::session::Session;
    use crate::ide::transport::LocalTransport;
    use crate::ide::workspace::Workspace;
    use std::sync::Arc;

    fn fixture_state(dir: &std::path::Path) -> (AppState, Session) {
        let ws = Workspace::from_root(dir).unwrap();
        let transport = LocalTransport::new(ws);
        let session = transport.accept().unwrap();
        let registry = register_all(MethodRegistry::new());
        let state = AppState { registry, transport: Arc::new(transport) };
        (state, session)
    }

    #[tokio::test]
    async fn handle_text_frame_initialize_returns_success_response() {
        let dir = tempfile::tempdir().unwrap();
        let (state, session) = fixture_state(dir.path());
        let frame = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"t","version":"0"}}}"#;
        let resp = handle_text_frame(frame, &session, &state).await.expect("request gets a response");
        assert!(resp.error.is_none(), "should succeed: {resp:?}");
        let result = resp.result.expect("result present");
        assert_eq!(result["serverInfo"]["name"], "neo");
    }

    #[tokio::test]
    async fn handle_text_frame_unknown_method_returns_method_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let (state, session) = fixture_state(dir.path());
        let frame = r#"{"jsonrpc":"2.0","id":7,"method":"does/not/exist"}"#;
        let resp = handle_text_frame(frame, &session, &state).await.expect("request gets a response");
        let err = resp.error.expect("error present");
        assert_eq!(err.code, crate::ide::rpc::error_codes::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn handle_text_frame_invalid_json_returns_parse_error_with_null_id() {
        let dir = tempfile::tempdir().unwrap();
        let (state, session) = fixture_state(dir.path());
        let resp = handle_text_frame("{garbage", &session, &state).await.expect("response on parse error");
        assert!(resp.id.is_none(), "parse-error response id must be null: {resp:?}");
        assert_eq!(resp.error.unwrap().code, crate::ide::rpc::error_codes::PARSE_ERROR);
    }

    #[tokio::test]
    async fn handle_text_frame_notification_yields_no_response() {
        let dir = tempfile::tempdir().unwrap();
        let (state, session) = fixture_state(dir.path());
        let frame = r#"{"jsonrpc":"2.0","method":"$/someEventClientSentUs"}"#;
        let resp = handle_text_frame(frame, &session, &state).await;
        assert!(resp.is_none(), "notifications do not get responses: {resp:?}");
    }
}

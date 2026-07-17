use serde::{Deserialize, Serialize};

use crate::errors::NeoError;
use crate::ide::collab::runtime::CollabRole;
use crate::ide::session::Session;

#[derive(Debug, Deserialize)]
pub struct CollabStatusParams {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabStatusResult {
    pub enabled: bool,
    pub role: Option<CollabRole>,
    pub actor_id: Option<String>,
    pub peer_count: usize,
}

pub async fn handle(
    session: Session,
    _params: CollabStatusParams,
) -> Result<CollabStatusResult, NeoError> {
    let Some(runtime) = session.collab.clone() else {
        return Ok(CollabStatusResult {
            enabled: false,
            role: None,
            actor_id: None,
            peer_count: 0,
        });
    };
    runtime
        .request_snapshot_repair()
        .await
        .map_err(|error| NeoError::IdeCollaboration {
            reason: error.to_string(),
        })?;
    for notification in runtime.bootstrap_notifications().await {
        if let Ok(params) = serde_json::to_value(notification) {
            session.notify("$/collab", params);
        }
    }
    Ok(CollabStatusResult {
        enabled: true,
        role: Some(runtime.role()),
        actor_id: Some(runtime.actor_id().to_owned()),
        peer_count: runtime.peer_count(),
    })
}

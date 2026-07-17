use serde::{Deserialize, Serialize};

use crate::errors::NeoError;
use crate::ide::collab::Presence;
use crate::ide::session::Session;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabPresenceParams {
    pub presence: Presence,
}

#[derive(Debug, Serialize)]
pub struct CollabPresenceResult {
    pub accepted: bool,
}

pub async fn handle(
    session: Session,
    params: CollabPresenceParams,
) -> Result<CollabPresenceResult, NeoError> {
    let runtime = session.collab.ok_or_else(|| NeoError::IdeCollaboration {
        reason: "this IDE was not started with --share or --join".to_owned(),
    })?;
    runtime
        .update_presence(params.presence)
        .await
        .map_err(|error| NeoError::IdeCollaboration {
            reason: error.to_string(),
        })?;
    Ok(CollabPresenceResult { accepted: true })
}

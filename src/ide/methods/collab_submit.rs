use serde::{Deserialize, Serialize};

use crate::errors::NeoError;
use crate::ide::collab::BoardCommand;
use crate::ide::session::Session;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabSubmitParams {
    pub command: BoardCommand,
}

#[derive(Debug, Serialize)]
pub struct CollabSubmitResult {
    pub accepted: bool,
}

pub async fn handle(
    session: Session,
    params: CollabSubmitParams,
) -> Result<CollabSubmitResult, NeoError> {
    let runtime = session.collab.ok_or_else(|| NeoError::IdeCollaboration {
        reason: "this IDE was not started with --share or --join".to_owned(),
    })?;
    runtime
        .submit(params.command)
        .await
        .map_err(|error| NeoError::IdeCollaboration {
            reason: error.to_string(),
        })?;
    Ok(CollabSubmitResult { accepted: true })
}

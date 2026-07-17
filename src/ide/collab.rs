use std::collections::{BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod runtime;

const TICKET_PREFIX: &str = "neoide";

#[derive(Debug, Error)]
pub enum CollabError {
    #[error("invalid collaboration ticket")]
    InvalidTicket,
    #[error("invalid room capability")]
    InvalidCapability,
    #[error("message was not signed by the collaboration host")]
    InvalidAuthority,
    #[error("node `{0}` does not exist")]
    UnknownNode(String),
    #[error("node position must contain finite coordinates")]
    InvalidPosition,
    #[error("event model is missing `{0}`")]
    InvalidModel(&'static str),
    #[error("invalid signed collaboration message")]
    InvalidWireMessage,
    #[error("invalid snapshot chunk")]
    InvalidSnapshotChunk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareTicket {
    pub version: u8,
    pub topic_id: [u8; 32],
    pub capability: [u8; 32],
    pub bootstrap: BTreeSet<String>,
}

impl ShareTicket {
    pub fn new(
        bootstrap_endpoint: impl Into<String>,
        topic_id: [u8; 32],
        capability: [u8; 32],
    ) -> Self {
        Self {
            version: 1,
            topic_id,
            capability,
            bootstrap: [bootstrap_endpoint.into()].into_iter().collect(),
        }
    }

    #[cfg(test)]
    fn new_for_test(bootstrap_endpoint: &str, topic_id: [u8; 32], capability: [u8; 32]) -> Self {
        Self::new(bootstrap_endpoint, topic_id, capability)
    }

    pub fn serialize(&self) -> String {
        let bytes = postcard::to_stdvec(self).expect("share ticket serialization is infallible");
        format!(
            "{TICKET_PREFIX}{}",
            data_encoding::BASE32_NOPAD.encode(&bytes).to_lowercase()
        )
    }

    pub fn deserialize(input: &str) -> Result<Self, CollabError> {
        let encoded = input
            .strip_prefix(TICKET_PREFIX)
            .ok_or(CollabError::InvalidTicket)?;
        let bytes = data_encoding::BASE32_NOPAD
            .decode(encoded.to_ascii_uppercase().as_bytes())
            .map_err(|_| CollabError::InvalidTicket)?;
        let ticket: Self = postcard::from_bytes(&bytes).map_err(|_| CollabError::InvalidTicket)?;
        if ticket.version != 1 || ticket.bootstrap.is_empty() {
            return Err(CollabError::InvalidTicket);
        }
        Ok(ticket)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardCommand {
    pub command_id: String,
    pub actor_id: String,
    pub base_revision: u64,
    pub operation: BoardOperation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BoardOperation {
    MoveNode { node_id: String, x: f64, y: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardEvent {
    pub sequence: u64,
    pub command: BoardCommand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WireMessage {
    Hello {
        capability: [u8; 32],
    },
    Proposal {
        capability: [u8; 32],
        command: BoardCommand,
    },
    Committed {
        capability: [u8; 32],
        event: BoardEvent,
    },
    Rejected {
        capability: [u8; 32],
        recipient: String,
        command_id: String,
        reason: String,
    },
    SnapshotChunk {
        capability: [u8; 32],
        chunk: SnapshotChunk,
    },
    Presence {
        capability: [u8; 32],
        presence: Presence,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub actor_id: String,
    pub display_name: String,
    pub color: String,
    pub active_feature_id: String,
    pub cursor: Option<CursorPosition>,
    pub selected_node_ids: Vec<String>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotChunk {
    pub recipient: String,
    pub transfer_id: String,
    pub sequence: u64,
    pub index: u32,
    pub total: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, PartialEq)]
pub struct CompletedSnapshot {
    pub sequence: u64,
    pub content: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct SignedWire {
    from: iroh::PublicKey,
    data: Vec<u8>,
    signature: iroh::Signature,
}

impl SignedWire {
    fn encode(key: &iroh::SecretKey, message: &WireMessage) -> Result<Vec<u8>, CollabError> {
        let data = serde_json::to_vec(message).map_err(|_| CollabError::InvalidWireMessage)?;
        let signed = Self {
            from: key.public(),
            signature: key.sign(&data),
            data,
        };
        postcard::to_stdvec(&signed).map_err(|_| CollabError::InvalidWireMessage)
    }

    fn decode(bytes: &[u8]) -> Result<(iroh::EndpointId, WireMessage), CollabError> {
        let signed: Self =
            postcard::from_bytes(bytes).map_err(|_| CollabError::InvalidWireMessage)?;
        signed
            .from
            .verify(&signed.data, &signed.signature)
            .map_err(|_| CollabError::InvalidWireMessage)?;
        let message =
            serde_json::from_slice(&signed.data).map_err(|_| CollabError::InvalidWireMessage)?;
        Ok((signed.from, message))
    }
}

pub fn snapshot_chunks(
    recipient: &str,
    sequence: u64,
    content: &[u8],
    max_payload: usize,
) -> Vec<SnapshotChunk> {
    assert!(max_payload > 0, "snapshot payload size must be non-zero");
    let total = content.len().max(1).div_ceil(max_payload) as u32;
    let transfer_id = format!("{recipient}:{sequence}:{}", rand::random::<u64>());
    if content.is_empty() {
        return vec![SnapshotChunk {
            recipient: recipient.to_owned(),
            transfer_id,
            sequence,
            index: 0,
            total: 1,
            payload: Vec::new(),
        }];
    }
    content
        .chunks(max_payload)
        .enumerate()
        .map(|(index, payload)| SnapshotChunk {
            recipient: recipient.to_owned(),
            transfer_id: transfer_id.clone(),
            sequence,
            index: index as u32,
            total,
            payload: payload.to_vec(),
        })
        .collect()
}

#[derive(Default)]
pub struct SnapshotAssembler {
    pending: HashMap<String, PendingSnapshot>,
    completed: HashSet<String>,
}

struct PendingSnapshot {
    sequence: u64,
    parts: Vec<Option<Vec<u8>>>,
}

impl SnapshotAssembler {
    pub fn push(&mut self, chunk: SnapshotChunk) -> Result<Option<CompletedSnapshot>, CollabError> {
        if chunk.total == 0 || chunk.total > 16_384 || chunk.index >= chunk.total {
            return Err(CollabError::InvalidSnapshotChunk);
        }
        if self.completed.contains(&chunk.transfer_id) {
            return Ok(None);
        }
        let pending = self
            .pending
            .entry(chunk.transfer_id.clone())
            .or_insert_with(|| PendingSnapshot {
                sequence: chunk.sequence,
                parts: vec![None; chunk.total as usize],
            });
        if pending.sequence != chunk.sequence || pending.parts.len() != chunk.total as usize {
            return Err(CollabError::InvalidSnapshotChunk);
        }
        pending.parts[chunk.index as usize] = Some(chunk.payload);
        if pending.parts.iter().any(Option::is_none) {
            return Ok(None);
        }
        let pending = self
            .pending
            .remove(&chunk.transfer_id)
            .expect("completed snapshot exists");
        let content = pending.parts.into_iter().flatten().flatten().collect();
        self.completed.insert(chunk.transfer_id);
        Ok(Some(CompletedSnapshot {
            sequence: pending.sequence,
            content,
        }))
    }
}

pub fn authorize(expected: &[u8; 32], presented: &[u8; 32]) -> Result<(), CollabError> {
    let mismatch = expected
        .iter()
        .zip(presented)
        .fold(0u8, |acc, (left, right)| acc | (left ^ right));
    if mismatch == 0 {
        Ok(())
    } else {
        Err(CollabError::InvalidCapability)
    }
}

pub fn apply_command(
    mut model: serde_json::Value,
    command: &BoardCommand,
) -> Result<serde_json::Value, CollabError> {
    match &command.operation {
        BoardOperation::MoveNode { node_id, x, y } => {
            if !x.is_finite() || !y.is_finite() {
                return Err(CollabError::InvalidPosition);
            }
            let nodes = model
                .get("nodes")
                .and_then(serde_json::Value::as_array)
                .ok_or(CollabError::InvalidModel("nodes"))?;
            if !nodes
                .iter()
                .any(|node| node.get("id").and_then(serde_json::Value::as_str) == Some(node_id))
            {
                return Err(CollabError::UnknownNode(node_id.clone()));
            }
            let positions = model
                .get_mut("layout")
                .and_then(|layout| layout.get_mut("nodePositions"))
                .and_then(serde_json::Value::as_object_mut)
                .ok_or(CollabError::InvalidModel("layout.nodePositions"))?;
            positions.insert(node_id.clone(), serde_json::json!({ "x": x, "y": y }));
        }
    }
    Ok(model)
}

pub struct HostSequencer {
    model: serde_json::Value,
    capability: [u8; 32],
    revision: u64,
    committed: HashMap<String, BoardEvent>,
}

impl HostSequencer {
    pub fn new(model: serde_json::Value, capability: [u8; 32]) -> Self {
        Self {
            model,
            capability,
            revision: 0,
            committed: HashMap::new(),
        }
    }

    pub fn commit(
        &mut self,
        presented_capability: [u8; 32],
        command: BoardCommand,
    ) -> Result<BoardEvent, CollabError> {
        authorize(&self.capability, &presented_capability)?;
        if let Some(event) = self.committed.get(&command.command_id) {
            return Ok(event.clone());
        }
        let updated_model = apply_command(self.model.clone(), &command)?;
        self.model = updated_model;
        self.revision += 1;
        let event = BoardEvent {
            sequence: self.revision,
            command,
        };
        self.committed
            .insert(event.command.command_id.clone(), event.clone());
        Ok(event)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn model(&self) -> &serde_json::Value {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_with_node() -> serde_json::Value {
        serde_json::json!({
            "id": "model-1",
            "name": "demo",
            "chapters": [],
            "submodels": [],
            "entities": [],
            "slices": [],
            "nodes": [{"id": "node-1", "type": "event", "name": "Created", "entityId": null, "sliceId": null}],
            "edges": [],
            "layout": {
                "nodePositions": {"node-1": {"x": 10.0, "y": 20.0}},
                "viewport": {"x": 0.0, "y": 0.0, "zoom": 1.0}
            }
        })
    }

    #[test]
    fn ticket_round_trips_without_losing_capability() {
        let ticket = ShareTicket::new_for_test("endpoint-a", [7; 32], [9; 32]);
        let encoded = ticket.serialize();
        let decoded = ShareTicket::deserialize(&encoded).expect("ticket decodes");

        assert_eq!(decoded, ticket);
        assert!(!encoded.contains("endpoint-a"), "ticket should be opaque");
    }

    #[test]
    fn move_command_updates_existing_node_deterministically() {
        let command = BoardCommand {
            command_id: "cmd-1".into(),
            actor_id: "actor-a".into(),
            base_revision: 0,
            operation: BoardOperation::MoveNode {
                node_id: "node-1".into(),
                x: 125.5,
                y: -8.0,
            },
        };
        let first = apply_command(model_with_node(), &command).expect("valid move");
        let second = apply_command(model_with_node(), &command).expect("valid move");

        assert_eq!(first, second);
        assert_eq!(first["layout"]["nodePositions"]["node-1"]["x"], 125.5);
        assert_eq!(first["layout"]["nodePositions"]["node-1"]["y"], -8.0);
    }

    #[test]
    fn move_command_rejects_unknown_node() {
        let command = BoardCommand {
            command_id: "cmd-2".into(),
            actor_id: "actor-a".into(),
            base_revision: 0,
            operation: BoardOperation::MoveNode {
                node_id: "missing".into(),
                x: 1.0,
                y: 2.0,
            },
        };

        let err = apply_command(model_with_node(), &command).unwrap_err();
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn host_rejects_wrong_room_capability() {
        let expected = [3; 32];
        let err = authorize(&expected, &[4; 32]).unwrap_err();
        assert!(err.to_string().contains("capability"));
        authorize(&expected, &expected).expect("matching capability accepted");
    }

    #[test]
    fn duplicate_command_is_committed_only_once() {
        let mut sequencer = HostSequencer::new(model_with_node(), [5; 32]);
        let command = BoardCommand {
            command_id: "same-command".into(),
            actor_id: "actor-a".into(),
            base_revision: 0,
            operation: BoardOperation::MoveNode {
                node_id: "node-1".into(),
                x: 44.0,
                y: 55.0,
            },
        };

        let first = sequencer
            .commit([5; 32], command.clone())
            .expect("first commit");
        let second = sequencer
            .commit([5; 32], command)
            .expect("duplicate lookup");

        assert_eq!(first, second);
        assert_eq!(first.sequence, 1);
        assert_eq!(sequencer.revision(), 1);
    }

    #[test]
    fn move_command_accepts_frontend_camel_case_fields() {
        let command: BoardCommand = serde_json::from_value(serde_json::json!({
            "commandId": "cmd-1",
            "actorId": "actor-a",
            "baseRevision": 0,
            "operation": {
                "type": "moveNode",
                "nodeId": "node-1",
                "x": 12.0,
                "y": 34.0
            }
        }))
        .unwrap();

        assert!(matches!(
            command.operation,
            BoardOperation::MoveNode { node_id, .. } if node_id == "node-1"
        ));
    }

    #[test]
    fn rejected_command_does_not_corrupt_host_model() {
        let capability = [11; 32];
        let original = model_with_node();
        let mut sequencer = HostSequencer::new(original.clone(), capability);
        let invalid = BoardCommand {
            command_id: "invalid".into(),
            actor_id: "actor-a".into(),
            base_revision: 0,
            operation: BoardOperation::MoveNode {
                node_id: "missing-node".into(),
                x: 1.0,
                y: 2.0,
            },
        };

        assert!(matches!(
            sequencer.commit(capability, invalid),
            Err(CollabError::UnknownNode(_))
        ));
        assert_eq!(sequencer.model(), &original);
        assert!(
            sequencer
                .commit(
                    capability,
                    BoardCommand {
                        command_id: "valid".into(),
                        actor_id: "actor-a".into(),
                        base_revision: 0,
                        operation: BoardOperation::MoveNode {
                            node_id: "node-1".into(),
                            x: 3.0,
                            y: 4.0,
                        },
                    }
                )
                .is_ok()
        );
    }

    #[test]
    fn signed_wire_message_rejects_tampering() {
        let key = iroh::SecretKey::generate();
        let message = WireMessage::Hello {
            capability: [8; 32],
        };
        let encoded = SignedWire::encode(&key, &message).expect("wire encodes");
        let (_, decoded) = SignedWire::decode(&encoded).expect("valid signature");
        assert_eq!(decoded, message);

        let mut tampered = encoded;
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(SignedWire::decode(&tampered).is_err());
    }

    #[test]
    fn snapshot_assembler_accepts_out_of_order_chunks_once() {
        let chunks = snapshot_chunks("peer-b", 7, b"abcdefghij", 4);
        assert_eq!(chunks.len(), 3);
        let mut assembler = SnapshotAssembler::default();
        assert!(assembler.push(chunks[2].clone()).unwrap().is_none());
        assert!(assembler.push(chunks[0].clone()).unwrap().is_none());
        let completed = assembler
            .push(chunks[1].clone())
            .unwrap()
            .expect("complete");
        assert_eq!(completed.sequence, 7);
        assert_eq!(completed.content, b"abcdefghij");
        assert!(assembler.push(chunks[1].clone()).unwrap().is_none());
    }
}

use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use iroh::{EndpointId, SecretKey, protocol::Router};
use iroh_gossip::{
    api::{Event as GossipEvent, GossipSender},
    net::{GOSSIP_ALPN, Gossip},
    proto::TopicId,
};
use n0_future::StreamExt;
use serde::Serialize;
use tokio::sync::{Mutex, broadcast};

use super::{
    BoardCommand, BoardEvent, CollabError, HostSequencer, Presence, ShareTicket, SignedWire,
    SnapshotAssembler, WireMessage, authorize, snapshot_chunks,
};

const SNAPSHOT_CHUNK_BYTES: usize = 48 * 1024;
const NOTIFICATION_CAPACITY: usize = 128;

pub type RuntimeResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CollabRole {
    Host,
    Join,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Notification {
    Snapshot { sequence: u64, content: String },
    Event { event: BoardEvent },
    CommandRejected { command_id: String, reason: String },
    Presence { presence: Presence },
    PeerCount { count: usize },
    RepairRequired,
}

struct HostState {
    sequencer: HostSequencer,
    model_path: PathBuf,
}

#[derive(Default)]
struct JoinBootstrap {
    snapshot: Option<(u64, String)>,
    events: Vec<BoardEvent>,
}

pub struct CollabRuntime {
    role: CollabRole,
    actor_id: String,
    authority: String,
    capability: [u8; 32],
    key: SecretKey,
    sender: Arc<Mutex<GossipSender>>,
    notifications: broadcast::Sender<Notification>,
    host: Option<Mutex<HostState>>,
    bootstrap: Mutex<JoinBootstrap>,
    assembler: Mutex<SnapshotAssembler>,
    peer_count: AtomicUsize,
    router: Router,
    receive_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl std::fmt::Debug for CollabRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CollabRuntime")
            .field("role", &self.role)
            .field("actor_id", &self.actor_id)
            .field("peer_count", &self.peer_count())
            .finish_non_exhaustive()
    }
}

impl CollabRuntime {
    pub async fn host(model_path: PathBuf) -> RuntimeResult<(Arc<Self>, ShareTicket)> {
        let content = std::fs::read_to_string(&model_path)?;
        let model = serde_json::from_str(&content)?;
        let capability = rand::random();
        let topic_id: [u8; 32] = rand::random();
        let key = SecretKey::generate();
        let (router, sender, receiver) = spawn_network(&key, topic_id, Vec::new()).await?;
        let actor_id = router.endpoint().id().to_string();
        let ticket = ShareTicket::new(actor_id.clone(), topic_id, capability);
        let (notifications, _) = broadcast::channel(NOTIFICATION_CAPACITY);
        let runtime = Arc::new(Self {
            role: CollabRole::Host,
            authority: actor_id.clone(),
            actor_id,
            capability,
            key,
            sender,
            notifications,
            host: Some(Mutex::new(HostState {
                sequencer: HostSequencer::new(model, capability),
                model_path,
            })),
            bootstrap: Mutex::new(JoinBootstrap::default()),
            assembler: Mutex::new(SnapshotAssembler::default()),
            peer_count: AtomicUsize::new(0),
            router,
            receive_task: Mutex::new(None),
        });
        runtime.start_receive_loop(receiver).await;
        Ok((runtime, ticket))
    }

    pub async fn join(ticket: ShareTicket) -> RuntimeResult<Arc<Self>> {
        let authority = ticket
            .bootstrap
            .first()
            .cloned()
            .ok_or(CollabError::InvalidTicket)?;
        let key = SecretKey::generate();
        let bootstrap = ticket
            .bootstrap
            .iter()
            .map(|value| EndpointId::from_str(value))
            .collect::<Result<Vec<_>, _>>()?;
        let (router, sender, receiver) = spawn_network(&key, ticket.topic_id, bootstrap).await?;
        let actor_id = router.endpoint().id().to_string();
        let (notifications, _) = broadcast::channel(NOTIFICATION_CAPACITY);
        let runtime = Arc::new(Self {
            role: CollabRole::Join,
            actor_id,
            authority,
            capability: ticket.capability,
            key,
            sender,
            notifications,
            host: None,
            bootstrap: Mutex::new(JoinBootstrap::default()),
            assembler: Mutex::new(SnapshotAssembler::default()),
            peer_count: AtomicUsize::new(0),
            router,
            receive_task: Mutex::new(None),
        });
        runtime.start_receive_loop(receiver).await;
        runtime
            .broadcast(WireMessage::Hello {
                capability: runtime.capability,
            })
            .await?;
        Ok(runtime)
    }

    pub fn role(&self) -> CollabRole {
        self.role
    }

    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }

    pub fn peer_count(&self) -> usize {
        self.peer_count.load(Ordering::Relaxed)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Notification> {
        self.notifications.subscribe()
    }

    pub async fn request_snapshot_repair(&self) -> RuntimeResult<()> {
        if self.role == CollabRole::Join {
            self.broadcast(WireMessage::Hello {
                capability: self.capability,
            })
            .await?;
        }
        Ok(())
    }

    pub async fn bootstrap_notifications(&self) -> Vec<Notification> {
        if self.role == CollabRole::Host {
            let Some(state) = &self.host else {
                return Vec::new();
            };
            let state = state.lock().await;
            return match serde_json::to_string_pretty(state.sequencer.model()) {
                Ok(content) => vec![Notification::Snapshot {
                    sequence: state.sequencer.revision(),
                    content,
                }],
                Err(error) => {
                    tracing::warn!(%error, "failed to serialise host collaboration bootstrap");
                    Vec::new()
                }
            };
        }
        let bootstrap = self.bootstrap.lock().await;
        let mut notifications = Vec::with_capacity(bootstrap.events.len() + 1);
        if let Some((sequence, content)) = &bootstrap.snapshot {
            notifications.push(Notification::Snapshot {
                sequence: *sequence,
                content: content.clone(),
            });
        }
        notifications.extend(
            bootstrap
                .events
                .iter()
                .cloned()
                .map(|event| Notification::Event { event }),
        );
        notifications
    }

    pub async fn submit(&self, mut command: BoardCommand) -> RuntimeResult<()> {
        command.actor_id = self.actor_id.clone();
        if self.role == CollabRole::Host {
            self.commit_host(command).await?;
        } else {
            self.broadcast(WireMessage::Proposal {
                capability: self.capability,
                command,
            })
            .await?;
        }
        Ok(())
    }

    pub async fn update_presence(&self, mut presence: Presence) -> RuntimeResult<()> {
        presence.actor_id = self.actor_id.clone();
        self.broadcast(WireMessage::Presence {
            capability: self.capability,
            presence,
        })
        .await
    }

    pub async fn shutdown(&self) {
        if let Some(task) = self.receive_task.lock().await.take() {
            task.abort();
        }
        if let Err(error) = self.router.shutdown().await {
            tracing::warn!(%error, "collaboration router shutdown failed");
        }
        self.router.endpoint().close().await;
    }

    async fn start_receive_loop(self: &Arc<Self>, mut receiver: iroh_gossip::api::GossipReceiver) {
        let runtime = Arc::clone(self);
        let task = tokio::spawn(async move {
            loop {
                match receiver.try_next().await {
                    Ok(Some(event)) => runtime.handle_gossip_event(event).await,
                    Ok(None) => break,
                    Err(error) => {
                        tracing::warn!(%error, "collaboration gossip receiver stopped");
                        break;
                    }
                }
            }
        });
        *self.receive_task.lock().await = Some(task);
    }

    async fn handle_gossip_event(&self, event: GossipEvent) {
        match event {
            GossipEvent::NeighborUp(_) => {
                self.change_peer_count(1);
                if self.role == CollabRole::Join
                    && let Err(error) = self
                        .broadcast(WireMessage::Hello {
                            capability: self.capability,
                        })
                        .await
                {
                    tracing::warn!(%error, "failed to request collaboration snapshot");
                }
            }
            GossipEvent::NeighborDown(_) => self.change_peer_count(-1),
            GossipEvent::Lagged => tracing::warn!("collaboration gossip receiver lagged"),
            GossipEvent::Received(message) => {
                let Ok((from, wire)) = SignedWire::decode(&message.content) else {
                    tracing::warn!("ignored invalid signed collaboration message");
                    return;
                };
                if let Err(error) = self.handle_wire(from, wire).await {
                    tracing::warn!(%error, "ignored invalid collaboration message");
                }
            }
        }
    }

    async fn handle_wire(&self, from: EndpointId, wire: WireMessage) -> RuntimeResult<()> {
        let from = from.to_string();
        match wire {
            WireMessage::Hello { capability } => {
                authorize(&self.capability, &capability)?;
                if self.role == CollabRole::Host {
                    self.send_snapshot(&from).await?;
                }
            }
            WireMessage::Proposal {
                capability,
                mut command,
            } => {
                authorize(&self.capability, &capability)?;
                if self.role == CollabRole::Host {
                    command.actor_id = from.clone();
                    let command_id = command.command_id.clone();
                    if let Err(error) = self.commit_host(command).await {
                        self.broadcast(WireMessage::Rejected {
                            capability: self.capability,
                            recipient: from,
                            command_id,
                            reason: error.to_string(),
                        })
                        .await?;
                    }
                }
            }
            WireMessage::Committed { capability, event } => {
                authorize(&self.capability, &capability)?;
                if self.role == CollabRole::Join {
                    self.authorize_host(&from)?;
                    let mut bootstrap = self.bootstrap.lock().await;
                    if !bootstrap
                        .events
                        .iter()
                        .any(|known| known.sequence == event.sequence)
                    {
                        bootstrap.events.push(event.clone());
                        if bootstrap.events.len() > 4096 {
                            bootstrap.events.remove(0);
                        }
                    }
                    drop(bootstrap);
                    let _ = self.notifications.send(Notification::Event { event });
                }
            }
            WireMessage::Rejected {
                capability,
                recipient,
                command_id,
                reason,
            } => {
                authorize(&self.capability, &capability)?;
                self.authorize_host(&from)?;
                if self.role == CollabRole::Join && recipient == self.actor_id {
                    let _ = self
                        .notifications
                        .send(Notification::CommandRejected { command_id, reason });
                }
            }
            WireMessage::SnapshotChunk { capability, chunk } => {
                authorize(&self.capability, &capability)?;
                if self.role == CollabRole::Join && chunk.recipient == self.actor_id {
                    self.authorize_host(&from)?;
                    if let Some(snapshot) = self.assembler.lock().await.push(chunk)? {
                        let content = String::from_utf8(snapshot.content)?;
                        let mut bootstrap = self.bootstrap.lock().await;
                        bootstrap.snapshot = Some((snapshot.sequence, content.clone()));
                        bootstrap
                            .events
                            .retain(|event| event.sequence > snapshot.sequence);
                        drop(bootstrap);
                        let _ = self.notifications.send(Notification::Snapshot {
                            sequence: snapshot.sequence,
                            content,
                        });
                    }
                }
            }
            WireMessage::Presence {
                capability,
                mut presence,
            } => {
                authorize(&self.capability, &capability)?;
                if from != self.actor_id {
                    presence.actor_id = from;
                    let _ = self.notifications.send(Notification::Presence { presence });
                }
            }
        }
        Ok(())
    }

    async fn commit_host(&self, command: BoardCommand) -> RuntimeResult<()> {
        let host = self.host.as_ref().expect("host state exists for host role");
        let mut host = host.lock().await;
        let event = commit_and_persist(&mut host, self.capability, command)?;
        drop(host);
        let _ = self.notifications.send(Notification::Event {
            event: event.clone(),
        });
        self.broadcast(WireMessage::Committed {
            capability: self.capability,
            event,
        })
        .await
    }

    fn authorize_host(&self, signer: &str) -> Result<(), CollabError> {
        if signer == self.authority {
            Ok(())
        } else {
            Err(CollabError::InvalidAuthority)
        }
    }

    async fn send_snapshot(&self, recipient: &str) -> RuntimeResult<()> {
        let host = self.host.as_ref().expect("host state exists for host role");
        let host = host.lock().await;
        let content = serde_json::to_vec(host.sequencer.model())?;
        let sequence = host.sequencer.revision();
        drop(host);
        for chunk in snapshot_chunks(recipient, sequence, &content, SNAPSHOT_CHUNK_BYTES) {
            self.broadcast(WireMessage::SnapshotChunk {
                capability: self.capability,
                chunk,
            })
            .await?;
        }
        Ok(())
    }

    async fn broadcast(&self, message: WireMessage) -> RuntimeResult<()> {
        let encoded = SignedWire::encode(&self.key, &message)?;
        self.sender.lock().await.broadcast(encoded.into()).await?;
        Ok(())
    }

    fn change_peer_count(&self, delta: isize) {
        let count = if delta > 0 {
            self.peer_count.fetch_add(delta as usize, Ordering::Relaxed) + delta as usize
        } else {
            let decrement = (-delta) as usize;
            self.peer_count
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    Some(value.saturating_sub(decrement))
                })
                .unwrap_or(0)
                .saturating_sub(decrement)
        };
        let _ = self.notifications.send(Notification::PeerCount { count });
    }
}

fn commit_and_persist(
    host: &mut HostState,
    capability: [u8; 32],
    command: BoardCommand,
) -> RuntimeResult<BoardEvent> {
    let command_id = command.command_id.clone();
    let was_committed = host.sequencer.committed.contains_key(&command_id);
    let previous_model = host.sequencer.model.clone();
    let previous_revision = host.sequencer.revision;
    let event = host.sequencer.commit(capability, command)?;
    if let Err(error) = persist_model(&host.model_path, host.sequencer.model()) {
        host.sequencer.model = previous_model;
        host.sequencer.revision = previous_revision;
        if !was_committed {
            host.sequencer.committed.remove(&command_id);
        }
        return Err(error.into());
    }
    Ok(event)
}

async fn spawn_network(
    key: &SecretKey,
    topic_id: [u8; 32],
    bootstrap: Vec<EndpointId>,
) -> RuntimeResult<(
    Router,
    Arc<Mutex<GossipSender>>,
    iroh_gossip::api::GossipReceiver,
)> {
    let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::N0)
        .secret_key(key.clone())
        .alpns(vec![GOSSIP_ALPN.to_vec()])
        .bind()
        .await?;
    let gossip = Gossip::builder().spawn(endpoint.clone());
    let router = Router::builder(endpoint)
        .accept(GOSSIP_ALPN, gossip.clone())
        .spawn();
    let topic = gossip
        .subscribe(TopicId::from_bytes(topic_id), bootstrap)
        .await?;
    let (sender, receiver) = topic.split();
    Ok((router, Arc::new(Mutex::new(sender)), receiver))
}

fn persist_model(path: &Path, model: &serde_json::Value) -> RuntimeResult<()> {
    use std::io::Write;

    let parent = path
        .parent()
        .ok_or(CollabError::InvalidModel("workspace root"))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temp, model)?;
    temp.write_all(b"\n")?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ide::collab::{BoardCommand, BoardOperation};
    use std::time::Duration;

    fn model_json() -> String {
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
        .to_string()
    }

    #[test]
    fn persistence_failure_does_not_advance_host_state() {
        let dir = tempfile::tempdir().unwrap();
        let invalid_model_path = dir.path().join("event-model.json");
        std::fs::create_dir(&invalid_model_path).unwrap();
        let capability = [3; 32];
        let mut host = HostState {
            sequencer: HostSequencer::new(serde_json::from_str(&model_json()).unwrap(), capability),
            model_path: invalid_model_path,
        };
        let result = commit_and_persist(
            &mut host,
            capability,
            BoardCommand {
                command_id: "must-not-commit".into(),
                actor_id: "actor-a".into(),
                base_revision: 0,
                operation: super::super::BoardOperation::MoveNode {
                    node_id: "node-1".into(),
                    x: 99.0,
                    y: 100.0,
                },
            },
        );

        assert!(result.is_err());
        assert_eq!(host.sequencer.revision(), 0);
        assert_eq!(
            host.sequencer.model()["layout"]["nodePositions"]["node-1"]["x"],
            10.0
        );
    }

    #[tokio::test]
    async fn join_receives_snapshot_and_host_commits_remote_move() {
        let dir = tempfile::tempdir().unwrap();
        let model_path = dir.path().join("event-model.json");
        std::fs::write(&model_path, model_json()).unwrap();

        let (host, ticket) = CollabRuntime::host(model_path.clone()).await.unwrap();
        let host_bootstrap = host.bootstrap_notifications().await;
        assert!(matches!(
            host_bootstrap.as_slice(),
            [Notification::Snapshot { sequence: 0, content }] if content.contains("model-1")
        ));
        let join = CollabRuntime::join(ticket).await.unwrap();
        let mut join_events = join.subscribe();
        let mut host_events = host.subscribe();

        let snapshot = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Ok(Notification::Snapshot { content, .. }) = join_events.recv().await {
                    break content;
                }
            }
        })
        .await
        .expect("join snapshot timeout");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&snapshot).unwrap()["id"],
            "model-1"
        );

        join.submit(BoardCommand {
            command_id: "rejected-move".into(),
            actor_id: join.actor_id().to_owned(),
            base_revision: 0,
            operation: super::super::BoardOperation::MoveNode {
                node_id: "missing-node".into(),
                x: 1.0,
                y: 2.0,
            },
        })
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Ok(Notification::CommandRejected { command_id, .. }) =
                    join_events.recv().await
                    && command_id == "rejected-move"
                {
                    break;
                }
            }
        })
        .await
        .expect("join rejection timeout");

        join.submit(BoardCommand {
            command_id: "remote-move".into(),
            actor_id: join.actor_id().to_owned(),
            base_revision: 0,
            operation: BoardOperation::MoveNode {
                node_id: "node-1".into(),
                x: 88.0,
                y: 99.0,
            },
        })
        .await
        .unwrap();

        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Ok(Notification::Event { event }) = host_events.recv().await {
                    if event.command.command_id == "remote-move" {
                        break;
                    }
                }
            }
        })
        .await
        .expect("host commit timeout");

        let persisted: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(model_path).unwrap()).unwrap();
        assert_eq!(persisted["layout"]["nodePositions"]["node-1"]["x"], 88.0);
        assert_eq!(persisted["layout"]["nodePositions"]["node-1"]["y"], 99.0);

        join.shutdown().await;
        host.shutdown().await;
    }

    #[tokio::test]
    async fn late_frontend_can_replay_cached_join_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let model_path = dir.path().join("event-model.json");
        std::fs::write(&model_path, model_json()).unwrap();

        let (host, ticket) = CollabRuntime::host(model_path).await.unwrap();
        let join = CollabRuntime::join(ticket).await.unwrap();

        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let cached = join.bootstrap_notifications().await;
                if matches!(cached.first(), Some(Notification::Snapshot { .. })) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("cached snapshot timeout");

        let cached = join.bootstrap_notifications().await;
        let Notification::Snapshot { content, .. } = &cached[0] else {
            panic!("first cached notification must be a snapshot");
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(content).unwrap()["id"],
            "model-1"
        );

        join.shutdown().await;
        host.shutdown().await;
    }
}

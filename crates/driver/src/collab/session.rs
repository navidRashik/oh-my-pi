//! Process-local owner for replica-backed collaboration relay sessions.

use std::{collections::BTreeMap, time::Duration};

use bytes::Bytes;
use omp_collab::{
	PROTOCOL_REVISION,
	codec::RelayRoute,
	host::{AuthenticatedPeer, AuthorizedMutation, HostAdmission},
	link::{CollabLink, HostedRoom, RelayEndpoint},
	presence::{CollabRole, ConnectionState, PresenceFacts},
	relay::{Handshake, RelayClient, RelayInbound, RelayRole, SendDisposition},
};
use omp_core::{Str, base64_url};
use omp_dom::{Dom, Event, Snapshot, SnapshotDecodeError};
use omp_journal::EntryId;
use omp_proto::collab::v1::{
	AbortRequest, AgentCommand, CollabFrame, ErrorMessage, Hello, ImageAttachment, JournalRecord,
	Participant, PromptRequest, RegistrySnapshot, SessionHeader, SessionStateUpdate, SnapshotChunk,
	UiResponse, VisibilityClass, Welcome, collab_frame,
};
use serde::Deserialize;
use serde_json::value::RawValue;
use tokio::{sync::watch, task::JoinHandle};
use tokio_util::sync::CancellationToken;

const INITIAL_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const SNAPSHOT_CHUNK_BYTES: usize = 256 * 1024;

/// One operation serialized through the collaboration owner.
#[derive(Clone, Debug)]
pub enum CollabOwnerCommand {
	/// Host a generated room and begin broadcasting this session's snapshot and
	/// ordered patch stream.
	Start {
		/// Validated relay origin.
		relay:    RelayEndpoint,
		/// Race-free session snapshot captured with `events`.
		snapshot: Snapshot,
		/// Events following `snapshot` in journal order.
		events:   flume::Receiver<Event>,
	},
	/// Join a parsed room link under the resolved local identity.
	Join {
		/// Parsed room endpoint and credentials.
		link:         CollabLink,
		/// Local participant name.
		display_name: Str,
	},
	/// Submit a prompt through the authenticated host controller.
	Prompt {
		/// User-authored text.
		text:   Str,
		/// Inline images transported to the host's blob authority.
		images: Vec<ImageAttachment>,
	},
	/// Interrupt the host's active generation.
	Abort,
	/// Control one host-visible agent.
	Agent(AgentCommand),
	/// Answer a host-owned UI request.
	UiResponse(UiResponse),
	/// Leave or close the active room.
	Leave,
	/// Read the current room state.
	Status,
}

/// Settled collaboration command result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollabCommandResult {
	/// Current presence facts.
	pub presence:    Option<PresenceFacts>,
	/// Writable guest link while hosting.
	pub editor_link: Option<Str>,
	/// Read-only guest link while hosting.
	pub viewer_link: Option<Str>,
}

/// Collaboration owner failure.
#[derive(Debug, thiserror::Error)]
pub enum CollabCommandFault {
	/// Owner task has stopped.
	#[error("collaboration owner stopped")]
	OwnerStopped,
	/// Relay operation failed.
	#[error("collaboration relay failed")]
	Relay(#[from] omp_collab::relay::RelayError),
	/// Room key was invalid.
	#[error("collaboration room key was invalid")]
	Crypto(#[from] omp_collab::crypto::CryptoError),
	/// Snapshot or patch projection failed.
	#[error("collaboration replication projection failed")]
	Projection(#[from] serde_json::Error),
	/// A replica snapshot was malformed or internally inconsistent.
	#[error("collaboration replica snapshot was invalid")]
	Snapshot(#[from] SnapshotDecodeError),
	/// A replicated DOM event could not be applied.
	#[error("collaboration replica event was invalid")]
	Dom(#[from] omp_dom::DomError),
	/// The host did not complete the welcome and snapshot handshake in time.
	#[error("collaboration host handshake timed out")]
	HandshakeTimeout,
	/// The host refused the guest handshake.
	#[error("collaboration host refused the guest handshake")]
	HandshakeRefused,
	/// The host welcome did not include a complete DOM snapshot.
	#[error("collaboration host welcome omitted the session snapshot")]
	MissingSnapshot,
	/// A DOM snapshot fragment was not a valid authenticated chunk.
	#[error("collaboration host sent an invalid session snapshot fragment")]
	InvalidSnapshotFragment,
	/// A local mutation was attempted from a read-only guest link.
	#[error("collaboration link is read-only")]
	ReadOnly,
	/// No room is active.
	#[error("not joined to a collaboration room")]
	NotJoined,
	/// This operation is available only while joined as a guest.
	#[error("collaboration operation requires a guest connection")]
	NotGuest,
	/// The relay did not confirm delivery of a guest mutation.
	#[error("collaboration mutation was not delivered while the relay was connected")]
	MutationNotDelivered,
}

struct Request {
	command: CollabOwnerCommand,
	reply:   flume::Sender<Result<CollabCommandResult, CollabCommandFault>>,
}

/// Cloneable command, presence, replica, and admitted-mutation projection.
#[derive(Clone)]
pub struct CollabCommandHandle {
	commands:         flume::Sender<Request>,
	presence:         watch::Receiver<Option<PresenceFacts>>,
	replica:          watch::Receiver<Option<Snapshot>>,
	replica_events:   flume::Receiver<Event>,
	remote_mutations: flume::Receiver<AuthorizedMutation>,
}

impl CollabCommandHandle {
	/// Requests one serialized owner operation.
	pub async fn request(
		&self,
		command: CollabOwnerCommand,
	) -> Result<CollabCommandResult, CollabCommandFault> {
		let (reply, result) = flume::bounded(1);
		self
			.commands
			.send_async(Request { command, reply })
			.await
			.map_err(|_| CollabCommandFault::OwnerStopped)?;
		result
			.recv_async()
			.await
			.map_err(|_| CollabCommandFault::OwnerStopped)?
	}

	/// Returns current presence facts.
	#[must_use]
	pub fn presence(&self) -> Option<PresenceFacts> {
		*self.presence.borrow()
	}

	/// Subscribes to presence changes.
	#[must_use]
	pub fn subscribe_presence(&self) -> watch::Receiver<Option<PresenceFacts>> {
		self.presence.clone()
	}

	/// Returns the latest complete guest replica snapshot.
	#[must_use]
	pub fn replica_snapshot(&self) -> Option<Snapshot> {
		self.replica.borrow().clone()
	}

	/// Returns the single ordered queue of post-snapshot replica events.
	///
	/// The app controller must create one receiver and retain it for its
	/// lifetime. Clones compete for delivery and therefore are not actor
	/// subscriptions.
	#[must_use]
	pub fn replica_events(&self) -> flume::Receiver<Event> {
		self.replica_events.clone()
	}

	/// Returns the host controller's authenticated remote-mutation queue.
	///
	/// Every item was admitted against the room's write token before entering
	/// this queue.
	#[must_use]
	pub fn remote_mutations(&self) -> flume::Receiver<AuthorizedMutation> {
		self.remote_mutations.clone()
	}
}

struct Outbound {
	frame: CollabFrame,
	reply: flume::Sender<bool>,
}

struct ActiveSession {
	cancel:      CancellationToken,
	task:        JoinHandle<()>,
	presence:    watch::Receiver<Option<PresenceFacts>>,
	outbound:    Option<flume::Sender<Outbound>>,
	editor_link: Option<Str>,
	viewer_link: Option<Str>,
}

impl ActiveSession {
	fn result(&self) -> CollabCommandResult {
		CollabCommandResult {
			presence:    *self.presence.borrow(),
			editor_link: self.editor_link.clone(),
			viewer_link: self.viewer_link.clone(),
		}
	}

	async fn close(self) {
		self.cancel.cancel();
		let _ = self.task.await;
	}
}

/// Receiving half retained by the relay lifecycle owner.
pub struct CollabSessionAuthority {
	commands:         flume::Receiver<Request>,
	presence:         watch::Sender<Option<PresenceFacts>>,
	replica:          watch::Sender<Option<Snapshot>>,
	replica_events:   flume::Sender<Event>,
	remote_mutations: flume::Sender<AuthorizedMutation>,
}

impl CollabSessionAuthority {
	/// Constructs the collaboration owner.
	#[must_use]
	pub fn new() -> (Self, CollabCommandHandle) {
		let (commands, requests) = flume::bounded(16);
		let (presence, observed) = watch::channel(None);
		let (replica, replica_observed) = watch::channel(None);
		let (replica_events, observed_events) = flume::unbounded();
		let (remote_mutations, observed_mutations) = flume::unbounded();
		(
			Self { commands: requests, presence, replica, replica_events, remote_mutations },
			CollabCommandHandle {
				commands,
				presence: observed,
				replica: replica_observed,
				replica_events: observed_events,
				remote_mutations: observed_mutations,
			},
		)
	}

	async fn run(self) {
		let mut active: Option<ActiveSession> = None;
		while let Ok(request) = self.commands.recv_async().await {
			let result = match request.command {
				CollabOwnerCommand::Start { relay, snapshot, events } => {
					if let Some(previous) = active.take() {
						previous.close().await;
					}
					self.replica.send_replace(None);
					match start_host(
						relay,
						snapshot,
						events,
						self.presence.clone(),
						self.remote_mutations.clone(),
					)
					.await
					{
						Ok(session) => {
							let result = session.result();
							active = Some(session);
							Ok(result)
						},
						Err(error) => Err(error),
					}
				},
				CollabOwnerCommand::Join { link, display_name } => {
					if let Some(previous) = active.take() {
						previous.close().await;
					}
					self.replica.send_replace(None);
					match start_guest(
						link,
						display_name,
						self.presence.clone(),
						self.replica.clone(),
						self.replica_events.clone(),
					)
					.await
					{
						Ok(session) => {
							let result = session.result();
							active = Some(session);
							Ok(result)
						},
						Err(error) => Err(error),
					}
				},
				CollabOwnerCommand::Prompt { text, images } => {
					send_guest_frame(
						active.as_ref(),
						collab_frame::Payload::Prompt(PromptRequest { text: text.to_string(), images }),
					)
					.await
				},
				CollabOwnerCommand::Abort => {
					send_guest_frame(
						active.as_ref(),
						collab_frame::Payload::Abort(AbortRequest {
							reason: "User interrupt".to_owned(),
						}),
					)
					.await
				},
				CollabOwnerCommand::Agent(command) => {
					send_guest_frame(active.as_ref(), collab_frame::Payload::AgentCommand(command)).await
				},
				CollabOwnerCommand::UiResponse(response) => {
					send_guest_frame(active.as_ref(), collab_frame::Payload::UiResponse(response)).await
				},
				CollabOwnerCommand::Leave => match active.take() {
					Some(session) => {
						session.close().await;
						self.presence.send_replace(None);
						self.replica.send_replace(None);
						Ok(disconnected_result())
					},
					None => Err(CollabCommandFault::NotJoined),
				},
				CollabOwnerCommand::Status => active
					.as_ref()
					.map(ActiveSession::result)
					.ok_or(CollabCommandFault::NotJoined),
			};
			let _ = request.reply.send(result);
		}
		if let Some(session) = active {
			session.close().await;
		}
	}
}

async fn send_guest_frame(
	active: Option<&ActiveSession>,
	payload: collab_frame::Payload,
) -> Result<CollabCommandResult, CollabCommandFault> {
	let active = active.ok_or(CollabCommandFault::NotJoined)?;
	let presence = (*active.presence.borrow()).ok_or(CollabCommandFault::NotJoined)?;
	if presence.role() != CollabRole::Guest {
		return Err(CollabCommandFault::NotGuest);
	}
	if presence.read_only() {
		return Err(CollabCommandFault::ReadOnly);
	}
	let outbound = active
		.outbound
		.as_ref()
		.ok_or(CollabCommandFault::NotGuest)?;
	let (reply, delivered) = flume::bounded(1);
	outbound
		.send_async(Outbound {
			frame: CollabFrame {
				protocol_revision: PROTOCOL_REVISION,
				payload: Some(payload),
				..CollabFrame::default()
			},
			reply,
		})
		.await
		.map_err(|_| CollabCommandFault::OwnerStopped)?;
	if !delivered
		.recv_async()
		.await
		.map_err(|_| CollabCommandFault::OwnerStopped)?
	{
		return Err(CollabCommandFault::MutationNotDelivered);
	}
	Ok(active.result())
}

fn disconnected_result() -> CollabCommandResult {
	CollabCommandResult { presence: None, editor_link: None, viewer_link: None }
}

async fn start_host(
	relay_endpoint: RelayEndpoint,
	snapshot: Snapshot,
	events: flume::Receiver<Event>,
	presence_tx: watch::Sender<Option<PresenceFacts>>,
	remote_mutations: flume::Sender<AuthorizedMutation>,
) -> Result<ActiveSession, CollabCommandFault> {
	let room = HostedRoom::generate(relay_endpoint)?;
	let room_id = Str::from(base64_url::encode_raw(room.full.room_id().as_bytes()).into_string());
	let admission = HostAdmission::new(room_id, room.write_token.clone());
	let mut relay = RelayClient::new(room.full.room_url(), RelayRole::Host, room.room_key)?;
	presence_tx.send_replace(Some(PresenceFacts::host(ConnectionState::Connecting, 0)));
	relay.connect().await?;
	presence_tx.send_replace(Some(PresenceFacts::host(ConnectionState::Connected, 0)));
	let cancel = CancellationToken::new();
	let task_cancel = cancel.clone();
	let editor_link = Some(Str::new(room.full.compact()));
	let viewer_link = Some(Str::new(room.view.compact()));
	let presence = presence_tx.subscribe();
	let task = tokio::spawn(async move {
		let mut replica = Dom::from_snapshot(&snapshot);
		let mut peers = BTreeMap::<u32, AuthenticatedPeer>::new();
		let mut sequence = 0_u64;
		loop {
			enum Wake {
				Cancel,
				Event(Result<Event, flume::RecvError>),
				Inbound(Result<Option<RelayInbound>, omp_collab::relay::RelayError>),
			}
			let wake = tokio::select! {
				() = task_cancel.cancelled() => Wake::Cancel,
				event = events.recv_async() => Wake::Event(event),
				inbound = relay.receive() => Wake::Inbound(inbound),
			};
			match wake {
				Wake::Cancel => break,
				Wake::Event(Ok(event)) => {
					if replica.apply_event(&event).is_err() {
						break;
					}
					sequence = sequence.saturating_add(1);
					let Ok(record) = event_record(sequence, event) else {
						break;
					};
					let frame = live_record_frame(sequence, record);
					if relay.send(RelayRoute { peer_id: 0 }, &frame).await.is_err() {
						break;
					}
				},
				Wake::Event(Err(_)) => break,
				Wake::Inbound(Ok(Some(RelayInbound::PeerJoined(_)))) => {},
				Wake::Inbound(Ok(Some(RelayInbound::PeerLeft(left)))) => {
					peers.remove(&left.peer_id);
					presence_tx
						.send_replace(Some(PresenceFacts::host(ConnectionState::Connected, peers.len())));
					sequence = sequence.saturating_add(1);
					let state = state_frame(sequence, &peers);
					let _ = relay.send(RelayRoute { peer_id: 0 }, &state).await;
				},
				Wake::Inbound(Ok(Some(RelayInbound::Frame(routed)))) => {
					let peer_id = routed.route.peer_id;
					match routed.frame.payload.as_ref() {
						Some(collab_frame::Payload::Hello(hello)) => {
							let mut handshake = Handshake::new(RelayRole::Host);
							if handshake.accept(&routed.frame).is_err() {
								send_error(&mut relay, peer_id, "protocol", "Protocol mismatch").await;
								continue;
							}
							let Ok(peer) = admission.authenticate(peer_id, hello) else {
								send_error(&mut relay, peer_id, "admission", "Guest admission failed")
									.await;
								continue;
							};
							let read_only = peer.read_only();
							peers.insert(peer_id, peer);
							presence_tx.send_replace(Some(PresenceFacts::host(
								ConnectionState::Connected,
								peers.len(),
							)));
							let snapshot = replica.snapshot();
							let chunks = snapshot_chunks(snapshot.as_bytes());
							let chunk_count = chunks.len();
							sequence = sequence.saturating_add(1);
							let welcome = welcome_frame(
								sequence,
								read_only,
								u32::try_from(chunk_count).unwrap_or(u32::MAX),
								&peers,
							);
							if relay.send(RelayRoute { peer_id }, &welcome).await.is_err() {
								continue;
							}
							for (index, bytes) in chunks.into_iter().enumerate() {
								sequence = sequence.saturating_add(1);
								let frame = snapshot_frame(
									sequence,
									bytes,
									index + 1 == chunk_count,
									u64::try_from(chunk_count).unwrap_or(u64::MAX),
								);
								if relay.send(RelayRoute { peer_id }, &frame).await.is_err() {
									break;
								}
							}
							sequence = sequence.saturating_add(1);
							let state = state_frame(sequence, &peers);
							let _ = relay.send(RelayRoute { peer_id: 0 }, &state).await;
						},
						Some(payload) => {
							let Some(peer) = peers.get(&peer_id) else {
								send_error(&mut relay, peer_id, "hello_required", "Guest hello required")
									.await;
								continue;
							};
							match admission.admit_mutation(peer, payload) {
								Ok(mutation) => {
									let _ = remote_mutations.send(mutation);
								},
								Err(_) => {
									send_error(
										&mut relay,
										peer_id,
										"read_only",
										"Mutation is disabled on a read-only link",
									)
									.await;
								},
							}
						},
						None => {},
					}
				},
				Wake::Inbound(Ok(None)) | Wake::Inbound(Err(_)) => {
					peers.clear();
					presence_tx
						.send_replace(Some(PresenceFacts::host(ConnectionState::Reconnecting, 0)));
					if !reconnect(&mut relay, &task_cancel).await {
						break;
					}
					presence_tx.send_replace(Some(PresenceFacts::host(ConnectionState::Connected, 0)));
				},
			}
		}
		presence_tx.send_replace(Some(PresenceFacts::host(ConnectionState::Disconnected, 0)));
		let _ = relay.close().await;
	});
	Ok(ActiveSession { cancel, task, presence, outbound: None, editor_link, viewer_link })
}

async fn start_guest(
	link: CollabLink,
	display_name: Str,
	presence_tx: watch::Sender<Option<PresenceFacts>>,
	replica_tx: watch::Sender<Option<Snapshot>>,
	replica_events: flume::Sender<Event>,
) -> Result<ActiveSession, CollabCommandFault> {
	let key = omp_collab::crypto::RoomKey::from_bytes(*link.credentials().key())?;
	let mut relay = RelayClient::new(link.room_url(), RelayRole::Guest, key)?;
	let read_only = link.credentials().is_read_only();
	presence_tx.send_replace(Some(PresenceFacts::guest(ConnectionState::Connecting, 1, read_only)));
	relay.connect().await?;
	let hello = Hello {
		protocol_revision: PROTOCOL_REVISION,
		display_name:      display_name.to_string(),
		write_token:       link
			.credentials()
			.write_token()
			.map(|token| Bytes::copy_from_slice(token.as_bytes())),
		client_version:    env!("CARGO_PKG_VERSION").to_owned(),
	};
	let mut sequence = 1_u64;
	let hello_frame = Handshake::hello(sequence, hello.clone());
	let _ = relay.send(RelayRoute { peer_id: 0 }, &hello_frame).await?;
	let cancel = CancellationToken::new();
	let task_cancel = cancel.clone();
	let (outbound, outbound_rx) = flume::bounded::<Outbound>(64);
	let (ready_tx, ready_rx) = flume::bounded(1);
	let presence = presence_tx.subscribe();
	let task = tokio::spawn(async move {
		let mut handshake = Handshake::new(RelayRole::Guest);
		let mut replica: Option<Dom> = None;
		let mut snapshot_records = Vec::<JournalRecord>::new();
		let mut initial = true;
		loop {
			enum Wake {
				Cancel,
				Outbound(Result<Outbound, flume::RecvError>),
				Inbound(Result<Option<RelayInbound>, omp_collab::relay::RelayError>),
			}
			let wake = tokio::select! {
				() = task_cancel.cancelled() => Wake::Cancel,
				frame = outbound_rx.recv_async() => Wake::Outbound(frame),
				inbound = relay.receive() => Wake::Inbound(inbound),
			};
			match wake {
				Wake::Cancel => break,
				Wake::Outbound(Ok(mut outbound)) => {
					sequence = sequence.saturating_add(1);
					outbound.frame.sequence = sequence;
					outbound.frame.protocol_revision = PROTOCOL_REVISION;
					let delivered = matches!(
						relay.send(RelayRoute { peer_id: 0 }, &outbound.frame).await,
						Ok(SendDisposition::Sent)
					);
					let _ = outbound.reply.send(delivered);
					if !delivered {
						continue;
					}
				},
				Wake::Outbound(Err(_)) => break,
				Wake::Inbound(Ok(Some(RelayInbound::Frame(routed)))) => {
					if matches!(&routed.frame.payload, Some(collab_frame::Payload::Welcome(_)))
						&& handshake.accept(&routed.frame).is_err()
					{
						if initial {
							let _ = ready_tx.send(Err(CollabCommandFault::HandshakeRefused));
						}
						break;
					}
					match routed.frame.payload {
						Some(collab_frame::Payload::Welcome(welcome)) => {
							snapshot_records.clear();
							presence_tx.send_replace(Some(PresenceFacts::guest(
								ConnectionState::Connecting,
								welcome
									.initial_state
									.as_ref()
									.map_or(1, |state| state.participants.len().max(1)),
								welcome.read_only,
							)));
						},
						Some(collab_frame::Payload::SnapshotChunk(chunk)) => {
							snapshot_records.extend(chunk.entries);
							if chunk.r#final {
								if snapshot_records.is_empty() {
									if initial {
										let _ = ready_tx.send(Err(CollabCommandFault::MissingSnapshot));
									}
									break;
								}
								let encoded = match decode_snapshot_chunks(&snapshot_records) {
									Ok(encoded) => encoded,
									Err(error) => {
										if initial {
											let _ = ready_tx.send(Err(error));
										}
										break;
									},
								};
								match Snapshot::from_bytes(&encoded) {
									Ok(snapshot) => {
										replica = Some(Dom::from_snapshot(&snapshot));
										replica_tx.send_replace(Some(snapshot.clone()));
										if !initial {
											let _ = replica_events.send(Event::Reset { snapshot });
										}
										presence_tx.send_replace(Some(PresenceFacts::guest(
											ConnectionState::Connected,
											(*presence_tx.borrow())
												.map_or(1, PresenceFacts::participant_count),
											read_only,
										)));
										if initial {
											initial = false;
											let _ = ready_tx.send(Ok(()));
										}
									},
									Err(error) => {
										if initial {
											let _ = ready_tx.send(Err(CollabCommandFault::Snapshot(error)));
										}
										break;
									},
								}
							}
						},
						Some(collab_frame::Payload::JournalRecord(record)) => {
							let Some(dom) = replica.as_mut() else {
								continue;
							};
							match decode_event(&record).and_then(|event| {
								dom.apply_event(&event)?;
								Ok(event)
							}) {
								Ok(event) => {
									let _ = replica_events.send(event);
								},
								Err(_) => break,
							}
						},
						Some(collab_frame::Payload::State(state)) => {
							presence_tx.send_replace(Some(PresenceFacts::guest(
								ConnectionState::Connected,
								state.participants.len().max(1),
								read_only,
							)));
						},
						Some(collab_frame::Payload::Error(_)) if initial => {
							let _ = ready_tx.send(Err(CollabCommandFault::HandshakeRefused));
							break;
						},
						Some(collab_frame::Payload::Bye(_)) => break,
						_ => {},
					}
				},
				Wake::Inbound(Ok(Some(RelayInbound::PeerJoined(_) | RelayInbound::PeerLeft(_)))) => {},
				Wake::Inbound(Ok(None)) | Wake::Inbound(Err(_)) => {
					presence_tx.send_replace(Some(PresenceFacts::guest(
						ConnectionState::Reconnecting,
						(*presence_tx.borrow()).map_or(1, PresenceFacts::participant_count),
						read_only,
					)));
					if !reconnect(&mut relay, &task_cancel).await {
						if initial {
							let _ = ready_tx.send(Err(CollabCommandFault::HandshakeRefused));
						}
						break;
					}
					handshake = Handshake::new(RelayRole::Guest);
					sequence = sequence.saturating_add(1);
					let frame = Handshake::hello(sequence, hello.clone());
					if relay.send(RelayRoute { peer_id: 0 }, &frame).await.is_err() {
						break;
					}
				},
			}
		}
		presence_tx.send_replace(Some(PresenceFacts::guest(
			ConnectionState::Disconnected,
			1,
			read_only,
		)));
		let _ = relay.close().await;
	});
	match tokio::time::timeout(INITIAL_HANDSHAKE_TIMEOUT, ready_rx.recv_async()).await {
		Ok(Ok(Ok(()))) => Ok(ActiveSession {
			cancel,
			task,
			presence,
			outbound: Some(outbound),
			editor_link: None,
			viewer_link: None,
		}),
		Ok(Ok(Err(error))) => {
			cancel.cancel();
			let _ = task.await;
			Err(error)
		},
		Ok(Err(_)) => {
			cancel.cancel();
			let _ = task.await;
			Err(CollabCommandFault::OwnerStopped)
		},
		Err(_) => {
			cancel.cancel();
			let _ = task.await;
			Err(CollabCommandFault::HandshakeTimeout)
		},
	}
}

async fn reconnect(relay: &mut RelayClient, cancel: &CancellationToken) -> bool {
	loop {
		let Ok(delay) = relay.reconnect_delay() else {
			return false;
		};
		tokio::select! {
			() = cancel.cancelled() => return false,
			() = tokio::time::sleep(delay) => {},
		}
		if relay.connect().await.is_ok() {
			return true;
		}
	}
}

async fn send_error(
	relay: &mut RelayClient,
	peer_id: u32,
	code: &'static str,
	message: &'static str,
) {
	let frame = CollabFrame {
		protocol_revision: PROTOCOL_REVISION,
		payload: Some(collab_frame::Payload::Error(ErrorMessage {
			code:    code.to_owned(),
			message: message.to_owned(),
		})),
		..CollabFrame::default()
	};
	let _ = relay.send(RelayRoute { peer_id }, &frame).await;
}

fn welcome_frame(
	sequence: u64,
	read_only: bool,
	total_entry_count: u32,
	peers: &BTreeMap<u32, AuthenticatedPeer>,
) -> CollabFrame {
	Handshake::welcome(sequence, Welcome {
		protocol_revision: PROTOCOL_REVISION,
		header: Some(SessionHeader::default()),
		initial_state: Some(session_state(peers)),
		initial_agents: Some(RegistrySnapshot::default()),
		total_entry_count,
		read_only,
	})
}

fn state_frame(sequence: u64, peers: &BTreeMap<u32, AuthenticatedPeer>) -> CollabFrame {
	CollabFrame {
		protocol_revision: PROTOCOL_REVISION,
		sequence,
		payload: Some(collab_frame::Payload::State(session_state(peers))),
		..CollabFrame::default()
	}
}

fn session_state(peers: &BTreeMap<u32, AuthenticatedPeer>) -> SessionStateUpdate {
	let mut participants = Vec::with_capacity(peers.len() + 1);
	participants.push(Participant {
		display_name: "Host".to_owned(),
		is_host:      true,
		read_only:    false,
		peer_id:      0,
	});
	participants.extend(peers.iter().map(|(&peer_id, peer)| Participant {
		display_name: peer.principal().display_name().to_owned(),
		is_host: false,
		read_only: peer.read_only(),
		peer_id,
	}));
	SessionStateUpdate { participants, ..SessionStateUpdate::default() }
}

fn snapshot_chunks(bytes: &[u8]) -> Vec<Bytes> {
	bytes
		.chunks(SNAPSHOT_CHUNK_BYTES)
		.enumerate()
		.map(|(index, chunk)| {
			Bytes::from(
				serde_json::to_vec(&serde_json::json!({
					"kind": "dom.snapshot.chunk@1",
					"index": index,
					"data": base64_url::encode_raw(chunk).into_string(),
				}))
				.expect("snapshot chunk JSON is infallible"),
			)
		})
		.collect()
}

fn decode_snapshot_chunks(records: &[JournalRecord]) -> Result<Vec<u8>, CollabCommandFault> {
	let mut decoded = Vec::new();
	for (expected, record) in records.iter().enumerate() {
		let value: serde_json::Value = serde_json::from_slice(&record.transcript_v4_json)?;
		if value.get("kind").and_then(serde_json::Value::as_str) != Some("dom.snapshot.chunk@1")
			|| value.get("index").and_then(serde_json::Value::as_u64) != u64::try_from(expected).ok()
		{
			return Err(CollabCommandFault::InvalidSnapshotFragment);
		}
		let data = value
			.get("data")
			.and_then(serde_json::Value::as_str)
			.ok_or(CollabCommandFault::InvalidSnapshotFragment)?;
		let bytes = base64_url::decode_raw(data.as_bytes())
			.into_vec()
			.map_err(|_| CollabCommandFault::InvalidSnapshotFragment)?;
		decoded.extend_from_slice(&bytes);
	}
	Ok(decoded)
}

fn event_record(revision: u64, event: Event) -> Result<JournalRecord, serde_json::Error> {
	let value = match event {
		Event::Patch(patch) => serde_json::json!({"kind": "patch@1", "data": patch}),
		Event::Reset { snapshot } => serde_json::json!({
			"kind": "snapshot@1",
			"data": serde_json::from_slice::<serde_json::Value>(snapshot.as_bytes())?,
		}),
		Event::Stream { cause, sid, op, node, prop, text } => serde_json::json!({
			"kind": "stream@1",
			"cause": cause,
			"sid": sid,
			"op": op,
			"node": node,
			"prop": prop,
			"text": text,
		}),
	};
	Ok(JournalRecord {
		revision,
		transcript_v4_json: Bytes::from(serde_json::to_vec(&value)?),
		visibility_class: VisibilityClass::PublicTranscript as i32,
	})
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum ReplicatedEvent {
	#[serde(rename = "patch@1")]
	Patch { data: omp_dom::Patch },
	#[serde(rename = "snapshot@1")]
	Reset { data: Box<RawValue> },
	#[serde(rename = "stream@1")]
	Stream {
		cause: EntryId,
		sid:   omp_dom::Sid,
		op:    omp_dom::StreamOp,
		node:  Option<omp_dom::Handle>,
		prop:  Option<omp_dom::PropKey>,
		text:  Option<Str>,
	},
}

fn decode_event(record: &JournalRecord) -> Result<Event, CollabCommandFault> {
	let event: ReplicatedEvent = serde_json::from_slice(&record.transcript_v4_json)?;
	Ok(match event {
		ReplicatedEvent::Patch { data } => Event::Patch(data),
		ReplicatedEvent::Reset { data } => {
			Event::Reset { snapshot: Snapshot::from_bytes(data.get().as_bytes())? }
		},
		ReplicatedEvent::Stream { cause, sid, op, node, prop, text } => {
			Event::Stream { cause, sid, op, node, prop, text }
		},
	})
}

fn snapshot_frame(sequence: u64, bytes: Bytes, r#final: bool, watermark: u64) -> CollabFrame {
	CollabFrame {
		protocol_revision: PROTOCOL_REVISION,
		sequence,
		payload: Some(collab_frame::Payload::SnapshotChunk(SnapshotChunk {
			entries: vec![JournalRecord {
				revision:           sequence,
				transcript_v4_json: bytes,
				visibility_class:   VisibilityClass::PublicTranscript as i32,
			}],
			r#final,
			host_revision_watermark: watermark,
		})),
		..CollabFrame::default()
	}
}

fn live_record_frame(sequence: u64, record: JournalRecord) -> CollabFrame {
	CollabFrame {
		protocol_revision: PROTOCOL_REVISION,
		sequence,
		payload: Some(collab_frame::Payload::JournalRecord(record)),
		..CollabFrame::default()
	}
}

/// Starts the native relay-backed command owner.
#[must_use]
pub fn spawn_session_owner(authority: CollabSessionAuthority) -> JoinHandle<()> {
	tokio::spawn(authority.run())
}

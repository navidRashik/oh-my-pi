//! Durable exploration checkpoint creation and turn-boundary rewind scheduling.

use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use omp_core::{Str, sf};
use omp_tool::{
	Abort, ArgIssue, ArgIssueKind, CommitError, Constraint, DocEffects, Effects, Ev, IncomingParams,
	ParamError, Part, PromptCaps, Rev, Tool, ToolSpec, ToolTerminal,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

/// Environment bridge to the active Agent Journal and its boundary command
/// queue. Rewind must enqueue, never mutate the journal inline.
pub trait CheckpointControl: Clone + Send + Sync + 'static {
	/// Activates one durable checkpoint and returns an opaque session token.
	fn checkpoint(
		&self,
		goal: Str,
		cancel: CancellationToken,
	) -> impl Future<Output = Result<CheckpointAck, CheckpointFault>> + Send;

	/// Restores the captured workspace and schedules rewind only after the
	/// document authority reports a complete commit.
	fn schedule_rewind(
		&self,
		report: Str,
		cancel: CancellationToken,
	) -> impl Future<Output = Result<RewindAck, CheckpointFault>> + Send;
}

/// Typed workspace generation captured with an exploration checkpoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceSnapshot {
	/// Content-addressed manifest identity.
	pub snapshot_id: Str,
	/// Canonical environment-owned root URI.
	pub root_uri:    Str,
	/// Monotonic workspace generation.
	pub generation:  u64,
	/// Stable tree digest.
	pub tree_hash:   Str,
	/// Captured regular-file count.
	pub files:       u64,
	/// Captured content bytes.
	pub bytes:       u64,
}

/// Typed workspace restoration committed before the journal branches.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceRestore {
	/// Restored checkpoint manifest.
	pub snapshot_id:      Str,
	/// Pre-restore generation retained for recovery.
	pub undo_snapshot_id: Str,
	/// Files created or replaced through document transactions.
	pub written:          u64,
	/// Files deleted through document transactions.
	pub deleted:          u64,
	/// Files already equal to the checkpoint.
	pub unchanged:        u64,
	/// Generation observed before restoration.
	pub from_generation:  u64,
	/// Generation committed after restoration.
	pub to_generation:    u64,
}

/// Authoritative checkpoint activation acknowledgement.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CheckpointAck {
	/// Opaque session-owned checkpoint token.
	pub token:      Str,
	/// Checkpoint creation time in epoch milliseconds.
	pub started_at: u64,
	/// Environment-owned workspace generation paired with the journal point.
	pub workspace:  WorkspaceSnapshot,
}

/// Stable checkpoint-domain failure returned by the active agent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CheckpointFault {
	/// Machine-readable failure class.
	pub code:    FaultCode,
	/// Stable user-facing guidance.
	pub message: Str,
}

/// Authoritative enqueue acknowledgement.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RewindAck {
	/// Opaque checkpoint token accepted by the active session.
	pub token:     Str,
	/// Agent-issued durable command or receipt identifier.
	pub receipt:   Str,
	/// Fully committed workspace restoration.
	pub workspace: WorkspaceRestore,
}

/// Checkpoint creation arguments.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointParams {
	/// Goal of the speculative exploration branch.
	pub goal: Str,
}

/// Rewind scheduling arguments for `rewind@3`.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RewindParams {
	/// Findings retained after the active exploration branch is discarded.
	pub report: Str,
}

/// Durable checkpoint token.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CheckpointPayload {
	/// Opaque token accepted only by this session.
	pub token:      Str,
	/// Goal recorded on the durable entry.
	pub goal:       Str,
	/// Checkpoint creation time in epoch milliseconds.
	pub started_at: u64,
	/// Typed workspace generation retained by the environment authority.
	pub workspace:  WorkspaceSnapshot,
}

/// Scheduled rewind receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RewindPayload {
	/// Validated opaque checkpoint token.
	pub token:     Str,
	/// Findings retained with the rewind command.
	pub report:    Str,
	/// Agent-issued command receipt identifier.
	pub receipt:   Str,
	/// Fully committed workspace restoration.
	pub workspace: WorkspaceRestore,
	/// Stable settlement verdict.
	pub scheduled: bool,
}

/// Checkpoint tools do not stream updates.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Update {}

/// Stable checkpoint failure class.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultCode {
	/// A checkpoint is already active.
	AlreadyActive,
	/// No checkpoint has been created.
	NoActive,
	/// The most recent checkpoint already completed.
	AlreadyCompleted,
	/// The supplied token belongs to another session or checkpoint.
	WrongToken,
	/// The report is empty after trimming.
	EmptyReport,
	/// A rewind is already queued.
	AlreadyScheduled,
	/// Workspace capture failed before the checkpoint was activated.
	SnapshotFailed,
	/// Dirty documents or another workspace transition blocked restoration.
	RestoreConflict,
	/// The caller cancelled workspace capture or restoration.
	RestoreCancelled,
	/// Workspace restoration failed or partially committed.
	RestoreFailed,
	/// The active agent control bridge failed.
	Control,
}

/// Journal bridge or checkpoint validation failure.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct Fault {
	code:    FaultCode,
	message: Str,
}

/// Creates durable checkpoint entries.
pub struct Checkpoint<C> {
	control: C,
	spec:    ToolSpec,
}
/// Schedules a boundary rewind to a durable checkpoint token.
pub struct Rewind<C> {
	control: C,
	spec:    ToolSpec,
}

/// Creates the paired tools over one active-agent bridge.
pub fn tools<C: CheckpointControl>(control: C) -> (Checkpoint<C>, Rewind<C>) {
	let checkpoint = Checkpoint {
		control: control.clone(),
		spec:    spec(
			"checkpoint",
			"Creates a durable exploration checkpoint and captures the environment-owned workspace \
			 generation before returning its opaque session token.",
			omp_tool::schema::<CheckpointParams>(),
			2,
			Effects {
				documents: Some(DocEffects { read: true, write_globs: Arc::<[Str]>::from([]) }),
				..Effects::empty()
			},
		),
	};
	let rewind = Rewind {
		control,
		spec: spec(
			"rewind",
			"Restores the checkpoint workspace through document authority, then schedules journal \
			 rewind at the next turn boundary while retaining the exploration findings report.",
			omp_tool::schema::<RewindParams>(),
			3,
			Effects {
				documents: Some(DocEffects {
					read:        true,
					write_globs: Arc::from([Str::new_static("**")]),
				}),
				..Effects::empty()
			},
		),
	};
	(checkpoint, rewind)
}

fn spec(
	name: &'static str,
	description: &'static str,
	schema: bytes::Bytes,
	revision: u16,
	effects: Effects,
) -> ToolSpec {
	ToolSpec {
		name: sf!(name),
		rev: Rev { family: Default::default(), n: revision },
		description: sf!(description),
		schema,
		constraint: Constraint::Schema {
			priority:       255,
			on_unsupported: omp_tool::Fallback::Unspecified,
		},
		effects,
		projection_code: omp_tool::native_projection_code(
			env!("CARGO_PKG_NAME"),
			env!("CARGO_PKG_VERSION"),
			include_bytes!("checkpoint.rs"),
		)
		.into(),
	}
}

impl<C: CheckpointControl> Tool for Checkpoint<C> {
	type Fault = Fault;
	type Params = CheckpointParams;
	type Payload = CheckpointPayload;
	type Update = Update;

	fn spec(&self) -> &ToolSpec {
		&self.spec
	}

	fn call<'c>(
		&'c self,
		mut incoming: IncomingParams<'c>,
	) -> impl Stream<Item = Ev<Update, CheckpointPayload, Fault>> + Send + 'c {
		stream! {
			let params = match incoming.whole::<CheckpointParams>().await { Ok(value) => value, Err(error) => { yield param_event(error); return; } };
			if params.goal.trim().is_empty() {
				yield done_checkpoint(Err(fault(FaultCode::Control, "goal must not be empty")));
				return;
			}
			if let Err(error) = incoming.interruptable().committed().await { yield commit_checkpoint(error); return; }
			let goal = params.goal;
			let cancellation = CancellationToken::new();
			let execution = self.control.checkpoint(goal.clone(), cancellation.clone());
			tokio::pin!(execution);
			tokio::select! {
				result = &mut execution => {
					let result = result
						.map(|ack| CheckpointPayload {
							token: ack.token,
							goal,
							started_at: ack.started_at,
							workspace: ack.workspace,
						})
						.map_err(|fault| Fault { code: fault.code, message: fault.message });
					yield done_checkpoint(result);
				},
				interrupt = incoming.next_interrupt() => {
					cancellation.cancel();
					let _ = execution.await;
					yield interrupted_event(interrupt);
				},
			}
		}
	}

	fn prompt(&self, view: Result<&CheckpointPayload, &Fault>, _: &PromptCaps) -> Vec<Part> {
		vec![Part::Text {
			text: match view {
				Ok(payload) => {
					sf!("Checkpoint {} created for: {}", payload.token, payload.goal)
				},
				Err(fault) => fault.message.clone(),
			},
		}]
	}
}

impl<C: CheckpointControl> Tool for Rewind<C> {
	type Fault = Fault;
	type Params = RewindParams;
	type Payload = RewindPayload;
	type Update = Update;

	fn spec(&self) -> &ToolSpec {
		&self.spec
	}

	fn call<'c>(
		&'c self,
		mut incoming: IncomingParams<'c>,
	) -> impl Stream<Item = Ev<Update, RewindPayload, Fault>> + Send + 'c {
		stream! {
			let params = match incoming.whole::<RewindParams>().await { Ok(value) => value, Err(error) => { yield param_event(error); return; } };
			if params.report.trim().is_empty() {
				yield done_rewind(Err(fault(FaultCode::EmptyReport, "report must not be empty")));
				return;
			}
			if let Err(error) = incoming.interruptable().committed().await { yield commit_rewind(error); return; }
			let report = params.report;
			let cancellation = CancellationToken::new();
			let execution = self.control.schedule_rewind(report.clone(), cancellation.clone());
			tokio::pin!(execution);
			tokio::select! {
				result = &mut execution => {
					let result = result
						.map(|ack| RewindPayload {
							token: ack.token,
							report,
							receipt: ack.receipt,
							workspace: ack.workspace,
							scheduled: true,
						})
						.map_err(|fault| Fault { code: fault.code, message: fault.message });
					yield done_rewind(result);
				},
				interrupt = incoming.next_interrupt() => {
					cancellation.cancel();
					match execution.await {
						Ok(ack) => {
							yield done_rewind(Ok(RewindPayload {
								token: ack.token,
								report,
								receipt: ack.receipt,
								workspace: ack.workspace,
								scheduled: true,
							}));
						},
						Err(_) => yield interrupted_event(interrupt),
					}
				},
			}
		}
	}

	fn prompt(&self, view: Result<&RewindPayload, &Fault>, _: &PromptCaps) -> Vec<Part> {
		vec![Part::Text {
			text: match view {
				Ok(payload) => sf!(
					"Workspace restored ({} written, {} deleted, {} unchanged); rewind to checkpoint \
					 {} scheduled at turn boundary (receipt {}).",
					payload.workspace.written,
					payload.workspace.deleted,
					payload.workspace.unchanged,
					payload.token,
					payload.receipt
				),
				Err(fault) => fault.message.clone(),
			},
		}]
	}
}

const fn fault(code: FaultCode, message: &'static str) -> Fault {
	Fault { code, message: sf!(message) }
}
const fn done_checkpoint(
	result: Result<CheckpointPayload, Fault>,
) -> Ev<Update, CheckpointPayload, Fault> {
	Ev::Done(ToolTerminal::Done { result, useless: false })
}
const fn done_rewind(result: Result<RewindPayload, Fault>) -> Ev<Update, RewindPayload, Fault> {
	Ev::Done(ToolTerminal::Done { result, useless: false })
}
fn param_event<P>(error: ParamError) -> Ev<Update, P, Fault> {
	match error {
		ParamError::Args(issue) => Ev::Args(*issue),
		ParamError::Interrupted(interrupt) => {
			Ev::Aborted(Abort::Interrupted { reason: interrupt.reason })
		},
		ParamError::Protocol(message) => Ev::Args(protocol_issue(message)),
	}
}
fn commit_checkpoint(error: CommitError) -> Ev<Update, CheckpointPayload, Fault> {
	commit_event(error)
}
fn commit_rewind(error: CommitError) -> Ev<Update, RewindPayload, Fault> {
	commit_event(error)
}
fn commit_event<P>(error: CommitError) -> Ev<Update, P, Fault> {
	match error {
		CommitError::Aborted => Ev::Aborted(Abort::InputDropped),
		CommitError::Interrupted(interrupt) => {
			Ev::Aborted(Abort::Interrupted { reason: interrupt.reason })
		},
		CommitError::Protocol(message) => Ev::Args(protocol_issue(message)),
	}
}
fn interrupted_event<P>(
	interrupt: Result<omp_tool::Interrupt, omp_tool::InterruptWaitError>,
) -> Ev<Update, P, Fault> {
	match interrupt {
		Ok(interrupt) => Ev::Aborted(Abort::Interrupted { reason: interrupt.reason }),
		Err(_) => Ev::Aborted(Abort::InputDropped),
	}
}

fn protocol_issue(message: Str) -> ArgIssue {
	ArgIssue {
		path:     Vec::new(),
		expected: sf!("one committed JSON argument object"),
		kind:     ArgIssueKind::Protocol,
		example:  None,
		found:    Some(message),
	}
}

#[cfg(test)]
mod tests {
	use std::{
		future,
		sync::{Arc, Mutex},
	};

	use futures::StreamExt as _;

	use super::*;

	fn snapshot() -> WorkspaceSnapshot {
		WorkspaceSnapshot {
			snapshot_id: sf!("snapshot"),
			root_uri:    sf!("file:///workspace"),
			generation:  7,
			tree_hash:   sf!("tree"),
			files:       2,
			bytes:       12,
		}
	}

	fn restore() -> WorkspaceRestore {
		WorkspaceRestore {
			snapshot_id:      sf!("snapshot"),
			undo_snapshot_id: sf!("undo"),
			written:          1,
			deleted:          1,
			unchanged:        0,
			from_generation:  8,
			to_generation:    9,
		}
	}

	#[derive(Clone)]
	struct Control;
	impl CheckpointControl for Control {
		fn checkpoint(
			&self,
			_: Str,
			_: CancellationToken,
		) -> impl Future<Output = Result<CheckpointAck, CheckpointFault>> + Send {
			future::ready(Ok(CheckpointAck {
				token:      sf!("opaque"),
				started_at: 42,
				workspace:  snapshot(),
			}))
		}

		fn schedule_rewind(
			&self,
			_: Str,
			_: CancellationToken,
		) -> impl Future<Output = Result<RewindAck, CheckpointFault>> + Send {
			future::ready(Ok(RewindAck {
				token:     sf!("opaque"),
				receipt:   sf!("rewind-1"),
				workspace: restore(),
			}))
		}
	}

	#[derive(Clone, Default)]
	struct RecordingControl(Arc<Mutex<Option<Str>>>);

	impl CheckpointControl for RecordingControl {
		fn checkpoint(
			&self,
			_: Str,
			_: CancellationToken,
		) -> impl Future<Output = Result<CheckpointAck, CheckpointFault>> + Send {
			future::ready(Ok(CheckpointAck {
				token:      sf!("opaque"),
				started_at: 42,
				workspace:  snapshot(),
			}))
		}

		fn schedule_rewind(
			&self,
			report: Str,
			_: CancellationToken,
		) -> impl Future<Output = Result<RewindAck, CheckpointFault>> + Send {
			self.0.lock().expect("recording control").replace(report);
			future::ready(Ok(RewindAck {
				token:     sf!("opaque"),
				receipt:   sf!("rewind-1"),
				workspace: restore(),
			}))
		}
	}

	#[test]
	fn pair_has_distinct_canonical_slots() {
		let (checkpoint, rewind) = tools(Control);
		assert_eq!(checkpoint.spec().name, "checkpoint");
		assert_eq!(rewind.spec().name, "rewind");
		assert_eq!(checkpoint.spec().rev.n, 2);
		assert_eq!(rewind.spec().rev.n, 3);
		assert_eq!(
			checkpoint
				.spec()
				.effects
				.documents
				.as_ref()
				.map(|value| value.read),
			Some(true)
		);
		assert_eq!(
			rewind
				.spec()
				.effects
				.documents
				.as_ref()
				.map(|value| value.write_globs.as_ref()),
			Some([Str::new_static("**")].as_slice())
		);
	}

	#[test]
	fn rewind_schema_is_exactly_report_only() {
		let (_, rewind) = tools(Control);
		let schema: serde_json::Value =
			serde_json::from_slice(&rewind.spec().schema).expect("rewind schema");
		assert_eq!(schema["additionalProperties"], false);
		assert_eq!(schema["required"], serde_json::json!(["i", "report"]));
		assert_eq!(
			schema["properties"]
				.as_object()
				.expect("properties")
				.keys()
				.map(String::as_str)
				.collect::<std::collections::BTreeSet<_>>(),
			["i", "notrunc", "report"].into_iter().collect()
		);
		assert_eq!(schema["properties"]["report"]["type"], "string");
	}

	#[tokio::test]
	async fn rewind_routes_report_to_the_active_checkpoint_control() {
		let control = RecordingControl::default();
		let (_, rewind) = tools(control.clone());
		let raw = r#"{"report":"keep this finding"}"#;
		let (feed, incoming) = IncomingParams::channel();
		feed.arg_text(raw.into()).expect("stream args");
		feed.args_committed(raw.into()).expect("commit args");
		let events = rewind.call(incoming).collect::<Vec<_>>().await;
		assert!(matches!(
			events.last(),
			Some(Ev::Done(ToolTerminal::Done { result: Ok(payload), .. }))
				if payload.token == "opaque" && payload.report == "keep this finding"
		));
		assert_eq!(
			control.0.lock().expect("recording control").as_deref(),
			Some("keep this finding")
		);
	}

	#[test]
	fn argument_contracts_are_closed() {
		assert!(
			serde_json::from_value::<CheckpointParams>(
				serde_json::json!({"goal":"inspect","extra":true})
			)
			.is_err()
		);
		assert!(
			serde_json::from_value::<RewindParams>(
				serde_json::json!({"token":"opaque","report":"finding"})
			)
			.is_err()
		);
		assert!(
			serde_json::from_value::<RewindParams>(serde_json::json!({"report":"finding"})).is_ok()
		);
	}
}

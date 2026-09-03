//! Typed JSON payloads for the closed revision-1 kind set.

use omp_core::Str;
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::value::RawValue;
use strum::{Display, EnumString, IntoStaticStr};

use crate::{EntryId, blob::BlobRef};

/// `journal@1` genesis payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genesis {
	/// Journal format version.
	pub version: u32,
	/// Session working directory.
	pub cwd:     Str,
	/// Creation time in the controller's canonical representation.
	pub created: Str,
}

/// `turn.start@1` payload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnStart {}

/// One user attachment in a `msg.user@1` payload.
///
/// The content-addressed bytes plus the media type pi's
/// `ImageContent.mimeType` carries, so the projection can hand providers a
/// typed media part without reopening the blob. Serialized flat beside the
/// reference: `{"h","n","mime"}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
	/// Content-addressed bytes (digest + byte length).
	#[serde(flatten)]
	pub blob: BlobRef,
	/// Declared media type (`image/png`, …).
	pub mime: Str,
}

/// `msg.user@1` payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsgUser {
	/// User-authored text.
	pub text:        Str,
	/// Attached media, positional: `[Image #N]` in `text` names
	/// `attachments[N-1]`.
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub attachments: Vec<Attachment>,
}

/// One settled job in the typed payload of a journaled
/// `<user async_result=true>` patch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsyncJobDelivery {
	/// Stable job identity.
	pub id:          Str,
	/// User-facing execution type (`bash`, `task`, `eval`, or another tool).
	#[serde(rename = "type")]
	pub job_type:    Str,
	/// Work label captured when the job started.
	pub label:       Str,
	/// Exact elapsed wall time captured when the job settled.
	pub duration_ms: u64,
	/// Terminal lifecycle status.
	pub status:      AsyncJobStatus,
	/// Full-output artifact when the completed result was spilled.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub artifact:    Option<Str>,
	/// Stable terminal diagnostic for a failed or cancelled job.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub fault:       Option<Str>,
}

/// Terminal state carried by a background-job delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum AsyncJobStatus {
	/// The job produced its result successfully.
	Completed,
	/// The job settled with an error.
	Failed,
	/// The job was cancelled before completion.
	Cancelled,
}

/// Replay-stable typed payload of a journaled `<user async_result=true>` patch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsyncResult {
	/// Jobs delivered together, oldest first.
	pub jobs: Vec<AsyncJobDelivery>,
}

/// One supervised-process completion carried by a journaled
/// `<user launch_completion=true>` patch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchDaemonCompletion {
	/// Stable daemon name supplied to `hub start`.
	pub name:        Str,
	/// Terminal success/failure classification.
	pub status:      LaunchDaemonStatus,
	/// Process exit code, absent when launch or supervision failed before exit.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub exit_code:   Option<i32>,
	/// Exact elapsed wall time captured by the process supervisor.
	pub duration_ms: u64,
	/// Typed terminal fault, absent for successful completion.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub fault:       Option<LaunchDaemonFault>,
}

/// Terminal status of a supervised process completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum LaunchDaemonStatus {
	/// The process exited successfully.
	Completed,
	/// The process failed, was denied, timed out, or was cancelled.
	Failed,
}

/// Typed reason a supervised process did not complete successfully.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchDaemonFault {
	/// Stable failure classification.
	pub kind:    LaunchDaemonFaultKind,
	/// Supervisor diagnostic, when one is available.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub message: Option<Str>,
	/// Terminating signal, when one was reported.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub signal:  Option<Str>,
}

/// Stable supervised-process failure classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum LaunchDaemonFaultKind {
	/// The process reported a failed execution outcome.
	Failed,
	/// The process exceeded its configured timeout.
	Timeout,
	/// The process was cancelled or explicitly stopped.
	Cancelled,
	/// Host policy denied process execution.
	Denied,
	/// The supervisor itself could not launch or monitor the process.
	Supervisor,
}

/// Replay-stable typed payload for one or more supervised-process completions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchCompletion {
	/// Terminal daemon rows, in delivery order.
	pub daemons: Vec<LaunchDaemonCompletion>,
}

/// Direction of a replay-stable IRC transcript observation.
#[derive(
	Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum IrcDirection {
	/// A peer message received by this session.
	Incoming,
	/// A reply emitted automatically on this session's behalf.
	Autoreply,
	/// An observation of traffic relayed between two other agents.
	Relay,
	/// A work-pool assignment or dispatch observation.
	Workpool,
}

/// Typed payload of a journaled IRC transcript observation.
///
/// The controller records this payload on a `<notice kind=irc>` patch. The
/// message body is duplicated as the element content so generic fallback and
/// copy remain useful even when a future actor does not understand this revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IrcTraffic {
	/// Direction-specific presentation kind.
	pub direction:    IrcDirection,
	/// Sending peer, when the observation has one.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub from:         Option<Str>,
	/// Receiving peer, when the observation has one.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub to:           Option<Str>,
	/// Message body.
	pub body:         Str,
	/// Stable identity of the message this replies to.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub reply_to:     Option<Str>,
	/// Work-pool identity for a pool dispatch.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub pool:         Option<Str>,
	/// Work-pool scheduling mode.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub mode:         Option<Str>,
	/// Producer-observed Unix timestamp in milliseconds.
	pub timestamp_ms: u64,
}

/// `msg.assistant.start@1` payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsgAssistantStart {
	/// Requested model identifier.
	pub model:    Str,
	/// Provider identifier.
	pub provider: Str,
	/// Resolved route identifier.
	pub route:    Str,
}

/// Operation carried by a `stream@1` entry.
#[derive(
	Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum StreamOp {
	/// Bind a new stream id to a node property.
	Open,
	/// Append a text delta.
	Append,
	/// Close the stream id.
	Close,
}

/// `stream@1` payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stream {
	/// Session-local stream identity.
	pub sid:  u32,
	/// Stream operation.
	pub op:   StreamOp,
	/// DOM handle bound by an open operation.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub node: Option<u64>,
	/// DOM property bound by an open operation.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub prop: Option<Str>,
	/// Text carried by an append operation.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub text: Option<Str>,
}

/// `msg.assistant.end@1` payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsgAssistantEnd {
	/// Provider stop reason.
	pub stop_reason: Str,
}

/// `tool.call@1` payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
	/// Tool name.
	pub name:    Str,
	/// Tool contract revision.
	pub rev:     u32,
	/// Provider/tool-loop call identity.
	pub call_id: Str,
	/// Model-supplied call intent.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub i:       Option<Str>,
	/// Complete arguments, when they did not arrive through a stream.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub args:    Option<Box<RawValue>>,
	/// Argument stream identity, when arguments arrive incrementally.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub sid:     Option<u32>,
}

/// `tool.update@1` payload: the tool's own typed update JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolUpdate(pub Box<RawValue>);

/// `tool.result@1` terminal payload.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum ToolResult {
	/// Successful terminal payload.
	Outcome {
		/// Tool-defined outcome JSON.
		outcome:      Box<RawValue>,
		/// Durable model-facing projection produced by the exact tool revision.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		prompt_parts: Option<Box<RawValue>>,
		/// Environment-provided outcome artifact adopted into the session CAS.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		source_blob:  Option<BlobRef>,
	},
	/// Failed terminal payload.
	Fault {
		/// Tool-defined fault JSON.
		fault:        Box<RawValue>,
		/// Durable model-facing projection produced by the exact tool revision.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		prompt_parts: Option<Box<RawValue>>,
		/// Environment-provided outcome artifact adopted into the session CAS.
		#[serde(default, skip_serializing_if = "Option::is_none")]
		source_blob:  Option<BlobRef>,
	},
}

impl<'de> Deserialize<'de> for ToolResult {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[derive(Deserialize)]
		#[serde(deny_unknown_fields)]
		struct Wire {
			#[serde(default, deserialize_with = "deserialize_present_raw")]
			outcome:      Option<Box<RawValue>>,
			#[serde(default, deserialize_with = "deserialize_present_raw")]
			fault:        Option<Box<RawValue>>,
			#[serde(default)]
			prompt_parts: Option<Box<RawValue>>,
			#[serde(default)]
			source_blob:  Option<BlobRef>,
		}

		fn deserialize_present_raw<'de, D>(deserializer: D) -> Result<Option<Box<RawValue>>, D::Error>
		where
			D: Deserializer<'de>,
		{
			Box::<RawValue>::deserialize(deserializer).map(Some)
		}

		let wire = Wire::deserialize(deserializer)?;
		match (wire.outcome, wire.fault) {
			(Some(outcome), None) => Ok(Self::Outcome {
				outcome,
				prompt_parts: wire.prompt_parts,
				source_blob: wire.source_blob,
			}),
			(None, Some(fault)) => Ok(Self::Fault {
				fault,
				prompt_parts: wire.prompt_parts,
				source_blob: wire.source_blob,
			}),
			_ => Err(de::Error::custom("tool result must contain exactly one of outcome or fault")),
		}
	}
}

/// The inference role which produced a receipt.
#[derive(Clone, Copy, Debug, Default, IntoStaticStr, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ReceiptRole {
	/// The primary model serving the user-visible turn.
	#[default]
	Primary,
	/// The auxiliary advisor reviewing the primary model's work.
	Advisor,
}

/// Credential-free identity of the inference which produced a receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptIdentity {
	/// Semantic role of the inference.
	pub role:     ReceiptRole,
	/// Concrete serving provider.
	pub provider: Str,
	/// Concrete serving model.
	pub model:    Str,
}

/// `turn.receipt@1` payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnReceipt {
	/// Input token count.
	pub tokens_in:                   u64,
	/// Output token count.
	pub tokens_out:                  u64,
	/// Cost in billionths of a US dollar.
	pub cost_nano_usd:               u64,
	/// Prompt-cache tokens read; absent in journals written before the field
	/// existed.
	#[serde(default, skip_serializing_if = "is_zero")]
	pub cache_read:                  u64,
	/// Prompt-cache tokens written.
	#[serde(default, skip_serializing_if = "is_zero")]
	pub cache_write:                 u64,
	/// Milliseconds from request start to the first streamed token, measured
	/// on the kernel's clock.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub ttft_ms:                     Option<u64>,
	/// Milliseconds from request start to completion.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub duration_ms:                 Option<u64>,
	/// Provider premium-request units billed for this request in millionths
	/// (`1_000_000` = one premium request; GitHub Copilot
	/// `premium_interactions`, fractional for discounted models); zero for
	/// every other route.
	#[serde(default, skip_serializing_if = "is_zero")]
	pub premium_requests_millionths: u64,
	/// Credential-free serving identity for auxiliary inference. Primary
	/// receipts written before this field existed remain `None`.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub identity:                    Option<ReceiptIdentity>,
}

impl TurnReceipt {
	/// A receipt carrying only token and cost totals.
	#[must_use]
	pub const fn tokens(tokens_in: u64, tokens_out: u64, cost_nano_usd: u64) -> Self {
		Self {
			tokens_in,
			tokens_out,
			cost_nano_usd,
			cache_read: 0,
			cache_write: 0,
			ttft_ms: None,
			duration_ms: None,
			premium_requests_millionths: 0,
			identity: None,
		}
	}
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde skip predicate signature")]
const fn is_zero(value: &u64) -> bool {
	*value == 0
}

/// `patch@1` payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Patch {
	/// Serialized array of DOM operations; `omp-dom` owns their Rust type.
	pub ops: Box<RawValue>,
}

/// `compaction@1` payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compaction {
	/// Content-addressed summary.
	pub summary:       BlobRef,
	/// Last entry hidden by the summary.
	pub boundary:      EntryId,
	/// Maintenance method that produced the summary (`auto`, `remote`, `soft`,
	/// `handoff`, `snapcompact`, `shake`, `branch`); absent for legacy
	/// entries, which render as plain compaction.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub method:        Option<Str>,
	/// Estimated context tokens before the step.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub tokens_before: Option<u64>,
	/// Estimated context tokens after the step.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub tokens_after:  Option<u64>,
	/// Dead-end warning stamped by a progress guard.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub warning:       Option<Str>,
}

impl Compaction {
	/// A compaction with no method or token facts.
	#[must_use]
	pub const fn new(summary: BlobRef, boundary: EntryId) -> Self {
		Self {
			summary,
			boundary,
			method: None,
			tokens_before: None,
			tokens_after: None,
			warning: None,
		}
	}
}

#[cfg(test)]
mod tests {
	use omp_core::Hash32;

	use super::*;

	#[test]
	fn user_attachment_serializes_flat_beside_its_blob_reference() {
		let payload = MsgUser {
			text:        Str::new_static("look [Image #1]"),
			attachments: vec![Attachment {
				blob: BlobRef { hash: Hash32::new([0xab; 32]), size: 5 },
				mime: Str::new_static("image/png"),
			}],
		};
		let json = serde_json::to_string(&payload).unwrap();
		assert_eq!(
			json,
			concat!(
				r#"{"text":"look [Image #1]","attachments":[{"h":""#,
				"abababababababababababababababababababababababababababababababab",
				r#"","n":5,"mime":"image/png"}]}"#
			)
		);
		assert_eq!(serde_json::from_str::<MsgUser>(&json).unwrap(), payload);
		let bare: MsgUser = serde_json::from_str(r#"{"text":"hi"}"#).unwrap();
		assert!(bare.attachments.is_empty());
	}

	#[test]
	fn receipt_identity_is_optional_and_round_trips_advisor_billing() {
		let legacy: TurnReceipt =
			serde_json::from_str(r#"{"tokens_in":1,"tokens_out":2,"cost_nano_usd":3}"#)
				.expect("legacy receipt");
		assert_eq!(legacy.identity, None);

		let advisor = TurnReceipt {
			cost_nano_usd: 80_000_000,
			identity: Some(ReceiptIdentity {
				role:     ReceiptRole::Advisor,
				provider: Str::new_static("anthropic"),
				model:    Str::new_static("claude-sonnet-4-5"),
			}),
			..TurnReceipt::default()
		};
		let json = serde_json::to_string(&advisor).expect("advisor receipt");
		assert!(json.contains(
			r#""identity":{"role":"advisor","provider":"anthropic","model":"claude-sonnet-4-5"}"#
		));
		assert_eq!(serde_json::from_str::<TurnReceipt>(&json).expect("round trip"), advisor);
	}

	#[test]
	fn irc_traffic_round_trips_every_directional_fact() {
		let payload = IrcTraffic {
			direction:    IrcDirection::Workpool,
			from:         Some(Str::new_static("scheduler")),
			to:           Some(Str::new_static("Scout")),
			body:         Str::new_static("inspect the parser"),
			reply_to:     Some(Str::new_static("01K4A")),
			pool:         Some(Str::new_static("audit")),
			mode:         Some(Str::new_static("parallel")),
			timestamp_ms: 1_777_777_777_000,
		};
		let json = serde_json::to_string(&payload).expect("traffic serializes");
		assert_eq!(
			serde_json::from_str::<IrcTraffic>(&json).expect("traffic decodes"),
			payload
		);
		assert!(json.contains(r#""direction":"workpool""#));
		assert!(json.contains(r#""reply_to":"01K4A""#));
		assert!(
			serde_json::from_str::<IrcTraffic>(&json.replace(
				"}",
				r#","untyped":"discard me"}"#,
			))
			.is_err()
		);
	}

	#[test]
	fn launch_completion_round_trips_typed_terminal_facts() {
		let payload = LaunchCompletion {
			daemons: vec![LaunchDaemonCompletion {
				name:        Str::new_static("web"),
				status:      LaunchDaemonStatus::Failed,
				exit_code:   Some(17),
				duration_ms: 2_500,
				fault:       Some(LaunchDaemonFault {
					kind:    LaunchDaemonFaultKind::Failed,
					message: Some(Str::new_static("readiness process exited")),
					signal:  Some(Str::new_static("SIGTERM")),
				}),
			}],
		};
		let json = serde_json::to_string(&payload).expect("completion serializes");
		assert_eq!(
			serde_json::from_str::<LaunchCompletion>(&json).expect("completion decodes"),
			payload
		);
		assert!(
			serde_json::from_str::<LaunchCompletion>(r#"{"daemons":[],"untyped":"discard me"}"#)
				.is_err()
		);
	}
}

//! Production voice backend for `omp chat`.
//!
//! Push-to-talk owns local capture and recognition behind the composer's
//! space-hold gesture. `/live` leases the canonical Codex OAuth generation,
//! negotiates the native WebRTC peer plus authenticated sideband, and projects
//! only typed observer events back through the chat mailbox. The chat actor
//! never owns audio, credentials, transport, or controller lifecycle.

#[cfg(feature = "local-stt")]
use std::sync::atomic::AtomicUsize;
use std::{
	future::Future,
	sync::{
		Arc,
		atomic::{AtomicBool, AtomicU8, Ordering},
	},
	time::Duration,
};

use omp_chat::{
	HostAction, HostMailbox, SttFailureKind, SttUiEvent,
	overlays::live::{
		LiveControl, LiveDevice, LivePhase, LiveTranscript, LiveTranscriptRole, LiveUiEvent,
		MicrophonePermission as LiveMicrophonePermission, level_percent,
	},
};
use omp_con::Ctx;
use omp_core::Str;
use omp_inference::{
	answer::{RealtimeEvent, RealtimePhase},
	auth::{AuthManager, CodexLiveCredentialError},
};
#[cfg(feature = "local-stt")]
use omp_voice::vad::{EndpointerEvent, StreamEndpointer};
use omp_voice::{
	VoiceError,
	audio::CaptureStream,
	device::{
		self, AudioDevice, DeviceSnapshot, DeviceWatcher,
		MicrophonePermission as DeviceMicrophonePermission,
	},
	live::{LiveCallbacks, LiveMediaSession},
	transport::{
		DirectSidebandConnector, EventDeduplicator, LiveClientMessage, LiveDelegationAdmission,
		LiveDelegationBridge, LiveDelegationSettlement, LiveDelegationTerminal, LiveOAuthAccess,
		LiveServerEvent, LiveSignalingClient, LiveSignalingRequest, LiveSignalingResponse,
		LiveTransportOptions, LiveTurnRole, complete_live_transport, parse_live_server_event,
		receive_sideband, send_sideband,
	},
};
use parking_lot::Mutex;
use thiserror::Error;

use crate::{
	audio_coordinator::InteractiveAudioController,
	voice::settings::{
		CL_LIVE_INPUT_DEVICE, CL_LIVE_OUTPUT_DEVICE, CL_LIVE_VOICE, CL_STT_LANGUAGE, CL_STT_MODEL,
		CL_STT_SUBMIT_TRIGGER, CL_VOICE_STT_ENABLED, LiveVoice, SttModel, SttSubmitTrigger,
	},
};

/// Mono capture rate the local recognizers consume.
#[cfg(feature = "local-stt")]
const SAMPLE_RATE: u32 = 16_000;
/// Captured audio waiting for the streaming endpointer is bounded to roughly
/// twenty seconds at the native callback cadence.
#[cfg(feature = "local-stt")]
const STT_AUDIO_QUEUE_DEPTH: usize = 1024;
/// A single dictation cannot retain more than five minutes of mono samples.
#[cfg(feature = "local-stt")]
const MAX_STT_SAMPLES: usize = SAMPLE_RATE as usize * 60 * 5;
const STT_STATE_SETUP: u8 = 0;
const STT_STATE_RECORDING: u8 = 1;
const STT_STATE_TRANSCRIBING: u8 = 2;
const MAX_LIVE_SDP_BYTES: usize = 1024 * 1024;
const LIVE_CLOSE_TIMEOUT: Duration = Duration::from_millis(500);
const LIVE_INSTRUCTIONS: &str = "You are omp Live, the realtime voice surface of one unified \
                                 coding assistant. Respond directly, briefly, conversationally, \
                                 and without markdown. Delegate repository work, coding, tools, \
                                 commands, and verification to the client backend; never claim \
                                 results before the backend reports them.";
#[derive(Debug, Error)]
#[cfg_attr(
	not(feature = "local-stt"),
	allow(dead_code, reason = "microphone failures are constructed by the local-stt backend")
)]
enum SttError {
	#[error("the chat host mailbox is unavailable")]
	MailboxUnavailable,
	#[error("the speech-to-text microphone lease is unavailable")]
	MicrophoneLease {
		#[source]
		source: omp_voice::coordinator::CoordinatorError,
	},
	#[error("microphone capture failed")]
	Capture {
		#[source]
		source: VoiceError,
	},
	#[error("microphone capture shutdown failed")]
	CaptureStop {
		#[source]
		source: VoiceError,
	},
	#[error("speech capture exceeded the five-minute audio bound")]
	AudioLimit,
	#[error("speech capture outran the bounded recognition queue")]
	Backpressure,
	#[cfg(feature = "local-stt")]
	#[error("the speech model data directory is unavailable")]
	DataDir {
		#[source]
		source: omp_core::dirs::DataDirError,
	},
	#[cfg(feature = "local-stt")]
	#[error("the speech model directory could not be created")]
	CreateModelDir {
		#[source]
		source: std::io::Error,
	},
	#[cfg(feature = "local-stt")]
	#[error("the speech artifact store failed")]
	Artifact {
		#[source]
		source: omp_inference::local::ArtifactError,
	},
	#[cfg(feature = "local-stt")]
	#[error("the speech artifact catalog is invalid")]
	Catalog {
		#[source]
		source: omp_inference::local::SpeechCatalogError,
	},
	#[cfg(feature = "local-stt")]
	#[error("the local speech recognizer failed")]
	Recognition {
		#[source]
		source: omp_inference::local::LocalError,
	},
	#[cfg(feature = "local-stt")]
	#[error("the speech recognition worker failed")]
	Worker {
		#[source]
		source: tokio::task::JoinError,
	},
	#[cfg(not(feature = "local-stt"))]
	#[error("speech-to-text is not built; rebuild omp with `--features local-stt`")]
	NotBuilt,
}

impl SttError {
	fn kind(&self) -> SttFailureKind {
		match self {
			Self::MailboxUnavailable => SttFailureKind::Setup,
			#[cfg(feature = "local-stt")]
			Self::DataDir { .. }
			| Self::CreateModelDir { .. }
			| Self::Artifact { .. }
			| Self::Catalog { .. } => SttFailureKind::Setup,
			#[cfg(not(feature = "local-stt"))]
			Self::NotBuilt => SttFailureKind::Setup,
			Self::MicrophoneLease { .. } | Self::Capture { .. } | Self::CaptureStop { .. } => {
				SttFailureKind::Microphone
			},
			Self::AudioLimit => SttFailureKind::AudioLimit,
			Self::Backpressure => SttFailureKind::Backpressure,
			#[cfg(feature = "local-stt")]
			Self::Recognition { .. } | Self::Worker { .. } => SttFailureKind::Recognition,
		}
	}
}

#[cfg_attr(
	not(feature = "local-stt"),
	allow(dead_code, reason = "capture shutdown details are consumed by the local-stt backend")
)]
enum SttRuntimeCommand {
	Stop { capture_error: Option<VoiceError> },
	Cancel,
}

struct SttRuntime {
	commands: flume::Sender<SttRuntimeCommand>,
	task:     tokio::task::JoinHandle<()>,
	cancel:   tokio_util::sync::CancellationToken,
	capture:  Arc<Mutex<Option<CaptureStream>>>,
	state:    Arc<AtomicU8>,
	audio:    InteractiveAudioController,
}

impl SttRuntime {
	fn stop(&self) {
		self.state.store(STT_STATE_TRANSCRIBING, Ordering::Release);
		let capture_error = stop_capture(&self.capture).err();
		self.audio.stop_stt();
		let _ = self
			.commands
			.send(SttRuntimeCommand::Stop { capture_error });
	}

	fn cancel(&self) {
		self.cancel.cancel();
		let _ = self.commands.send(SttRuntimeCommand::Cancel);
		let _ = stop_capture(&self.capture);
		self.audio.stop_stt();
	}
}

impl Drop for SttRuntime {
	fn drop(&mut self) {
		self.cancel();
		self.task.abort();
	}
}

enum LiveRuntimeCommand {
	SetMuted(bool),
	Send(Vec<LiveClientMessage>),
	SelectInputDevice(Str),
	SelectOutputDevice(Str),
	Reconnect,
	Close,
}

enum LiveRuntimeEvent {
	DataChannel(Str),
	InputLevel(f64),
	OutputLevel(f64),
	Failure(Str),
}

struct LiveRuntime {
	commands: flume::Sender<LiveRuntimeCommand>,
	task:     tokio::task::JoinHandle<()>,
	muted:    bool,
	speaking: Arc<AtomicBool>,
}

impl LiveRuntime {
	fn close(&self) -> bool {
		self.commands.send(LiveRuntimeCommand::Close).is_ok()
	}
}

impl Drop for LiveRuntime {
	fn drop(&mut self) {
		let _ = self.close();
	}
}

/// One session's push-to-talk recorder and live transport owner.
pub struct PushToTalk {
	audio:      InteractiveAudioController,
	con:        Arc<Ctx>,
	stt:        Option<SttRuntime>,
	live_auth:  Option<AuthManager>,
	session_id: Str,
	live:       Option<LiveRuntime>,
	delegation: LiveDelegationBridge,
}

impl PushToTalk {
	/// Creates an idle recorder over the session's audio controller.
	#[must_use]
	pub fn new(
		audio: InteractiveAudioController,
		con: Arc<Ctx>,
		live_auth: Option<AuthManager>,
		session_id: Str,
	) -> Self {
		Self {
			audio,
			con,
			stt: None,
			live_auth,
			session_id,
			live: None,
			delegation: LiveDelegationBridge::default(),
		}
	}

	/// Authenticates and orders one provider-issued coding delegation.
	pub fn admit_delegation(&mut self, id: Str, request: Str) -> LiveDelegationAdmission {
		if !self
			.live
			.as_ref()
			.is_some_and(|live| !live.task.is_finished())
		{
			self.delegation.cancel_all();
			return LiveDelegationAdmission::Ignored;
		}
		self.delegation.admit(id, request)
	}

	/// Marks the point where an admitted delegation actually owns the kernel.
	pub fn delegation_started(&self, id: &str, ctx: &Ctx) {
		if self.delegation.active_id() == Some(id) {
			post_live(ctx, LiveUiEvent::Phase(LivePhase::Working));
		}
	}

	/// Streams one ordered assistant delta back to the active delegation.
	pub fn delegation_progress(&mut self, id: &str, text: &str) {
		let frames = self.delegation.progress(id, text);
		if frames.is_empty() {
			return;
		}
		if self.live.as_ref().is_none_or(|live| {
			live
				.commands
				.send(LiveRuntimeCommand::Send(frames))
				.is_err()
		}) {
			self.delegation.cancel_all();
		}
	}

	/// Settles one delegated kernel turn and promotes the next admitted request.
	pub fn settle_delegation(
		&mut self,
		id: &str,
		terminal: LiveDelegationTerminal,
		final_text: &str,
		ctx: &Ctx,
	) -> Option<omp_voice::transport::LiveDelegationRequest> {
		if !self
			.live
			.as_ref()
			.is_some_and(|live| !live.task.is_finished())
		{
			self.delegation.cancel_all();
			return None;
		}
		let LiveDelegationSettlement { outbound, next } =
			self.delegation.settle(id, terminal, final_text)?;
		if !outbound.is_empty()
			&& self.live.as_ref().is_none_or(|live| {
				live
					.commands
					.send(LiveRuntimeCommand::Send(outbound))
					.is_err()
			}) {
			self.delegation.cancel_all();
			return None;
		}
		self.post_idle_phase(ctx);
		next
	}

	/// Cancels every active and queued delegation, returning the running ID.
	pub fn cancel_delegations(&mut self, ctx: &Ctx) -> Option<Str> {
		let active = self.delegation.cancel_all();
		if active.is_some() {
			self.post_idle_phase(ctx);
		}
		active
	}

	fn post_idle_phase(&self, ctx: &Ctx) {
		let Some(live) = self.live.as_ref().filter(|live| !live.task.is_finished()) else {
			return;
		};
		if !live.speaking.load(Ordering::Acquire) {
			post_live(
				ctx,
				LiveUiEvent::Phase(if live.muted {
					LivePhase::Muted
				} else {
					LivePhase::Listening
				}),
			);
		}
	}

	/// Changes the live-session identity after the controller admits a session
	/// switch. The old transport is closed because it was authenticated for
	/// the previous session.
	pub fn switch_session(&mut self, session_id: Str) -> Option<Str> {
		let active = self.delegation.cancel_all();
		if let Some(live) = self.live.take() {
			live.close();
		}
		self.session_id = session_id;
		active
	}

	/// Whether microphone capture is currently delivering audio.
	#[must_use]
	pub fn recording(&self) -> bool {
		self.stt.as_ref().is_some_and(|runtime| {
			!runtime.task.is_finished() && runtime.state.load(Ordering::Acquire) == STT_STATE_RECORDING
		})
	}

	/// Applies one recording edge from the host.
	pub fn set_active(&mut self, active: bool, ctx: &Ctx) {
		if self
			.stt
			.as_ref()
			.is_some_and(|runtime| runtime.task.is_finished())
		{
			self.stt.take();
		}
		if active {
			if !CL_VOICE_STT_ENABLED.get(ctx) {
				post_stt(ctx, SttUiEvent::Failed {
					kind:    SttFailureKind::Setup,
					message: Str::new_static("Speech-to-text is disabled; set cl_voice_stt_enabled 1"),
				});
				return;
			}
			if self.stt.is_some() {
				post_stt(ctx, SttUiEvent::Transcribing);
				return;
			}
			if let Err(error) = self.start(ctx) {
				post_stt_error(ctx, error);
			}
		} else if let Some(runtime) = &self.stt {
			runtime.stop();
			post_stt(ctx, SttUiEvent::Transcribing);
		}
	}

	/// Cancels capture, queued audio, and active local inference idempotently.
	///
	/// Session switching and controller teardown call this before replacing
	/// controller state, so microphone ownership never leaks across sessions.
	pub fn cancel(&mut self, ctx: &Ctx) {
		let Some(runtime) = self.stt.take() else {
			return;
		};
		runtime.cancel();
		post_stt(ctx, SttUiEvent::Cancelled);
	}

	/// Applies one typed `/live` control request to the production transport.
	pub fn control_live(&mut self, control: LiveControl, ctx: &Ctx) {
		match control {
			LiveControl::Start => {
				if self.stt.is_some() {
					self.cancel(ctx);
				}
				if self
					.live
					.as_ref()
					.is_some_and(|live| !live.task.is_finished())
				{
					return;
				}
				self.live.take();
				let Some(auth) = self.live_auth.clone() else {
					post_live(ctx, LiveUiEvent::Error {
						message:     Str::new_static(
							"Live voice requires a local production Codex login.",
						),
						recoverable: false,
					});
					return;
				};
				let Some(mailbox) = ctx
					.user::<HostMailbox>()
					.map(|mailbox| Arc::clone(&mailbox))
				else {
					return;
				};
				let (commands, command_rx) = flume::unbounded();
				let audio = self.audio.clone();
				let speaking = Arc::new(AtomicBool::new(false));
				let session_id = self.session_id.clone();
				let voice = Str::from(<&'static str>::from(CL_LIVE_VOICE.get(ctx)));
				let selected_input = CL_LIVE_INPUT_DEVICE.get(ctx);
				let selected_output = CL_LIVE_OUTPUT_DEVICE.get(ctx);
				post_mailbox(&mailbox, LiveUiEvent::Phase(LivePhase::Connecting));
				let task = tokio::spawn(run_live_transport(
					audio,
					Arc::clone(&self.con),
					auth,
					session_id,
					voice,
					selected_input,
					selected_output,
					Arc::clone(&mailbox),
					command_rx,
					Arc::clone(&speaking),
				));
				self.live = Some(LiveRuntime { commands, task, muted: false, speaking });
			},
			LiveControl::Stop => {
				if self.live.take().is_none_or(|live| !live.close()) {
					post_live(ctx, LiveUiEvent::Closed);
				}
			},
			LiveControl::ToggleMute => {
				let Some(live) = self.live.as_mut() else {
					post_live(ctx, LiveUiEvent::Error {
						message:     Str::new_static("The live microphone is no longer active."),
						recoverable: true,
					});
					return;
				};
				live.muted = !live.muted;
				if live
					.commands
					.try_send(LiveRuntimeCommand::SetMuted(live.muted))
					.is_err()
				{
					post_live(ctx, LiveUiEvent::Error {
						message:     Str::new_static("The live transport is no longer active."),
						recoverable: true,
					});
				}
			},
			LiveControl::Reconnect => {
				let Some(live) = self.live.as_ref() else {
					self.control_live(LiveControl::Start, ctx);
					return;
				};
				post_live(ctx, LiveUiEvent::Phase(LivePhase::Reconnecting));
				if live
					.commands
					.try_send(LiveRuntimeCommand::Reconnect)
					.is_err()
				{
					self.live.take();
					self.control_live(LiveControl::Start, ctx);
				}
			},
			LiveControl::SelectVoice(voice) => match voice.parse::<LiveVoice>() {
				Ok(voice) => {
					if let Err(error) = CL_LIVE_VOICE.set(ctx, voice) {
						post_live(ctx, LiveUiEvent::Error {
							message:     Str::new(format!("Could not save the live voice: {error}")),
							recoverable: true,
						});
					}
				},
				Err(_) => post_live(ctx, LiveUiEvent::Error {
					message:     Str::new_static("The realtime provider rejected that voice."),
					recoverable: true,
				}),
			},
			LiveControl::SelectInputDevice(device) => {
				self.select_live_device(device, true, ctx);
			},
			LiveControl::SelectOutputDevice(device) => {
				self.select_live_device(device, false, ctx);
			},
		}
	}

	fn select_live_device(&mut self, selected: Str, input: bool, ctx: &Ctx) {
		if let Some(live) = self.live.as_ref().filter(|live| !live.task.is_finished()) {
			let command = if input {
				LiveRuntimeCommand::SelectInputDevice(selected)
			} else {
				LiveRuntimeCommand::SelectOutputDevice(selected)
			};
			if live.commands.try_send(command).is_err() {
				post_live(ctx, LiveUiEvent::Error {
					message:     Str::new_static("The live transport is no longer active."),
					recoverable: true,
				});
			}
			return;
		}
		self.live.take();
		let snapshot = match device::snapshot() {
			Ok(snapshot) => snapshot,
			Err(error) => {
				post_live(ctx, LiveUiEvent::Error {
					message: Str::new(format!("Could not enumerate audio devices: {error}")),
					recoverable: true,
				});
				return;
			},
		};
		let devices = if input { &snapshot.input } else { &snapshot.output };
		if !selected.is_empty() && !devices.iter().any(|device| device.id == selected) {
			let direction = if input { "microphone" } else { "speaker" };
			post_live(ctx, LiveUiEvent::Error {
				message: Str::new(format!(
					"The selected {direction} is no longer available ({selected})."
				)),
				recoverable: true,
			});
			post_device_snapshot(
				ctx.user::<HostMailbox>().as_deref(),
				&snapshot,
				&LiveDeviceSelection {
					input:  CL_LIVE_INPUT_DEVICE.get(ctx),
					output: CL_LIVE_OUTPUT_DEVICE.get(ctx),
				},
			);
			return;
		}
		let result = if input {
			CL_LIVE_INPUT_DEVICE.set(ctx, selected)
		} else {
			CL_LIVE_OUTPUT_DEVICE.set(ctx, selected)
		};
		if let Err(error) = result {
			let direction = if input { "microphone" } else { "speaker" };
			post_live(ctx, LiveUiEvent::Error {
				message: Str::new(format!("Could not save the {direction} selection: {error}")),
				recoverable: true,
			});
			return;
		}
		post_device_snapshot(
			ctx.user::<HostMailbox>().as_deref(),
			&snapshot,
			&LiveDeviceSelection {
				input:  CL_LIVE_INPUT_DEVICE.get(ctx),
				output: CL_LIVE_OUTPUT_DEVICE.get(ctx),
			},
		);
	}

	fn start(&mut self, ctx: &Ctx) -> Result<(), SttError> {
		let mailbox = ctx
			.user::<HostMailbox>()
			.map(|mailbox| Arc::clone(&mailbox))
			.ok_or(SttError::MailboxUnavailable)?;
		let model = CL_STT_MODEL.get(ctx);
		let language = CL_STT_LANGUAGE.get(ctx);
		let trigger = CL_STT_SUBMIT_TRIGGER.get(ctx);
		let audio = self.audio.clone();
		let capture = Arc::new(Mutex::new(None));
		let cancel = tokio_util::sync::CancellationToken::new();
		let state = Arc::new(AtomicU8::new(STT_STATE_SETUP));
		let (commands, command_rx) = flume::unbounded();
		let task = tokio::spawn(run_stt(
			audio.clone(),
			model,
			language,
			trigger,
			Arc::clone(&mailbox),
			command_rx,
			cancel.clone(),
			Arc::clone(&capture),
			Arc::clone(&state),
		));
		self.stt = Some(SttRuntime { commands, task, cancel, capture, state, audio });
		Ok(())
	}
}

impl Drop for PushToTalk {
	fn drop(&mut self) {
		if let Some(runtime) = self.stt.take() {
			runtime.cancel();
		}
		if let Some(live) = self.live.take() {
			live.close();
		}
	}
}

fn stop_capture(capture: &Mutex<Option<CaptureStream>>) -> Result<(), VoiceError> {
	let Some(mut capture) = capture.lock().take() else {
		return Ok(());
	};
	capture.stop()
}

fn post_stt(ctx: &Ctx, event: SttUiEvent) {
	if let Some(mailbox) = ctx.user::<HostMailbox>() {
		post_stt_mailbox(&mailbox, event);
	}
}

fn post_stt_mailbox(mailbox: &HostMailbox, event: SttUiEvent) {
	mailbox.post(HostAction::SttEvent(event));
}

fn post_stt_error(ctx: &Ctx, error: SttError) {
	if let Some(mailbox) = ctx.user::<HostMailbox>() {
		post_stt_error_mailbox(&mailbox, error);
	}
}

fn post_stt_error_mailbox(mailbox: &HostMailbox, error: SttError) {
	use std::{error::Error as _, fmt::Write as _};

	let kind = error.kind();
	let mut message = error.to_string();
	if let Some(source) = error.source() {
		let _ = write!(message, ": {source}");
	}
	post_stt_mailbox(mailbox, SttUiEvent::Failed { kind, message: Str::new(message) });
}

#[cfg(feature = "local-stt")]
async fn prepare_stt(
	model: SttModel,
	mailbox: Arc<HostMailbox>,
	cancel: tokio_util::sync::CancellationToken,
) -> Result<omp_inference::local::stt::SpeechToTextAdapter, SttError> {
	use omp_inference::local::{
		ArtifactCacheStatus, ArtifactStore, MemoryPool, SystemArtifactFetcher,
		speech_catalog::SpeechArtifactManifests,
		stt::{SpeechToTextAdapter, SttRuntimeOptions, resolve_stt_preset},
	};

	let data_dir = omp_core::dirs::data_dir(None).map_err(|source| SttError::DataDir { source })?;
	let root = data_dir.join("models");
	std::fs::create_dir_all(&root).map_err(|source| SttError::CreateModelDir { source })?;
	let store = ArtifactStore::open(&root).map_err(|source| SttError::Artifact { source })?;
	let artifacts =
		SpeechArtifactManifests::curated().map_err(|source| SttError::Catalog { source })?;
	let preset = resolve_stt_preset(Some(<&'static str>::from(model)));
	let manifest = artifacts.stt_manifest(preset);
	let cache = store
		.inspect_manifest(manifest, &cancel)
		.map_err(|source| SttError::Artifact { source })?;
	if cache.status != ArtifactCacheStatus::Ready {
		let progress_model = Str::from(<&'static str>::from(model));
		store
			.acquire(manifest, &SystemArtifactFetcher::new(), &cancel, |progress| {
				post_stt_mailbox(&mailbox, SttUiEvent::SetupProgress {
					model:            progress_model.clone(),
					downloaded_bytes: progress.downloaded_bytes,
					total_bytes:      progress.total_bytes,
				});
			})
			.await
			.map_err(|source| SttError::Artifact { source })?;
	}
	let options = SttRuntimeOptions {
		threads:      std::thread::available_parallelism().map_or(4, usize::from),
		whisper_gpu:  true,
		idle_timeout: Duration::from_secs(120),
	};
	let memory = Arc::new(MemoryPool::new(2 * 1024 * 1024 * 1024));
	let selected = <&'static str>::from(model);
	tokio::task::spawn_blocking(move || {
		SpeechToTextAdapter::from_verified_artifacts(
			&store,
			&artifacts,
			Some(selected),
			options,
			memory,
			&cancel,
		)
		.map_err(|source| SttError::Recognition { source })
	})
	.await
	.map_err(|source| SttError::Worker { source })?
}

#[cfg(feature = "local-stt")]
fn normalize_stt_text(text: &str, committed: bool) -> Str {
	let mut normalized = String::new();
	for word in text.split_whitespace() {
		if !normalized.is_empty() {
			normalized.push(' ');
		}
		normalized.push_str(word);
	}
	if normalized.is_empty() {
		return Str::default();
	}
	if committed {
		normalized.insert(0, ' ');
	}
	Str::new(normalized)
}

#[cfg(feature = "local-stt")]
fn decode_stt_audio(
	adapter: &omp_inference::local::stt::SpeechToTextAdapter,
	samples: &[f32],
	language: &Str,
	cancel: &tokio_util::sync::CancellationToken,
) -> Result<Str, SttError> {
	use omp_inference::local::stt::TranscriptionOptions;

	let transcription = adapter
		.transcribe_mono_16khz(
			samples,
			&TranscriptionOptions {
				language: (!language.trim().is_empty()).then_some(language.clone()),
				..TranscriptionOptions::default()
			},
			cancel,
		)
		.map_err(|source| SttError::Recognition { source })?;
	Ok(Str::new(transcription.text.trim()))
}

#[cfg(feature = "local-stt")]
fn decode_stt_stream(
	adapter: omp_inference::local::stt::SpeechToTextAdapter,
	language: Str,
	trigger: SttSubmitTrigger,
	mailbox: Arc<HostMailbox>,
	commands: flume::Receiver<SttRuntimeCommand>,
	audio: flume::Receiver<Vec<f32>>,
	cancel: tokio_util::sync::CancellationToken,
) -> Result<Option<(bool, usize, bool)>, SttError> {
	use std::time::Duration as StdDuration;

	use xutf::Text as _;

	if let Err(source) = adapter.prewarm(&cancel) {
		if cancel.is_cancelled() {
			return Ok(None);
		}
		return Err(SttError::Recognition { source });
	}
	let mut endpointer = StreamEndpointer::default();
	let mut committed = false;
	let mut utterance = String::new();

	let mut apply = |event: EndpointerEvent| -> Result<(), SttError> {
		match event {
			EndpointerEvent::Partial(samples) => {
				let text = decode_stt_audio(&adapter, &samples, &language, &cancel)?;
				let preview = normalize_stt_text(text.as_str(), committed);
				post_stt_mailbox(&mailbox, SttUiEvent::Partial(preview));
			},
			EndpointerEvent::Segment(samples) => {
				let text = decode_stt_audio(&adapter, &samples, &language, &cancel)?;
				let segment = normalize_stt_text(text.as_str(), committed);
				if segment.is_empty() {
					post_stt_mailbox(&mailbox, SttUiEvent::Partial(Str::default()));
				} else {
					utterance.push_str(segment.as_str());
					committed = true;
					post_stt_mailbox(&mailbox, SttUiEvent::Segment(segment));
				}
			},
		}
		Ok(())
	};

	loop {
		if cancel.is_cancelled() {
			return Ok(None);
		}
		let command = match commands.try_recv() {
			Ok(command) => Some(command),
			Err(flume::TryRecvError::Disconnected) => return Ok(None),
			Err(flume::TryRecvError::Empty) => {
				match audio.recv_timeout(StdDuration::from_millis(25)) {
					Ok(samples) => {
						for event in endpointer.push(&samples) {
							apply(event)?;
						}
						None
					},
					Err(flume::RecvTimeoutError::Timeout) => None,
					// Capture shutdown drops the realtime callback (and its
					// sender) before the controller publishes Stop. Wait for that
					// ordered control edge rather than losing the trailing audio.
					Err(flume::RecvTimeoutError::Disconnected) => match commands.recv() {
						Ok(command) => Some(command),
						Err(_) => return Ok(None),
					},
				}
			},
		};
		match command {
			Some(SttRuntimeCommand::Cancel) => return Ok(None),
			Some(SttRuntimeCommand::Stop { capture_error }) => {
				if let Some(source) = capture_error {
					return Err(SttError::CaptureStop { source });
				}
				while let Ok(samples) = audio.try_recv() {
					for event in endpointer.push(&samples) {
						apply(event)?;
					}
				}
				for event in endpointer.flush() {
					apply(event)?;
				}
				drop(apply);
				let (trimmed, submit) = stt_submission(Str::new(&utterance), trigger);
				let trim_trailing = utterance
					.graphemes()
					.count()
					.saturating_sub(trimmed.graphemes().count());
				return Ok(Some((committed, trim_trailing, submit)));
			},
			None => {},
		}
	}
}

#[cfg(feature = "local-stt")]
async fn run_stt(
	audio_owner: InteractiveAudioController,
	model: SttModel,
	language: Str,
	trigger: SttSubmitTrigger,
	mailbox: Arc<HostMailbox>,
	commands: flume::Receiver<SttRuntimeCommand>,
	cancel: tokio_util::sync::CancellationToken,
	capture_owner: Arc<Mutex<Option<CaptureStream>>>,
	state: Arc<AtomicU8>,
) {
	let adapter = match prepare_stt(model, Arc::clone(&mailbox), cancel.clone()).await {
		Ok(adapter) => adapter,
		Err(_) if cancel.is_cancelled() => return,
		Err(error) => {
			post_stt_error_mailbox(&mailbox, error);
			return;
		},
	};
	match commands.try_recv() {
		Ok(SttRuntimeCommand::Cancel) => return,
		Ok(SttRuntimeCommand::Stop { .. }) => {
			post_stt_mailbox(&mailbox, SttUiEvent::Finished {
				had_speech:    false,
				trim_trailing: 0,
				submit:        false,
			});
			return;
		},
		Err(_) => {},
	}
	if let Err(source) = audio_owner.start_stt() {
		post_stt_error_mailbox(&mailbox, SttError::MicrophoneLease { source });
		return;
	}
	match commands.try_recv() {
		Ok(SttRuntimeCommand::Cancel) => {
			audio_owner.stop_stt();
			return;
		},
		Ok(SttRuntimeCommand::Stop { capture_error }) => {
			audio_owner.stop_stt();
			if let Some(source) = capture_error {
				post_stt_error_mailbox(&mailbox, SttError::CaptureStop { source });
			} else {
				post_stt_mailbox(&mailbox, SttUiEvent::Finished {
					had_speech:    false,
					trim_trailing: 0,
					submit:        false,
				});
			}
			return;
		},
		Err(_) => {},
	}
	let (audio_tx, audio_rx) = flume::bounded(STT_AUDIO_QUEUE_DEPTH);
	let callback_cancel = cancel.clone();
	let callback_mailbox = Arc::clone(&mailbox);
	let captured = Arc::new(AtomicUsize::new(0));
	let failed = Arc::new(AtomicBool::new(false));
	let callback_failed = Arc::clone(&failed);
	let capture = CaptureStream::start(SAMPLE_RATE, move |samples| {
		let prior = captured.fetch_add(samples.len(), Ordering::AcqRel);
		let failure = if prior > MAX_STT_SAMPLES.saturating_sub(samples.len()) {
			Some(SttError::AudioLimit)
		} else if audio_tx.try_send(samples.to_vec()).is_err() {
			Some(SttError::Backpressure)
		} else {
			None
		};
		if let Some(error) = failure.filter(|_| !callback_failed.swap(true, Ordering::AcqRel)) {
			post_stt_error_mailbox(&callback_mailbox, error);
			callback_cancel.cancel();
		}
	});
	let capture = match capture {
		Ok(capture) => capture,
		Err(source) => {
			audio_owner.stop_stt();
			post_stt_error_mailbox(&mailbox, SttError::Capture { source });
			return;
		},
	};
	*capture_owner.lock() = Some(capture);
	state.store(STT_STATE_RECORDING, Ordering::Release);
	post_stt_mailbox(&mailbox, SttUiEvent::Recording);

	let result = tokio::task::spawn_blocking({
		let mailbox = Arc::clone(&mailbox);
		let cancel = cancel.clone();
		move || decode_stt_stream(adapter, language, trigger, mailbox, commands, audio_rx, cancel)
	})
	.await;
	let capture_error = stop_capture(&capture_owner).err();
	audio_owner.stop_stt();
	if failed.load(Ordering::Acquire) {
		return;
	}
	if let Some(source) = capture_error {
		post_stt_error_mailbox(&mailbox, SttError::CaptureStop { source });
		return;
	}
	match result {
		Ok(Ok(Some((had_speech, trim_trailing, submit)))) => {
			post_stt_mailbox(&mailbox, SttUiEvent::Finished { had_speech, trim_trailing, submit })
		},
		Ok(Ok(None)) => {
			if !cancel.is_cancelled() {
				post_stt_mailbox(&mailbox, SttUiEvent::Cancelled);
			}
		},
		Ok(Err(error)) => post_stt_error_mailbox(&mailbox, error),
		Err(source) => post_stt_error_mailbox(&mailbox, SttError::Worker { source }),
	}
}

#[cfg(not(feature = "local-stt"))]
async fn run_stt(
	_audio_owner: InteractiveAudioController,
	_model: SttModel,
	_language: Str,
	_trigger: SttSubmitTrigger,
	mailbox: Arc<HostMailbox>,
	_commands: flume::Receiver<SttRuntimeCommand>,
	_cancel: tokio_util::sync::CancellationToken,
	_capture_owner: Arc<Mutex<Option<CaptureStream>>>,
	_state: Arc<AtomicU8>,
) {
	post_stt_error_mailbox(&mailbox, SttError::NotBuilt);
}

#[derive(Debug, Error)]
enum CodexSignalingError {
	#[error(transparent)]
	Credential(#[from] CodexLiveCredentialError),
	#[error("Codex live signaling request contains an invalid header")]
	Header {
		#[source]
		source: reqwest::header::InvalidHeaderName,
	},
	#[error("Codex live signaling HTTP request failed")]
	Http {
		#[source]
		source: reqwest::Error,
	},
	#[error("Codex live signaling was rejected with HTTP {status}")]
	Rejected { status: reqwest::StatusCode },
	#[error("Codex live signaling returned an empty SDP answer")]
	EmptyAnswer,
	#[error("Codex live signaling returned an oversized SDP answer")]
	OversizedAnswer,
	#[error("Codex live signaling returned non-UTF-8 SDP")]
	InvalidAnswer {
		#[source]
		source: std::str::Utf8Error,
	},
}

struct CodexSignalingClient {
	auth: AuthManager,
	http: reqwest::Client,
}

impl CodexSignalingClient {
	fn new(auth: AuthManager) -> Self {
		Self { auth, http: reqwest::Client::new() }
	}
}

impl LiveSignalingClient for CodexSignalingClient {
	type Error = CodexSignalingError;

	fn signal(
		&mut self,
		request: LiveSignalingRequest,
	) -> impl Future<Output = Result<LiveSignalingResponse, Self::Error>> + Send {
		async move {
			let http = if let Some(proxy) = request.proxy.as_ref() {
				reqwest::Client::builder()
					.proxy(
						reqwest::Proxy::all(proxy.as_str())
							.map_err(|source| CodexSignalingError::Http { source })?,
					)
					.build()
					.map_err(|source| CodexSignalingError::Http { source })?
			} else {
				self.http.clone()
			};
			let mut credential = self.auth.lease_codex_live().await?;
			for attempt in 0..2 {
				let authorization = credential.authorization_header()?;
				let mut builder = http
					.post(request.url)
					.header(reqwest::header::AUTHORIZATION, authorization.clone())
					.body(request.body.clone());
				for (name, value) in &request.headers {
					let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
						.map_err(|source| CodexSignalingError::Header { source })?;
					builder = builder.header(name, value.clone());
				}
				let mut response = builder
					.send()
					.await
					.map_err(|source| CodexSignalingError::Http { source })?;
				let status = response.status();
				let location = response
					.headers()
					.get(reqwest::header::LOCATION)
					.and_then(|value| value.to_str().ok())
					.map_or_else(Str::default, Str::from);
				if status == reqwest::StatusCode::UNAUTHORIZED {
					if attempt == 0 {
						self
							.auth
							.reject_codex_live(&credential, status.as_u16())
							.await
							.map_err(CodexLiveCredentialError::from)?;
						credential = self.auth.refresh_codex_live(&credential).await?;
						continue;
					}
					return Err(CodexSignalingError::Rejected { status });
				}
				if !status.is_success() {
					return Err(CodexSignalingError::Rejected { status });
				}
				if response
					.content_length()
					.is_some_and(|length| length > MAX_LIVE_SDP_BYTES as u64)
				{
					return Err(CodexSignalingError::OversizedAnswer);
				}
				let mut answer = Vec::new();
				while let Some(chunk) = response
					.chunk()
					.await
					.map_err(|source| CodexSignalingError::Http { source })?
				{
					if answer.len().saturating_add(chunk.len()) > MAX_LIVE_SDP_BYTES {
						return Err(CodexSignalingError::OversizedAnswer);
					}
					answer.extend_from_slice(&chunk);
				}
				let answer = std::str::from_utf8(&answer)
					.map_err(|source| CodexSignalingError::InvalidAnswer { source })?;
				if answer.trim().is_empty() {
					return Err(CodexSignalingError::EmptyAnswer);
				}
				return Ok(LiveSignalingResponse {
					answer: Str::from(answer),
					location,
					access: LiveOAuthAccess {
						authorization,
						account_id: credential.account_id().cloned(),
					},
				});
			}
			unreachable!("Codex signaling attempts always return or continue once")
		}
	}
}

#[derive(Clone, Copy)]
enum LiveAttemptExit {
	Close,
	Reconnect,
	RestartDevices,
	Failed,
}

#[derive(Clone, Copy)]
enum LiveAuthorization {
	Granted,
	Recoverable,
	Terminal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LiveDeviceSelection {
	input:  Str,
	output: Str,
}

impl LiveDeviceSelection {
	fn input_id(&self) -> Option<&str> {
		(!self.input.is_empty()).then_some(self.input.as_str())
	}

	fn output_id(&self) -> Option<&str> {
		(!self.output.is_empty()).then_some(self.output.as_str())
	}
}

fn projected_devices(devices: &[AudioDevice], selected: &str) -> Vec<LiveDevice> {
	let selected_present =
		!selected.is_empty() && devices.iter().any(|device| device.id == selected);
	devices
		.iter()
		.map(|device| LiveDevice {
			id:         device.id.clone(),
			label:      device.label.clone(),
			is_default: device.is_default,
			selected:   if selected_present {
				device.id == selected
			} else {
				device.is_default
			},
		})
		.collect()
}

fn post_device_snapshot(
	mailbox: Option<&HostMailbox>,
	snapshot: &DeviceSnapshot,
	selected: &LiveDeviceSelection,
) {
	let Some(mailbox) = mailbox else {
		return;
	};
	post_mailbox(
		mailbox,
		LiveUiEvent::Devices {
			input:  projected_devices(&snapshot.input, selected.input.as_str()),
			output: projected_devices(&snapshot.output, selected.output.as_str()),
		},
	);
}

fn resolved_device_id(devices: &[AudioDevice], selected: &str) -> Option<Str> {
	devices
		.iter()
		.find(|device| {
			if selected.is_empty() {
				device.is_default
			} else {
				device.id == selected
			}
		})
		.map(|device| device.id.clone())
}

fn device_description(devices: &[AudioDevice], id: &str) -> Str {
	devices
		.iter()
		.find(|device| device.id == id)
		.map_or_else(|| Str::from(id), |device| device.label.clone())
}

fn post_device_loss(
	mailbox: &HostMailbox,
	direction: &'static str,
	id: &str,
	previous: &DeviceSnapshot,
	input: bool,
) {
	let devices = if input { &previous.input } else { &previous.output };
	let label = device_description(devices, id);
	post_mailbox(
		mailbox,
		LiveUiEvent::Error {
			message: Str::new(format!(
				"The live {direction} was disconnected: {label} ({id}). Reconnecting with an \
				 available device."
			)),
			recoverable: true,
		},
	);
}

fn refresh_device_selection(
	mailbox: &HostMailbox,
	previous: &DeviceSnapshot,
	mut next: DeviceSnapshot,
	selected: &mut LiveDeviceSelection,
	active_input: Option<&Str>,
	active_output: Option<&Str>,
) -> (DeviceSnapshot, bool) {
	let input_missing = active_input
		.is_some_and(|id| !next.input.iter().any(|device| device.id == *id));
	let output_missing = active_output
		.is_some_and(|id| !next.output.iter().any(|device| device.id == *id));
	if input_missing || output_missing {
		match device::snapshot() {
			Ok(refreshed) => next = refreshed,
			Err(error) => post_mailbox(
				mailbox,
				LiveUiEvent::Error {
					message: Str::new(format!(
						"Could not refresh audio devices after a device change: {error}"
					)),
					recoverable: true,
				},
			),
		}
	}

	let mut restart = false;
	if let Some(id) = active_input {
		let still_present = next
			.input
			.iter()
			.any(|device| device.id.as_str() == id.as_str());
		let default_changed = selected.input.is_empty()
			&& resolved_device_id(&next.input, "").as_ref() != Some(id);
		if input_missing || !still_present {
			post_device_loss(mailbox, "microphone", id, previous, true);
			if !still_present && selected.input.as_str() == id.as_str() {
				selected.input = Str::default();
			}
			restart = true;
		} else if default_changed {
			restart = true;
		}
	}
	if let Some(id) = active_output {
		let still_present = next
			.output
			.iter()
			.any(|device| device.id.as_str() == id.as_str());
		let default_changed = selected.output.is_empty()
			&& resolved_device_id(&next.output, "").as_ref() != Some(id);
		if output_missing || !still_present {
			post_device_loss(mailbox, "speaker", id, previous, false);
			if !still_present && selected.output.as_str() == id.as_str() {
				selected.output = Str::default();
			}
			restart = true;
		} else if default_changed {
			restart = true;
		}
	}
	post_device_snapshot(Some(mailbox), &next, selected);
	(next, restart)
}

fn post_device_permission(mailbox: &HostMailbox, permission: DeviceMicrophonePermission) {
	let permission = match permission {
		DeviceMicrophonePermission::Unknown => LiveMicrophonePermission::Unknown,
		DeviceMicrophonePermission::Requesting => LiveMicrophonePermission::Requesting,
		DeviceMicrophonePermission::Granted => LiveMicrophonePermission::Granted,
		DeviceMicrophonePermission::Denied => LiveMicrophonePermission::Denied,
		DeviceMicrophonePermission::Restricted => LiveMicrophonePermission::Restricted,
		DeviceMicrophonePermission::Unavailable => LiveMicrophonePermission::Unavailable,
	};
	post_mailbox(mailbox, LiveUiEvent::Permission(permission));
}

async fn authorize_live_microphone(
	mailbox: &HostMailbox,
	permission: DeviceMicrophonePermission,
) -> LiveAuthorization {
	let permission = match permission {
		DeviceMicrophonePermission::Unknown | DeviceMicrophonePermission::Requesting => {
			post_device_permission(mailbox, DeviceMicrophonePermission::Requesting);
			match device::request_microphone_permission().await {
				Ok(permission) => permission,
				Err(error) => {
					post_mailbox(
						mailbox,
						LiveUiEvent::Error {
							message: Str::new(format!(
								"Could not request microphone permission: {error}"
							)),
							recoverable: true,
						},
					);
					return LiveAuthorization::Recoverable;
				},
			}
		},
		permission => permission,
	};
	post_device_permission(mailbox, permission);
	match permission {
		DeviceMicrophonePermission::Unknown | DeviceMicrophonePermission::Granted => {
			LiveAuthorization::Granted
		},
		DeviceMicrophonePermission::Denied => LiveAuthorization::Recoverable,
		DeviceMicrophonePermission::Restricted => {
			post_mailbox(
				mailbox,
				LiveUiEvent::Error {
					message: Str::new_static(
						"Microphone access is restricted by system policy and cannot be requested.",
					),
					recoverable: false,
				},
			);
			LiveAuthorization::Terminal
		},
		DeviceMicrophonePermission::Unavailable => {
			post_mailbox(
				mailbox,
				LiveUiEvent::Error {
					message: Str::new_static(
						"Native microphone capture is unavailable on this platform.",
					),
					recoverable: false,
				},
			);
			LiveAuthorization::Terminal
		},
		DeviceMicrophonePermission::Requesting => {
			post_mailbox(
				mailbox,
				LiveUiEvent::Error {
					message: Str::new_static(
						"Microphone permission did not reach a final operating-system decision.",
					),
					recoverable: true,
				},
			);
			LiveAuthorization::Recoverable
		},
	}
}

fn active_live_permission(
	mailbox: &HostMailbox,
	permission: DeviceMicrophonePermission,
) -> LiveAuthorization {
	post_device_permission(mailbox, permission);
	match permission {
		DeviceMicrophonePermission::Unknown | DeviceMicrophonePermission::Granted => {
			LiveAuthorization::Granted
		},
		DeviceMicrophonePermission::Denied => LiveAuthorization::Recoverable,
		DeviceMicrophonePermission::Restricted => {
			post_mailbox(
				mailbox,
				LiveUiEvent::Error {
					message: Str::new_static(
						"Microphone access was revoked by system policy during the live session.",
					),
					recoverable: false,
				},
			);
			LiveAuthorization::Terminal
		},
		DeviceMicrophonePermission::Unavailable => {
			post_mailbox(
				mailbox,
				LiveUiEvent::Error {
					message: Str::new_static(
						"Native microphone capture became unavailable during the live session.",
					),
					recoverable: false,
				},
			);
			LiveAuthorization::Terminal
		},
		DeviceMicrophonePermission::Requesting => {
			post_mailbox(
				mailbox,
				LiveUiEvent::Error {
					message: Str::new_static(
						"Microphone permission became unsettled during the live session.",
					),
					recoverable: true,
				},
			);
			LiveAuthorization::Recoverable
		},
	}
}

fn normalize_live_selection(
	mailbox: &HostMailbox,
	snapshot: &DeviceSnapshot,
	selected: &mut LiveDeviceSelection,
) {
	if !selected.input.is_empty()
		&& !snapshot.input.iter().any(|device| device.id == selected.input)
	{
		let missing = std::mem::take(&mut selected.input);
		post_mailbox(
			mailbox,
			LiveUiEvent::Error {
				message: Str::new(format!(
					"The selected microphone is unavailable ({missing}); using the system default."
				)),
				recoverable: true,
			},
		);
	}
	if !selected.output.is_empty()
		&& !snapshot
			.output
			.iter()
			.any(|device| device.id == selected.output)
	{
		let missing = std::mem::take(&mut selected.output);
		post_mailbox(
			mailbox,
			LiveUiEvent::Error {
				message: Str::new(format!(
					"The selected speaker is unavailable ({missing}); using the system default."
				)),
				recoverable: true,
			},
		);
	}
}

fn persist_live_selection(
	ctx: &Ctx,
	mailbox: &HostMailbox,
	selected: &LiveDeviceSelection,
) {
	if let Err(error) = CL_LIVE_INPUT_DEVICE.set(ctx, selected.input.clone()) {
		post_mailbox(
			mailbox,
			LiveUiEvent::Error {
				message: Str::new(format!("Could not save the microphone selection: {error}")),
				recoverable: true,
			},
		);
	}
	if let Err(error) = CL_LIVE_OUTPUT_DEVICE.set(ctx, selected.output.clone()) {
		post_mailbox(
			mailbox,
			LiveUiEvent::Error {
				message: Str::new(format!("Could not save the speaker selection: {error}")),
				recoverable: true,
			},
		);
	}
}

fn validate_selected_device(
	mailbox: &HostMailbox,
	snapshot: &DeviceSnapshot,
	selected: &str,
	input: bool,
) -> bool {
	if selected.is_empty() {
		return true;
	}
	let devices = if input { &snapshot.input } else { &snapshot.output };
	if devices.iter().any(|device| device.id == selected) {
		return true;
	}
	let direction = if input { "microphone" } else { "speaker" };
	post_mailbox(
		mailbox,
		LiveUiEvent::Error {
			message: Str::new(format!(
				"The selected {direction} is no longer available ({selected})."
			)),
			recoverable: true,
		},
	);
	false
}

#[derive(Default)]
struct RoleTranscript {
	turn:      u64,
	text:      Str,
	finalized: bool,
}

#[derive(Default)]
struct LiveTranscripts {
	user:      RoleTranscript,
	assistant: RoleTranscript,
}

impl LiveTranscripts {
	fn update(
		&mut self,
		role: LiveTranscriptRole,
		text: &str,
		finalized: bool,
	) -> Option<LiveTranscript> {
		let slot = match role {
			LiveTranscriptRole::User => &mut self.user,
			LiveTranscriptRole::Assistant => &mut self.assistant,
		};
		let text = text.trim();
		if text.is_empty() {
			return None;
		}
		if slot.text.is_empty() || slot.finalized {
			if slot.finalized && slot.text == text {
				return None;
			}
			slot.turn = slot.turn.saturating_add(1);
			slot.text = Str::from(text);
		} else if text.starts_with(slot.text.as_str()) {
			slot.text = Str::from(text);
		} else if !slot.text.ends_with(text) {
			slot.text = Str::from(format!("{}{}", slot.text, text));
		}
		slot.finalized = finalized;
		Some(LiveTranscript { role, turn: slot.turn, text: slot.text.clone(), finalized })
	}
}

async fn run_live_transport(
	audio: InteractiveAudioController,
	con: Arc<Ctx>,
	auth: AuthManager,
	session_id: Str,
	voice: Str,
	selected_input: Str,
	selected_output: Str,
	mailbox: Arc<HostMailbox>,
	commands: flume::Receiver<LiveRuntimeCommand>,
	speaking: Arc<AtomicBool>,
) {
	let mut selected = LiveDeviceSelection { input: selected_input, output: selected_output };
	let mut committed = selected.clone();
	let mut rollback_selection = None;
	let mut snapshot = match device::snapshot() {
		Ok(snapshot) => snapshot,
		Err(error) => {
			post_mailbox(
				&mailbox,
				LiveUiEvent::Error {
					message: Str::new(format!("Could not enumerate audio devices: {error}")),
					recoverable: true,
				},
			);
			return;
		},
	};
	post_device_snapshot(Some(&mailbox), &snapshot, &selected);
	match authorize_live_microphone(&mailbox, snapshot.microphone_permission).await {
		LiveAuthorization::Granted => {
			post_mailbox(&mailbox, LiveUiEvent::Phase(LivePhase::Connecting));
		},
		LiveAuthorization::Recoverable => return,
		LiveAuthorization::Terminal => {
			post_mailbox(&mailbox, LiveUiEvent::Closed);
			return;
		},
	}
	let mut watcher: DeviceWatcher = match device::watch() {
		Ok(watcher) => watcher,
		Err(error) => {
			post_mailbox(
				&mailbox,
				LiveUiEvent::Error {
					message: Str::new(format!("Could not monitor audio devices: {error}")),
					recoverable: true,
				},
			);
			return;
		},
	};
	normalize_live_selection(&mailbox, &snapshot, &mut selected);
	post_device_snapshot(Some(&mailbox), &snapshot, &selected);
	let _logical_tts = audio.begin_live_restart_scope();
	let coordinator = audio.coordinator();
	let mut muted = false;
	let mut pending_outbound = Vec::new();
	let mut dedup = EventDeduplicator::default();
	let mut transcripts = LiveTranscripts::default();
	loop {
		speaking.store(false, Ordering::Release);
		let (events, event_rx) = flume::unbounded();
		let event_tx = events.clone();
		let input_tx = events.clone();
		let output_tx = events.clone();
		let failure_tx = events;
		let callbacks = LiveCallbacks {
			event:        Box::new(move |payload| {
				let _ = event_tx.send(LiveRuntimeEvent::DataChannel(Str::from(payload)));
			}),
			input_level:  Box::new(move |level| {
				let _ = input_tx.send(LiveRuntimeEvent::InputLevel(level));
			}),
			output_level: Box::new(move |level| {
				let _ = output_tx.send(LiveRuntimeEvent::OutputLevel(level));
			}),
			failure:      Box::new(move |message| {
				let _ = failure_tx.send(LiveRuntimeEvent::Failure(Str::from(message)));
			}),
		};
		let options = LiveTransportOptions::new(
			session_id.clone(),
			Str::new_static(LIVE_INSTRUCTIONS),
			voice.clone(),
			Str::new_static(omp_inference::codec::openai_codex::CODEX_CLIENT_VERSION),
		);
		let active_input = resolved_device_id(&snapshot.input, selected.input.as_str());
		let active_output = resolved_device_id(&snapshot.output, selected.output.as_str());
		let (media, offer) = match LiveMediaSession::start_on(
			&coordinator,
			callbacks,
			selected.input_id(),
			selected.output_id(),
		)
		.await
		{
			Ok(started) => started,
			Err(error) => {
				post_mailbox(&mailbox, LiveUiEvent::Error {
					message:     Str::new(format!("Could not start live audio: {error}")),
					recoverable: true,
				});
				if let Some(previous) = rollback_selection.take() {
					selected = previous;
					post_device_snapshot(Some(&mailbox), &snapshot, &selected);
					post_mailbox(&mailbox, LiveUiEvent::Phase(LivePhase::Reconnecting));
					continue;
				}
				break;
			},
		};
		persist_live_selection(&con, &mailbox, &selected);
		committed = LiveDeviceSelection {
			input:  CL_LIVE_INPUT_DEVICE.get(&con),
			output: CL_LIVE_OUTPUT_DEVICE.get(&con),
		};
		rollback_selection = None;
		let mut signaling = CodexSignalingClient::new(auth.clone());
		let mut connector = DirectSidebandConnector;
		let mut establishing = Box::pin(complete_live_transport(
			Arc::clone(&media),
			offer,
			&options,
			&mut signaling,
			&mut connector,
		));
		let mut close_requested = false;
		let mut recoverable_exit = false;
		let mut device_restart_requested = false;
		let established = loop {
			tokio::select! {
				result = &mut establishing => break Some(result),
				command = commands.recv_async() => match command {
					Ok(LiveRuntimeCommand::SetMuted(next)) => muted = next,
					Ok(LiveRuntimeCommand::Send(frames)) => pending_outbound.extend(frames),
					Ok(LiveRuntimeCommand::SelectInputDevice(next)) => {
						if next != selected.input
							&& validate_selected_device(&mailbox, &snapshot, next.as_str(), true)
						{
							rollback_selection = Some(committed.clone());
							selected.input = next;
							device_restart_requested = true;
							break None;
						}
					},
					Ok(LiveRuntimeCommand::SelectOutputDevice(next)) => {
						if next != selected.output
							&& validate_selected_device(&mailbox, &snapshot, next.as_str(), false)
						{
							rollback_selection = Some(committed.clone());
							selected.output = next;
							device_restart_requested = true;
							break None;
						}
					},
					Ok(LiveRuntimeCommand::Reconnect) => break None,
					Ok(LiveRuntimeCommand::Close) | Err(_) => {
						close_requested = true;
						break None;
					},
				},
				changed = watcher.changed() => match changed {
					Some(Ok(next)) if next != snapshot => {
						let previous = snapshot.clone();
						let (next, restart) = refresh_device_selection(
							&mailbox,
							&previous,
							next,
							&mut selected,
							active_input.as_ref(),
							active_output.as_ref(),
						);
						snapshot = next;
						if snapshot.microphone_permission != previous.microphone_permission {
							match active_live_permission(&mailbox, snapshot.microphone_permission) {
								LiveAuthorization::Granted => {},
								LiveAuthorization::Recoverable => {
									recoverable_exit = true;
									break None;
								},
								LiveAuthorization::Terminal => {
									close_requested = true;
									break None;
								},
							}
						}
						if restart {
							rollback_selection = None;
							device_restart_requested = true;
							break None;
						}
					},
					Some(Ok(_)) => {},
					Some(Err(error)) => post_mailbox(
						&mailbox,
						LiveUiEvent::Error {
							message: Str::new(format!("Could not refresh audio devices: {error}")),
							recoverable: true,
						},
					),
					None => {
						post_mailbox(
							&mailbox,
							LiveUiEvent::Error {
								message: Str::new_static("Audio device monitoring stopped unexpectedly."),
								recoverable: true,
							},
						);
						recoverable_exit = true;
						break None;
					},
				},
			}
		};
		drop(establishing);
		if close_requested || recoverable_exit {
			media.close().await;
			if close_requested {
				post_mailbox(&mailbox, LiveUiEvent::Closed);
			}
			return;
		}
		let Some(established) = established else {
			media.close().await;
			post_mailbox(&mailbox, LiveUiEvent::Phase(LivePhase::Reconnecting));
			if device_restart_requested {
				post_device_snapshot(Some(&mailbox), &snapshot, &selected);
			}
			continue;
		};
		let mut transport = match established {
			Ok(transport) => transport,
			Err(error) => {
				post_mailbox(&mailbox, LiveUiEvent::Error {
					message:     Str::new(format!("Could not connect live voice: {error}")),
					recoverable: true,
				});
				break;
			},
		};
		if muted {
			let _ = transport.media().peer().set_muted(true);
		}
		let mut ready = false;
		let mut input_level = 0.0;
		let mut output_level = 0.0;
		let mut initial_send_error = None;
		for frame in pending_outbound.drain(..) {
			if let Err(error) = send_sideband(transport.sideband_mut(), &frame).await {
				initial_send_error = Some(error);
				break;
			}
		}
		let exit = if let Some(error) = initial_send_error {
			post_mailbox(&mailbox, LiveUiEvent::Error {
				message:     Str::new(format!("Codex live sideband send failed: {error}")),
				recoverable: true,
			});
			LiveAttemptExit::Failed
		} else {
			loop {
				tokio::select! {
					command = commands.recv_async() => match command {
						Ok(LiveRuntimeCommand::SetMuted(next)) => {
							muted = next;
							if let Err(error) = transport.media().peer().set_muted(next) {
								post_mailbox(&mailbox, LiveUiEvent::Error {
									message: Str::new(format!("Could not change live microphone state: {error}")),
									recoverable: true,
								});
							} else if ready {
								post_mailbox(&mailbox, LiveUiEvent::Muted(next));
							}
						},
						Ok(LiveRuntimeCommand::Send(frames)) => {
							let mut failed = false;
							for frame in frames {
								if let Err(error) = send_sideband(transport.sideband_mut(), &frame).await {
									post_mailbox(&mailbox, LiveUiEvent::Error {
										message: Str::new(format!("Codex live sideband send failed: {error}")),
										recoverable: true,
									});
									failed = true;
									break;
								}
							}
							if failed {
								break LiveAttemptExit::Failed;
							}
						},
						Ok(LiveRuntimeCommand::SelectInputDevice(next)) => {
							if next != selected.input
								&& validate_selected_device(&mailbox, &snapshot, next.as_str(), true)
							{
								rollback_selection = Some(committed.clone());
								selected.input = next;
								break LiveAttemptExit::RestartDevices;
							}
						},
						Ok(LiveRuntimeCommand::SelectOutputDevice(next)) => {
							if next != selected.output
								&& validate_selected_device(&mailbox, &snapshot, next.as_str(), false)
							{
								rollback_selection = Some(committed.clone());
								selected.output = next;
								break LiveAttemptExit::RestartDevices;
							}
						},
						Ok(LiveRuntimeCommand::Reconnect) => break LiveAttemptExit::Reconnect,
						Ok(LiveRuntimeCommand::Close) | Err(_) => break LiveAttemptExit::Close,
					},
					changed = watcher.changed() => match changed {
						Some(Ok(next)) if next != snapshot => {
							let previous = snapshot.clone();
							let (next, restart) = refresh_device_selection(
								&mailbox,
								&previous,
								next,
								&mut selected,
								active_input.as_ref(),
								active_output.as_ref(),
							);
							snapshot = next;
							if snapshot.microphone_permission != previous.microphone_permission {
								match active_live_permission(
									&mailbox,
									snapshot.microphone_permission,
								) {
									LiveAuthorization::Granted => {},
									LiveAuthorization::Recoverable => {
										break LiveAttemptExit::Failed;
									},
									LiveAuthorization::Terminal => break LiveAttemptExit::Close,
								}
							}
							if restart {
								rollback_selection = None;
								break LiveAttemptExit::RestartDevices;
							}
						},
						Some(Ok(_)) => {},
						Some(Err(error)) => post_mailbox(
							&mailbox,
							LiveUiEvent::Error {
								message: Str::new(format!("Could not refresh audio devices: {error}")),
								recoverable: true,
							},
						),
						None => {
							post_mailbox(
								&mailbox,
								LiveUiEvent::Error {
									message: Str::new_static("Audio device monitoring stopped unexpectedly."),
									recoverable: true,
								},
							);
							break LiveAttemptExit::Failed;
						},
					},
					event = event_rx.recv_async() => match event {
						Ok(LiveRuntimeEvent::DataChannel(payload)) => {
							if !apply_live_payload(
								&mailbox,
								&mut transcripts,
								&mut dedup,
								&mut ready,
								muted,
								payload.as_str(),
							) {
								break LiveAttemptExit::Failed;
							}
						},
						Ok(LiveRuntimeEvent::InputLevel(level)) => {
							input_level = if muted { 0.0 } else { level };
							post_mailbox(&mailbox, LiveUiEvent::Levels {
								input: level_percent(input_level as f32),
								output: level_percent(output_level as f32),
							});
						},
						Ok(LiveRuntimeEvent::OutputLevel(level)) => {
							output_level = level;
							speaking.store(level > 0.015, Ordering::Release);
							post_mailbox(&mailbox, LiveUiEvent::Levels {
								input: level_percent(input_level as f32),
								output: level_percent(output_level as f32),
							});
							if ready {
								post_realtime(&mailbox, RealtimeEvent::Phase(if level > 0.015 {
									RealtimePhase::Speaking
								} else if muted {
									RealtimePhase::Muted
								} else {
									RealtimePhase::Listening
								}));
							}
						},
						Ok(LiveRuntimeEvent::Failure(message)) => {
							post_mailbox(&mailbox, LiveUiEvent::Error {
								message,
								recoverable: true,
							});
							break LiveAttemptExit::Failed;
						},
						Err(_) => break LiveAttemptExit::Failed,
					},
					sideband = receive_sideband(transport.sideband_mut()) => match sideband {
						Ok(Some(payload)) => {
							if !apply_live_payload(
								&mailbox,
								&mut transcripts,
								&mut dedup,
								&mut ready,
								muted,
								payload.as_str(),
							) {
								break LiveAttemptExit::Failed;
							}
						},
						Ok(None) => {
							post_mailbox(&mailbox, LiveUiEvent::Error {
								message: Str::new_static("Codex closed the live sideband."),
								recoverable: true,
							});
							break LiveAttemptExit::Failed;
						},
						Err(error) => {
							post_mailbox(&mailbox, LiveUiEvent::Error {
								message: Str::new(format!("Codex live sideband failed: {error}")),
								recoverable: true,
							});
							break LiveAttemptExit::Failed;
						},
					},
				}
			}
		};
		if matches!(exit, LiveAttemptExit::Close) {
			let _ = tokio::time::timeout(
				LIVE_CLOSE_TIMEOUT,
				send_sideband(
					transport.sideband_mut(),
					&omp_voice::transport::LiveClientMessage::SessionClose,
				),
			)
			.await;
		}
		let _ = tokio::time::timeout(LIVE_CLOSE_TIMEOUT, transport.sideband_mut().close(None)).await;
		transport.close().await;
		match exit {
			LiveAttemptExit::Reconnect | LiveAttemptExit::RestartDevices => {
				post_mailbox(&mailbox, LiveUiEvent::Phase(LivePhase::Reconnecting));
				if matches!(exit, LiveAttemptExit::RestartDevices) {
					post_device_snapshot(Some(&mailbox), &snapshot, &selected);
				}
			},
			LiveAttemptExit::Close => {
				post_mailbox(&mailbox, LiveUiEvent::Closed);
				return;
			},
			LiveAttemptExit::Failed => return,
		}
	}
}

fn apply_live_payload(
	mailbox: &HostMailbox,
	transcripts: &mut LiveTranscripts,
	dedup: &mut EventDeduplicator,
	ready: &mut bool,
	muted: bool,
	payload: &str,
) -> bool {
	if !dedup.admit(payload) {
		return true;
	}
	let Some(event) = parse_live_server_event(payload) else {
		return true;
	};
	let started = matches!(&event, LiveServerEvent::SessionStarted { .. });
	*ready |= started;
	if !apply_live_wire(mailbox, transcripts, event) {
		return false;
	}
	if started && muted {
		post_mailbox(mailbox, LiveUiEvent::Muted(true));
	}
	true
}

fn apply_live_wire(
	mailbox: &HostMailbox,
	transcripts: &mut LiveTranscripts,
	event: LiveServerEvent,
) -> bool {
	match event {
		LiveServerEvent::SessionStarted { .. } => {
			post_realtime(mailbox, RealtimeEvent::Ready);
		},
		LiveServerEvent::SessionUpdated { .. }
		| LiveServerEvent::OutputAudioDelta(_)
		| LiveServerEvent::Unknown(_) => {},
		LiveServerEvent::DelegationCreated { id, request } => {
			if !id.trim().is_empty() && !request.trim().is_empty() {
				mailbox.post(HostAction::LiveDelegation { id, request });
			}
		},
		LiveServerEvent::InputTranscriptAdded(text) => {
			if let Some(update) = transcripts.update(LiveTranscriptRole::User, text.as_str(), false) {
				post_mailbox(mailbox, LiveUiEvent::Transcript(update));
			}
		},
		LiveServerEvent::OutputTranscriptAdded(text) => {
			if let Some(update) =
				transcripts.update(LiveTranscriptRole::Assistant, text.as_str(), false)
			{
				post_mailbox(mailbox, LiveUiEvent::Transcript(update));
			}
		},
		LiveServerEvent::TurnDone { role, transcript } => {
			let role = match role {
				LiveTurnRole::User => LiveTranscriptRole::User,
				LiveTurnRole::Assistant => LiveTranscriptRole::Assistant,
			};
			if let Some(update) = transcripts.update(role, transcript.as_str(), true) {
				post_mailbox(mailbox, LiveUiEvent::Transcript(update));
			}
		},
		LiveServerEvent::Error(message) => {
			post_mailbox(mailbox, LiveUiEvent::Error { message, recoverable: true });
			return false;
		},
	}
	true
}

fn post_realtime(mailbox: &HostMailbox, event: RealtimeEvent) {
	if let Some(event) = LiveUiEvent::from_realtime(&event) {
		post_mailbox(mailbox, event);
	}
}

fn post_mailbox(mailbox: &HostMailbox, event: LiveUiEvent) {
	mailbox.post(HostAction::LiveEvent(event));
}

#[cfg(any(feature = "local-stt", test))]
fn stt_submission(text: Str, trigger: SttSubmitTrigger) -> (Str, bool) {
	let trimmed = text.trim();
	match trigger {
		SttSubmitTrigger::Never => (text, false),
		SttSubmitTrigger::Release => (text, trimmed.split_whitespace().count() >= 2),
		SttSubmitTrigger::ReleaseComplete => {
			(text, trimmed.ends_with(['.', '?', '!', '…', '。', '？', '！']))
		},
		SttSubmitTrigger::SaySubmit => {
			let without_punctuation = trimmed.trim_end_matches(|ch: char| {
				ch.is_ascii_punctuation() || matches!(ch, '…' | '。' | '？' | '！')
			});
			let start = without_punctuation
				.rfind(char::is_whitespace)
				.map_or(0, |index| index + 1);
			let trigger = &without_punctuation[start..];
			if trigger
				.as_bytes()
				.windows("submit".len())
				.any(|window| window.eq_ignore_ascii_case(b"submit"))
			{
				(Str::new(without_punctuation[..start].trim_end()), true)
			} else {
				(text, false)
			}
		},
	}
}

fn post_live(ctx: &Ctx, event: LiveUiEvent) {
	if let Some(mailbox) = ctx.user::<HostMailbox>() {
		mailbox.post(HostAction::LiveEvent(event));
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[cfg(feature = "local-stt")]
	#[test]
	fn streaming_segments_normalize_spacing_without_touching_first_insert() {
		assert_eq!(normalize_stt_text("  hello \n world ", false), "hello world");
		assert_eq!(normalize_stt_text("next   phrase", true), " next phrase");
		assert!(normalize_stt_text(" \n ", true).is_empty());
	}

	#[test]
	fn streaming_failures_keep_stable_typed_categories() {
		assert_eq!(SttError::AudioLimit.kind(), SttFailureKind::AudioLimit);
		assert_eq!(SttError::Backpressure.kind(), SttFailureKind::Backpressure);
	}

	#[test]
	fn pi_submit_triggers_preserve_and_trim_the_dictation_contract() {
		assert_eq!(
			stt_submission(Str::new_static("one word"), SttSubmitTrigger::Never),
			(Str::new_static("one word"), false)
		);
		assert_eq!(
			stt_submission(Str::new_static("one"), SttSubmitTrigger::Release),
			(Str::new_static("one"), false)
		);
		assert_eq!(
			stt_submission(Str::new_static("two words"), SttSubmitTrigger::Release),
			(Str::new_static("two words"), true)
		);
		assert_eq!(
			stt_submission(Str::new_static("done。"), SttSubmitTrigger::ReleaseComplete),
			(Str::new_static("done。"), true)
		);
		assert_eq!(
			stt_submission(Str::new_static("keep this reSUBMIT!"), SttSubmitTrigger::SaySubmit),
			(Str::new_static("keep this"), true)
		);
		assert_eq!(
			stt_submission(Str::new_static("submit"), SttSubmitTrigger::SaySubmit),
			(Str::new_static(""), true)
		);
	}
}

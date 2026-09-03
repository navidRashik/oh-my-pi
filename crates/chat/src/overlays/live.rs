//! Observer-local `/live` voice surface.
//!
//! The application owns microphones, provider transport, reconnect policy,
//! and delegated agent work. This panel is a retained projection of typed
//! [`LiveUiEvent`] values and emits typed [`LiveControl`] requests; it never
//! reads or mutates controller state (ADR 0005).

use std::time::Duration;

use omp_core::{Str, sf};
use omp_tui::{
	Component, Frame, Key, MouseReport, PaintCtx, Prop, Props, Rect, Size, Slot, Style, Ui,
	UiContext, UiEvent, dom, next_slot,
};
use strum::{Display, IntoStaticStr};

use super::{Panel, PanelAnchor, PanelCx, PanelEvent, PanelNote};

/// Pi's Codex-backed realtime model. It is intentionally not the chat model.
pub const LIVE_MODEL: &str = "gpt-live-1-codex";
/// Pi's visualizer cadence and peak-decay cadence.
const FRAME_INTERVAL: Duration = Duration::from_millis(80);
/// Meter value used by the reusable progress component.
const LEVEL_MAX: u16 = 100;
/// Voices accepted by the Codex live endpoint, in pi settings order.
pub const LIVE_VOICES: &[&str] =
	&["arbor", "breeze", "cove", "ember", "juniper", "maple", "sol", "spruce", "vale"];

/// Clamps a native RMS level to the integer percentage carried by
/// [`LiveUiEvent::Levels`].
#[must_use]
pub fn level_percent(level: f32) -> u16 {
	if !level.is_finite() || level <= 0.0 {
		0
	} else {
		(level.min(1.0) * f32::from(LEVEL_MAX)).round() as u16
	}
}

/// Realtime-call presentation phase.
#[derive(Clone, Copy, Debug, Display, Eq, IntoStaticStr, PartialEq)]
#[strum(serialize_all = "lowercase")]
pub enum LivePhase {
	/// Waiting for the operating system's microphone decision.
	Permission,
	/// Establishing signaling, media, and sideband channels.
	Connecting,
	/// Retrying a recoverable transport failure.
	Reconnecting,
	/// Waiting for caller audio.
	Listening,
	/// Running a delegated coding turn.
	Working,
	/// Playing realtime assistant audio.
	Speaking,
	/// Connected with caller audio suppressed.
	Muted,
	/// Gracefully releasing call resources.
	Closing,
	/// Terminal or recoverable failure.
	Error,
}

/// Speaker represented by a live transcript update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveTranscriptRole {
	/// Caller microphone transcript.
	User,
	/// Realtime assistant transcript.
	Assistant,
}

/// Incremental or finalized transcript for one role-local turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveTranscript {
	/// Transcript speaker.
	pub role:      LiveTranscriptRole,
	/// Monotonic role-local turn number.
	pub turn:      u64,
	/// Latest complete text for the turn.
	pub text:      Str,
	/// Whether this turn text is final.
	pub finalized: bool,
}

/// One selectable audio endpoint published by the application's device host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveDevice {
	/// Stable platform device identity.
	pub id:         Str,
	/// Human-readable device label.
	pub label:      Str,
	/// Whether the operating system currently uses this endpoint by default.
	pub is_default: bool,
	/// Whether this endpoint is currently selected.
	pub selected:   bool,
}

/// Operating-system microphone permission state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MicrophonePermission {
	/// The platform cannot distinguish authorization before capture opens.
	Unknown,
	/// Permission has not settled yet.
	Requesting,
	/// Capture may open.
	Granted,
	/// The user denied capture.
	Denied,
	/// Device policy prevents capture and user retry cannot change it.
	Restricted,
	/// Native microphone capture is unavailable.
	Unavailable,
}

/// Typed observer event posted by the live controller through `HostMailbox`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveUiEvent {
	/// Call phase changed.
	Phase(LivePhase),
	/// Microphone permission changed.
	Permission(MicrophonePermission),
	/// Clamped RMS levels as integer percentages.
	Levels {
		/// Microphone input level.
		input:  u16,
		/// Speaker output level.
		output: u16,
	},
	/// Incremental or final transcript changed.
	Transcript(LiveTranscript),
	/// Effective mute state changed.
	Muted(bool),
	/// Available input and output endpoints changed.
	Devices {
		/// Microphone endpoints.
		input:  Vec<LiveDevice>,
		/// Speaker endpoints.
		output: Vec<LiveDevice>,
	},
	/// A recoverable reconnect attempt is scheduled or underway.
	Reconnect {
		/// One-based attempt number.
		attempt: u8,
		/// Maximum attempts before the error becomes terminal.
		maximum: u8,
	},
	/// Classified controller failure.
	Error {
		/// User-facing diagnostic.
		message:     Str,
		/// Whether `R` may retry in place.
		recoverable: bool,
	},
	/// Controller cleanup completed.
	Closed,
}

impl LiveUiEvent {
	/// Projects a provider-neutral inference event into observer-only live UI
	/// state. Audio bytes and delegation requests stay with the controller.
	#[must_use]
	pub fn from_realtime(event: &omp_inference::answer::RealtimeEvent) -> Option<Self> {
		use omp_inference::answer::{RealtimeEvent, RealtimePhase, RealtimeTranscriptRole};

		match event {
			RealtimeEvent::Ready => Some(Self::Phase(LivePhase::Listening)),
			RealtimeEvent::Phase(phase) => Some(Self::Phase(match phase {
				RealtimePhase::Connecting => LivePhase::Connecting,
				RealtimePhase::Listening => LivePhase::Listening,
				RealtimePhase::Working => LivePhase::Working,
				RealtimePhase::Speaking => LivePhase::Speaking,
				RealtimePhase::Muted => LivePhase::Muted,
				RealtimePhase::Closing => LivePhase::Closing,
				RealtimePhase::Error => LivePhase::Error,
			})),
			RealtimeEvent::Transcript(transcript) => Some(Self::Transcript(LiveTranscript {
				role:      match transcript.role {
					RealtimeTranscriptRole::User => LiveTranscriptRole::User,
					RealtimeTranscriptRole::Assistant => LiveTranscriptRole::Assistant,
				},
				turn:      transcript.turn,
				text:      transcript.text.clone(),
				finalized: transcript.finalized,
			})),
			RealtimeEvent::Muted(muted) => Some(Self::Muted(*muted)),
			RealtimeEvent::CloseReceipt(_) | RealtimeEvent::Closed => Some(Self::Closed),
			RealtimeEvent::Chat(_)
			| RealtimeEvent::Audio(_)
			| RealtimeEvent::InputCommitted
			| RealtimeEvent::Delegation(_) => None,
		}
	}
}

/// Requests emitted by the live panel to the controller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveControl {
	/// Start a call with the archived voice/device choices.
	Start,
	/// Gracefully stop the call and close the panel.
	Stop,
	/// Toggle microphone input while keeping output connected.
	ToggleMute,
	/// Retry a recoverable failed connection.
	Reconnect,
	/// Select and archive a realtime voice.
	SelectVoice(Str),
	/// Select and archive a microphone endpoint.
	SelectInputDevice(Str),
	/// Select and archive a speaker endpoint.
	SelectOutputDevice(Str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Picker {
	Voice,
	Input,
	Output,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TranscriptState {
	turn:      u64,
	text:      Str,
	finalized: bool,
}

/// Pure retained reducer for the `/live` actor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveViewState {
	phase:          LivePhase,
	permission:     Option<MicrophonePermission>,
	muted:          bool,
	input_level:    u16,
	output_level:   u16,
	input_peak:     u16,
	output_peak:    u16,
	user:           TranscriptState,
	assistant:      TranscriptState,
	input_devices:  Vec<LiveDevice>,
	output_devices: Vec<LiveDevice>,
	voice:          Str,
	reconnect:      Option<(u8, u8)>,
	error:          Option<(Str, bool)>,
	closed:         bool,
}

impl LiveViewState {
	/// Creates the connecting state for a new call.
	#[must_use]
	pub fn new(voice: impl Into<Str>) -> Self {
		let voice = voice.into();
		let voice = if LIVE_VOICES.contains(&voice.as_str()) {
			voice
		} else {
			Str::new_static("sol")
		};
		Self {
			phase: LivePhase::Connecting,
			permission: None,
			muted: false,
			input_level: 0,
			output_level: 0,
			input_peak: 0,
			output_peak: 0,
			user: TranscriptState::default(),
			assistant: TranscriptState::default(),
			input_devices: Vec::new(),
			output_devices: Vec::new(),
			voice,
			reconnect: None,
			error: None,
			closed: false,
		}
	}

	/// Applies one controller event. Stale role-local transcript updates are
	/// ignored so an async final cannot replace a newer turn.
	pub fn apply(&mut self, event: &LiveUiEvent) {
		match event {
			LiveUiEvent::Phase(phase) => {
				self.phase = *phase;
				if *phase != LivePhase::Error {
					self.error = None;
				}
				if *phase != LivePhase::Reconnecting {
					self.reconnect = None;
				}
			},
			LiveUiEvent::Permission(permission) => {
				self.permission = Some(*permission);
				self.phase = match permission {
					MicrophonePermission::Unknown | MicrophonePermission::Requesting => {
						LivePhase::Permission
					},
					MicrophonePermission::Granted => LivePhase::Connecting,
					MicrophonePermission::Denied
					| MicrophonePermission::Restricted
					| MicrophonePermission::Unavailable => LivePhase::Error,
				};
				self.error = match permission {
					MicrophonePermission::Denied => Some((
						Str::new_static(
							"Microphone access was denied. Allow access in system settings, then retry.",
						),
						true,
					)),
					MicrophonePermission::Restricted => Some((
						Str::new_static("Microphone access is restricted by system policy."),
						false,
					)),
					MicrophonePermission::Unavailable => Some((
						Str::new_static("No native microphone permission service is available."),
						false,
					)),
					MicrophonePermission::Unknown
					| MicrophonePermission::Requesting
					| MicrophonePermission::Granted => None,
				};
			},
			LiveUiEvent::Levels { input, output } => {
				self.input_level = (*input).min(LEVEL_MAX);
				self.output_level = (*output).min(LEVEL_MAX);
				self.input_peak = self.input_level;
				self.output_peak = self.output_level;
			},
			LiveUiEvent::Transcript(update) => {
				let slot = match update.role {
					LiveTranscriptRole::User => &mut self.user,
					LiveTranscriptRole::Assistant => &mut self.assistant,
				};
				if update.turn < slot.turn {
					return;
				}
				if update.turn == slot.turn && slot.finalized && !update.finalized {
					return;
				}
				slot.turn = update.turn;
				slot.text = update.text.trim().into();
				slot.finalized = update.finalized;
			},
			LiveUiEvent::Muted(muted) => {
				self.muted = *muted;
				self.input_level = 0;
				self.input_peak = 0;
				self.phase = if *muted {
					LivePhase::Muted
				} else {
					LivePhase::Listening
				};
			},
			LiveUiEvent::Devices { input, output } => {
				self.input_devices.clone_from(input);
				self.output_devices.clone_from(output);
			},
			LiveUiEvent::Reconnect { attempt, maximum } => {
				self.reconnect = Some((*attempt, *maximum));
				self.phase = LivePhase::Reconnecting;
				self.error = None;
			},
			LiveUiEvent::Error { message, recoverable } => {
				self.error = Some((message.clone(), *recoverable));
				self.phase = LivePhase::Error;
			},
			LiveUiEvent::Closed => {
				self.closed = true;
				self.phase = LivePhase::Closing;
				self.input_level = 0;
				self.output_level = 0;
			},
		}
	}
}

/// Allocation-free two-row microphone spectrum used by the live panel.
///
/// The waveform is presentation-only: the controller sends one RMS value and
/// the actor derives animation from its paint frame. ASCII terminals receive
/// the same geometry with `#`/`.` cells.
pub struct LiveSpectrum {
	props: Props,
	slot:  Slot,
	level: u16,
	tone:  LivePhase,
}

impl LiveSpectrum {
	/// Creates a spectrum snapshot.
	#[must_use]
	pub fn new(level: u16, tone: LivePhase) -> Self {
		let mut props = Props::new();
		props.set(Prop::Grow, true);
		Self { props, slot: next_slot(), level: level.min(LEVEL_MAX), tone }
	}
}

impl Component for LiveSpectrum {
	fn props(&self) -> &Props {
		&self.props
	}

	fn props_mut(&mut self) -> &mut Props {
		&mut self.props
	}

	fn slot(&self) -> Slot {
		self.slot
	}

	fn measure(&mut self, _ctx: &UiContext) -> (u16, u16) {
		(2, u16::MAX)
	}

	fn height(&mut self, _ctx: &UiContext, _width: u16) -> u16 {
		2
	}

	fn paint(&mut self, pc: &mut PaintCtx<'_>, rect: Rect) {
		const BLOCKS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
		pc.wake(self.slot, pc.now + FRAME_INTERVAL);
		let energy = if self.tone == LivePhase::Muted {
			0.0
		} else {
			(f32::from(self.level) / 20.0).sqrt().min(1.0)
		};
		let color = match self.tone {
			LivePhase::Muted | LivePhase::Closing => pc.ctx.theme.muted,
			LivePhase::Error => pc.ctx.theme.err,
			_ => pc.ctx.theme.ok,
		};
		let style = Style::new().fg(color);
		let phase = (pc.now.as_millis() / FRAME_INTERVAL.as_millis()) as f32;
		for column in 0..rect.width {
			let x = f32::from(column);
			let carrier = 0.5 + 0.5 * (phase.mul_add(0.43, x * 0.71)).sin();
			let shimmer = 0.5 + 0.5 * (phase.mul_add(0.19, -(x * 1.17))).sin();
			let height =
				(energy * carrier.mul_add(0.5, shimmer.mul_add(0.2, 0.3)) * 16.0).round() as i16;
			for row in 0..rect.height.min(2) {
				let units = (height - i16::try_from((1 - row) * 8).unwrap_or(0)).clamp(0, 8);
				let glyph = if pc.ctx.charset == omp_tui::Charset::Ascii {
					if units > 0 { "#" } else { "." }
				} else {
					BLOCKS[units as usize]
				};
				pc.frame.put(rect.x + column, rect.y + row, glyph, style);
			}
		}
		self.level = self.level.saturating_mul(84) / 100;
	}
}

/// Retained bottom panel replacing the composer while a live call is active.
pub struct LivePanel {
	state:     LiveViewState,
	picker:    Option<Picker>,
	selection: usize,
	ui:        Ui,
	ctx:       UiContext,
	size:      Size,
	next_wake: Option<Duration>,
	pending:   Option<LiveControl>,
}

impl LivePanel {
	/// Opens the visualizer with the currently archived realtime voice.
	#[must_use]
	pub fn open(voice: impl Into<Str>, cx: &PanelCx<'_>) -> Self {
		let mut panel = Self {
			state:     LiveViewState::new(voice),
			picker:    None,
			selection: 0,
			ui:        Ui::from_root(dom! { <col/> }, cx.viewport.width, cx.ui.clone()),
			ctx:       cx.ui.clone(),
			size:      cx.viewport,
			next_wake: Some(Duration::ZERO),
			pending:   Some(LiveControl::Start),
		};
		panel.rebuild();
		panel
	}

	/// Current presentation state, for debug inspectors and headless actors.
	#[must_use]
	pub const fn state(&self) -> &LiveViewState {
		&self.state
	}

	fn options(&self, picker: Picker) -> Vec<(Str, Str, bool)> {
		match picker {
			Picker::Voice => LIVE_VOICES
				.iter()
				.copied()
				.map(|voice| {
					(Str::new_static(voice), Str::new_static(voice), voice == self.state.voice.as_str())
				})
				.collect(),
			Picker::Input => self
				.state
				.input_devices
				.iter()
				.map(|device| {
					let label = if device.is_default {
						sf!("{} · system default", device.label)
					} else {
						device.label.clone()
					};
					(device.id.clone(), label, device.selected)
				})
				.collect(),
			Picker::Output => self
				.state
				.output_devices
				.iter()
				.map(|device| {
					let label = if device.is_default {
						sf!("{} · system default", device.label)
					} else {
						device.label.clone()
					};
					(device.id.clone(), label, device.selected)
				})
				.collect(),
		}
	}

	fn open_picker(&mut self, picker: Picker) {
		let options = self.options(picker);
		if options.is_empty() {
			return;
		}
		self.selection = options
			.iter()
			.position(|(_, _, selected)| *selected)
			.unwrap_or(0);
		self.picker = Some(picker);
		self.rebuild();
	}

	fn step(&mut self, delta: isize) {
		let Some(picker) = self.picker else { return };
		let count = self.options(picker).len();
		if count == 0 {
			return;
		}
		self.selection = (self.selection as isize + delta).rem_euclid(count as isize) as usize;
		self.rebuild();
	}

	fn select(&mut self) -> PanelEvent {
		let Some(picker) = self.picker else {
			return PanelEvent::Consumed;
		};
		let Some((id, ..)) = self.options(picker).get(self.selection).cloned() else {
			return PanelEvent::Consumed;
		};
		self.picker = None;
		if picker == Picker::Voice {
			self.state.voice = id.clone();
		}
		self.rebuild();
		PanelEvent::Live(match picker {
			Picker::Voice => LiveControl::SelectVoice(id),
			Picker::Input => LiveControl::SelectInputDevice(id),
			Picker::Output => LiveControl::SelectOutputDevice(id),
		})
	}

	fn control(&mut self, id: &str) -> PanelEvent {
		if let Some(index) = id
			.strip_prefix("live-option:")
			.and_then(|index| index.parse::<usize>().ok())
		{
			self.selection = index;
			return self.select();
		}
		match id {
			"live-mute" => PanelEvent::Live(LiveControl::ToggleMute),
			"live-voice" => {
				self.open_picker(Picker::Voice);
				PanelEvent::Consumed
			},
			"live-input" => {
				self.open_picker(Picker::Input);
				PanelEvent::Consumed
			},
			"live-output" => {
				self.open_picker(Picker::Output);
				PanelEvent::Consumed
			},
			"live-reconnect" => PanelEvent::Live(LiveControl::Reconnect),
			"live-end" => PanelEvent::Live(LiveControl::Stop),
			_ => PanelEvent::Consumed,
		}
	}

	fn rebuild(&mut self) {
		let phase = self.state.phase.to_string();
		let voice = self.state.voice.clone();
		let input = self.state.input_peak;
		let output = self.state.output_peak;
		let spectrum = LiveSpectrum::new(input, self.state.phase);
		let muted = self.state.muted;
		let user = self.state.user.text.clone();
		let user_final = self.state.user.finalized;
		let assistant = self.state.assistant.text.clone();
		let assistant_final = self.state.assistant.finalized;
		let reconnect = self.state.reconnect;
		let error = self.state.error.clone();
		let can_retry = error.as_ref().is_some_and(|(_, recoverable)| *recoverable);
		let input_name = (!self.state.input_devices.is_empty()).then(|| {
			self
				.state
				.input_devices
				.iter()
				.find(|device| device.selected)
				.map_or_else(|| Str::new_static("Microphone"), |device| device.label.clone())
		});
		let output_name = (!self.state.output_devices.is_empty()).then(|| {
			self
				.state
				.output_devices
				.iter()
				.find(|device| device.selected)
				.map_or_else(|| Str::new_static("Speaker"), |device| device.label.clone())
		});
		let options = self
			.picker
			.map(|picker| self.options(picker))
			.unwrap_or_default();
		let picker_title = self.picker.map(|picker| match picker {
			Picker::Voice => "Voice",
			Picker::Input => "Microphone",
			Picker::Output => "Speaker",
		});
		let compact = self.size.width < 56;
		let status_tone = match self.state.phase {
			LivePhase::Error => "err",
			LivePhase::Muted | LivePhase::Closing => "muted",
			LivePhase::Working | LivePhase::Reconnecting => "warn",
			LivePhase::Speaking => "accent",
			LivePhase::Permission | LivePhase::Connecting => "info",
			LivePhase::Listening => "ok",
		};
		let tree = dom! {
			<box border=round bc={status_tone} pad-x=1 title_pad=3>
				<row kind=title gap=1>
					if matches!(self.state.phase, LivePhase::Connecting | LivePhase::Permission | LivePhase::Working | LivePhase::Reconnecting | LivePhase::Closing) { <spinner kind=status/> }
					else if self.state.phase == LivePhase::Error { <i:error fg=err/> }
					else if muted { <icon name="muted" fg=muted/> }
					else { <icon name="mic" fg={status_tone}/> }
					<text bold fg={status_tone}>{"Live voice"}</text>
					<text fg=muted>{phase}</text>
					<spacer grow/>
					if !compact { <text fg=muted>{"Codex"}</text><text fg=muted>{"·"}</text><text fg=muted>{LIVE_MODEL}</text> }
				</row>
				<col gap=0>
					{spectrum}
					<row gap=1 fg=muted>
						<text>{sf!("mic {input}%")}</text>
						<text>{"·"}</text>
						<text>{sf!("speaker {output}%")}</text>
					</row>
					if !user.is_empty() {
						<row gap=1><text bold fg=accent>{"You"}</text><text fg=accent grow truncate>{user}</text>
							if !user_final { <spinner kind=status/> }
						</row>
					} else {
						<text fg=muted>{if muted { "Microphone muted" } else { "Speak naturally — your words appear here" }}</text>
					}
					if !assistant.is_empty() {
						<row gap=1><text bold fg=output>{"Assistant"}</text><text fg=output grow truncate>{assistant}</text>
							if !assistant_final { <spinner kind=status/> }
						</row>
					}
					if let Some((attempt, maximum)) = reconnect {
						<row gap=1><spinner kind=status/><text fg=warn>{sf!("Reconnecting · attempt {attempt} of {maximum}")}</text></row>
					}
											if let Some((message, _)) = error {
							<callout kind=error>{message}</callout>
						}
						if let Some(title) = picker_title {
						<hr title={title} title_pad=3 bc=muted/>
						for (index, (_, label, selected)) in options.into_iter().enumerate() {
							<row gap=1>
								if index == self.selection { <icon name="cursor" fg=accent/> } else { <pre>{"  "}</pre> }
								if selected { <i:checked fg=ok/> } else { <i:unchecked fg=muted/> }
								<button id={sf!("live-option:{index}")} variant=ghost active={index == self.selection}>{label}</button>
							</row>
						}
						<text fg=muted>{"↑/↓ select · Enter apply · Esc back"}</text>
					} else {
						<row gap=1>
							<button id="live-mute" variant=soft active={muted}>{if muted { "Unmute" } else { "Mute" }}</button>
							<button id="live-voice" variant=soft>{sf!("Voice: {voice}")}</button>
							if let Some(input_name) = input_name { <button id="live-input" variant=ghost>{sf!("Mic: {input_name}")}</button> }
							if let Some(output_name) = output_name { <button id="live-output" variant=ghost>{sf!("Speaker: {output_name}")}</button> }
							if can_retry { <button id="live-reconnect" variant=tint color=warn active>{"Reconnect"}</button> }
							<spacer grow/>
							<button id="live-end" variant=ghost>{"End"}</button>
						</row>
						<text fg=muted>{if compact { "space mute · v voice · esc end" } else { "Space mute · V voice · D microphone · Shift+D speaker · R reconnect · Esc end" }}</text>
					}
				</col>
			</box>
		};
		self.ui = Ui::from_root(tree, self.size.width, self.ctx.clone());
	}
}

impl Panel for LivePanel {
	fn id(&self) -> &'static str {
		"live"
	}

	fn anchor(&self) -> PanelAnchor {
		PanelAnchor::Bottom
	}

	fn key(&mut self, key: Key) -> PanelEvent {
		if self.picker.is_some() {
			return match key {
				Key::Esc => {
					self.picker = None;
					self.rebuild();
					PanelEvent::Consumed
				},
				Key::Up | Key::Char('k') => {
					self.step(-1);
					PanelEvent::Consumed
				},
				Key::Down | Key::Char('j') => {
					self.step(1);
					PanelEvent::Consumed
				},
				Key::Enter | Key::Space => self.select(),
				_ => PanelEvent::Consumed,
			};
		}
		match key {
			Key::Esc | Key::Ctrl('c') => PanelEvent::Live(LiveControl::Stop),
			Key::Space | Key::Char('m' | 'M') => PanelEvent::Live(LiveControl::ToggleMute),
			Key::Char('v' | 'V') => {
				self.open_picker(Picker::Voice);
				PanelEvent::Consumed
			},
			Key::Char('d') => {
				self.open_picker(Picker::Input);
				PanelEvent::Consumed
			},
			Key::Char('D') => {
				self.open_picker(Picker::Output);
				PanelEvent::Consumed
			},
			Key::Char('r' | 'R') if self.state.error.as_ref().is_some_and(|(_, retry)| *retry) => {
				PanelEvent::Live(LiveControl::Reconnect)
			},
			_ => PanelEvent::Consumed,
		}
	}

	fn mouse(&mut self, report: MouseReport) -> PanelEvent {
		match self
			.ui
			.handle_mouse_with_mods(report.col, report.row, report.kind, report.mods)
		{
			UiEvent::Pressed(id) => self.control(id.as_str()),
			UiEvent::Cancel => {
				if self.picker.take().is_some() {
					self.rebuild();
					PanelEvent::Consumed
				} else {
					PanelEvent::Live(LiveControl::Stop)
				}
			},
			_ => PanelEvent::Consumed,
		}
	}

	fn notify(&mut self, note: PanelNote<'_>) -> PanelEvent {
		let PanelNote::Live(event) = note else {
			return PanelEvent::Ignored;
		};
		self.state.apply(event);
		if let Some(picker) = self.picker {
			self.selection = self
				.selection
				.min(self.options(picker).len().saturating_sub(1));
		}
		self.rebuild();
		PanelEvent::Consumed
	}

	fn frame(&mut self, viewport: Size) -> &Frame {
		if self.size != viewport {
			self.size = viewport;
			self.rebuild();
		}
		self.ui.frame()
	}

	fn tick(&mut self, now: Duration) -> bool {
		if self.state.closed {
			return false;
		}
		self.next_wake = Some(now + FRAME_INTERVAL);
		let start_due = self.pending.is_some();
		start_due | self.ui.tick(now)
	}

	fn next_wake(&self) -> Option<Duration> {
		self.next_wake
	}

	fn finished(&self) -> bool {
		self.state.closed
	}

	fn settled(&mut self) -> Option<PanelEvent> {
		self.pending.take().map(PanelEvent::Live)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn permission_states_preserve_recovery_policy() {
		let mut state = LiveViewState::new("sol");
		state.apply(&LiveUiEvent::Permission(MicrophonePermission::Denied));
		assert_eq!(state.phase, LivePhase::Error);
		assert!(
			state
				.error
				.as_ref()
				.is_some_and(|(_, recoverable)| *recoverable)
		);

		state.apply(&LiveUiEvent::Permission(MicrophonePermission::Restricted));
		assert_eq!(state.phase, LivePhase::Error);
		assert!(
			state
				.error
				.as_ref()
				.is_some_and(|(_, recoverable)| !*recoverable)
		);

		state.apply(&LiveUiEvent::Permission(MicrophonePermission::Granted));
		assert_eq!(state.phase, LivePhase::Connecting);
		assert!(state.error.is_none());
	}

	#[test]
	fn hotplug_snapshot_replaces_removed_device_rows() {
		let mut state = LiveViewState::new("sol");
		state.apply(&LiveUiEvent::Devices {
			input:  vec![LiveDevice {
				id:         Str::new_static("old-mic"),
				label:      Str::new_static("Old microphone"),
				is_default: true,
				selected:   true,
			}],
			output: vec![],
		});
		state.apply(&LiveUiEvent::Devices {
			input:  vec![LiveDevice {
				id:         Str::new_static("new-mic"),
				label:      Str::new_static("New microphone"),
				is_default: true,
				selected:   true,
			}],
			output: vec![],
		});

		assert_eq!(state.input_devices.len(), 1);
		assert_eq!(state.input_devices[0].id.as_str(), "new-mic");
	}
}

//! Voice command-stream variables and one-shot legacy migration keys.

use omp_core::Str;
pub use omp_inference::speech_settings::{
	AI_TTS_PROVIDER, CL_TTS_MODEL, CL_TTS_VOICE, KokoroVoice, TtsModel, TtsProvider,
};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, IntoStaticStr};

/// Dictation auto-submit policy.
#[derive(
	Clone,
	Copy,
	Debug,
	Default,
	Deserialize,
	Display,
	EnumString,
	Eq,
	IntoStaticStr,
	PartialEq,
	Serialize,
	strum::VariantNames,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case", ascii_case_insensitive)]
pub enum SttSubmitTrigger {
	/// Never submit automatically.
	#[default]
	Never,
	/// Submit a sufficiently long utterance when capture is released.
	Release,
	/// Submit only a complete sentence when capture is released.
	ReleaseComplete,
	/// Submit when the user speaks the submit trigger.
	SaySubmit,
}

/// Which assistant output is vocalized.
#[derive(
	Clone,
	Copy,
	Debug,
	Default,
	Deserialize,
	Display,
	EnumString,
	Eq,
	IntoStaticStr,
	PartialEq,
	Serialize,
	strum::VariantNames,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum SpeechMode {
	/// Speak assistant messages and thinking.
	All,
	/// Speak assistant messages without thinking.
	#[default]
	Assistant,
	/// Speak only the final message at turn completion.
	Yield,
}

omp_con::con_enum!(SttSubmitTrigger);
omp_con::con_enum!(SpeechMode);

/// Speech recognition model selection.
#[derive(
	Clone, Copy, Debug, Default, EnumString, Eq, IntoStaticStr, PartialEq, strum::VariantNames,
)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum SttModel {
	/// Parakeet TDT v3.
	#[default]
	Parakeet,
	/// Whisper Base.
	Fast,
	/// Whisper Small.
	Balanced,
	/// Whisper Large v3 Turbo.
	Turbo,
}

omp_con::con_enum!(SttModel);

/// Realtime provider voice selection.
#[derive(
	Clone, Copy, Debug, Default, EnumString, Eq, IntoStaticStr, PartialEq, strum::VariantNames,
)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum LiveVoice {
	/// Arbor.
	Arbor,
	/// Breeze.
	Breeze,
	/// Cove.
	Cove,
	/// Ember.
	Ember,
	/// Juniper.
	Juniper,
	/// Maple.
	Maple,
	/// Sol.
	#[default]
	Sol,
	/// Spruce.
	Spruce,
	/// Vale.
	Vale,
}

omp_con::con_enum!(LiveVoice);

omp_con::var! {
	/// Enables microphone dictation.
	pub static CL_VOICE_STT_ENABLED = cl_voice_stt_enabled: bool { default: false, flags: archive };
	/// Speech recognition language hint.
	pub static CL_STT_LANGUAGE = cl_stt_language: Str { default: Str::new_static("en"), flags: archive };
	/// Local speech recognition model.
	pub static CL_STT_MODEL = cl_stt_model: SttModel { default: SttModel::Parakeet, flags: archive };
	/// Dictation submission policy.
	pub static CL_STT_SUBMIT_TRIGGER = cl_stt_submit_trigger: SttSubmitTrigger { default: SttSubmitTrigger::Never, flags: archive };
	/// Enables generated speech tools.
	pub static CL_SPEECHGEN_ENABLED = cl_speechgen_enabled: bool { default: false, flags: archive };
	/// Enables assistant vocalization.
	pub static CL_SPEECH_ENABLED = cl_speech_enabled: bool { default: false, flags: archive };
	/// Selects assistant channels to vocalize.
	pub static CL_SPEECH_MODE = cl_speech_mode: SpeechMode { default: SpeechMode::Assistant, flags: archive };
	/// Enables natural speech rewriting.
	pub static CL_SPEECH_ENHANCED = cl_speech_enhanced: bool { default: false, flags: archive };
	/// Assistant vocalization voice.
	pub static CL_SPEECH_VOICE = cl_speech_voice: KokoroVoice { default: KokoroVoice::AfHeart, flags: archive };
	/// Realtime provider voice.
	pub static CL_LIVE_VOICE = cl_live_voice: LiveVoice { default: LiveVoice::Sol, flags: archive };
	/// Stable realtime microphone device ID; empty selects the system default.
	pub static CL_LIVE_INPUT_DEVICE = cl_live_input_device: Str { default: Str::default(), flags: archive };
	/// Stable realtime speaker device ID; empty selects the system default.
	pub static CL_LIVE_OUTPUT_DEVICE = cl_live_output_device: Str { default: Str::default(), flags: archive };
}

/// Legacy settings keys and their command-stream replacements.
pub const LEGACY_CONVAR_MAPPINGS: &[(&str, &str)] = &[
	("stt.enabled", "cl_voice_stt_enabled"),
	("stt.language", "cl_stt_language"),
	("stt.modelName", "cl_stt_model"),
	("stt.submitTrigger", "cl_stt_submit_trigger"),
	("tts.localModel", "cl_tts_model"),
	("tts.localVoice", "cl_tts_voice"),
	("speechgen.enabled", "cl_speechgen_enabled"),
	("speech.enabled", "cl_speech_enabled"),
	("speech.mode", "cl_speech_mode"),
	("speech.enhanced", "cl_speech_enhanced"),
	("speech.voice", "cl_speech_voice"),
	("live.voice", "cl_live_voice"),
	("providers.tts", "ai_tts_provider"),
];

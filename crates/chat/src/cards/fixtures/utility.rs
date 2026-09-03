use super::{CardFixture, FixtureState};

const fn states(
	streaming: &'static str,
	args: &'static str,
	result: &'static str,
	fault: &'static str,
) -> [FixtureState; 4] {
	[
		FixtureState { args: streaming, update: None, result: None, fault: None },
		FixtureState { args, update: None, result: None, fault: None },
		FixtureState { args, update: None, result: Some(result), fault: None },
		FixtureState { args, update: None, result: None, fault: Some(fault) },
	]
}

pub(super) const FIXTURES: &[CardFixture] = &[
	CardFixture {
		tool:   "checkpoint",
		title:  "Checkpoint",
		states: states(
			r#"{"goal":"Investigate the parser without changing"#,
			r#"{"goal":"Investigate the parser without changing the public API"}"#,
			r#"{"token":"checkpoint-01","goal":"Investigate the parser without changing the public API","started_at":1788400000000}"#,
			r#"{"code":"already_active","message":"A checkpoint is already active for this session."}"#,
		),
	},
	CardFixture {
		tool:   "rewind",
		title:  "Rewind",
		states: states(
			r#"{"report":"The parser already handles escaped"#,
			r#"{"report":"The parser already handles escaped delimiters; no source change is needed."}"#,
			r#"{"token":"checkpoint-01","report":"The parser already handles escaped delimiters; no source change is needed.","receipt":"rewind-01","scheduled":true}"#,
			r#"{"code":"no_active","message":"No active checkpoint exists for this session."}"#,
		),
	},
	CardFixture {
		tool:   "yield",
		title:  "Submit result",
		states: states(
			r#"{"type":"result","result":{"data":{"answer":"The parser"#,
			r#"{"type":"result","result":{"data":{"answer":"The parser is sound."}}}"#,
			r#"{"incremental":false,"use_last_turn":false,"validation":null}"#,
			r#"{"code":"empty_sections"}"#,
		),
	},
	CardFixture {
		tool:   "memory_edit",
		title:  "Memory",
		states: states(
			r#"{"op":"update","id":"memory://working/runtime-pref","content":"Use Bun"#,
			r#"{"op":"update","id":"memory://working/runtime-pref","content":"Use Bun for repository scripts."}"#,
			r#"{"operation":"update","status":"updated","id":"memory://working/runtime-pref","bank":"working"}"#,
			r#"{"kind":"unavailable"}"#,
		),
	},
	CardFixture {
		tool:   "learn",
		title:  "Learn",
		states: states(
			r#"{"memory":"The provider rejects effort unless adaptive"#,
			r#"{"memory":"The provider rejects effort unless adaptive thinking is enabled.","context":"Anthropic request encoding"}"#,
			r#"{"memory_id":"memory://working/anthropic-effort","skill":{"status":"not_requested"},"partial":false}"#,
			r#"{"kind":"memory"}"#,
		),
	},
	CardFixture {
		tool:   "manage_skill",
		title:  "Skill",
		states: states(
			r#"{"action":"create","name":"provider-debugging","description":"Diagnose provider"#,
			r#"{"action":"create","name":"provider-debugging","description":"Diagnose provider request failures","body":"Inspect the typed request and catalog mode."}"#,
			r#"{"action":"create","name":"provider-debugging","path":"provider-debugging/SKILL.md","revision":7}"#,
			r#"{"kind":"authority","source":{"kind":"already_exists"}}"#,
		),
	},
	CardFixture {
		tool:   "image_gen",
		title:  "Image",
		states: states(
			r#"{"subject":"A cyanotype frog","action":"studying a blueprint"#,
			r#"{"subject":"A cyanotype frog","action":"studying a blueprint","scene":"an engineer's workbench","composition":"top-down","lighting":"soft window light","style":"cyanotype","aspect_ratio":"16:9"}"#,
			r#"{"artifact_id":"artifact://sha256/1111111111111111111111111111111111111111111111111111111111111111","media_type":"image/png","output_path":"art/frog.png","blob":null}"#,
			r#"{"code":"backend_unavailable","backend":"image","message":"No image provider is configured."}"#,
		),
	},
	CardFixture {
		tool:   "tts",
		title:  "Speech",
		states: states(
			r#"{"text":"The release is ready for"#,
			r#"{"text":"The release is ready for review.","voice_id":"af_heart","language":"en-US","output_path":"art/release.wav"}"#,
			r#"{"artifact_id":"artifact://sha256/2222222222222222222222222222222222222222222222222222222222222222","media_type":"audio/wav","output_path":"art/release.wav","blob":null}"#,
			r#"{"code":"backend_unavailable","backend":"tts","message":"Speech synthesis is unavailable."}"#,
		),
	},
	CardFixture {
		tool:   "security_scan",
		title:  "Security scan",
		states: states(
			r#"{"action":"preflight","target_kind":"scoped_path","include_paths":["crates/envd"#,
			r#"{"action":"preflight","target_kind":"scoped_path","include_paths":["crates/envd"],"exclude_paths":["target"]}"#,
			r#"{"action":"preflight","output":"No high-severity findings in crates/envd.","data":{"findings":0,"files":184}}"#,
			r#"{"kind":"unavailable"}"#,
		),
	},
];

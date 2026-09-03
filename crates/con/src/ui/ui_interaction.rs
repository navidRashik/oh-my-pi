//! Mechanical projection of pi's complete Interaction settings tab.

use super::*;

pub(super) const ENTRIES: &[UiSpec] = &[
	ui!(
		"steeringMode",
		"cl_steering_mode",
		Interaction,
		"Input",
		"Steering Mode",
		"How to process queued messages while agent is working",
		UiWidget::Enum(&["all", "one-at-a-time"]),
		None,
		Identity
	),
	ui!(
		"followUpMode",
		"cl_follow_up_mode",
		Interaction,
		"Input",
		"Follow-Up Mode",
		"How to drain follow-up messages after a turn completes",
		UiWidget::Enum(&["all", "one-at-a-time"]),
		None,
		Identity
	),
	ui!(
		"interruptMode",
		"cl_interrupt_mode",
		Interaction,
		"Input",
		"Interrupt Mode",
		"When steering messages interrupt tool execution",
		UiWidget::Enum(&["immediate", "wait"]),
		None,
		Identity
	),
	ui!(
		"loop.mode",
		"cl_loop_mode",
		Interaction,
		"Input",
		"Loop Mode",
		"What happens between /loop iterations before re-submitting the prompt",
		UiWidget::Submenu(&[
			UiOption::new(
				"prompt",
				"Prompt",
				"Re-submit the prompt as a follow-up message (current behavior)"
			),
			UiOption::new(
				"compact",
				"Compact",
				"Compact the session context, then re-submit the prompt"
			),
			UiOption::new("reset", "Reset", "Start a new session, then re-submit the prompt")
		]),
		None,
		Identity
	),
	ui!(
		"doubleEscapeAction",
		"cl_double_escape",
		Interaction,
		"Input",
		"Double-Escape Action",
		"What pressing Escape twice with an empty editor does: open the transcript rewind selector, \
		 open the session tree, or nothing",
		UiWidget::Enum(&["rewind", "tree", "none"]),
		None,
		Identity
	),
	ui!(
		"treeFilterMode",
		"cl_tree_filter_mode",
		Interaction,
		"Input",
		"Session Tree Filter",
		"Default filter mode when opening the session tree",
		UiWidget::Enum(&["default", "no-tools", "user-only", "labeled-only", "all"]),
		None,
		Identity
	),
	ui!(
		"autocompleteMaxVisible",
		"cl_autocomplete_max_visible",
		Interaction,
		"Input",
		"Autocomplete Items",
		"Max visible items in autocomplete dropdown (3-20)",
		UiWidget::Submenu(&[
			UiOption::new("3", "3 items", ""),
			UiOption::new("5", "5 items", ""),
			UiOption::new("7", "7 items", ""),
			UiOption::new("10", "10 items", ""),
			UiOption::new("15", "15 items", ""),
			UiOption::new("20", "20 items", "")
		]),
		None,
		Identity
	),
	ui!(
		"spelling.typoDetection",
		"cl_spelling_typo_detection",
		Interaction,
		"Input",
		"Typo Detection (macOS)",
		"Mark misspelled prompt words with the active macOS dictionaries",
		UiWidget::Boolean,
		Some(UiCondition::MacOs),
		Identity
	),
	ui!(
		"spelling.autocomplete",
		"cl_spelling_autocomplete",
		Interaction,
		"Input",
		"Word Autocomplete (macOS)",
		"Show macOS dictionary word completions as inline hints accepted with Tab",
		UiWidget::Boolean,
		Some(UiCondition::MacOs),
		Identity
	),
	ui!(
		"spelling.autocorrect",
		"cl_spelling_autocorrect",
		Interaction,
		"Input",
		"Autocorrect (macOS)",
		"Apply confident macOS spelling corrections after completed words",
		UiWidget::Boolean,
		Some(UiCondition::MacOs),
		Identity
	),
	ui!(
		"emojiAutocomplete",
		"cl_emoji_autocomplete",
		Interaction,
		"Input",
		"Emoji Autocomplete",
		"Suggest emojis from `:name:` shortcodes and expand text emoticons like `:D` or `:-)`",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"paste.largeMenuThreshold",
		"cl_paste_large_menu_threshold",
		Interaction,
		"Input",
		"Large Paste Menu",
		"When a paste reaches this many lines, offer a menu to wrap it in a code block, wrap it in \
		 XML tags, or save it to a file. 0 disables the menu (large pastes still collapse to a \
		 [Paste] marker).",
		UiWidget::Submenu(&[
			UiOption::new("0", "Off", ""),
			UiOption::new("100", "100 lines", ""),
			UiOption::new("250", "250 lines", ""),
			UiOption::new("500", "500 lines", ""),
			UiOption::new("1000", "1000 lines", "")
		]),
		None,
		Identity
	),
	ui!(
		"tools.approval",
		"sv_tools_approval",
		Interaction,
		"Approvals",
		"Tool Approval Policies",
		"Per-tool approval policies. Set to 'allow' to auto-approve, 'prompt' to require \
		 confirmation, or 'deny' to block. Overrides are honored in every approval mode.",
		UiWidget::Text { secret: false },
		None,
		Identity
	),
	ui!(
		"tools.approvalMode",
		"sv_tools_approval_mode",
		Interaction,
		"Approvals",
		"Tool Approval",
		"Default approval behavior for tool calls. 'Always ask' auto-approves read-only tools only. \
		 'Write' auto-approves read and workspace-write tools. 'Yolo' auto-approves all tiers; user \
		 policy may still prompt or block.",
		UiWidget::Submenu(&[
			UiOption::new(
				"always-ask",
				"Always ask",
				"Auto-approve read-only tools; require confirmation for write and exec tools."
			),
			UiOption::new(
				"write",
				"Write",
				"Auto-approve read-only and write tools; require confirmation for exec tools such as \
				 bash, eval, browser, and task."
			),
			UiOption::new(
				"yolo",
				"Yolo",
				"Auto-approve read, write, and exec tools. User policy can still require confirmation \
				 or block calls."
			)
		]),
		None,
		Identity
	),
	ui!(
		"completion.notify",
		"cl_notify_completion",
		Interaction,
		"Notifications",
		"Completion Notification",
		"Notify when the agent finishes a turn",
		UiWidget::Enum(&["on", "off"]),
		None,
		OnOffBoolean
	),
	ui!(
		"error.notify",
		"cl_notify_error",
		Interaction,
		"Notifications",
		"Error Notification",
		"Notify when the agent stops with an error",
		UiWidget::Enum(&["on", "off"]),
		None,
		OnOffBoolean
	),
	ui!(
		"ask.timeout",
		"cl_ask_timeout",
		Interaction,
		"Notifications",
		"Ask Timeout",
		"Auto-select the recommended ask option after this many seconds (0 disables)",
		UiWidget::Submenu(&[
			UiOption::new("0", "Disabled", ""),
			UiOption::new("15", "15 seconds", ""),
			UiOption::new("30", "30 seconds", ""),
			UiOption::new("60", "60 seconds", ""),
			UiOption::new("120", "120 seconds", "")
		]),
		None,
		Identity
	),
	ui!(
		"ask.notify",
		"cl_notify_ask",
		Interaction,
		"Notifications",
		"Ask Notification",
		"Notify when the ask tool is waiting for input",
		UiWidget::Enum(&["on", "off"]),
		None,
		OnOffBoolean
	),
	ui!(
		"recap.enabled",
		"cl_recap_enabled",
		Interaction,
		"Notifications",
		"Idle Recap",
		"Generate a brief LLM recap of where things stand after the terminal has been idle",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"recap.idleSeconds",
		"cl_recap_idle_seconds",
		Interaction,
		"Notifications",
		"Idle Recap Delay",
		"Seconds to wait while idle before showing the recap",
		UiWidget::Submenu(&[
			UiOption::new("60", "1 minute", ""),
			UiOption::new("120", "2 minutes", ""),
			UiOption::new("240", "4 minutes", ""),
			UiOption::new("300", "5 minutes", ""),
			UiOption::new("600", "10 minutes", "")
		]),
		None,
		Identity
	),
	ui!(
		"stt.enabled",
		"cl_voice_stt_enabled",
		Interaction,
		"Speech",
		"Speech-to-Text",
		"Enable speech-to-text input via microphone",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"stt.modelName",
		"cl_stt_model",
		Interaction,
		"Speech",
		"Speech Model",
		"Local on-device speech model. Parakeet TDT v3 (sherpa-onnx) is the SoTA default; Whisper \
		 base/small/large-v3-turbo tiers (transformers.js) trade size for multilingual coverage. \
		 Downloaded on first use.",
		UiWidget::Submenu(&[
			UiOption::new(
				"fast",
				"Fast (Whisper base)",
				"Whisper base, multilingual. Smallest + fastest; lowest accuracy. Best for \
				 low-resource machines."
			),
			UiOption::new(
				"balanced",
				"Balanced (Whisper small)",
				"Whisper small, multilingual. More accurate than Fast, still light on CPU/RAM."
			),
			UiOption::new(
				"turbo",
				"Turbo (Whisper large-v3)",
				"Whisper large-v3-turbo, 99 languages. Widest language coverage; large download, \
				 slower."
			),
			UiOption::new(
				"parakeet",
				"Parakeet TDT v3 (SoTA)",
				"NVIDIA Parakeet TDT 0.6B v3, 25 languages. Open ASR Leaderboard leader — best \
				 accuracy and far fastest decoding. Default."
			)
		]),
		None,
		Identity
	),
	ui!(
		"stt.submitTrigger",
		"cl_stt_submit_trigger",
		Interaction,
		"Speech",
		"Speech-to-Text Submit Trigger",
		"Choose when speech dictation automatically submits: Never, Release (2+ words), Release \
		 with complete sentence, or When I Say Submit.",
		UiWidget::Submenu(&[
			UiOption::new(
				"never",
				"Never",
				"Never automatically submit; insert dictation and remain in editor."
			),
			UiOption::new(
				"release",
				"Release",
				"Submit on release if the utterance has 2+ words to avoid accidental sends."
			),
			UiOption::new(
				"release-complete",
				"Release with complete sentence",
				"Submit on release if the utterance ends with sentence-terminal punctuation (. ? ! \
				 etc.)."
			),
			UiOption::new(
				"say-submit",
				"When I Say Submit",
				"Submit if the utterance ends with a word containing 'submit' (strips that word \
				 before submitting)."
			)
		]),
		None,
		Identity
	),
	ui!(
		"collab.relayUrl",
		"cl_collab_relay_url",
		Interaction,
		"Collab",
		"Relay URL",
		"Relay used by /collab (wss://host[:port])",
		UiWidget::Text { secret: false },
		None,
		Identity
	),
	ui!(
		"collab.webUrl",
		"cl_collab_web_url",
		Interaction,
		"Collab",
		"Web UI URL",
		"Browser UI used by /collab links; empty derives from collab.relayUrl; explicit http:// is \
		 localhost-only",
		UiWidget::Text { secret: false },
		None,
		Identity
	),
	ui!(
		"collab.displayName",
		"cl_collab_display_name",
		Interaction,
		"Collab",
		"Display Name",
		"Name shown to other collab participants (default: OS username)",
		UiWidget::Text { secret: false },
		None,
		Identity
	),
	ui!(
		"share.serverUrl",
		"sv_share_server",
		Interaction,
		"Collab",
		"Share Server",
		"Share viewer/upload base used by /share (encrypted blob upload + viewer; links are \
		 <base>/<id>#<key>)",
		UiWidget::Text { secret: false },
		None,
		Identity
	),
	ui!(
		"share.store",
		"sv_share_store",
		Interaction,
		"Collab",
		"Share Store",
		"Where /share uploads the encrypted session blob",
		UiWidget::Submenu(&[
			UiOption::new(
				"blob",
				"Encrypted Blob",
				"Upload to the share server (no GitHub account needed; avoids gist API rate limits)"
			),
			UiOption::new(
				"gist",
				"GitHub Gist",
				"Push to a secret gist (needs authenticated gh), falling back to the share server"
			)
		]),
		None,
		Identity
	),
	ui!(
		"share.redactSecrets",
		"sv_share_redact_secrets",
		Interaction,
		"Collab",
		"Share Secret Redaction",
		"Run the secret obfuscator over /share snapshots before upload (uses the secrets.* config)",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"magicKeywords.enabled",
		"cl_magic_keywords_enabled",
		Interaction,
		"Magic Keywords",
		"Magic Keywords",
		"Enable hidden notices for standalone ultrathink, orchestrate, and workflowz keywords",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"magicKeywords.ultrathink",
		"cl_magic_keywords_ultrathink",
		Interaction,
		"Magic Keywords",
		"Ultrathink Keyword",
		"Let standalone ultrathink request maximum automatic thinking and append its hidden notice",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"magicKeywords.orchestrate",
		"cl_magic_keywords_orchestrate",
		Interaction,
		"Magic Keywords",
		"Orchestrate Keyword",
		"Let standalone orchestrate append its hidden multi-agent orchestration notice",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"magicKeywords.workflow",
		"cl_magic_keywords_workflow",
		Interaction,
		"Magic Keywords",
		"Workflow Keyword",
		"Let standalone workflowz append its hidden eval workflow notice",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"autoResume",
		"cl_auto_resume",
		Interaction,
		"Startup & Updates",
		"Auto Resume",
		"Automatically resume the most recent session in the current directory",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"startup.quiet",
		"cl_startup_quiet",
		Interaction,
		"Startup & Updates",
		"Quiet Startup",
		"Skip welcome screen and startup status messages",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"startup.showSplash",
		"cl_startup_show_splash",
		Interaction,
		"Startup & Updates",
		"Show Startup Splash",
		"Show the full animated setup splash on normal interactive startup without rerunning setup. \
		 Quiet Startup still suppresses it.",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"startup.setupWizard",
		"cl_startup_setup_wizard",
		Interaction,
		"Startup & Updates",
		"Setup Wizard",
		"Show newly added onboarding steps once per setup version",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"startup.checkUpdate",
		"cl_startup_check_update",
		Interaction,
		"Startup & Updates",
		"Check for Updates",
		"Check for omp updates on startup",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"update.channel",
		"cl_update_channel",
		Interaction,
		"Startup & Updates",
		"Update Channel",
		"Update channel used by omp update and the startup update check",
		UiWidget::Submenu(&[
			UiOption::new("stable", "Stable", ""),
			UiOption::new("canary", "Canary", "")
		]),
		None,
		Identity
	),
	ui!(
		"marketplace.autoUpdate",
		"cl_marketplace_auto_update",
		Interaction,
		"Startup & Updates",
		"Marketplace Auto-Update",
		"Check for plugin updates on startup",
		UiWidget::Submenu(&[
			UiOption::new("off", "Off", "Don't check for plugin updates"),
			UiOption::new(
				"notify",
				"Notify",
				"Check on startup and notify when updates are available"
			),
			UiOption::new("auto", "Auto", "Check on startup and auto-install updates")
		]),
		None,
		Identity
	),
	ui!(
		"startup.changelogMode",
		"cl_startup_changelog_mode",
		Interaction,
		"Startup & Updates",
		"Startup Changelog",
		"Choose whether update notes start as a summary, full details, or stay hidden",
		UiWidget::Submenu(&[
			UiOption::new(
				"summary",
				"Summary",
				"Show release and change counts with a /changelog hint"
			),
			UiOption::new("expanded", "Expanded", "Show the recent release notes in full"),
			UiOption::new("hidden", "Hidden", "Do not show release notes on startup")
		]),
		None,
		Identity
	),
	ui!(
		"power.sleepPrevention",
		"cl_power_sleep_prevention",
		Interaction,
		"Power",
		"Sleep Prevention",
		"Prevent the system sleeping during active sessions. Each level is cumulative — it adds the \
		 flags of all lower levels.",
		UiWidget::Submenu(&[
			UiOption::new("off", "Off", "Do not prevent any sleep"),
			UiOption::new(
				"idle",
				"Prevent Idle Sleep",
				"Keep the system awake while a session is open (macOS `caffeinate -i`)"
			),
			UiOption::new(
				"display",
				"Prevent Display Sleep",
				"Also keep the display from idle-sleeping (macOS `caffeinate -i -d`)"
			),
			UiOption::new(
				"system",
				"Prevent System Sleep",
				"Also block all system sleep on AC and declare the user active (macOS `caffeinate -i \
				 -d -s -u`)"
			)
		]),
		None,
		Identity
	),
	ui!(
		"features.unexpectedStopDetection",
		"ai_features_unexpected_stop_detection",
		Interaction,
		"Agent",
		"Unexpected Stops",
		"Automatically recover when the assistant stops without a visible message. Smart also \
		 classifies text-only stops with a small model.",
		UiWidget::Submenu(&[
			UiOption::new("none", "None", "Disabled"),
			UiOption::new(
				"mechanical",
				"Mechanical",
				"Retry stops with no visible assistant message; tool calls are excluded (default)"
			),
			UiOption::new(
				"smart",
				"Smart",
				"Mechanical + small-model classification of text-only stops"
			)
		]),
		None,
		Identity
	),
	ui!(
		"git.enabled",
		"ai_git_enabled",
		Interaction,
		"Git",
		"Enable Git Integration",
		"Show git branch, status, and PR information in the TUI and watch repository metadata.",
		UiWidget::Boolean,
		None,
		Identity
	),
];

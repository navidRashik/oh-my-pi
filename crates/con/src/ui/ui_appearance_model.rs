//! Mechanical projection of current pi settings UI metadata.

use super::*;

const BLOB_BACKEND_CHOICES: &[UiOption] = &[
	UiOption::new("imgur", "Imgur", "Uploads require either an Imgur access token or client ID."),
	UiOption::new("imageshack", "ImageShack", "The API requires a paid subscription."),
	UiOption::new("flickr", "Flickr", "image-host"),
	UiOption::new("chevereto", "Chevereto", "self-hosted"),
	UiOption::new("vgyme", "vgy.me", "image-host"),
	UiOption::new("dropbox", "Dropbox", "cloud-files"),
	UiOption::new("ftp", "FTP / FTPS / SFTP", "file-transfer"),
	UiOption::new("onedrive", "OneDrive", "cloud-files"),
	UiOption::new("google-drive", "Google Drive", "cloud-files"),
	UiOption::new(
		"puush",
		"puush-compatible endpoint",
		"The public service is defunct; a replacement endpoint is required.",
	),
	UiOption::new("box", "Box", "cloud-files"),
	UiOption::new("amazon-s3", "Amazon S3", "s3"),
	UiOption::new("google-cloud-storage", "Google Cloud Storage", "object-storage"),
	UiOption::new("azure-storage", "Azure Blob Storage", "object-storage"),
	UiOption::new(
		"backblaze-b2",
		"Backblaze B2",
		"Configure either native B2 application keys or S3-compatible access keys.",
	),
	UiOption::new("owncloud", "ownCloud / Nextcloud", "webdav"),
	UiOption::new(
		"mediafire",
		"MediaFire-compatible endpoint",
		"The public API is deprecated; a replacement endpoint is required.",
	),
	UiOption::new(
		"sendspace",
		"SendSpace-compatible endpoint",
		"The public discovery API is deprecated; a replacement endpoint is required.",
	),
	UiOption::new(
		"localhostr",
		"Hostr-compatible endpoint",
		"The public service is offline; a replacement endpoint is required.",
	),
	UiOption::new(
		"lambda",
		"Lambda-compatible endpoint",
		"The public service is offline; a replacement endpoint is required.",
	),
	UiOption::new("pomf", "Pomf", "pomf"),
	UiOption::new("uguu", "Uguu", "Public uploads expire after approximately three hours."),
	UiOption::new("seafile", "Seafile", "cloud-files"),
	UiOption::new("s-ul", "s-ul", "file-host"),
	UiOption::new(
		"lobfile",
		"LobFile-compatible endpoint",
		"The public service is offline; a replacement endpoint is required.",
	),
	UiOption::new(
		"transfer-sh",
		"transfer.sh-compatible endpoint",
		"The defunct public endpoint is blocked; a self-hosted replacement is required.",
	),
	UiOption::new("plik", "Plik", "self-hosted"),
	UiOption::new("shared-folder", "Shared folder", "filesystem"),
	UiOption::new("catbox", "Catbox", "anonymous-host"),
	UiOption::new("litterbox", "Litterbox", "Uploads are temporary."),
	UiOption::new(
		"0x0",
		"0x0.st",
		"Public uploads expire after a retention window determined by file size.",
	),
	UiOption::new("tmpfiles", "tmpfiles.org", "Public uploads are temporary."),
	UiOption::new("discord", "Discord", "messaging"),
	UiOption::new(
		"provider-files",
		"Model provider files",
		"Provider file references are API-local rather than public image URLs.",
	),
	UiOption::new("direct", "Direct public URL", "local-serving"),
	UiOption::new("cloudflared", "Cloudflare quick tunnel", "tunnel"),
	UiOption::new("ngrok", "ngrok", "tunnel"),
	UiOption::new("tailscale", "Tailscale Funnel", "tunnel"),
	UiOption::new("ssh", "SSH reverse tunnel", "tunnel"),
	UiOption::new("command", "Uploader command", "external-command"),
	UiOption::new("localhost-run", "localhost.run", "tunnel"),
	UiOption::new("pinggy", "Pinggy", "tunnel"),
	UiOption::new(
		"devtunnel",
		"Microsoft dev tunnel",
		"The devtunnel CLI must be logged in locally.",
	),
	UiOption::new("zrok", "zrok", "The local zrok environment must be enabled."),
	UiOption::new("bore", "bore", "tunnel"),
	UiOption::new("named-cloudflared", "Named Cloudflare Tunnel", "tunnel"),
	UiOption::new("r2", "Cloudflare R2", "s3"),
	UiOption::new("tigris", "Tigris", "s3"),
	UiOption::new("minio", "MinIO", "s3"),
	UiOption::new("garage", "Garage", "s3"),
];

pub(super) const ENTRIES: &[UiSpec] = &[
	ui!(
		"theme.dark",
		"cl_theme_dark",
		Appearance,
		"Theme",
		"Dark Theme",
		"Theme used when the terminal has a dark background",
		UiWidget::RuntimeSubmenu(UiRuntimeOptions::Themes),
		None,
		Identity
	),
	ui!(
		"theme.light",
		"cl_theme_light",
		Appearance,
		"Theme",
		"Light Theme",
		"Theme used when the terminal has a light background",
		UiWidget::RuntimeSubmenu(UiRuntimeOptions::Themes),
		None,
		Identity
	),
	ui!(
		"symbolPreset",
		"cl_charset",
		Appearance,
		"Theme",
		"Symbol Preset",
		"Glyph set for icons and symbols (Unicode, Nerd Font, or ASCII)",
		UiWidget::Submenu(&[
			UiOption::new("unicode", "Unicode", "Standard symbols (default)"),
			UiOption::new("nerd", "Nerd Font", "Requires Nerd Font"),
			UiOption::new("ascii", "ASCII", "Maximum compatibility")
		]),
		None,
		Identity
	),
	ui!(
		"colorBlindMode",
		"cl_color_blind_mode",
		Appearance,
		"Theme",
		"Color-Blind Mode",
		"Use blue instead of green for diff additions",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"composer.shape",
		"cl_composer_shape",
		Appearance,
		"Composer",
		"Composer Shape",
		"Visual layout of the input editor and status line",
		UiWidget::RuntimeSubmenu(UiRuntimeOptions::ComposerShapes),
		None,
		Identity
	),
	ui!(
		"statusLine.preset",
		"cl_status_line_preset",
		Appearance,
		"Status Line",
		"Status Line Preset",
		"Pre-built status line configurations",
		UiWidget::Submenu(&[
			UiOption::new("default", "Default", "Model, path, git, context, tokens, cost"),
			UiOption::new("minimal", "Minimal", "Path and git only"),
			UiOption::new("compact", "Compact", "Model, git, cost, context"),
			UiOption::new("full", "Full", "All segments including time"),
			UiOption::new("nerd", "Nerd", "Maximum info with Nerd Font icons"),
			UiOption::new("ascii", "ASCII", "No special characters"),
			UiOption::new("custom", "Custom", "User-defined segments")
		]),
		None,
		Identity
	),
	ui!(
		"statusLine.separator",
		"cl_status_line_separator",
		Appearance,
		"Status Line",
		"Status Line Separator",
		"Style of separators between segments",
		UiWidget::Submenu(&[
			UiOption::new("powerline", "Powerline", "Solid arrows (Nerd Font)"),
			UiOption::new("powerline-thin", "Thin chevron", "Thin arrows (Nerd Font)"),
			UiOption::new("slash", "Slash", "Forward slashes"),
			UiOption::new("pipe", "Pipe", "Vertical pipes"),
			UiOption::new("block", "Block", "Solid blocks"),
			UiOption::new("none", "None", "Space only"),
			UiOption::new("ascii", "ASCII", "Greater-than signs")
		]),
		None,
		Identity
	),
	ui!(
		"statusLine.contextLine",
		"cl_status_line_context_line",
		Appearance,
		"Status Line",
		"Context-Reactive Line",
		"How the line between the left and right segments reflects context usage (box composer only)",
		UiWidget::Submenu(&[
			UiOption::new("off", "Off", "Solid accent line, no context feedback"),
			UiOption::new(
				"percentage",
				"Percentage",
				"Used portion in accent color, remainder dimmed"
			),
			UiOption::new(
				"annotated",
				"Annotated",
				"Percentage plus ticks at the speculative and auto-compaction boundaries"
			),
			UiOption::new(
				"embedded",
				"Embedded",
				"Annotated line with the context percentage and window embedded in the gauge"
			)
		]),
		None,
		Identity
	),
	ui!(
		"statusLine.sessionAccent",
		"cl_status_line_session_accent",
		Appearance,
		"Status Line",
		"Session Accent",
		"Use the session name color for the editor border and status line gap",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"statusLine.transparent",
		"cl_status_line_transparent",
		Appearance,
		"Status Line",
		"Transparent Status Line",
		"Use the terminal's default background for the status line instead of the theme's \
		 `statusLineBg`. Powerline end caps are dropped because they need a contrasting fill to \
		 bridge into the surrounding terminal.",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"statusLine.compactThinkingLevel",
		"cl_status_compact_thinking",
		Appearance,
		"Status Line",
		"Compact Thinking Level",
		"Show the thinking level as a single icon on the model name instead of a separate ` · \
		 <level>` suffix.",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"statusLine.showHookStatus",
		"cl_status_line_show_hook_status",
		Appearance,
		"Status Line",
		"Show Hook Status",
		"Display hook status messages below the status line",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"tui.resizeScrollback",
		"cl_resize_policy",
		Appearance,
		"Display",
		"Resize Scrollback",
		"How a settled terminal resize refreshes transcript rows retained in terminal scrollback",
		UiWidget::Submenu(&[
			UiOption::new(
				"append",
				"Append",
				"Replay the transcript at the new width below retained history"
			),
			UiOption::new(
				"rebuild",
				"Rebuild",
				"Erase all terminal scrollback, then replay one current-width transcript"
			),
			UiOption::new(
				"preserve",
				"Preserve",
				"Repaint only the viewport and keep history wrapped at its old width"
			)
		]),
		None,
		Identity
	),
	ui!(
		"terminal.showProgress",
		"cl_show_progress",
		Appearance,
		"Display",
		"Native Terminal Progress",
		"Emit OSC 9;4 indeterminate progress while the agent or context maintenance is running",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"tui.textSizing",
		"cl_tui_text_sizing",
		Appearance,
		"Display",
		"Large Headings (Kitty)",
		"Render Markdown H1 headings at 2x scale using Kitty's OSC 66 text-sizing protocol. Only \
		 takes effect on Kitty terminals; ignored everywhere else. Off by default.",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"tui.renderMermaid",
		"cl_tui_render_mermaid",
		Appearance,
		"Display",
		"Render Mermaid Diagrams",
		"Render Mermaid fenced code blocks as ASCII diagrams",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"tui.reactions",
		"cl_tui_reactions",
		Appearance,
		"Display",
		"Agent Reactions",
		"Invite the agent to react to your message with an emoji badge on its bubble",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"tui.codexResetFireworks",
		"cl_codex_fireworks",
		Appearance,
		"Display",
		"Codex Reset Fireworks",
		"Celebrate unscheduled Codex weekly usage resets and newly banked saved resets with a \
		 top-third fireworks overlay that remains until Escape",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"tui.titleState",
		"cl_title_state",
		Appearance,
		"Display",
		"Terminal Title Run State",
		"Show the agent run state in the terminal title's separator — an animated spinner while \
		 working (a static ':' on Windows), '>' when it's your turn, '!' when the agent is waiting \
		 on you",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"tui.hyperlinks",
		"cl_tui_hyperlinks",
		Appearance,
		"Display",
		"Terminal Hyperlinks",
		"Wrap paths and URLs in OSC 8 hyperlinks for terminal-native click-to-open (auto: detect \
		 support; off: never; always: unconditional)",
		UiWidget::Enum(&["off", "auto", "always"]),
		None,
		Identity
	),
	ui!(
		"tui.tight",
		"cl_tui_tight",
		Appearance,
		"Display",
		"Tight Layout",
		"Remove the 1-character horizontal padding from the left and right of the terminal output",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"display.shimmer",
		"cl_display_shimmer",
		Appearance,
		"Display",
		"Shimmer",
		"Animation style for working/loading messages",
		UiWidget::Submenu(&[
			UiOption::new("classic", "Classic", "Soft cosine wave sweeping across the text"),
			UiOption::new("kitt", "KITT Scanner", "Knight Rider 1982 red light bouncing left-right"),
			UiOption::new("disabled", "Disabled", "No animation; static muted text")
		]),
		None,
		Identity
	),
	ui!(
		"display.smoothStreaming",
		"cl_smooth_streaming",
		Appearance,
		"Display",
		"Smooth Streaming",
		"Reveal assistant text and streamed tool input smoothly while chunks arrive",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"display.hideToolActivity",
		"cl_showtools",
		Appearance,
		"Display",
		"Hide Tool Activity",
		"Hide model-initiated tool calls and results from the transcript",
		UiWidget::Boolean,
		None,
		InvertedBoolean
	),
	ui!(
		"display.showTokenUsage",
		"cl_display_show_token_usage",
		Appearance,
		"Display",
		"Show Token Usage",
		"Show per-turn token usage on assistant messages",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"display.showTurnTime",
		"cl_display_show_turn_time",
		Appearance,
		"Display",
		"Show Turn Time",
		"Show the total prompt-to-yield time (including tool calls) on assistant message usage rows",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"display.cacheMissMarker",
		"cl_display_cache_miss_marker",
		Appearance,
		"Display",
		"Cache Miss Marker",
		"Show a divider after an assistant turn whose request lost (missed) the prompt cache",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"display.collapseCompacted",
		"cl_display_collapse_compacted",
		Appearance,
		"Display",
		"Collapse Compacted History",
		"Collapse pre-compaction history behind the summary divider on the live transcript; disable \
		 to keep the full transcript inline with dividers at each compaction point",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"showHardwareCursor",
		"cl_show_hardware_cursor",
		Appearance,
		"Display",
		"Show Hardware Cursor",
		"Show terminal cursor for IME support",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"tui.imeSafeCursor",
		"cl_ime_safe_cursor",
		Appearance,
		"Display",
		"IME-Safe Prompt Layout",
		"Move the prompt's bottom border to a separate row so macOS IME preedit cannot displace it",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"task.showResolvedModelBadge",
		"cl_task_show_resolved_model_badge",
		Appearance,
		"Display",
		"Show Resolved Model Badge",
		"Display the actual model ID used by each subagent in the task widget status line",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"terminal.showImages",
		"cl_terminal_show_images",
		Appearance,
		"Images",
		"Show Inline Images",
		"Render images inline in the terminal",
		UiWidget::Boolean,
		Some(UiCondition::HasImageProtocol),
		Identity
	),
	ui!(
		"images.autoResize",
		"sv_images_auto_resize",
		Appearance,
		"Images",
		"Auto-Resize Images",
		"Resize large images to 2000x2000 max for better model compatibility",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"images.blockImages",
		"sv_images_block_images",
		Appearance,
		"Images",
		"Block Images",
		"Prevent images from being sent to LLM providers",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"defaultThinkingLevel",
		"ai_default_thinking",
		Model,
		"Thinking",
		"Thinking Level",
		"Reasoning depth for thinking-capable models",
		UiWidget::RuntimeSubmenu(UiRuntimeOptions::ThinkingLevels),
		None,
		Identity
	),
	ui!(
		"hideThinkingBlock",
		"cl_showthinking",
		Model,
		"Thinking",
		"Hide Thinking Blocks",
		"Hide thinking blocks in assistant responses",
		UiWidget::Boolean,
		None,
		InvertedBoolean
	),
	ui!(
		"proseOnlyThinking",
		"cl_thinking_prose_only",
		Model,
		"Thinking",
		"Prose Only Thinking",
		"Omit code blocks from thinking summaries and replace them with an ellipsis",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"omitThinking",
		"ai_omit_thinking",
		Model,
		"Thinking",
		"Omit Thinking summaries",
		"Instruct upstream providers to completely omit thinking summaries from responses (where \
		 supported)",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui_warn!(
		"externalThinking",
		"ai_external_thinking",
		Model,
		"Thinking",
		"External Thinking",
		"Private scratchpad; not shown to user. Disables supported GPT, Claude, and Gemini reasoning",
		"At your own risk: providers have flagged this request shape as abuse, up to account-level \
		 enforcement",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"model.loopGuard.enabled",
		"ai_model_loop_guard_enabled",
		Model,
		"Thinking",
		"Loop Guard",
		"Enable automatic stream loop detection for model reasoning and prose",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"model.loopGuard.checkAssistantContent",
		"ai_model_loop_guard_check_assistant_content",
		Model,
		"Thinking",
		"Loop Guard Scan Prose",
		"Apply loop guard to assistant prose messages in addition to thinking logs",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"model.loopGuard.toolCallReminder",
		"ai_model_loop_guard_tool_call_reminder",
		Model,
		"Thinking",
		"Loop Guard Tool-Call Reminder",
		"When a Gemini reasoning stream emits many consecutive planning headers without calling a \
		 tool, interrupt it and inject a reminder to issue a tool call (requires Loop Guard)",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"model.toolCallLoopGuard.enabled",
		"ai_model_tool_call_loop_guard_enabled",
		Model,
		"Thinking",
		"Tool-Call Loop Guard",
		"Detect consecutive identical tool calls across turns and inject a corrective steer",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"model.toolCallLoopGuard.threshold",
		"ai_model_tool_call_loop_guard_threshold",
		Model,
		"Thinking",
		"Tool-Call Loop Threshold",
		"Consecutive identical tool calls required before the corrective steer is injected",
		UiWidget::ConfigOnly,
		None,
		Identity
	),
	ui!(
		"model.toolCallLoopGuard.exemptTools",
		"ai_model_tool_call_loop_guard_exempt_tools",
		Model,
		"Thinking",
		"Tool-Call Loop Exempt Tools",
		"Tool names that may repeat consecutively without triggering the cross-turn loop guard",
		UiWidget::ConfigOnly,
		None,
		Identity
	),
	ui!(
		"providers.autoThinkingModel",
		"ai_auto_thinking_selector",
		Model,
		"Thinking",
		"Auto Thinking Model",
		"Difficulty classifier for the `auto` thinking level: online (the TINY role from /models, \
		 else smol) by default, or a local on-device model",
		UiWidget::Submenu(&[
			UiOption::new(
				"online",
				"Online (TINY role, else @smol)",
				"Classify prompt difficulty online with the TINY role model (set one in /models) or \
				 @smol; no local download or on-device inference."
			),
			UiOption::new(
				"qwen3-1.7b",
				"Qwen3 1.7B",
				"MLX only (providers.tinyModelDevice=mlx): onnxruntime-node cannot run this ONNX \
				 export's RotaryEmbedding cache updates."
			),
			UiOption::new(
				"llama3.2:3b",
				"Llama 3.2 3B",
				"Larger Llama 3.2 option for local memory/classifier tasks; higher quality potential \
				 at higher disk/RAM/latency cost."
			),
			UiOption::new(
				"gemma-3-1b",
				"Gemma 3 1B",
				"Best consolidation/dedup; lighter footprint, but leaks small talk during extraction."
			),
			UiOption::new(
				"qwen2.5-1.5b",
				"Qwen2.5 1.5B",
				"Best extraction granularity (atomic facts); weaker consolidation."
			),
			UiOption::new(
				"lfm2-1.2b",
				"LFM2 1.2B",
				"Fastest load; solid all-rounder, slightly noisier extraction labels."
			)
		]),
		Some(UiCondition::AutoThinkingActive),
		Identity
	),
	ui!(
		"providers.autoThinkingMaxEffort",
		"ai_providers_auto_thinking_max_effort",
		Model,
		"Thinking",
		"Auto Thinking Ceiling",
		"Highest effort the `auto` classifier may resolve. `xhigh` keeps the classifier one tier \
		 below the top, so only an explicit `ultrathink` reaches `max`; `max` lets a turn the \
		 classifier judges exceptional bill the top tier on models that expose it.",
		UiWidget::Submenu(&[
			UiOption::new("xhigh", "xhigh", "Classifier stops at xhigh (default)"),
			UiOption::new("max", "max", "Classifier may resolve max where the model supports it")
		]),
		Some(UiCondition::AutoThinkingActive),
		Identity
	),
	ui!(
		"temperature",
		"ai_sampling_temperature",
		Model,
		"Sampling",
		"Temperature",
		"Sampling temperature (0 = deterministic, 1 = creative, -1 = provider default)",
		UiWidget::Submenu(&[
			UiOption::new("-1", "Default", "Use provider default"),
			UiOption::new("0", "0", "Deterministic"),
			UiOption::new("0.2", "0.2", "Focused"),
			UiOption::new("0.5", "0.5", "Balanced"),
			UiOption::new("0.7", "0.7", "Creative"),
			UiOption::new("1", "1", "Maximum variety")
		]),
		None,
		Identity
	),
	ui!(
		"topP",
		"ai_sampling_top_p",
		Model,
		"Sampling",
		"Top P",
		"Nucleus sampling cutoff (0-1, -1 = provider default)",
		UiWidget::Submenu(&[
			UiOption::new("-1", "Default", "Use provider default"),
			UiOption::new("0.1", "0.1", "Very focused"),
			UiOption::new("0.3", "0.3", "Focused"),
			UiOption::new("0.5", "0.5", "Balanced"),
			UiOption::new("0.9", "0.9", "Broad"),
			UiOption::new("1", "1", "No nucleus filtering")
		]),
		None,
		Identity
	),
	ui!(
		"topK",
		"ai_sampling_top_k",
		Model,
		"Sampling",
		"Top K",
		"Sample from top-K tokens (-1 = provider default)",
		UiWidget::Submenu(&[
			UiOption::new("-1", "Default", "Use provider default"),
			UiOption::new("1", "1", "Greedy top token"),
			UiOption::new("20", "20", "Focused"),
			UiOption::new("40", "40", "Balanced"),
			UiOption::new("100", "100", "Broad")
		]),
		None,
		Identity
	),
	ui!(
		"minP",
		"ai_sampling_min_p",
		Model,
		"Sampling",
		"Min P",
		"Minimum probability threshold (0-1, -1 = provider default)",
		UiWidget::Submenu(&[
			UiOption::new("-1", "Default", "Use provider default"),
			UiOption::new("0.01", "0.01", "Very permissive"),
			UiOption::new("0.05", "0.05", "Balanced"),
			UiOption::new("0.1", "0.1", "Strict")
		]),
		None,
		Identity
	),
	ui!(
		"presencePenalty",
		"ai_sampling_presence_penalty",
		Model,
		"Sampling",
		"Presence Penalty",
		"Penalty for introducing already-present tokens (-1 = provider default)",
		UiWidget::Submenu(&[
			UiOption::new("-1", "Default", "Use provider default"),
			UiOption::new("0", "0", "No penalty"),
			UiOption::new("0.5", "0.5", "Mild novelty"),
			UiOption::new("1", "1", "Encourage novelty"),
			UiOption::new("2", "2", "Strong novelty")
		]),
		None,
		Identity
	),
	ui!(
		"repetitionPenalty",
		"ai_sampling_repetition_penalty",
		Model,
		"Sampling",
		"Repetition Penalty",
		"Penalty for repeated tokens (-1 = provider default)",
		UiWidget::Submenu(&[
			UiOption::new("-1", "Default", "Use provider default"),
			UiOption::new("0.8", "0.8", "Allow repetition"),
			UiOption::new("1", "1", "No penalty"),
			UiOption::new("1.1", "1.1", "Mild penalty"),
			UiOption::new("1.2", "1.2", "Balanced"),
			UiOption::new("1.5", "1.5", "Strong penalty")
		]),
		None,
		Identity
	),
	ui!(
		"textVerbosity",
		"ai_sampling_verbosity",
		Model,
		"Sampling",
		"Text Verbosity",
		"OpenAI Responses and Codex response verbosity (low, medium, or high)",
		UiWidget::Submenu(&[
			UiOption::new("low", "Low", "Prefer concise responses"),
			UiOption::new("medium", "Medium", "Balance brevity and detail (default)"),
			UiOption::new("high", "High", "Prefer detailed responses")
		]),
		None,
		Identity
	),
	ui!(
		"tier.openai",
		"ai_tier_openai",
		Model,
		"Sampling",
		"Service Tier — OpenAI",
		"Processing tier for OpenAI / OpenAI-Codex requests, and OpenAI-family models routed via \
		 OpenRouter (none = omit). Sent as `service_tier`.",
		UiWidget::Submenu(&[
			UiOption::new("none", "None", "Omit service_tier (standard processing)"),
			UiOption::new("auto", "Auto", "Provider default tier selection"),
			UiOption::new("default", "Default", "Standard priority processing"),
			UiOption::new("flex", "Flex", "Lower cost, higher latency when available"),
			UiOption::new("scale", "Scale", "Scale Tier credits when available"),
			UiOption::new("priority", "Priority", "Faster, higher cost (premium request)")
		]),
		None,
		Identity
	),
	ui!(
		"tier.anthropic",
		"ai_tier_anthropic",
		Model,
		"Sampling",
		"Service Tier — Anthropic",
		"Processing tier for Claude requests. `priority` realizes fast mode (`speed: \"fast\"`) on \
		 supported direct Anthropic models; ignored on Bedrock/Vertex Claude and via OpenRouter.",
		UiWidget::Submenu(&[
			UiOption::new("none", "None", "Standard processing"),
			UiOption::new(
				"priority",
				"Priority",
				"Fast mode (`speed: \"fast\"`) on supported direct Claude models; ignored on \
				 Bedrock/Vertex"
			)
		]),
		None,
		Identity
	),
	ui!(
		"tier.google",
		"ai_tier_google",
		Model,
		"Sampling",
		"Service Tier — Google",
		"Processing tier for Gemini (Google AI Studio + Vertex) requests, and Google-family models \
		 routed via OpenRouter (none = omit). Sent as the top-level `serviceTier` field.",
		UiWidget::Submenu(&[
			UiOption::new("none", "None", "Standard processing"),
			UiOption::new("flex", "Flex", "Lower cost, higher latency (Gemini API + Vertex)"),
			UiOption::new("priority", "Priority", "Faster, higher reliability (Gemini API + Vertex)")
		]),
		None,
		Identity
	),
	ui!(
		"modelRoleStorage",
		"ai_model_role_storage",
		Model,
		"Prompt",
		"Model Role Storage",
		"Where model selector role assignments are saved",
		UiWidget::Submenu(&[
			UiOption::new(
				"global",
				"Global",
				"Save role models in the active profile config (current behavior)"
			),
			UiOption::new(
				"project",
				"Per-project",
				"Save project role models in .omp/config.yml; missing project roles use global \
				 defaults"
			)
		]),
		None,
		Identity
	),
	ui!(
		"inlineToolDescriptors",
		"ai_inline_tool_descriptors",
		Model,
		"Prompt",
		"Inline Tool Descriptors",
		"Render full tool descriptors in the system prompt and strip top-level/nested descriptions \
		 from provider tool schemas so descriptor text is sent once. Auto enables this for Gemini \
		 models and disables it otherwise",
		UiWidget::Submenu(&[
			UiOption::new(
				"auto",
				"Auto",
				"Inline descriptors for Gemini models; keep them in tool schemas otherwise"
			),
			UiOption::new("on", "On", "Always inline descriptors in the system prompt"),
			UiOption::new("off", "Off", "Keep descriptors in provider tool schemas only")
		]),
		None,
		Identity
	),
	ui!(
		"includeModelInPrompt",
		"ai_include_model_in_prompt",
		Model,
		"Prompt",
		"Include Model in Prompt",
		"Surface the active model identifier in the system prompt so the agent knows which model it \
		 is",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"includeWorkspaceTree",
		"ai_include_workspace_tree",
		Model,
		"Prompt",
		"Include Workspace Tree",
		"Render the workspace directory tree in the system prompt. WARNING: This can bust prompt \
		 caching across sessions when files are modified.",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"skillful",
		"ai_skillful",
		Model,
		"Prompt",
		"List Skills in Prompt",
		"List available skills in the system prompt; disable to save context and toggle per-session \
		 with /skillful",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"personality",
		"ai_personality",
		Model,
		"Prompt",
		"Personality",
		"Communication style rendered into the system prompt's personality block",
		UiWidget::Submenu(&[
			UiOption::new(
				"default",
				"Default",
				"Terse, evidence-first engineer; dense, action-oriented replies"
			),
			UiOption::new(
				"friendly",
				"Friendly",
				"Warm, encouraging collaborator focused on momentum and morale"
			),
			UiOption::new(
				"pragmatic",
				"Pragmatic",
				"Direct, efficient engineer focused on clarity and rigor"
			),
			UiOption::new("none", "None", "Omit the personality block entirely")
		]),
		None,
		Identity
	),
	ui!(
		"retry.maxRetries",
		"ai_retry_max_retries",
		Model,
		"Retry & Fallback",
		"Retry Attempts",
		"Maximum retry attempts on API errors",
		UiWidget::Submenu(&[
			UiOption::new("1", "1 retry", ""),
			UiOption::new("2", "2 retries", ""),
			UiOption::new("3", "3 retries", ""),
			UiOption::new("5", "5 retries", ""),
			UiOption::new("10", "10 retries", "")
		]),
		None,
		Identity
	),
	ui!(
		"retry.maxDelayMs",
		"ai_retry_max_delay_ms",
		Model,
		"Retry & Fallback",
		"Max Retry Delay",
		"Maximum wait between retries, in ms. When the provider asks us to wait longer than this \
		 and no credential or model fallback succeeds, the request fails fast instead of sleeping \
		 (e.g. 3-hour Anthropic rate-limit windows). 0 disables the ceiling — to let the session \
		 auto-resume through provider-stated quota resets.",
		UiWidget::ConfigOnly,
		None,
		Identity
	),
	ui!(
		"retry.modelFallback",
		"ai_retry_model_fallback",
		Model,
		"Retry & Fallback",
		"Retry Model Fallback",
		"Allow retry recovery to switch to configured fallback models",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"retry.usageAwareFallback",
		"ai_retry_usage_aware_fallback",
		Model,
		"Retry & Fallback",
		"Usage-Aware Fallback",
		"Use reliable coding-plan quota reports to prefer same-provider accounts, then configured \
		 fallback models, before a hard usage limit. Ordinary configured API keys are excluded.",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"retry.usageReservePct",
		"ai_retry_usage_reserve_pct",
		Model,
		"Retry & Fallback",
		"Reserve Margin",
		"Treat a coding-plan model as near its limit below this remaining percentage. Unknown or \
		 unmapped usage keeps the primary model.",
		UiWidget::Submenu(&[
			UiOption::new("5", "5%", "Act only when nearly exhausted"),
			UiOption::new("10", "10%", "Balanced safety margin"),
			UiOption::new("15", "15%", "Conservative"),
			UiOption::new("20", "20%", "Early protection"),
			UiOption::new("25", "25%", "Very conservative")
		]),
		Some(UiCondition::UsageAwareFallbackEnabled),
		Identity
	),
	ui!(
		"retry.usageReservePolicy",
		"ai_retry_usage_reserve_policy",
		Model,
		"Retry & Fallback",
		"Reserve Policy",
		"What to do when every same-provider coding-plan account is inside the reserve margin.",
		UiWidget::Submenu(&[
			UiOption::new(
				"confirm",
				"Confirm interactively",
				"Keep interactive sessions on the primary until confirmed; background agents \
				 auto-fallback"
			),
			UiOption::new(
				"auto",
				"Auto-fallback",
				"Always select the next eligible configured fallback"
			),
			UiOption::new(
				"fail-closed",
				"Fail closed",
				"Do not spend reserve quota or select a fallback"
			)
		]),
		Some(UiCondition::UsageAwareFallbackEnabled),
		Identity
	),
	ui!(
		"retry.fallbackChains",
		"ai_retry_fallback_chains",
		Model,
		"Retry & Fallback",
		"Retry Fallback Chains",
		"JSON object mapping model roles, model selectors (\"provider/model-id\"), or provider \
		 wildcards (\"provider/*\") to ordered fallback selectors, e.g. \
		 {\"default\":[\"openai/gpt-4o-mini\"],\"google-antigravity/*\":[\"google/*\",\"\
		 google-vertex/*\"]}. Model-oriented keys apply whenever that model/provider is active, \
		 regardless of role; a \"provider/*\" entry keeps the failing model's id and swaps the \
		 provider. An id-prefixed wildcard (\"openrouter/google/*\") re-prefixes the failing \
		 model's bare id (google-antigravity/gemini-x -> openrouter/google/gemini-x) and, used as a \
		 key, matches only that provider's ids under the prefix.",
		UiWidget::Text { secret: false },
		None,
		Identity
	),
	ui!(
		"retry.fallbackRevertPolicy",
		"ai_retry_fallback_revert",
		Model,
		"Retry & Fallback",
		"Fallback Revert Policy",
		"When to return to the primary model after a fallback",
		UiWidget::Submenu(&[
			UiOption::new(
				"cooldown-expiry",
				"Cooldown expiry",
				"Return to the primary model after its suppression window ends"
			),
			UiOption::new("never", "Never", "Stay on the fallback model until manually changed")
		]),
		None,
		Identity
	),
	ui!(
		"providers.anthropic.serverSideFallback",
		"ai_retry_server_side_fallback",
		Model,
		"Retry & Fallback",
		"Anthropic Server-Side Fallback (Fable 5)",
		"When a Claude Fable 5 / Mythos 5 request is blocked by Anthropic's safety classifier, \
		 retry it on Claude Opus 4.8 server-side (Anthropic `server-side-fallback-2026-06-01` \
		 beta). Opt-in — leaving this off preserves the pre-fallback behavior for every request.",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"advisor.enabled",
		"ai_advisor_enabled",
		Model,
		"Advisor",
		"Enable Advisor",
		"Pair a second model (assigned to the 'advisor' role) that passively reviews each turn and \
		 injects notes.",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"advisor.syncBacklog",
		"ai_advisor_sync_backlog",
		Model,
		"Advisor",
		"Advisor Sync Backlog",
		"Pause the main agent for up to 30 seconds if the advisor falls behind by this many turns. \
		 Off disables catch-up delays.",
		UiWidget::Enum(&["off", "1", "3", "5"]),
		Some(UiCondition::AdvisorEnabled),
		Identity
	),
	ui!(
		"advisor.immuneTurns",
		"ai_advisor_immune_turns",
		Model,
		"Advisor",
		"Advisor Immune Turns",
		"After an advisor concern or blocker interrupts, route further concerns/blockers \
		 non-interruptingly for this many primary turns.",
		UiWidget::Submenu(&[
			UiOption::new("0", "0 turns", "Allow every concern/blocker to interrupt."),
			UiOption::new("1", "1 turn", ""),
			UiOption::new("2", "2 turns", ""),
			UiOption::new("3", "3 turns", "Default."),
			UiOption::new("4", "4 turns", ""),
			UiOption::new("5", "5 turns", "")
		]),
		Some(UiCondition::AdvisorEnabled),
		Identity
	),
	ui!(
		"prewalk.enabled",
		"ai_prewalk_enabled",
		Model,
		"Prewalk",
		"Enable Prewalk",
		"Start on the active model, then switch to a fast/cheap model (default the 'smol' role) at \
		 the first edit/write after the plan nudge's todo list exists — the strong model plans, \
		 commits the todos, and starts the implementation before handing off. Overridable per \
		 session with --prewalk / --no-prewalk.",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"images.describeForTextModels",
		"sv_images_describe_for_text_models",
		Model,
		"Vision",
		"Describe Images for Text Models",
		"When an image is attached to a model without vision support, save it under local:// and \
		 inject a description from a vision-capable model instead of dropping it",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"images.urls.enabled",
		"sv_images_urls_enabled",
		Model,
		"Vision",
		"Serve Images as URLs",
		"Publish outgoing images through the configured backend chain and send URL-fetching \
		 providers short URLs instead of inline base64. Falls back to inline automatically when \
		 every backend or a provider fetch fails",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"images.urls.backends",
		"sv_images_urls_backends",
		Model,
		"Vision",
		"Image URL Backends",
		"Ordered destinations tried when publishing images for provider access",
		UiWidget::MultiSelect { options: BLOB_BACKEND_CHOICES, ordered: true },
		None,
		Identity
	),
	ui!(
		"images.urls.command",
		"sv_images_urls_command",
		Model,
		"Vision",
		"Image Upload Command",
		"Argv template for the command backend; {file} is the image path, {mime}/{ext} optional. \
		 The last URL printed on stdout is used (e.g. pasta -b -f {file})",
		UiWidget::Text { secret: false },
		None,
		Identity
	),
	ui!(
		"images.urls.publicBaseUrl",
		"sv_images_urls_public_base_url",
		Model,
		"Vision",
		"Image URL Public Base",
		"Externally reachable base URL fronting the blob server (required for ssh, optional for \
		 direct)",
		UiWidget::Text { secret: false },
		None,
		Identity
	),
	ui!(
		"images.urls.ttlHours",
		"sv_images_urls_ttl_hours",
		Model,
		"Vision",
		"Image URL Lifetime (hours)",
		"Serving window for locally hosted image URLs, measured from the last time a conversation \
		 sent them; resuming a conversation re-arms the window at the same link. 0 keeps links \
		 alive while the broker runs",
		UiWidget::ConfigOnly,
		None,
		Identity
	),
	ui!(
		"images.urls.bindHost",
		"sv_images_urls_bind_host",
		Model,
		"Vision",
		"Image URL Bind Host",
		"Host the blob server binds to; loopback for tunnels, 0.0.0.0 for direct serving",
		UiWidget::Text { secret: false },
		None,
		Identity
	),
	ui!(
		"images.urls.sshTarget",
		"sv_images_urls_ssh_target",
		Model,
		"Vision",
		"Image URL SSH Target",
		"user@host destination for the ssh reverse forward",
		UiWidget::Text { secret: false },
		None,
		Identity
	),
	ui!(
		"images.urls.sshRemotePort",
		"sv_images_urls_ssh_remote_port",
		Model,
		"Vision",
		"Image URL SSH Remote Port",
		"Remote listen port of the ssh reverse forward that your web server proxies to",
		UiWidget::ConfigOnly,
		None,
		Identity
	),
];

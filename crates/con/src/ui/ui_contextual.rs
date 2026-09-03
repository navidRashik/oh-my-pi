//! Mechanical projection of current pi settings UI metadata.

use super::*;

pub(super) const ENTRIES: &[UiSpec] = &[
	ui!(
		"workspace.additionalDirectories",
		"sv_workspace_dirs",
		Context,
		"General",
		"Additional Workspace Dirs",
		"Extra workspace directories added to every session as additional roots (multi-root \
		 workspace). Managed live via /add-dir and /remove-dir. Paths resolve relative to cwd; \
		 absolute paths recommended. The agent is told these roots exist and can read/grep/glob \
		 them.",
		UiWidget::ConfigOnly,
		None,
		Identity
	),
	ui!(
		"contextPromotion.enabled",
		"ai_context_promotion_enabled",
		Context,
		"General",
		"Auto-Promote Context",
		"Promote to a larger-context model on context overflow instead of compacting",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"extendedContext",
		"ai_extended_context",
		Context,
		"General",
		"Extended Context",
		"Use premium long-context windows on models that bill extra past a threshold (e.g. GPT-5.6 \
		 1M charges 2x input above 272K); off caps them at the standard-pricing window",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"branchSummary.enabled",
		"ai_branch_summary_enabled",
		Context,
		"General",
		"Branch Summaries",
		"Prompt to summarize when leaving a branch",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"compaction.enabled",
		"ai_compaction_enabled",
		Context,
		"Compaction",
		"Auto-Compact",
		"Automatically compact context when it gets too large",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"compaction.midTurnEnabled",
		"ai_compaction_mid_turn_enabled",
		Context,
		"Compaction",
		"Mid-Turn Compaction",
		"Check thresholds at safe mid-turn tool-loop boundaries before the next provider request",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"compaction.methodOrder",
		"ai_compaction_method_order",
		Context,
		"Compaction",
		"Compaction Method Order",
		"Preferred fallback order for automatic context maintenance; unavailable or failed methods \
		 advance to the next choice",
		UiWidget::MultiSelect {
			options: &[
				UiOption::new(
					"remote",
					"OpenAI server compaction",
					"Use provider-native OpenAI-compatible server compaction when the active route \
					 supports it"
				),
				UiOption::new(
					"snapcompact",
					"Snapcompact",
					"Archive history onto dense bitmap images the active vision model reads back; no \
					 LLM call"
				),
				UiOption::new(
					"handoff",
					"Handoff",
					"Generate a handoff document and continue from it as the compaction summary"
				),
				UiOption::new(
					"soft",
					"Soft compaction",
					"Summarize in place with a compaction model without using server compaction"
				),
				UiOption::new(
					"shake",
					"Shake",
					"Drop recoverable heavy content in place without an LLM call"
				)
			],
			ordered: true,
		},
		None,
		Identity
	),
	ui!(
		"compaction.thresholdPercent",
		"ai_compact_threshold",
		Context,
		"Compaction",
		"Compaction Threshold",
		"Percent threshold for context maintenance; set to Default to use legacy reserve-based \
		 behavior",
		UiWidget::Submenu(&[
			UiOption::new("default", "Default", "Legacy reserve-based threshold"),
			UiOption::new("10", "10%", "Extremely early maintenance"),
			UiOption::new("20", "20%", "Very early maintenance"),
			UiOption::new("30", "30%", "Early maintenance"),
			UiOption::new("40", "40%", "Moderately early maintenance"),
			UiOption::new("50", "50%", "Halfway point"),
			UiOption::new("60", "60%", "Moderate context usage"),
			UiOption::new("70", "70%", "Balanced"),
			UiOption::new("75", "75%", "Slightly aggressive"),
			UiOption::new("80", "80%", "Typical threshold"),
			UiOption::new("85", "85%", "Aggressive context usage"),
			UiOption::new("90", "90%", "Very aggressive"),
			UiOption::new("95", "95%", "Near context limit")
		]),
		None,
		PercentFraction
	),
	ui!(
		"compaction.thresholdTokens",
		"ai_compaction_threshold_tokens",
		Context,
		"Compaction",
		"Compaction Token Limit",
		"Fixed token limit for context maintenance; overrides percentage if set",
		UiWidget::Submenu(&[
			UiOption::new("default", "Default", "Use percentage-based threshold"),
			UiOption::new("25000", "25K tokens", "1/8 of a 200K window"),
			UiOption::new("50000", "50K tokens", "1/4 of a 200K window"),
			UiOption::new("100000", "100K tokens", "1/2 of a 200K window"),
			UiOption::new("150000", "150K tokens", "3/4 of a 200K window"),
			UiOption::new("200000", "200K tokens", "Full standard context window"),
			UiOption::new("300000", "300K tokens", "Large context window"),
			UiOption::new("500000", "500K tokens", "Very large context window")
		]),
		None,
		DefaultMinusOne
	),
	ui!(
		"compaction.handoffSaveToDisk",
		"ai_compaction_handoff_save_to_disk",
		Context,
		"Compaction",
		"Save Handoff Docs",
		"Save generated handoff documents to markdown files for the auto-handoff flow",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"compaction.remoteStreamingV2Enabled",
		"ai_compaction_remote_streaming_v2_enabled",
		Context,
		"Compaction",
		"Remote Compaction V2",
		"Use Responses streaming compaction for compatible remote compaction models",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"compaction.asyncEnabled",
		"ai_compaction_async_enabled",
		Context,
		"Compaction",
		"Async Compaction",
		"Speculatively summarize in the background as context nears the compaction threshold, then \
		 splice the ready result in when the threshold is crossed",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"compaction.idleEnabled",
		"ai_compaction_idle_enabled",
		Context,
		"Compaction",
		"Idle Compaction",
		"Compact context while idle when token count exceeds threshold",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"compaction.idleThresholdTokens",
		"ai_compaction_idle_threshold_tokens",
		Context,
		"Compaction",
		"Idle Compaction Threshold",
		"Token count above which idle compaction triggers",
		UiWidget::Submenu(&[
			UiOption::new("100000", "100K tokens", ""),
			UiOption::new("200000", "200K tokens", ""),
			UiOption::new("300000", "300K tokens", ""),
			UiOption::new("400000", "400K tokens", ""),
			UiOption::new("500000", "500K tokens", ""),
			UiOption::new("600000", "600K tokens", ""),
			UiOption::new("700000", "700K tokens", ""),
			UiOption::new("800000", "800K tokens", ""),
			UiOption::new("900000", "900K tokens", "")
		]),
		None,
		Identity
	),
	ui!(
		"compaction.idleTimeoutSeconds",
		"ai_compaction_idle_timeout_seconds",
		Context,
		"Compaction",
		"Idle Compaction Delay",
		"Seconds to wait while idle before compacting",
		UiWidget::Submenu(&[
			UiOption::new("60", "1 minute", ""),
			UiOption::new("120", "2 minutes", ""),
			UiOption::new("300", "5 minutes", ""),
			UiOption::new("600", "10 minutes", ""),
			UiOption::new("1800", "30 minutes", ""),
			UiOption::new("3600", "1 hour", "")
		]),
		None,
		Identity
	),
	ui!(
		"compaction.supersedeReads",
		"ai_compaction_supersede_reads",
		Context,
		"Compaction",
		"Supersede Stale Reads",
		"Prune older read results when the same file is read again (cache-aware, runs every turn)",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"compaction.dropUseless",
		"ai_compaction_drop_useless",
		Context,
		"Compaction",
		"Elide Uneventful Results",
		"Prune tool results flagged contextually useless (no matches, timed-out waits) once \
		 consumed (cache-aware)",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"ttsr.enabled",
		"ai_ttsr_enabled",
		Context,
		"Rules (TTSR)",
		"TTSR",
		"Interrupt the agent mid-stream when output matches rule patterns (Time-Traveling Stream \
		 Rules)",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"ttsr.contextMode",
		"ai_ttsr_context_mode",
		Context,
		"Rules (TTSR)",
		"TTSR Context Mode",
		"What to do with partial output when TTSR triggers",
		UiWidget::Enum(&["discard", "keep"]),
		None,
		Identity
	),
	ui!(
		"ttsr.interruptMode",
		"ai_ttsr_interrupt_mode",
		Context,
		"Rules (TTSR)",
		"TTSR Interrupt Mode",
		"When to interrupt mid-stream vs inject warning after completion",
		UiWidget::Submenu(&[
			UiOption::new("always", "always", "Interrupt on prose and tool streams"),
			UiOption::new("prose-only", "prose-only", "Interrupt only on reply/thinking matches"),
			UiOption::new("tool-only", "tool-only", "Interrupt only on tool-call argument matches"),
			UiOption::new("never", "never", "Never interrupt; inject warning after completion")
		]),
		None,
		Identity
	),
	ui!(
		"ttsr.repeatMode",
		"ai_ttsr_repeat_mode",
		Context,
		"Rules (TTSR)",
		"TTSR Repeat Mode",
		"How rules can repeat: once per session or after a message gap",
		UiWidget::Enum(&["once", "after-gap"]),
		None,
		Identity
	),
	ui!(
		"ttsr.repeatGap",
		"ai_ttsr_repeat_gap",
		Context,
		"Rules (TTSR)",
		"TTSR Repeat Gap",
		"Messages before a rule can trigger again",
		UiWidget::Submenu(&[
			UiOption::new("5", "5 messages", ""),
			UiOption::new("10", "10 messages", ""),
			UiOption::new("15", "15 messages", ""),
			UiOption::new("20", "20 messages", ""),
			UiOption::new("30", "30 messages", "")
		]),
		None,
		Identity
	),
	ui!(
		"ttsr.builtinRules",
		"ai_ttsr_builtin_rules",
		Context,
		"Rules (TTSR)",
		"Built-in Rules",
		"Load the default rules shipped with the agent (override individually with \
		 ttsr.disabledRules)",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"ttsr.disabledRules",
		"ai_ttsr_disabled_rules",
		Context,
		"Rules (TTSR)",
		"Disabled Rules",
		"Rule names to ignore entirely (applies to bundled defaults and your own rules)",
		UiWidget::ConfigOnly,
		None,
		Identity
	),
	ui!(
		"snapcompact.systemPrompt",
		"ai_snapcompact_system_prompt",
		Context,
		"Experimental",
		"Snapcompact System Prompt",
		"Experimental: render selected system prompt text as dense PNG image(s) and attach to the \
		 first user message (vision models only). Saves tokens; loses prompt caching for imaged \
		 text.",
		UiWidget::Submenu(&[
			UiOption::new("none", "None", "Keep the system prompt as text."),
			UiOption::new(
				"agents-md",
				"AGENTS.md",
				"Only move loaded context-file instructions to images, when that saves tokens."
			),
			UiOption::new(
				"all",
				"All",
				"Move the full system prompt to images, when that saves tokens."
			)
		]),
		None,
		Identity
	),
	ui!(
		"snapcompact.toolResults",
		"ai_snapcompact_tool_results",
		Context,
		"Experimental",
		"Snapcompact Tool Results",
		"Experimental: render large historical tool results as dense PNG image(s) instead of text \
		 (vision models only). Saves tokens on accumulated read/search output.",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"tools.format",
		"sv_tools_format",
		Context,
		"Experimental",
		"Tool Calling Mode",
		"Controls how tools are exposed to the model. Auto uses provider-native tool calls unless \
		 the selected model is marked as not supporting them, then falls back to the GLM owned \
		 dialect. Native forces provider-native tools; the other values force the named owned \
		 dialect. Applies on session start.",
		UiWidget::Submenu(&[
			UiOption::new(
				"auto",
				"Auto",
				"Use native tool calls unless the model is known not to support them."
			),
			UiOption::new("native", "Native", "Use provider-native tool calls."),
			UiOption::new("glm", "GLM", "Use GLM-style in-band tool calls."),
			UiOption::new("hermes", "Hermes", "Use Hermes-style in-band tool calls."),
			UiOption::new("kimi", "Kimi", "Use Kimi-style in-band tool calls."),
			UiOption::new("xml", "XML", "Use generic XML in-band tool calls."),
			UiOption::new("anthropic", "Anthropic", "Use Anthropic-style in-band tool calls."),
			UiOption::new("deepseek", "DeepSeek", "Use DeepSeek-style in-band tool calls."),
			UiOption::new("harmony", "Harmony", "Use Harmony-style in-band tool calls."),
			UiOption::new("qwen3", "Qwen3", "Use the Qwen3 owned dialect."),
			UiOption::new("gemini", "Gemini", "Use the Gemini owned dialect."),
			UiOption::new("gemma", "Gemma", "Use the Gemma owned dialect."),
			UiOption::new("minimax", "MiniMax", "Use the MiniMax owned dialect.")
		]),
		None,
		Identity
	),
	ui!(
		"snapcompact.shape",
		"ai_snapcompact_shape",
		Context,
		"Experimental",
		"Snapcompact Shape",
		"Frame shape snapcompact prints text with (compaction archive and inline imaging). Auto \
		 picks a shape tuned for the current model.",
		UiWidget::Submenu(&[
			UiOption::new(
				"auto",
				"Auto",
				"Picks a shape tuned for the current model, falling back to its provider family."
			),
			UiOption::new(
				"8x8r-bw",
				"8x8 repeated, black",
				"unscii square cell, black ink, every line printed twice with the copy on a pale \
				 highlight band."
			),
			UiOption::new(
				"8x8r-sent",
				"8x8 repeated, sentence hues",
				"Repeated grid with ink cycling six hues at sentence boundaries."
			),
			UiOption::new(
				"8x8u-bw",
				"8x8, black",
				"Plain unscii square cell, single-printed lines, black ink."
			),
			UiOption::new(
				"8x8u-sent",
				"8x8, sentence hues",
				"Plain unscii square cell with sentence-hue ink."
			),
			UiOption::new(
				"6x6u-bw",
				"6x6 dense, black",
				"unscii squeezed to 6x6 — densest readable cell, fewest frames — in black ink."
			),
			UiOption::new(
				"6x6u-sent",
				"6x6 dense, sentence hues",
				"Densest cell with sentence-hue ink."
			),
			UiOption::new(
				"5x8-bw",
				"5x8 legacy, black",
				"Original X.org 5x8 glyphs on the 2576px frame, black ink."
			),
			UiOption::new(
				"5x8-sent",
				"5x8 legacy, sentence hues",
				"The original snapcompact shape (pre-shape-table sessions rendered this)."
			),
			UiOption::new(
				"6x12-dim",
				"6x12, dimmed stopwords",
				"X.org 6x12 glyphs, black ink, function words dimmed gray."
			),
			UiOption::new("8x13-bw", "8x13, black", "X.org 8x13 glyphs, black ink."),
			UiOption::new(
				"8on16-bw",
				"8x13 on 16px pitch, black",
				"8x13 glyphs on an 8x16 cell (extra leading), black ink."
			),
			UiOption::new(
				"8on22-bw",
				"8x13 on 22px pitch (leading), black",
				"8x13 glyphs on an 8x22 cell — extra line spacing so rows don't crowd. Default for \
				 OpenAI/Google."
			),
			UiOption::new(
				"11on16-bw",
				"8x13 on 11px advance (tracking), black",
				"8x13 glyphs on an 11x16 cell — extra letter spacing so characters don't merge. \
				 Default for Anthropic."
			),
			UiOption::new(
				"silver16-bw",
				"Silver 16, CJK",
				"Embedded Silver TrueType font on a 16px grid for CJK and other non-Latin text."
			),
			UiOption::new(
				"doc-8on16-bw",
				"Doc 8on16, black",
				"Two word-wrapped newspaper columns of 8x13 glyphs on a 16px pitch, black ink."
			),
			UiOption::new(
				"doc-8on16-sent",
				"Doc 8on16, sentence hues",
				"Two-column doc layout with sentence-hue ink."
			),
			UiOption::new(
				"doc-8on16-sent-dim",
				"Doc 8on16, sentence hues + dimmed stopwords",
				"Two-column doc layout, sentence-hue ink, function words dimmed gray."
			)
		]),
		None,
		Identity
	),
	ui!(
		"bash.enabled",
		"sv_shell_enabled",
		Shell,
		"Bash",
		"Bash",
		"Enable the bash tool for shell command execution",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"bash.autoBackground.enabled",
		"sv_shell_auto_background_enabled",
		Shell,
		"Bash",
		"Bash Auto-Background",
		"Automatically background long-running bash commands and deliver the result later",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"bashInterceptor.enabled",
		"sv_shell_interceptor_enabled",
		Shell,
		"Bash",
		"Bash Interceptor",
		"Block shell commands that have dedicated tools",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"bash.direnv",
		"sv_shell_direnv",
		Shell,
		"Bash",
		"direnv Auto-Load",
		"Auto-load a repo's direnv/devenv `.envrc` into the bash session so devenv tools and env \
		 vars are present without manual `direnv exec`. Honors direnv's allow list: an `.envrc` you \
		 haven't `direnv allow`ed is never executed",
		UiWidget::Enum(&["auto", "off"]),
		None,
		Identity
	),
	ui!(
		"eval.py",
		"sv_eval_py",
		Shell,
		"Eval & Runtimes",
		"Python Eval Backend",
		"Allow the eval tool to dispatch Python cells to the IPython kernel",
		UiWidget::Boolean,
		None,
		Identity
	),
];

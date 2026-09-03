//! Mechanical projection of pi's complete Shell settings tab.
//!
//! `shellMinimizer.enabled` and `shellMinimizer.sourceOutlineLevel` remain
//! absent by ADR 0009: the runtime retains complete output and applies its
//! single bound centrally instead of installing a lossy shell-output rewrite
//! layer. `eval.js` and `python.interpreter` remain absent by ADR 0036:
//! Eval has one bundled CPython runtime and never depends on a host
//! interpreter.

use super::*;

pub(super) const ENTRIES: &[UiSpec] = &[
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
		"bash.patterns",
		"sv_bash_patterns",
		Shell,
		"Bash",
		"Bash Approval Patterns",
		"Ordered bash command approval rules. Each item has match and approval fields; only '*' \
		 wildcards are supported.",
		UiWidget::ConfigOnly,
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
		"bash.direnvLoadTimeoutMs",
		"sv_shell_direnv_load_timeout_ms",
		Shell,
		"Bash",
		"direnv Load Timeout (ms)",
		"Max wait for the first `direnv export` (a cold devenv shell can be slow); on timeout the \
		 session runs without the direnv env",
		UiWidget::ConfigOnly,
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
	ui!(
		"eval.tools.enabled",
		"sv_eval_tools_enabled",
		Shell,
		"Eval & Runtimes",
		"Eval-Defined Tools",
		"Let eval cells define tools (@tool in Python, tool(fn) in JS) that task, agent(), and \
		 workpool() subagents can call",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"eval.workpool.freshAgents",
		"sv_eval_workpool_fresh_agents",
		Shell,
		"Eval & Runtimes",
		"Fresh Workpool Agents",
		"Spawn a new subagent for every workpool item instead of reusing workers or batching queued \
		 items",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"eval.autoBackground.enabled",
		"sv_eval_auto_background_enabled",
		Shell,
		"Eval & Runtimes",
		"Eval Auto-Background",
		"Automatically background long-running eval cells and deliver the result later",
		UiWidget::Boolean,
		None,
		Identity
	),
	ui!(
		"python.kernelMode",
		"sv_python_kernel_mode",
		Shell,
		"Eval & Runtimes",
		"Python Kernel Mode",
		"Keep the IPython kernel alive across eval calls or start fresh each time",
		UiWidget::Enum(&["session", "per-call"]),
		None,
		Identity
	),
];

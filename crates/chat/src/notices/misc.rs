//! Custom transcript notices: background-job deliveries, late LSP
//! diagnostics, `/tan` breadcrumbs, advisor notes, collaboration guest
//! bubbles, and collapsed synthetic input.

use std::fmt::Write as _;

use omp_core::{Str, StrMut, sf};
use omp_dom::{Node, PropId, Value};
use omp_journal::data::{
	AsyncJobStatus, AsyncResult, LaunchCompletion, LaunchDaemonCompletion, LaunchDaemonStatus,
};
use omp_tui::{IntoComponent as _, dom};

use super::{format_duration, prop_text};
use crate::cards::Component;

/// Reads an async-result payload from its journal-derived user node.
#[must_use]
pub(crate) fn async_result(node: &Node) -> Option<AsyncResult> {
	if node.prop(&omp_dom::PropKey::Custom(Str::new_static("async_result")))
		!= Some(&Value::Bool(true))
	{
		return None;
	}
	let Value::Json(data) = node.prop(&PropId::Data.into())? else {
		return None;
	};
	let result: AsyncResult = serde_json::from_str(data.get()).ok()?;
	(!result.jobs.is_empty()).then_some(result)
}

/// Plain text represented by an async-result block. It carries every compact
/// row fact so transcript dumps and non-terminal projections match the visible
/// presentation; artifact and fault details remain in the model-facing body.
#[must_use]
pub(crate) fn async_result_text(result: &AsyncResult) -> Str {
	let mut text = StrMut::new("");
	for (index, job) in result.jobs.iter().enumerate() {
		if index > 0 {
			text.push('\n');
		}
		let _ = write!(text, "Background job {} [{}] {}", job.status, job.job_type, job.id);
		if !job.label.is_empty() {
			let _ = write!(text, " — {}", job.label);
		}
		let _ = write!(text, " ({})", format_duration(job.duration_ms));
	}
	text.freeze()
}

/// One compact success/failure row per completed background-job delivery.
#[must_use]
pub(crate) fn async_result_block(result: &AsyncResult) -> Component {
	let rows = result
		.jobs
		.iter()
		.map(|job| {
			let failed = job.status != AsyncJobStatus::Completed;
			let state = sf!("Background job {}", job.status);
			let kind = sf!("[{}]", job.job_type);
			let duration = sf!("({})", format_duration(job.duration_ms));
			let label = (!job.label.is_empty()).then(|| job.label.clone());
			dom! {
				<row gap=1 pad-x=1>
					if failed { <i:error fg=err/> } else { <i:done fg=ok/> }
					<text fg={if failed { "err" } else { "ok" }}>{state}</text>
					<text fg=muted dim>{kind}</text>
					<text fg=accent>{job.id.clone()}</text>
					if let Some(label) = label {
						<i:dash fg=muted dim/>
						<text fg=muted dim truncate=end>{label}</text>
					}
					<text fg=muted dim>{duration}</text>
				</row>
			}
			.into_component()
		})
		.collect::<Vec<Component>>();
	dom! { <col>{rows}</col> }.into_component()
}

/// Reads a supervised-process completion payload from its journal-derived
/// user node.
#[must_use]
pub(crate) fn launch_completion(node: &Node) -> Option<LaunchCompletion> {
	if node.prop(&omp_dom::PropKey::Custom(Str::new_static("launch_completion")))
		!= Some(&Value::Bool(true))
	{
		return None;
	}
	let Value::Json(data) = node.prop(&PropId::Data.into())? else {
		return None;
	};
	let completion: LaunchCompletion = serde_json::from_str(data.get()).ok()?;
	(!completion.daemons.is_empty()).then_some(completion)
}

/// Plain-text projection of supervised-process completion rows.
#[must_use]
pub(crate) fn launch_completion_text(completion: &LaunchCompletion) -> Str {
	let mut text = StrMut::new("");
	for (index, daemon) in completion.daemons.iter().enumerate() {
		if index > 0 {
			text.push('\n');
		}
		let _ = write!(text, "Supervised process {} {}", daemon.status, daemon.name);
		if let Some(code) = daemon.exit_code {
			let _ = write!(text, " (exit {code})");
		}
		let _ = write!(text, " ({})", format_duration(daemon.duration_ms));
		if let Some(fault) = &daemon.fault {
			let _ = write!(text, " — {}", fault.kind);
			if let Some(message) = &fault.message {
				let _ = write!(text, ": {message}");
			}
			if let Some(signal) = &fault.signal {
				let _ = write!(text, " ({signal})");
			}
		}
	}
	text.freeze()
}

/// Compact success/failure projection of one supervised-process completion.
#[must_use]
fn launch_daemon_row(daemon: &LaunchDaemonCompletion) -> Component {
	let failed = daemon.status == LaunchDaemonStatus::Failed;
	let state = sf!("Supervised process {}", daemon.status);
	let exit = daemon.exit_code.map(|code| sf!("(exit {code})"));
	let duration = sf!("({})", format_duration(daemon.duration_ms));
	let name = daemon.name.clone();
	dom! {
		<row gap=1 pad-x=1>
			if failed { <i:error fg=err/> } else { <i:done fg=ok/> }
			<text fg={if failed { "err" } else { "ok" }}>{state}</text>
			<text fg=accent>{name}</text>
			if let Some(exit) = exit { <text fg=muted dim>{exit}</text> }
			<text fg=muted dim>{duration}</text>
		</row>
	}
	.into_component()
}

/// One compact row per terminal supervised process.
#[must_use]
pub(crate) fn launch_completion_block(completion: &LaunchCompletion) -> Component {
	let rows = completion
		.daemons
		.iter()
		.map(launch_daemon_row)
		.collect::<Vec<Component>>();
	dom! { <col>{rows}</col> }.into_component()
}

/// Dispatches a `<notice kind=K>` custom kind to its renderer; `None` for
/// the controller kinds (`error | warn | info | success`) and anything else.
#[must_use]
pub fn custom_notice(kind: &str, node: &Node) -> Option<Component> {
	match kind {
		"diagnostics" => Some(diagnostics_card(node)),
		"tangent" => Some(tangent_pill(node)),
		"advisor" => Some(advisor_card(node)),
		_ => None,
	}
}

/// One parsed `path:line:col [severity] [source] message (code)` line
/// (pi `parseDiagnosticMessage`, `tools/render-utils.ts:346-359`).
struct Diagnostic<'a> {
	location: String,
	severity: &'a str,
	message:  &'a str,
	code:     Option<&'a str>,
}

impl<'a> Diagnostic<'a> {
	fn parse(line: &'a str) -> Option<Self> {
		let (path, rest) = line.split_once(':')?;
		let (line_no, rest) = rest.split_once(':')?;
		let (col, rest) = rest.split_once(' ')?;
		if path.is_empty() || !is_digits(line_no) || !is_digits(col) {
			return None;
		}
		let rest = rest.trim_start();
		let severity = rest.strip_prefix('[')?;
		let (severity, rest) = severity.split_once(']')?;
		if !matches!(severity, "error" | "warning" | "info" | "hint") {
			return None;
		}
		let mut message = rest.trim_start();
		if let Some(tail) = message.strip_prefix('[')
			&& let Some((_, tail)) = tail.split_once(']')
		{
			message = tail.trim_start();
		}
		let code = message
			.strip_suffix(')')
			.and_then(|body| body.rfind(" ("))
			.map(|at| (&message[at + 2..message.len() - 1], &message[..at]));
		let (code, message) = match code {
			Some((code, body)) => (Some(code), body),
			None => (None, message),
		};
		Some(Self { location: format!("{path}:{line_no}:{col}"), severity, message, code })
	}

	fn icon(&self) -> &'static str {
		match self.severity {
			"error" => "error",
			"warning" => "warning-status",
			_ => "info-status",
		}
	}

	fn color(&self) -> &'static str {
		match self.severity {
			"error" => "error",
			"warning" => "warning",
			_ => "muted",
		}
	}
}

fn is_digits(text: &str) -> bool {
	!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Late LSP diagnostics that arrived after an edit tool returned
/// (pi `late-diagnostics-message.ts` over `formatDiagnostics`,
/// `tools/render-utils.ts:361-487`): a `Late diagnostics` header naming the
/// server, then one tree row per line with a severity glyph, `path:line:col`,
/// and the message tinted by severity.
#[must_use]
pub fn diagnostics_card(node: &Node) -> Component {
	let server = prop_text(node, PropId::Name);
	let lines: Vec<&str> = node
		.content
		.as_deref()
		.map(|content| {
			content
				.lines()
				.map(str::trim_end)
				.filter(|line| !line.trim().is_empty())
				.collect()
		})
		.unwrap_or_default();
	let count = lines.len();
	let errored = lines.iter().any(|line| line.contains("[error]"));
	let rows: Vec<Component> = lines
		.into_iter()
		.enumerate()
		.map(|(index, line)| {
			let last = index + 1 == count;
			match Diagnostic::parse(line) {
				Some(diagnostic) => {
					let icon = diagnostic.icon();
					let color = diagnostic.color();
					let message = diagnostic.message.replace('\t', "    ");
					let location = diagnostic.location;
					let code = diagnostic.code.map(|code| sf!("({code})"));
					dom! {
						<row gap=1>
							<icon name={if last { "tree-last" } else { "tree-branch" }} fg=muted dim/>
							<icon name={icon} fg={color}/>
							<text fg=muted dim>{location}</text>
							<text fg={color}>{message}</text>
							if let Some(code) = code { <text fg=muted dim>{code}</text> }
						</row>
					}
					.into_component()
				},
				None => {
					let color = if line.contains("[error]") {
						"error"
					} else if line.contains("[warning]") {
						"warning"
					} else {
						"muted"
					};
					let text = line.replace('\t', "    ");
					dom! {
						<row gap=1>
							<icon name={if last { "tree-last" } else { "tree-branch" }} fg=muted dim/>
							<text fg={color} grow>{text}</text>
						</row>
					}
					.into_component()
				},
			}
		})
		.collect();
	dom! {
		<col pad-x=1>
			<row gap=1>
				<icon name="lsp" fg=accent/>
				if errored { <icon name="error" fg=error/> } else { <icon name="warning-status" fg=warning/> }
				<text bold>{"Late diagnostics"}</text>
				if let Some(server) = server { <text fg=muted dim>{sf!("({server})")}</text> }
			</row>
			<col pad-x=1>{rows}</col>
		</col>
	}
	.into_component()
}

/// pi `TAN_WORK_PREVIEW_LENGTH` (`background-tan-message.ts:7`).
const TAN_WORK_PREVIEW_LENGTH: usize = 56;

/// pi `previewWork` (`background-tan-message.ts:9-13`): tabs to spaces,
/// whitespace runs collapsed, cut to 55 characters plus `…`.
fn preview_work(work: &str) -> Str {
	let mut text = StrMut::with_capacity(work.len());
	for word in work.split_whitespace() {
		if !text.is_empty() {
			text.push(' ');
		}
		text.push_str(word);
	}
	if text.chars().count() <= TAN_WORK_PREVIEW_LENGTH {
		return text.freeze();
	}
	let mut cut = StrMut::with_capacity(TAN_WORK_PREVIEW_LENGTH + 3);
	cut.extend(text.chars().take(TAN_WORK_PREVIEW_LENGTH - 1));
	cut.push('…');
	cut.freeze()
}

/// `/tan` background-dispatch breadcrumb (pi `background-tan-message.ts`):
/// one muted row `<output> Tangent dispatched [task] <id> — <work>`, with the
/// job id in accent and the work preview dimmed.
#[must_use]
pub fn tangent_pill(node: &Node) -> Component {
	let id = prop_text(node, PropId::Id).unwrap_or_else(|| Str::new_static("unknown"));
	let work = prop_text(node, PropId::Label).map(|label| preview_work(&label));
	dom! {
		<row gap=1 pad-x=1>
			<icon name="output" fg=muted/>
			<text fg=muted>{"Tangent dispatched"}</text>
			<text fg=muted dim>{"[task]"}</text>
			<text fg=accent>{id}</text>
			if let Some(work) = work {
				<icon name="dash" fg=muted dim/>
				<text fg=muted dim>{work}</text>
			}
		</row>
	}
	.into_component()
}

/// pi `severityColor` (`advisor-message.ts:32-41`).
fn severity_color(severity: Option<&str>) -> &'static str {
	match severity {
		Some("blocker") => "error",
		Some("concern") => "warning",
		_ => "muted",
	}
}

/// Advisor note injected into the primary session (pi `advisor-message.ts`):
/// a bold `Advisor` header tag with the severity badge, then a heavy rail
/// tinted per severity beside the bold summary and the note's paragraphs.
#[must_use]
pub fn advisor_card(node: &Node) -> Component {
	let severity = prop_text(node, PropId::Severity);
	let color = severity_color(severity.as_deref());
	let summary = prop_text(node, PropId::Label);
	let paragraphs: Vec<Str> = node
		.content
		.as_deref()
		.map(|content| {
			content
				.split('\n')
				.filter(|paragraph| !paragraph.trim().is_empty())
				.map(|paragraph| Str::new(paragraph.replace('\t', "    ")))
				.collect()
		})
		.unwrap_or_default();
	dom! {
		<col pad-x=1>
			<row gap=1>
				<icon name="advisor" fg=accent/>
				<text bold fg=accent>{"Advisor"}</text>
				if let Some(severity) = severity {
					<row>
						<icon name="bracket-left" fg={color}/>
						<text fg={color}>{severity}</text>
						<icon name="bracket-right" fg={color}/>
					</row>
				}
			</row>
			<row gap=1 pad-x=1>
				<hr vertical border=heavy fg={color}/>
				<col grow>
					if let Some(summary) = summary { <text bold>{summary}</text> }
					for paragraph in paragraphs { <text>{paragraph}</text> }
				</col>
			</row>
		</col>
	}
	.into_component()
}

/// Collaboration guest prompt (pi `collab-prompt-message.ts`): the user
/// bubble under a bold accent `«author» ›` tag naming who typed it.
#[must_use]
pub fn guest_bubble(author: &str, text: Str) -> Component {
	let author = author.trim();
	let tag = sf!("«{}» ›", if author.is_empty() { "guest" } else { author });
	dom! {
		<col>
			<text fg=accent bold pad-x=1>{tag}</text>
			{bubble(text)}
		</col>
	}
	.into_component()
}

/// pi `user-message.ts` bubble: Markdown on the `userMessageBg` tint with
/// one cell of padding on every side.
fn bubble(text: Str) -> Component {
	dom! { <md bg=surface pad="1 1">{text}</md> }.into_component()
}

/// pi `formatBytes` (`utils/format.ts:54-59`): `512B`, `1.5KB`, `2.3MB`.
pub(crate) fn format_bytes(bytes: usize) -> String {
	const KB: f64 = 1024.0;
	#[allow(clippy::cast_precision_loss, reason = "display rounding only")]
	let n = bytes as f64;
	if n < KB {
		format!("{bytes}B")
	} else if n < KB * KB {
		format!("{:.1}KB", n / KB)
	} else if n < KB * KB * KB {
		format!("{:.1}MB", n / (KB * KB))
	} else {
		format!("{:.1}GB", n / (KB * KB * KB))
	}
}

/// pi `syntheticInputLabel` (`user-message.ts:184-192`): the first Markdown
/// heading's text, else `Synthetic input`.
fn synthetic_label(text: &str) -> &str {
	for raw in text.lines() {
		let line = raw.trim();
		if line.is_empty() {
			continue;
		}
		let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
		if (1..=6).contains(&hashes)
			&& let Some(heading) = line[hashes..].strip_prefix(char::is_whitespace)
		{
			let heading = heading.trim();
			return if heading.is_empty() {
				"Synthetic input"
			} else {
				heading
			};
		}
		return "Synthetic input";
	}
	"Synthetic input"
}

/// pi `summarizeSyntheticInput` (`user-message.ts:176-181`):
/// `<label> · <size> · <n> lines`.
fn synthetic_summary(text: &str) -> Str {
	let lines = if text.is_empty() {
		0
	} else {
		text.split('\n').count()
	};
	sf!(
		"{} · {} · {lines} line{}",
		synthetic_label(text),
		format_bytes(text.len()),
		if lines == 1 { "" } else { "s" }
	)
}

/// Synthetic (agent-attributed) user input (pi
/// `CollapsedSyntheticMessageComponent`, `user-message.ts:100-150`): one dim
/// `<label> · <size> · <n> lines · ctrl+o` row; expanded, the full bubble
/// follows it.
#[must_use]
pub fn synthetic_row(text: &str, expanded: bool) -> Component {
	let summary = sf!("{} · ctrl+o", synthetic_summary(text));
	let body = expanded.then(|| bubble(Str::new(text)));
	dom! {
		<col>
			<text fg=muted dim pad-x=1 truncate=end>{summary}</text>
			if let Some(body) = body { {body} }
		</col>
	}
	.into_component()
}

#[cfg(test)]
mod tests {
	use omp_dom::{KnownTag, PropKey, Tag, Value};
	use omp_tui::{CellContent, Color, Ui, UiContext, frame_text};
	use smallvec::smallvec;

	use super::*;

	fn notice(kind: &str, props: &[(PropId, &str)], content: Option<&str>) -> Node {
		let mut all: smallvec::SmallVec<(PropKey, Value), 4> =
			smallvec![(PropId::Kind.into(), Value::Str(Str::new(kind)))];
		for (prop, value) in props {
			all.push(((*prop).into(), Value::Str(Str::new(value))));
		}
		Node {
			tag:     Tag::Known(KnownTag::Notice),
			props:   all,
			kids:    Vec::new(),
			content: content.map(Str::new),
		}
	}

	fn render(component: Component, width: u16) -> String {
		let ui = Ui::from_root(component, width, UiContext::default());
		frame_text(ui.frame())
	}

	#[test]
	fn launch_completion_is_typed_and_compact() {
		let completion = LaunchCompletion {
			daemons: vec![
				LaunchDaemonCompletion {
					name:        Str::new_static("web"),
					status:      LaunchDaemonStatus::Completed,
					exit_code:   Some(0),
					duration_ms: 2_500,
					fault:       None,
				},
				LaunchDaemonCompletion {
					name:        Str::new_static("worker"),
					status:      LaunchDaemonStatus::Failed,
					exit_code:   Some(17),
					duration_ms: 80_000,
					fault:       Some(omp_journal::data::LaunchDaemonFault {
						kind:    omp_journal::data::LaunchDaemonFaultKind::Failed,
						message: Some(Str::new_static("readiness process exited")),
						signal:  Some(Str::new_static("SIGTERM")),
					}),
				},
			],
		};
		let data = serde_json::value::to_raw_value(&completion).expect("completion serializes");
		let node = Node {
			tag:     Tag::Known(KnownTag::User),
			props:   smallvec![
				(PropKey::Custom(Str::new_static("launch_completion")), Value::Bool(true),),
				(PropId::Data.into(), Value::Json(data)),
			],
			kids:    Vec::new(),
			content: Some(Str::new_static("model-facing completion notice")),
		};
		assert_eq!(launch_completion(&node), Some(completion.clone()));

		let text = launch_completion_text(&completion);
		assert_eq!(
			text.as_str(),
			"Supervised process completed web (exit 0) (2.5s)\nSupervised process failed worker \
			 (exit 17) (1m20s) — failed: readiness process exited (SIGTERM)"
		);
		let rendered = render(launch_completion_block(&completion), 80);
		assert!(
			rendered.contains("Supervised process completed web (exit 0) (2.5s)"),
			"{rendered:?}"
		);
		assert!(
			rendered.contains("Supervised process failed worker (exit 17) (1m20s)"),
			"{rendered:?}"
		);
		assert!(
			!rendered.contains("readiness process exited"),
			"fault detail stays out of the compact row: {rendered:?}"
		);
	}

	#[test]
	fn tangent_pill_text() {
		let node = notice(
			"tangent",
			&[(PropId::Id, "job-7"), (PropId::Label, "refactor\tthe   parser\nand add tests")],
			None,
		);
		let text = render(tangent_pill(&node), 80);
		assert_eq!(text, " ⤴ Tangent dispatched [task] job-7 — refactor the parser and add tests");

		let long = "a".repeat(40) + " " + &"b".repeat(40);
		let preview = preview_work(&long);
		assert_eq!(preview.chars().count(), TAN_WORK_PREVIEW_LENGTH);
		assert!(preview.ends_with('…'));
		let exact = "x".repeat(TAN_WORK_PREVIEW_LENGTH);
		assert_eq!(preview_work(&exact), exact.as_str(), "exactly 56 characters is not cut");

		let bare = notice("tangent", &[(PropId::Id, "job-8")], None);
		assert_eq!(render(tangent_pill(&bare), 80), " ⤴ Tangent dispatched [task] job-8");
		assert!(custom_notice("tangent", &bare).is_some());
		assert!(custom_notice("error", &bare).is_none());
	}

	fn rail_color(component: Component) -> omp_tui::Color {
		let ui = Ui::from_root(component, 40, UiContext::default());
		let frame = ui.frame();
		for y in 0..frame.size().height {
			for x in 0..frame.size().width {
				if let CellContent::Grapheme { text, .. } = frame.cell(x, y).content()
					&& text == "┃"
				{
					return frame.cell(x, y).style().foreground_color();
				}
			}
		}
		panic!("no rail painted:\n{}", frame_text(frame));
	}

	#[test]
	fn advisor_rail_color_follows_severity() {
		let theme = UiContext::default().theme;
		let body = "The retry loop re-reads the config every time.\n\nCache it outside the loop.";
		let blocker = notice(
			"advisor",
			&[(PropId::Severity, "blocker"), (PropId::Label, "Config reread per retry")],
			Some(body),
		);
		assert_eq!(rail_color(advisor_card(&blocker)), theme.err);
		let concern = notice("advisor", &[(PropId::Severity, "concern")], Some(body));
		assert_eq!(rail_color(advisor_card(&concern)), theme.warn);
		let plain = notice("advisor", &[], Some(body));
		assert_eq!(rail_color(advisor_card(&plain)), theme.muted);

		let text = render(advisor_card(&blocker), 60);
		let lines: Vec<&str> = text.lines().collect();
		assert!(lines[0].contains("Advisor ⟦blocker⟧"), "{text:?}");
		assert!(lines[1].contains("┃ Config reread per retry"), "{text:?}");
		assert!(text.contains("┃ Cache it outside the loop."), "{text:?}");
		assert_eq!(
			lines.iter().filter(|line| line.contains('┃')).count(),
			3,
			"the rail spans header and both paragraphs: {text:?}"
		);
		let narrow = render(advisor_card(&blocker), 30);
		assert_eq!(
			narrow.lines().filter(|line| line.contains('┃')).count(),
			5,
			"the rail follows wrapped paragraph rows: {narrow:?}"
		);
	}

	#[test]
	fn synthetic_row_collapses_size_and_lines() {
		let mut text = String::from("# Session update\n");
		for index in 0..13 {
			text.push_str(&format!("line {index} of the replay dump {}\n", "x".repeat(64)));
		}
		let text = text.trim_end().to_owned();
		assert_eq!(text.split('\n').count(), 14);
		assert_eq!(text.len(), 1202, "1202 bytes is 1.2KB");
		assert_eq!(synthetic_summary(&text), "Session update · 1.2KB · 14 lines");
		assert_eq!(synthetic_summary(""), "Synthetic input · 0B · 0 lines");
		assert_eq!(synthetic_summary("one"), "Synthetic input · 3B · 1 line");
		assert_eq!(synthetic_summary("#no heading\nmore"), "Synthetic input · 16B · 2 lines");

		let collapsed = render(synthetic_row(&text, false), 60);
		assert_eq!(collapsed, " Session update · 1.2KB · 14 lines · ctrl+o");
		let expanded = render(synthetic_row(&text, true), 60);
		assert!(expanded.starts_with(" Session update · 1.2KB · 14 lines · ctrl+o\n"));
		// pi `CollapsedSyntheticMessageComponent#renderExpanded`: the body is
		// the Markdown `UserMessageComponent`, so the heading renders as a
		// heading rather than its raw `#` source.
		assert!(expanded.contains("\n Session update\n\n line 0 of the replay dump"), "{expanded:?}");
		assert!(!expanded.contains("# Session update"), "{expanded:?}");
	}

	#[test]
	fn guest_bubble_prefixes_bold_author_and_user_tinted_markdown() {
		let ui = Ui::from_root(
			guest_bubble("alice", Str::new_static("can we ship **today**?")),
			40,
			UiContext::default(),
		);
		let text = frame_text(ui.frame());
		let lines: Vec<&str> = text.lines().collect();
		assert_eq!(lines[0], " «alice» ›");
		assert_eq!(lines[1], "", "tinted padding row above the bubble body");
		assert_eq!(lines[2], " can we ship today?");
		assert!(ui.frame().cell(1, 0).style().spec().bold, "authenticated author tag is bold");
		assert_ne!(
			ui.frame().cell(1, 2).style().background_color(),
			Color::Default,
			"Markdown body receives the semantic user-message tint"
		);
		let anonymous = render(guest_bubble("  ", Str::new_static("hi")), 40);
		assert!(anonymous.starts_with(" «guest» ›\n"));
	}

	#[test]
	fn diagnostics_rows_carry_severity_and_location() {
		let node = notice(
			"diagnostics",
			&[(PropId::Name, "rust-analyzer")],
			Some(
				"src/lib.rs:12:5 [error] [rustc] mismatched types (E0308)\nsrc/lib.rs:40:1 [warning] \
				 unused import\nserver restarted",
			),
		);
		let text = render(diagnostics_card(&node), 80);
		let lines: Vec<&str> = text.lines().collect();
		assert_eq!(lines[0], " 💡 ✘ Late diagnostics (rust-analyzer)");
		assert_eq!(lines[1], "  ├─ ✘ src/lib.rs:12:5 mismatched types (E0308)");
		assert_eq!(lines[2], "  ├─ ⚠ src/lib.rs:40:1 unused import");
		assert_eq!(lines[3], "  └─ server restarted");
	}
}

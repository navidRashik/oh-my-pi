//! Observer-local extension and hook status state.
//!
//! Status producers send typed updates keyed by their stable registration id.
//! This module owns ordering and trust-boundary sanitization; the status-band
//! component owns semantic color and overflow policy.

use std::collections::BTreeMap;

use omp_core::Str;
use omp_tui::{CellContent, Ui, UiContext};
use xutf::IntoAnsiStripped as _;

/// One typed extension or hook status update delivered to the chat actor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExtensionStatusEvent {
	/// Insert or replace the value for `key`.
	Set {
		/// Stable producer-local identity used only for replacement and ordering.
		key:  Str,
		/// Human-readable status text. ANSI/VT and control bytes are removed by
		/// [`ExtensionStatuses::apply`].
		text: Str,
	},
	/// Remove the value for `key`.
	Clear {
		/// Stable producer-local identity to remove.
		key: Str,
	},
	/// Drop every value when the actor adopts another session.
	Reset,
}

/// Lowers one extension-authored TML contribution to safe status text.
///
/// Parsing uses the extension trust boundary, so core-only chrome cannot be
/// instantiated. Semantic styling is intentionally not encoded into the
/// returned text: the status segment supplies its semantic accent and the
/// renderer remains the sole owner of width/overflow policy.
pub fn status_text_from_tml(source: &str) -> Result<Str, omp_tui::ParseError> {
	// Source byte length is an upper bound for ordinary rendered text width
	// and avoids imposing a guessed terminal width while extracting content.
	let width = source.len().clamp(1, usize::from(u16::MAX)) as u16;
	let ui = Ui::from_extension_markup(source, width, UiContext::default())?;
	let frame = ui.frame();
	let mut text = String::new();
	for y in 0..frame.size().height {
		let row_start = text.len();
		for x in 0..frame.size().width {
			match frame.cell(x, y).content() {
				CellContent::Blank => text.push(' '),
				CellContent::Grapheme { text: glyph, .. } => text.push_str(glyph),
				CellContent::Continuation | CellContent::Image { .. } => {},
			}
		}
		let row_end = text.trim_end_matches(' ').len();
		text.truncate(row_end);
		if text.len() > row_start {
			text.push(' ');
		}
	}
	Ok(sanitize_status(text.trim_end()))
}

/// Retained, observer-local extension status values.
///
/// Keys never enter presentation. Values are exposed in lexical key order,
/// already safe for a single-line status segment. Configuration gates only
/// visibility: hiding the segment does not discard updates, so enabling it
/// reveals the current producer state without waiting for another event.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExtensionStatuses {
	by_key: BTreeMap<Str, Str>,
	values: Vec<Str>,
}

impl ExtensionStatuses {
	/// Applies one update and reports whether the retained visible values
	/// changed. A `Set` whose sanitized value is empty behaves as a clear.
	pub fn apply(&mut self, event: ExtensionStatusEvent) -> bool {
		let changed = match event {
			ExtensionStatusEvent::Set { key, text } => {
				let text = sanitize_status(&text);
				if text.is_empty() {
					self.by_key.remove(&key).is_some()
				} else if self.by_key.get(&key) == Some(&text) {
					false
				} else {
					self.by_key.insert(key, text);
					true
				}
			},
			ExtensionStatusEvent::Clear { key } => self.by_key.remove(&key).is_some(),
			ExtensionStatusEvent::Reset => {
				if self.by_key.is_empty() {
					false
				} else {
					self.by_key.clear();
					true
				}
			},
		};
		if changed {
			self.values.clear();
			self.values.extend(self.by_key.values().cloned());
		}
		changed
	}

	/// Key-sorted, sanitized values when the curated status setting is on.
	/// Returns an empty slice while it is off without discarding retained
	/// producer state.
	#[must_use]
	pub fn visible(&self, configured: bool) -> &[Str] {
		if configured { &self.values } else { &[] }
	}

	/// Whether no producer currently contributes a visible value.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.values.is_empty()
	}
}

/// Strips ANSI/VT sequences, maps C0/C1 controls to spaces, collapses ASCII
/// spaces, and trims. This is pi's `sanitizeStatusText` contract at the actor
/// boundary rather than in a paint path.
fn sanitize_status(value: &str) -> Str {
	let stripped = value.to_owned().into_ansi_stripped();
	let mut clean = String::with_capacity(stripped.len());
	let mut ascii_space = true;
	for ch in stripped.chars() {
		let ch = if ch.is_control() { ' ' } else { ch };
		if ch == ' ' {
			if !ascii_space {
				clean.push(' ');
			}
			ascii_space = true;
		} else {
			clean.push(ch);
			ascii_space = false;
		}
	}
	if clean.ends_with(' ') {
		clean.pop();
	}
	Str::new(clean)
}

#[cfg(test)]
mod tests {
	use omp_core::Str;

	use super::{ExtensionStatusEvent, ExtensionStatuses, status_text_from_tml};

	fn set(key: &str, text: &str) -> ExtensionStatusEvent {
		ExtensionStatusEvent::Set { key: Str::new(key), text: Str::new(text) }
	}

	fn values(statuses: &ExtensionStatuses, configured: bool) -> Vec<&str> {
		statuses
			.visible(configured)
			.iter()
			.map(Str::as_str)
			.collect()
	}

	#[test]
	fn tml_status_uses_extension_parser_and_flattens_semantic_content() {
		let text =
			status_text_from_tml("<row><text fg=error>failed</text><text>\\n  safely</text></row>")
				.expect("valid extension TML");
		assert_eq!(text, "failed safely");
		assert!(
			status_text_from_tml("<md><button id=unsafe when=active>interactive</button></md>",)
				.is_err(),
		);
	}

	#[test]
	fn updates_are_key_sorted_and_idempotent() {
		let mut statuses = ExtensionStatuses::default();
		assert!(statuses.apply(set("zeta", "last")));
		assert!(statuses.apply(set("alpha", "first")));
		assert_eq!(values(&statuses, true), ["first", "last"]);
		assert!(!statuses.apply(set("alpha", "first")));
		assert!(statuses.apply(set("alpha", "updated")));
		assert_eq!(values(&statuses, true), ["updated", "last"]);
	}

	#[test]
	fn values_are_sanitized_before_retention() {
		let mut statuses = ExtensionStatuses::default();
		assert!(statuses.apply(set("hook", "  \u{1b}[31mred\u{1b}[0m\n\u{7f} ready   now  ",)));
		assert_eq!(values(&statuses, true), ["red ready now"]);
		assert!(statuses.apply(set("hook", "\u{1b}[2J")));
		assert!(statuses.is_empty());
	}

	#[test]
	fn configuration_hides_without_discarding_updates() {
		let mut statuses = ExtensionStatuses::default();
		statuses.apply(set("hook", "ready"));
		assert!(values(&statuses, false).is_empty());
		assert_eq!(values(&statuses, true), ["ready"]);
	}

	#[test]
	fn clear_and_session_reset_remove_stale_values() {
		let mut statuses = ExtensionStatuses::default();
		statuses.apply(set("alpha", "one"));
		statuses.apply(set("beta", "two"));
		assert!(statuses.apply(ExtensionStatusEvent::Clear { key: Str::new("alpha") }));
		assert_eq!(values(&statuses, true), ["two"]);
		assert!(!statuses.apply(ExtensionStatusEvent::Clear { key: Str::new("missing") }));
		assert!(statuses.apply(ExtensionStatusEvent::Reset));
		assert!(statuses.is_empty());
		assert!(!statuses.apply(ExtensionStatusEvent::Reset));
	}
}

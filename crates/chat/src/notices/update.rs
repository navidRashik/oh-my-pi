//! Observer-local startup update availability card.
//!
//! Release metadata is validated and reduced to these two scalar labels by
//! the application before it enters the actor. The card never parses either
//! value as markup and never offers an install action.

use omp_core::{Str, sf};
use omp_tui::{IntoComponent as _, dom};

use crate::cards::Component;

/// A newer version on the archived release channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateAvailable {
	/// Validated semantic version from the official manifest.
	version: Str,
	/// Closed channel label (`stable` or `canary`).
	channel: Str,
}

impl UpdateAvailable {
	/// Creates a presentation-only availability fact after independently
	/// checking the two labels are safe and closed.
	#[must_use]
	pub fn new(version: impl Into<Str>, channel: &str) -> Option<Self> {
		let version = version.into();
		if version.is_empty()
			|| version.len() > 128
			|| !version
				.bytes()
				.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
			|| !matches!(channel, "stable" | "canary")
		{
			return None;
		}
		Some(Self { version, channel: Str::new(channel) })
	}

	/// Plain semantic transcript projection.
	#[must_use]
	pub fn text(&self) -> Str {
		sf!(
			"Update Available\nNew version {} is available on the {} channel. Run: omp update",
			self.version,
			self.channel
		)
	}
}

/// Warning-framed typed component matching pi's startup notification.
#[must_use]
pub fn card(update: &UpdateAvailable) -> Component {
	let version = update.version.clone();
	let channel = update.channel.clone();
	dom! {
		<box border=round bc=warn bg=surface pad="0 1" title_pad=3>
			<row kind=title gap=1>
				<i:warning fg=warn/>
				<text fg=warn bold>{"Update Available"}</text>
			</row>
			<row gap=0 wrap=word>
				<text fg=muted>{"New version "}</text>
				<text fg=accent>{version}</text>
				<text fg=muted>{" is available on the "}</text>
				<text fg=accent>{channel}</text>
				<text fg=muted>{" channel. Run: "}</text>
				<text fg=accent>{"omp update"}</text>
			</row>
		</box>
	}
	.into_component()
}

#[cfg(test)]
mod tests {
	use omp_tui::{Ui, UiContext, frame_text};

	use super::*;

	#[test]
	fn update_card_is_typed_text_and_never_an_install_action() {
		let update = UpdateAvailable::new("19.2.0", "canary").expect("valid notice");
		assert_eq!(
			update.text(),
			"Update Available\nNew version 19.2.0 is available on the canary channel. Run: omp update"
		);
		let rendered =
			frame_text(Ui::from_root(card(&update), 80, UiContext::default()).frame());
		assert!(rendered.contains("Update Available"));
		assert!(rendered.contains("omp update"));
		assert!(!rendered.contains("installing"));
		assert!(UpdateAvailable::new("19.2.0\nforged", "stable").is_none());
		assert!(UpdateAvailable::new("19.2.0", "attacker").is_none());
	}
}

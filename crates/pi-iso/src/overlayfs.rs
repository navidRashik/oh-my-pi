//! Deprecated `OverlayFS` isolation backend.
//!
//! `OverlayFS` requires lower layers to remain unchanged while mounted. Task
//! isolation passes the live parent repository as the source tree, so copy-up
//! can observe parent writes and produce torn child files. The backend remains
//! in the public enum for settings/API compatibility, but it is no longer
//! selectable.

use std::path::Path;

use async_trait::async_trait;

use crate::{BackendKind, IsoError, IsoResult, IsolationBackend, ProbeResult};

pub struct OverlayfsBackend;

pub fn backend() -> &'static dyn IsolationBackend {
	&OverlayfsBackend
}

const OVERLAYFS_DISABLED_REASON: &str =
	"overlayfs isolation is disabled because live lower directories are not safe task snapshots";

#[async_trait]
impl IsolationBackend for OverlayfsBackend {
	fn kind(&self) -> BackendKind {
		BackendKind::Overlayfs
	}

	fn probe(&self) -> ProbeResult {
		ProbeResult::unavailable(OVERLAYFS_DISABLED_REASON)
	}

	fn start(&self, lower: &Path, merged: &Path) -> IsoResult<()> {
		let _ = (lower, merged);
		Err(IsoError::unavailable(OVERLAYFS_DISABLED_REASON))
	}

	fn stop(&self, merged: &Path) -> IsoResult<()> {
		let _ = merged;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use super::backend;

	#[test]
	fn overlayfs_probe_reports_unavailable() {
		let probe = backend().probe();

		assert!(!probe.available);
	}

	#[test]
	fn overlayfs_start_returns_unavailable_without_touching_paths() {
		let err = backend()
			.start(Path::new("unused-lower"), Path::new("unused-merged"))
			.expect_err("overlayfs start should be unavailable");

		assert!(err.is_unavailable());
	}
}

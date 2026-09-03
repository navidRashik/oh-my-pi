//! Journal pruning plus journal-rooted project/session artifact collection.

use std::{
	fs, io,
	path::{Path, PathBuf},
	time::SystemTime,
};

use miette::IntoDiagnostic as _;
use omp_journal::{
	Journal, abandoned,
	blob::{BlobStore, DEFAULT_GC_GRACE, GcPolicy},
	gc::{collect_blobs, prune_abandoned},
};
use serde_json::json;

use crate::cli::GcArgs;

/// Scans native `.oms` journals and optionally prunes abandoned branches,
/// unreferenced blobs, orphan local trees, and stale staging content.
pub fn run(args: GcArgs) -> miette::Result<()> {
	let data_dir = omp_core::dirs::data_dir(args.data_dir).into_diagnostic()?;
	let roots = args
		.sessions_dir
		.map_or_else(|| project_session_roots(&data_dir), |directory| Ok(vec![directory]))
		.into_diagnostic()?;
	let mut project_paths = Vec::with_capacity(roots.len());
	for sessions in roots {
		let mut paths = Vec::new();
		collect_journals(&sessions, &mut paths).into_diagnostic()?;
		paths.sort();
		project_paths.push((sessions, paths));
	}

	let mut journals = 0usize;
	let mut entries_pruned = 0usize;
	let mut bytes_reclaimed = 0u64;
	for path in project_paths.iter().flat_map(|(_, paths)| paths) {
		let entries = Journal::scan(path).into_diagnostic()?;
		let abandoned_count = abandoned(&entries).count();
		if abandoned_count == 0 {
			continue;
		}
		journals += 1;
		entries_pruned += abandoned_count;
		if args.apply {
			bytes_reclaimed += prune_abandoned(path).into_diagnostic()?.bytes_reclaimed();
		}
	}

	let mut blobs_examined = 0usize;
	let mut blobs_removed = 0usize;
	let mut blob_bytes_reclaimed = 0u64;
	let mut temporaries_removed = 0usize;
	let mut temporary_bytes_reclaimed = 0u64;
	let mut local_session_dirs_removed = 0usize;
	let mut local_temporaries_removed = 0usize;
	let mut local_bytes_reclaimed = 0u64;
	for (sessions, paths) in &project_paths {
		if args.apply {
			let store = BlobStore::open(sessions).into_diagnostic()?;
			let report = collect_blobs(&store, paths, GcPolicy::default()).into_diagnostic()?;
			blobs_examined += report.storage.blobs_examined;
			blobs_removed += report.storage.blobs_removed;
			blob_bytes_reclaimed =
				blob_bytes_reclaimed.saturating_add(report.storage.blob_bytes_reclaimed);
			temporaries_removed += report.storage.temporaries_removed;
			temporary_bytes_reclaimed =
				temporary_bytes_reclaimed.saturating_add(report.storage.temporary_bytes_reclaimed);
		}
		let local = collect_local_artifacts(sessions, args.apply).into_diagnostic()?;
		local_session_dirs_removed += local.session_dirs;
		local_temporaries_removed += local.temporaries;
		local_bytes_reclaimed = local_bytes_reclaimed.saturating_add(local.bytes);
	}

	if args.json {
		println!(
			"{}",
			json!({
				"applied": args.apply,
				"journals": journals,
				"entries_pruned": entries_pruned,
				"journal_bytes_reclaimed": bytes_reclaimed,
				"blobs_examined": blobs_examined,
				"blobs_removed": blobs_removed,
				"blob_bytes_reclaimed": blob_bytes_reclaimed,
				"temporaries_removed": temporaries_removed,
				"temporary_bytes_reclaimed": temporary_bytes_reclaimed,
				"local_session_dirs_removed": local_session_dirs_removed,
				"local_temporaries_removed": local_temporaries_removed,
				"local_bytes_reclaimed": local_bytes_reclaimed,
				"bytes_reclaimed": bytes_reclaimed
					.saturating_add(blob_bytes_reclaimed)
					.saturating_add(temporary_bytes_reclaimed)
					.saturating_add(local_bytes_reclaimed),
			})
		);
	} else if args.apply {
		let total = bytes_reclaimed
			.saturating_add(blob_bytes_reclaimed)
			.saturating_add(temporary_bytes_reclaimed)
			.saturating_add(local_bytes_reclaimed);
		println!(
			"pruned {entries_pruned} abandoned entries from {journals} journals; removed \
			 {blobs_removed} unreferenced blobs, {temporaries_removed} stale CAS temporaries, \
			 {local_session_dirs_removed} orphan local trees, and {local_temporaries_removed} stale \
			 local temporaries; reclaimed {total} bytes"
		);
	} else {
		println!(
			"dry run: {entries_pruned} abandoned entries in {journals} journals, \
			 {local_session_dirs_removed} orphan local trees, and {local_temporaries_removed} stale \
			 local temporaries; pass --apply to prune and collect"
		);
	}
	Ok(())
}

fn project_session_roots(data_dir: &Path) -> io::Result<Vec<PathBuf>> {
	let projects = data_dir.join("projects");
	let entries = match fs::read_dir(&projects) {
		Ok(entries) => entries,
		Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
		Err(error) => return Err(error),
	};
	let mut roots = Vec::new();
	for entry in entries {
		let sessions = entry?.path().join("sessions");
		if sessions.is_dir() {
			roots.push(sessions);
		}
	}
	roots.sort();
	Ok(roots)
}

#[derive(Clone, Copy, Debug, Default)]
struct LocalGcReport {
	session_dirs: usize,
	temporaries:  usize,
	bytes:        u64,
}

fn collect_local_artifacts(directory: &Path, apply: bool) -> io::Result<LocalGcReport> {
	let entries = match fs::read_dir(directory) {
		Ok(entries) => entries,
		Err(error) if error.kind() == io::ErrorKind::NotFound => {
			return Ok(LocalGcReport::default());
		},
		Err(error) => return Err(error),
	};
	let now = SystemTime::now();
	let mut report = LocalGcReport::default();
	for entry in entries {
		let entry = entry?;
		if !entry.file_type()?.is_dir() {
			continue;
		}
		let session_root = entry.path();
		let local = session_root.join("local");
		if !local.is_dir() {
			continue;
		}
		let journal = directory
			.join(entry.file_name())
			.with_extension(omp_journal::FILE_EXTENSION);
		if !journal.is_file() {
			report.session_dirs += 1;
			report.bytes = report.bytes.saturating_add(directory_bytes(&session_root)?);
			if apply {
				fs::remove_dir_all(&session_root)?;
			}
			continue;
		}
		collect_stale_local_temporaries(&local, now, apply, &mut report)?;
	}
	Ok(report)
}

fn collect_stale_local_temporaries(
	directory: &Path,
	now: SystemTime,
	apply: bool,
	report: &mut LocalGcReport,
) -> io::Result<()> {
	for entry in fs::read_dir(directory)? {
		let entry = entry?;
		let file_type = entry.file_type()?;
		if file_type.is_dir() {
			collect_stale_local_temporaries(&entry.path(), now, apply, report)?;
			continue;
		}
		if !file_type.is_file() {
			continue;
		}
		let name = entry.file_name();
		let name = name.to_string_lossy();
		if !name.starts_with('.') || !name.ends_with(".tmp") {
			continue;
		}
		let metadata = entry.metadata()?;
		let old = metadata
			.modified()
			.ok()
			.and_then(|modified| now.duration_since(modified).ok())
			.is_some_and(|age| age >= DEFAULT_GC_GRACE);
		if !old {
			continue;
		}
		report.temporaries += 1;
		report.bytes = report.bytes.saturating_add(metadata.len());
		if apply {
			fs::remove_file(entry.path())?;
		}
	}
	Ok(())
}

fn directory_bytes(directory: &Path) -> io::Result<u64> {
	let mut bytes = 0_u64;
	for entry in fs::read_dir(directory)? {
		let entry = entry?;
		let file_type = entry.file_type()?;
		if !file_type.is_file() && !file_type.is_dir() {
			continue;
		}
		let metadata = entry.metadata()?;
		bytes = bytes.saturating_add(if file_type.is_dir() {
			directory_bytes(&entry.path())?
		} else {
			metadata.len()
		});
	}
	Ok(bytes)
}

fn collect_journals(directory: &Path, output: &mut Vec<PathBuf>) -> io::Result<()> {
	let entries = match fs::read_dir(directory) {
		Ok(entries) => entries,
		Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
		Err(error) => return Err(error),
	};
	for entry in entries {
		let entry = entry?;
		if !entry.file_type()?.is_file() {
			continue;
		}
		let path = entry.path();
		if path.extension().and_then(|value| value.to_str()) == Some(omp_journal::FILE_EXTENSION) {
			output.push(path);
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use tempfile::tempdir;

	use super::*;

	#[test]
	fn defaults_to_every_project_session_root() {
		let scratch = tempdir().expect("scratch");
		let first = scratch.path().join("projects/first/sessions");
		let second = scratch.path().join("projects/second/sessions");
		fs::create_dir_all(&first).expect("first project");
		fs::create_dir_all(&second).expect("second project");
		fs::create_dir_all(scratch.path().join("projects/third/cache")).expect("unrelated state");

		let roots = project_session_roots(scratch.path()).expect("project roots");
		assert_eq!(roots, vec![first.clone(), second.clone()]);

		let first_journal = first.join("a.oms");
		let second_journal = second.join("b.oms");
		fs::write(&first_journal, "").expect("first journal");
		fs::write(&second_journal, "").expect("second journal");
		fs::create_dir_all(first.join("a/local")).expect("local tree");
		fs::write(first.join("a/local/example.oms"), "not a journal").expect("local artifact");
		let mut journals = Vec::new();
		for root in roots {
			collect_journals(&root, &mut journals).expect("collect");
		}
		journals.sort();
		assert_eq!(journals, vec![first_journal, second_journal]);
	}

	#[test]
	fn local_artifacts_follow_their_session_journal_lifetime() {
		let scratch = tempdir().expect("scratch");
		let sessions = scratch.path().join("sessions");
		let retained = sessions.join("retained");
		let orphan = sessions.join("deleted");
		fs::create_dir_all(retained.join("local")).expect("retained local");
		fs::create_dir_all(orphan.join("local")).expect("orphan local");
		fs::write(sessions.join("retained.oms"), b"journal").expect("retained journal");
		fs::write(retained.join("local/paste.md"), b"keep").expect("retained artifact");
		fs::write(orphan.join("local/paste.md"), b"remove").expect("orphan artifact");

		let dry_run = collect_local_artifacts(&sessions, false).expect("dry run");
		assert_eq!(dry_run.session_dirs, 1);
		assert!(orphan.exists(), "dry run must not mutate");

		let applied = collect_local_artifacts(&sessions, true).expect("apply");
		assert_eq!(applied.session_dirs, 1);
		assert!(!orphan.exists(), "deleted session local tree is reclaimed");
		assert!(retained.join("local/paste.md").is_file(), "live session local data survives");
	}
}

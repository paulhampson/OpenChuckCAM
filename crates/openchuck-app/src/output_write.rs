// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reviewed destinations and actual per-file export results (GC-05/12/15,
//! UI-12, NFR-07/12). A batch stops at its first failure; it is not atomic.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::{AppError, Regeneration};

/// A read-only preflight. Dropping it (Cancel) has no filesystem effects.
#[derive(Debug)]
pub struct OutputPlan {
    pub(crate) run: Arc<Regeneration>,
    paths: Vec<PathBuf>,
    contents: Vec<Vec<u8>>,
    existing: Vec<PathBuf>,
}

impl OutputPlan {
    /// Every full destination, in write order, including both setup sheets.
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
    /// Existing destinations that require explicit replacement approval.
    pub fn existing(&self) -> &[PathBuf] {
        &self.existing
    }
}

/// A destination that could not be written, with its actual failure reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFailure {
    /// Full destination whose write or preflight failed.
    pub path: PathBuf,
    /// The I/O or readiness reason, without a success claim.
    pub reason: String,
}

/// Completed writes, one failure, and the files not attempted after it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputReport {
    /// Destinations whose writes actually completed.
    pub written: Vec<PathBuf>,
    /// The first failed destination, if any.
    pub failed: Option<OutputFailure>,
    /// Destinations not attempted after a refusal or write failure.
    pub not_attempted: Vec<PathBuf>,
}

impl OutputReport {
    /// Whether every planned write succeeded.
    pub fn is_complete(&self) -> bool {
        !self.written.is_empty() && self.failed.is_none() && self.not_attempted.is_empty()
    }

    /// A count of completed writes, marked incomplete when a file failed.
    pub fn summary(&self) -> String {
        let n = self.written.len();
        format!(
            "Wrote {n} file{}{}",
            if n == 1 { "" } else { "s" },
            if self.failed.is_some() {
                "; export incomplete."
            } else {
                "."
            }
        )
    }

    /// The retained Messages text, also suitable for copying as a report.
    pub fn text(&self) -> String {
        let mut text = format!("Export: {}", self.summary());
        for path in &self.written {
            text.push_str(&format!("\nWritten: {}", path.display()));
        }
        if let Some(failure) = &self.failed {
            text.push_str(&format!(
                "\nFailed: {} — {}",
                failure.path.display(),
                failure.reason
            ));
        }
        for path in &self.not_attempted {
            text.push_str(&format!("\nNot attempted: {}", path.display()));
        }
        // A closing sentence, so Messages' sentence ending is never added to
        // the last path, which must stay a real file name (issue 331).
        text.push_str("\nEnd of the export result.");
        text
    }

    /// A refusal before any write, such as failed preflight or stale inputs.
    pub fn refused(paths: &[PathBuf], path: PathBuf, reason: impl Into<String>) -> Self {
        Self {
            written: Vec::new(),
            failed: Some(OutputFailure {
                path: path.clone(),
                reason: reason.into(),
            }),
            not_attempted: paths.iter().filter(|p| **p != path).cloned().collect(),
        }
    }
}

fn io_error(path: &Path, source: io::Error) -> AppError {
    AppError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn exists(path: &Path) -> io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            Err(io::Error::other("the destination is a directory"))
        }
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

pub(crate) fn prepare(
    folder: &Path,
    run: Arc<Regeneration>,
    files: Vec<(String, Vec<u8>)>,
) -> Result<OutputPlan, AppError> {
    let folder = std::path::absolute(folder).map_err(|e| io_error(folder, e))?;
    let mut paths = Vec::new();
    let mut contents = Vec::new();
    let mut existing = Vec::new();
    let mut unique = BTreeSet::new();
    for (name, bytes) in files {
        let path = folder.join(&name);
        if !matches!(
            Path::new(&name).components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        ) || !unique.insert(path.clone())
        {
            return Err(io_error(
                &path,
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "output filenames must be distinct single filenames",
                ),
            ));
        }
        if exists(&path).map_err(|e| io_error(&path, e))? {
            existing.push(path.clone());
        }
        paths.push(path);
        contents.push(bytes);
    }
    Ok(OutputPlan {
        run,
        paths,
        contents,
        existing,
    })
}

/// Every file is first written completely under a uniquely claimed staging
/// name in the destination folder, then published under its final name
/// (issue 332), so stopping part way leaves a hidden staging file, never a
/// final name holding part of a program. A new destination is published
/// without replacing anything: a file that appears there after preflight is
/// kept and the write fails. An approved existing destination is replaced
/// by renaming the staged file over it, never by deleting it first. Staged
/// bytes are flushed before publishing; this is best effort, not a promise
/// of durability after a power cut, and the batch stays non-atomic.
pub(crate) fn write_one(path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    write_one_with(path, bytes, replace, |from, to| {
        std::fs::hard_link(from, to)
    })
}

/// [`write_one`] with the no-clobber publication step given, so tests can
/// stand in for a file system without hard links.
fn write_one_with(
    path: &Path,
    bytes: &[u8],
    replace: bool,
    link: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("output path has no parent"))?;
    let staged = stage(parent, bytes)?;
    let published = if replace {
        std::fs::rename(&staged, path)
    } else {
        publish_new(&staged, path, link)
    };
    match published {
        Ok(()) => match std::fs::remove_file(&staged) {
            // A rename leaves nothing to remove; a hard link leaves the
            // staging name beside the published one.
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Ok(()) => Ok(()),
            Err(cleanup) => Err(io::Error::new(
                cleanup.kind(),
                format!(
                    "the file was written completely, but its staging file '{}' could not be removed: {cleanup}",
                    staged.display()
                ),
            )),
        },
        Err(error) => match std::fs::remove_file(&staged) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(io::Error::new(
                error.kind(),
                format!(
                    "{error}; staging file '{}' could not be removed: {cleanup}",
                    staged.display()
                ),
            )),
        },
    }
}

/// Writes `bytes` completely to a new, uniquely claimed hidden file in
/// `parent` and returns its path. A failed write removes it.
fn stage(parent: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    for _ in 0..32 {
        let staged = parent.join(format!(
            ".openchuck-export-{}-{}.part",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)
        {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        let result = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        return match result {
            Ok(()) => Ok(staged),
            Err(error) => match std::fs::remove_file(&staged) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(io::Error::new(
                    error.kind(),
                    format!(
                        "{error}; staging file '{}' could not be removed: {cleanup}",
                        staged.display()
                    ),
                )),
            },
        };
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not claim a staging file for the output",
    ))
}

/// Publishes the complete `staged` file at the new destination `path`
/// without replacing anything there. A hard link does this in one step. A
/// file system without hard links (FAT and exFAT memory cards, some network
/// shares) instead claims the destination exclusively as an empty file and
/// renames the staged file over that claim, so an interruption can leave an
/// empty file but never part of a program.
fn publish_new(
    staged: &Path,
    path: &Path,
    link: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let appeared = || {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "a file appeared at the destination after the review; it was kept and not replaced",
        )
    };
    match link(staged, path) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(appeared()),
        Err(_) => {}
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(claim) => drop(claim),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(appeared()),
        Err(e) => return Err(e),
    }
    std::fs::rename(staged, path).map_err(|error| match std::fs::remove_file(path) {
        Ok(()) => error,
        Err(cleanup) => io::Error::new(
            error.kind(),
            format!("{error}; the empty claimed file could not be removed: {cleanup}"),
        ),
    })
}

pub(crate) fn write(plan: &OutputPlan, replace: bool) -> OutputReport {
    write_with(plan, replace, write_one)
}

fn write_with(
    plan: &OutputPlan,
    replace: bool,
    mut writer: impl FnMut(&Path, &[u8], bool) -> io::Result<()>,
) -> OutputReport {
    // Complete preflight again before creating even the output directory.
    for path in &plan.paths {
        match exists(path) {
            Ok(now) if now != plan.existing.contains(path) => {
                return OutputReport::refused(
                    &plan.paths,
                    path.clone(),
                    "The destination changed after preflight. Review the export again.",
                );
            }
            Err(e) => return OutputReport::refused(&plan.paths, path.clone(), e.to_string()),
            _ => {}
        }
    }
    if !replace && let Some(path) = plan.existing.first() {
        return OutputReport::refused(
            &plan.paths,
            path.clone(),
            "Replacement has not been confirmed.",
        );
    }
    if let Some(first) = plan.paths.first()
        && let Some(parent) = first.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return OutputReport::refused(&plan.paths, first.clone(), e.to_string());
    }
    let mut report = OutputReport::default();
    for (i, (path, bytes)) in plan.paths.iter().zip(&plan.contents).enumerate() {
        match writer(path, bytes, plan.existing.contains(path)) {
            Ok(()) => report.written.push(path.clone()),
            Err(e) => {
                report.failed = Some(OutputFailure {
                    path: path.clone(),
                    reason: e.to_string(),
                });
                report
                    .not_attempted
                    .extend(plan.paths.iter().skip(i + 1).cloned());
                break;
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn req_gc_05_req_nfr_07_partial_write_reports_actual_files_and_stops() {
        let dir = std::env::temp_dir().join(format!("openchuck-partial-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let plan = prepare(
            &dir,
            Arc::new(Regeneration::default()),
            vec![
                ("one.nc".into(), b"one".to_vec()),
                ("two.nc".into(), b"two".to_vec()),
                ("three.nc".into(), b"three".to_vec()),
            ],
        )
        .unwrap();
        let report = write_with(&plan, false, |path, bytes, replace| {
            if path.ends_with("two.nc") {
                std::fs::create_dir(path)?;
            }
            write_one(path, bytes, replace)
        });
        assert_eq!(report.written, [dir.join("one.nc")]);
        assert_eq!(report.failed.as_ref().unwrap().path, dir.join("two.nc"));
        assert!(!report.failed.as_ref().unwrap().reason.is_empty());
        assert_eq!(report.not_attempted, [dir.join("three.nc")]);
        assert_eq!(std::fs::read(dir.join("one.nc")).unwrap(), b"one");
        assert!(!dir.join("three.nc").exists());
        assert!(!report.is_complete());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn req_nfr_07_failed_replacement_preserves_destination_and_removes_temporary_file() {
        let dir =
            std::env::temp_dir().join(format!("openchuck-replace-fail-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let destination = dir.join("target.nc");
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(destination.join("keep"), b"keep").unwrap();
        assert!(write_one(&destination, b"replacement", true).is_err());
        assert_eq!(std::fs::read(destination.join("keep")).unwrap(), b"keep");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn scratch(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openchuck-stage-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Issue 332: the final name appears only once the staged file holds
    /// every byte, so stopping during the write cannot leave part of a
    /// program under the final name.
    #[test]
    fn req_nfr_07_req_gc_05_a_new_file_is_complete_before_its_final_name_appears() {
        let dir = scratch("complete");
        let path = dir.join("01-part.nc");
        let bytes = b"G0 X0\nM30\n".repeat(1000);
        write_one_with(&path, &bytes, false, |staged, to| {
            assert!(!to.exists(), "the final name is not used while staging");
            assert_eq!(staged.parent(), to.parent(), "staged in the same folder");
            assert_eq!(std::fs::read(staged).unwrap(), bytes);
            std::fs::hard_link(staged, to)
        })
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(names(&dir), ["01-part.nc"], "no staging file is left");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// The accepted no-overwrite rule (decision 0240 item 4) holds at the
    /// moment of publication, with and without hard links.
    #[test]
    fn req_nfr_07_a_file_appearing_before_publication_is_kept() {
        for hard_links in [true, false] {
            let dir = scratch(&format!("appeared-{hard_links}"));
            let path = dir.join("01-part.nc");
            let error = write_one_with(&path, b"new", false, |staged, to| {
                std::fs::write(to, b"someone else's").unwrap();
                if hard_links {
                    std::fs::hard_link(staged, to)
                } else {
                    Err(io::Error::new(io::ErrorKind::Unsupported, "no hard links"))
                }
            })
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
            assert!(
                error.to_string().contains("kept and not replaced"),
                "{error}"
            );
            assert_eq!(std::fs::read(&path).unwrap(), b"someone else's");
            assert_eq!(names(&dir), ["01-part.nc"], "the staging file is removed");
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    /// A memory card formatted FAT or exFAT has no hard links: the file is
    /// still staged and published complete under its name.
    #[test]
    fn req_nfr_07_a_file_system_without_hard_links_still_publishes_complete_files() {
        let dir = scratch("no-links");
        let path = dir.join("01-part.nc");
        write_one_with(&path, b"G0 X0", false, |_, to| {
            assert!(!to.exists());
            Err(io::Error::new(io::ErrorKind::Unsupported, "no hard links"))
        })
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"G0 X0");
        assert_eq!(names(&dir), ["01-part.nc"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// A staging failure that the program sees is reported and leaves
    /// nothing under the final name.
    #[test]
    fn req_nfr_07_a_failed_staging_write_leaves_no_final_file() {
        let dir = scratch("missing");
        let path = dir.join("absent").join("01-part.nc");
        assert!(write_one(&path, b"G0", false).is_err());
        assert!(!path.exists());
        assert!(names(&dir).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Approved replacement keeps using a staged rename over the old file.
    #[test]
    fn req_nfr_07_an_approved_replacement_is_staged_and_renamed() {
        let dir = scratch("replace");
        let path = dir.join("01-part.nc");
        std::fs::write(&path, b"old").unwrap();
        write_one(&path, b"new", true).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert_eq!(names(&dir), ["01-part.nc"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Issue 331: the report's last line is a sentence, not a path.
    #[test]
    fn req_ui_12_the_report_text_ends_with_a_sentence_after_the_paths() {
        let report = OutputReport {
            written: vec![PathBuf::from("/out/a.nc")],
            failed: None,
            not_attempted: Vec::new(),
        };
        assert_eq!(
            report.text(),
            "Export: Wrote 1 file.\nWritten: /out/a.nc\nEnd of the export result."
        );
    }
}

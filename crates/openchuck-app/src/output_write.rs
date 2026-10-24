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

/// New files use exclusive creation, so a destination appearing after
/// preflight is never silently replaced. Existing files are prepared at a
/// uniquely claimed temporary path before rename; no delete-before-replace.
fn write_one(path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    if !replace {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        let result = file.write_all(bytes);
        drop(file);
        return match result {
            Ok(()) => Ok(()),
            Err(error) => match std::fs::remove_file(path) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(io::Error::new(
                    error.kind(),
                    format!("{error}; the incomplete file could not be removed: {cleanup}"),
                )),
            },
        };
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("output path has no parent"))?;
    for _ in 0..32 {
        let temporary = parent.join(format!(
            ".openchuck-export-{}-{}.part",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        let result = file.write_all(bytes);
        drop(file);
        let result = result.and_then(|()| std::fs::rename(&temporary, path));
        if let Err(error) = result {
            return match std::fs::remove_file(&temporary) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(io::Error::new(
                    error.kind(),
                    format!(
                        "{error}; temporary file '{}' could not be removed: {cleanup}",
                        temporary.display()
                    ),
                )),
            };
        }
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not claim a temporary output file",
    ))
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
}

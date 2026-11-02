// SPDX-License-Identifier: AGPL-3.0-or-later
//! The STEP importer (IMP-02, IMP-03, IMP-06; work breakdown slice 11; decision
//! records 0003 and 0017).
//!
//! The OpenCASCADE reader is not safe against damaged files: it dereferences
//! null handles and segfaults (SP-1 measured 155 crashes in 450 mutated
//! files). NFR-07 forbids a crash, so this importer never calls the kernel
//! itself. [`StepImporter`] writes the file into a private directory, starts
//! the `openchuck-step-worker` program on it with a time limit, and reads the
//! worker's answer back. If the worker crashes, hangs or is missing, the user
//! gets an [`ImportError::Worker`] and the application carries on.
//!
//! The worker (built with the `kernel` feature, see [`crate::kernel`]) reads
//! the file, numbers faces, edges and vertices in the kernel's traversal
//! order so the same file always gives the same IDs (architecture section
//! 5.4), converts every face to the plain data of
//! [`openchuck_types::brep`], tessellates at the requested tolerance, and
//! writes one JSON document. This module checks that document
//! ([`BrepModel::validate`]) before anything else sees it.
//!
//! # Units (IMP-03)
//!
//! A STEP file states its length unit, and the kernel converts to millimetres
//! while reading. The importer reports the file's unit in
//! [`Units::file`](crate::Units) and applies it ([`UnitSource::File`]).
//! [`ImportOptions::units`] overrides it: the file's numbers are then read as
//! if they were in the user's unit, which the worker does by scaling the
//! model before tessellating, so the tessellation tolerance stays in true
//! millimetres.
//!
//! # Finding the worker
//!
//! In order: [`StepImporter::with_worker`], the `OPENCHUCK_STEP_WORKER`
//! environment variable, then `openchuck-step-worker` (`.exe` on Windows)
//! next to the running program or one directory up from it (which is where
//! `cargo test` runs from). Build it with
//! `cargo build -p openchuck-import --features kernel --bin openchuck-step-worker`.

use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use openchuck_types::brep::{BrepModel, TessellationSettings};
use openchuck_types::job::JobContext;
use openchuck_types::model::{ModelFormat, ModelGeometry};
use openchuck_types::units::LengthUnit;
use serde::Deserialize;

use crate::{ImportError, ImportOptions, ImportedModel, ModelImporter, UnitSource, Units};

/// The worker program's file name without any extension.
pub const WORKER_NAME: &str = "openchuck-step-worker";

/// The environment variable that names the worker program.
pub const WORKER_ENV: &str = "OPENCHUCK_STEP_WORKER";

/// How long the worker may take before it is stopped. The SP-1 file of 300
/// solids and 7,800 faces took 5 s to read and 21 s to read and tessellate
/// twice.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// The largest answer the importer reads from the worker, in bytes.
const MAX_ANSWER: u64 = 1 << 30;

/// The importer [`crate::importer_for`] returns for STEP.
pub static STEP_IMPORTER: StepImporter = StepImporter::new();

/// What every STEP import tells the user (IMP-02, IMP-06). Most strategies
/// still drop the tool on the tessellation (OI-81), so the notice says so
/// rather than that every toolpath uses the exact faces (NFR-12; decision
/// 0209 item 4.4, OI-830).
pub fn step_tessellation_notice(settings: TessellationSettings) -> String {
    format!(
        "STEP keeps its exact faces and edges, which radial drilling, swarf and edge following \
read; pencil clean-up finds its inside corners on the exact edges. The other strategies, and \
pencil clean-up, drop the tool on the tessellation ({} mm, {} rad) for now, and each \
operation's notes say so; the display and the mesh checks use it too.",
        settings.linear, settings.angular
    )
}

/// The STEP importer: runs the worker program in a child process.
#[derive(Debug, Clone)]
pub struct StepImporter {
    worker: Option<PathBuf>,
    timeout: Duration,
}

impl StepImporter {
    /// An importer that finds the worker as described in the module
    /// documentation and allows it [`DEFAULT_TIMEOUT`].
    pub const fn new() -> Self {
        Self {
            worker: None,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Uses the worker program at `path`.
    #[must_use]
    pub fn with_worker(mut self, path: impl Into<PathBuf>) -> Self {
        self.worker = Some(path.into());
        self
    }

    /// Stops the worker after `timeout`.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The worker program this importer would run, if it can be found.
    pub fn locate_worker(&self) -> Option<PathBuf> {
        if let Some(path) = &self.worker {
            return Some(path.clone());
        }
        if let Some(path) = std::env::var_os(WORKER_ENV).filter(|p| !p.is_empty()) {
            return Some(PathBuf::from(path));
        }
        let exe = std::env::current_exe().ok()?;
        let file = format!("{WORKER_NAME}{}", std::env::consts::EXE_SUFFIX);
        let dir = exe.parent()?;
        [Some(dir), dir.parent()]
            .into_iter()
            .flatten()
            .map(|d| d.join(&file))
            .find(|p| p.is_file())
    }
}

impl Default for StepImporter {
    fn default() -> Self {
        Self::new()
    }
}

/// The answer the worker writes.
#[derive(Debug, Deserialize)]
struct Answer {
    /// Set when the worker failed.
    error: Option<String>,
    /// The length unit the file names, as the kernel spells it.
    file_unit: Option<String>,
    /// Millimetres per file unit; 0 when the kernel's name is not one it knows.
    file_unit_mm: Option<f64>,
    /// The scale the worker applied for a user's unit override.
    scale: Option<f64>,
    /// The model, in millimetres.
    model: Option<BrepModel>,
}

/// A private directory for one import, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, ImportError> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let name = format!(
            "openchuck-step-{}-{nanos}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        // `create_dir` fails if the name exists, so the directory is ours.
        std::fs::create_dir(&path)
            .map_err(|e| ImportError::Worker(format!("cannot create a scratch directory: {e}")))?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl ModelImporter for StepImporter {
    fn format(&self) -> ModelFormat {
        ModelFormat::Step
    }

    fn import(
        &self,
        bytes: &[u8],
        options: &ImportOptions,
        context: &JobContext<'_>,
    ) -> Result<ImportedModel, ImportError> {
        context
            .check_cancelled()
            .map_err(|_| ImportError::Cancelled)?;
        let settings = options.tessellation.unwrap_or_default();
        check_settings(settings)?;
        let worker = self.locate_worker().ok_or_else(|| {
            ImportError::Worker(format!(
                "this build cannot read STEP files: the STEP reader program '{WORKER_NAME}' was not \
found next to the application (or set {WORKER_ENV})"
            ))
        })?;
        let override_mm = options.units.map_or(0.0, LengthUnit::millimetres);
        let scratch = Scratch::new()?;
        std::fs::write(scratch.0.join("in.step"), bytes)
            .map_err(|e| ImportError::Worker(format!("cannot write the scratch file: {e}")))?;
        context.report("read STEP", 0, None);
        run_worker(
            &worker,
            &scratch.0,
            settings,
            override_mm,
            self.timeout,
            context,
        )?;
        context.report("check STEP", 0, None);
        let answer = read_answer(&scratch.0.join("out.json"))?;
        convert(answer, options, settings)
    }
}

fn check_settings(settings: TessellationSettings) -> Result<(), ImportError> {
    let ok = |v: f64| v.is_finite() && v > 0.0;
    if ok(settings.linear) && ok(settings.angular) {
        Ok(())
    } else {
        Err(ImportError::Invalid {
            format: "STEP",
            reason: "the tessellation tolerances must be positive numbers".to_owned(),
        })
    }
}

/// Runs the worker on `dir/in.step` and waits for it, watching the clock and
/// the cancel flag. The file names are relative to `dir` so a scratch path
/// with non-ASCII characters never reaches the kernel.
fn run_worker(
    worker: &Path,
    dir: &Path,
    settings: TessellationSettings,
    override_mm: f64,
    timeout: Duration,
    context: &JobContext<'_>,
) -> Result<(), ImportError> {
    let args: [OsString; 5] = [
        "in.step".into(),
        "out.json".into(),
        settings.linear.to_string().into(),
        settings.angular.to_string().into(),
        override_mm.to_string().into(),
    ];
    // The worker's own messages go to a file beside its answer, never to a
    // console: a GUI program has none, and a visible one is a defect (NFR-17).
    let stderr =
        std::fs::File::create(dir.join(STDERR_FILE)).map_or_else(|_| Stdio::null(), Stdio::from);
    let mut command = Command::new(worker);
    command
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr);
    hide_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| ImportError::Worker(format!("cannot start '{}': {e}", worker.display())))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                // A worker that reports its own error exits non-zero after
                // writing it; a crash writes nothing.
                if dir.join("out.json").is_file() {
                    return Ok(());
                }
                return Err(ImportError::Worker(format!(
                    "the STEP reader stopped without an answer ({}); the file is damaged or uses \
something the reader cannot handle{}",
                    describe_status(&status),
                    worker_messages(dir)
                )));
            }
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                return Err(ImportError::Worker(format!(
                    "cannot wait for the STEP reader: {e}"
                )));
            }
        }
        if context.check_cancelled().is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ImportError::Cancelled);
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ImportError::Worker(format!(
                "the STEP reader did not finish within {} s and was stopped",
                timeout.as_secs_f64()
            )));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// How many bytes of the end of the worker's standard error go into an error
/// message.
const WORKER_TAIL: u64 = 2048;

/// The file the worker's standard error goes to, in its scratch folder.
const STDERR_FILE: &str = "worker.log";

/// The Windows process creation flag that starts a program without a console
/// window (`CREATE_NO_WINDOW`).
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The process creation flags every helper program is started with: hidden on
/// Windows (NFR-17), none elsewhere.
const fn helper_creation_flags() -> u32 {
    if cfg!(windows) { CREATE_NO_WINDOW } else { 0 }
}

/// Starts `command` without a visible window on Windows; does nothing
/// elsewhere.
fn hide_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(helper_creation_flags());
    }
    #[cfg(not(windows))]
    {
        let _ = (command, helper_creation_flags());
    }
}

/// The last part of what the worker wrote to its standard error, as a
/// sentence to add to an error message (empty if it wrote nothing). The UI
/// writes the message to the log file.
fn worker_messages(dir: &Path) -> String {
    let Ok(file) = std::fs::File::open(dir.join(STDERR_FILE)) else {
        return String::new();
    };
    // The end of the file: the last thing the worker said is the reason.
    let mut file = file;
    let length = file.metadata().map_or(0, |m| m.len());
    if file
        .seek(SeekFrom::Start(length.saturating_sub(WORKER_TAIL)))
        .is_err()
    {
        return String::new();
    }
    let mut bytes = Vec::new();
    if file.take(WORKER_TAIL).read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim();
    if text.is_empty() {
        String::new()
    } else {
        format!(" (the reader said: {text})")
    }
}

fn describe_status(status: &std::process::ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!("killed by signal {signal}");
        }
    }
    status
        .code()
        .map_or_else(|| "unknown exit".to_owned(), |c| format!("exit code {c}"))
}

fn read_answer(path: &Path) -> Result<Answer, ImportError> {
    let file = std::fs::File::open(path)
        .map_err(|e| ImportError::Worker(format!("cannot read the STEP reader's answer: {e}")))?;
    let mut bytes = Vec::new();
    file.take(MAX_ANSWER)
        .read_to_end(&mut bytes)
        .map_err(|e| ImportError::Worker(format!("cannot read the STEP reader's answer: {e}")))?;
    parse_answer(&bytes)
}

fn parse_answer(bytes: &[u8]) -> Result<Answer, ImportError> {
    serde_json::from_slice(bytes)
        .map_err(|e| ImportError::Worker(format!("the STEP reader's answer is malformed: {e}")))
}

/// Turns the worker's answer into an [`ImportedModel`].
fn convert(
    answer: Answer,
    options: &ImportOptions,
    settings: TessellationSettings,
) -> Result<ImportedModel, ImportError> {
    if let Some(reason) = answer.error {
        return Err(ImportError::Invalid {
            format: "STEP",
            reason,
        });
    }
    let model = answer
        .model
        .ok_or_else(|| ImportError::Worker("the STEP reader's answer holds no model".to_owned()))?;
    if model.faces.is_empty() {
        return Err(ImportError::Invalid {
            format: "STEP",
            reason: "the file contains no faces".to_owned(),
        });
    }
    model.validate().map_err(|e| {
        ImportError::Worker(format!(
            "the STEP reader returned an inconsistent model: {e}"
        ))
    })?;
    let file_unit = answer
        .file_unit_mm
        .filter(|mm| *mm > 0.0)
        .and_then(unit_from_millimetres);
    let mut notes = vec![step_tessellation_notice(settings)];
    let units = match options.units {
        Some(user) => {
            if let Some(scale) = answer.scale.filter(|s| (s - 1.0).abs() > 1e-12) {
                notes.push(format!(
                    "the file's numbers were read in {user} instead of its own unit (scaled by {scale})"
                ));
            }
            Units {
                file: file_unit,
                applied: user,
                source: UnitSource::User,
            }
        }
        None => {
            if file_unit.is_none()
                && let Some(name) = answer.file_unit.as_deref().filter(|n| !n.is_empty())
            {
                notes.push(format!(
                    "the file's length unit '{name}' was converted to millimetres by the reader"
                ));
            }
            Units {
                file: file_unit,
                applied: file_unit.unwrap_or(LengthUnit::Millimetre),
                source: UnitSource::File,
            }
        }
    };
    if answer.file_unit.is_none() && options.units.is_none() {
        notes.push("the file does not state a length unit: millimetres were assumed".to_owned());
    }
    Ok(ImportedModel {
        format: ModelFormat::Step,
        geometry: ModelGeometry::Brep(model),
        units,
        unit_question: None,
        stated_normals: None,
        notes,
    })
}

/// The length unit whose size in millimetres is `mm`, if it is one we offer.
fn unit_from_millimetres(mm: f64) -> Option<LengthUnit> {
    LengthUnit::ALL
        .into_iter()
        .find(|u| (u.millimetres() - mm).abs() <= 1e-9 * mm.max(1.0))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use openchuck_types::brep::{
        BrepEdge, BrepFace, BrepVertex, EdgeId, FaceId, Frame, Surface, SurfaceKind, TrimLoop,
        UvPoint, VertexId,
    };
    use openchuck_types::geometry::{Point3, Vec3};
    use openchuck_types::job::{CancelToken, NoProgress};
    use openchuck_types::mesh::TriangleMesh;

    /// A 10 by 10 mm square plane face with one edge, as the worker would answer.
    fn square() -> BrepModel {
        let corner = |x: f64, y: f64| UvPoint { u: x, v: y };
        BrepModel {
            faces: vec![BrepFace {
                id: FaceId(0),
                surface: SurfaceKind::Plane,
                body: 0,
                edges: vec![EdgeId(0)],
                geometry: Surface::Plane {
                    frame: Frame {
                        origin: Point3::ORIGIN,
                        x: Vec3::new(1.0, 0.0, 0.0),
                        y: Vec3::new(0.0, 1.0, 0.0),
                        z: Vec3::new(0.0, 0.0, 1.0),
                    },
                },
                reversed: false,
                uv_bounds: [0.0, 10.0, 0.0, 10.0],
                trims: vec![TrimLoop(vec![
                    corner(0.0, 0.0),
                    corner(10.0, 0.0),
                    corner(10.0, 10.0),
                    corner(0.0, 10.0),
                ])],
                tessellation: TriangleMesh {
                    vertices: vec![
                        Point3::new(0.0, 0.0, 0.0),
                        Point3::new(10.0, 0.0, 0.0),
                        Point3::new(10.0, 10.0, 0.0),
                        Point3::new(0.0, 10.0, 0.0),
                    ],
                    triangles: vec![[0, 1, 2], [0, 2, 3]],
                },
            }],
            edges: vec![BrepEdge {
                id: EdgeId(0),
                faces: vec![FaceId(0)],
                vertices: Some([VertexId(0), VertexId(0)]),
                polyline: vec![Point3::ORIGIN, Point3::new(10.0, 0.0, 0.0)],
                curve: None,
            }],
            vertices: vec![BrepVertex {
                id: VertexId(0),
                point: Point3::ORIGIN,
            }],
            tessellation: TessellationSettings::default(),
        }
    }

    fn answer(unit: Option<&str>, mm: f64, model: Option<BrepModel>) -> Answer {
        Answer {
            error: None,
            file_unit: unit.map(str::to_owned),
            file_unit_mm: Some(mm),
            scale: Some(1.0),
            model,
        }
    }

    fn convert_default(a: Answer, options: &ImportOptions) -> Result<ImportedModel, ImportError> {
        convert(a, options, TessellationSettings::default())
    }

    #[test]
    fn req_imp_03_file_unit_is_reported_and_applied() {
        let m = convert_default(
            answer(Some("INCH"), 25.4, Some(square())),
            &ImportOptions::default(),
        )
        .unwrap();
        assert_eq!(m.units.file, Some(LengthUnit::Inch));
        assert_eq!(m.units.applied, LengthUnit::Inch);
        assert_eq!(m.units.source, UnitSource::File);
        assert!(m.unit_question.is_none());
        assert_eq!(m.units.describe(), "in, from the file");
    }

    #[test]
    fn req_imp_03_user_unit_overrides_the_file_unit() {
        let options = ImportOptions {
            units: Some(LengthUnit::Metre),
            ..ImportOptions::default()
        };
        let mut a = answer(Some("MM"), 1.0, Some(square()));
        a.scale = Some(1000.0);
        let m = convert_default(a, &options).unwrap();
        assert_eq!(m.units.file, Some(LengthUnit::Millimetre));
        assert_eq!(m.units.applied, LengthUnit::Metre);
        assert_eq!(m.units.source, UnitSource::User);
        assert!(m.notes.iter().any(|n| n.contains("scaled by 1000")));
    }

    #[test]
    fn req_imp_03_unknown_file_unit_is_converted_and_said() {
        let m = convert_default(
            answer(Some("FOOT"), 304.8, Some(square())),
            &ImportOptions::default(),
        )
        .unwrap();
        assert_eq!(m.units.file, None);
        assert_eq!(m.units.applied, LengthUnit::Millimetre);
        assert!(m.notes.iter().any(|n| n.contains("FOOT")));
        let none =
            convert_default(answer(None, 1.0, Some(square())), &ImportOptions::default()).unwrap();
        assert!(
            none.notes
                .iter()
                .any(|n| n.contains("does not state a length unit"))
        );
    }

    #[test]
    fn req_op_23_req_imp_02_req_nfr_12_the_notice_says_which_strategies_use_the_tessellation() {
        // Decision 0209 item 4.4 (OI-830): tool drops still use the
        // tessellation, so the notice does not say every toolpath is
        // computed on the exact surfaces.
        let notice = step_tessellation_notice(TessellationSettings::default());
        assert!(
            !notice.contains("toolpaths are computed on the exact"),
            "{notice}"
        );
        assert!(
            notice.contains("drop the tool on the tessellation (0.01 mm, 0.1 rad)")
                && notice.contains("radial drilling, swarf and edge following"),
            "{notice}"
        );
        // Pencil clean-up reads the exact edges for its inside corners only
        // (decision 0226).
        assert!(
            notice.contains("pencil clean-up finds its inside corners on the exact edges"),
            "{notice}"
        );
    }

    #[test]
    fn req_imp_02_a_file_without_faces_is_an_error() {
        let a = answer(Some("MM"), 1.0, Some(BrepModel::default()));
        let err = convert_default(a, &ImportOptions::default()).unwrap_err();
        assert!(matches!(err, ImportError::Invalid { .. }), "{err}");
    }

    #[test]
    fn req_nfr_07_worker_error_reaches_the_user() {
        let a = Answer {
            error: Some("the STEP reader could not parse the file".to_owned()),
            file_unit: None,
            file_unit_mm: None,
            scale: None,
            model: None,
        };
        let err = convert_default(a, &ImportOptions::default()).unwrap_err();
        assert!(err.to_string().contains("could not parse"), "{err}");
    }

    #[test]
    fn req_nfr_07_inconsistent_answer_is_an_error() {
        let mut bad = square();
        bad.edges[0].faces = vec![FaceId(7)];
        let err = convert_default(
            answer(Some("MM"), 1.0, Some(bad)),
            &ImportOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(err, ImportError::Worker(_)), "{err}");
        let mut bad = square();
        bad.faces[0].tessellation.triangles = vec![[0, 1, 9]];
        assert!(
            convert_default(
                answer(Some("MM"), 1.0, Some(bad)),
                &ImportOptions::default()
            )
            .is_err()
        );
        let mut bad = square();
        bad.faces[0].uv_bounds = [0.0, f64::NAN, 0.0, 1.0];
        assert!(
            convert_default(
                answer(Some("MM"), 1.0, Some(bad)),
                &ImportOptions::default()
            )
            .is_err()
        );
    }

    #[test]
    fn req_nfr_07_a_malformed_answer_is_an_error() {
        assert!(matches!(parse_answer(b"{"), Err(ImportError::Worker(_))));
        assert!(matches!(parse_answer(b""), Err(ImportError::Worker(_))));
        assert!(matches!(
            parse_answer(br#"{"model": {"faces": 3}}"#),
            Err(ImportError::Worker(_))
        ));
    }

    #[test]
    fn req_nfr_07_a_missing_worker_is_an_error_not_a_crash() {
        let importer = StepImporter::new().with_worker("/nonexistent/openchuck-step-worker");
        let (cancel, progress) = (CancelToken::new(), NoProgress);
        let context = JobContext::new(&cancel, &progress);
        let err = importer
            .import(b"ISO-10303-21;", &ImportOptions::default(), &context)
            .unwrap_err();
        assert!(matches!(err, ImportError::Worker(_)), "{err}");
    }

    #[test]
    fn req_imp_06_tessellation_tolerances_must_be_positive() {
        let importer = StepImporter::new().with_worker("/nonexistent/openchuck-step-worker");
        let (cancel, progress) = (CancelToken::new(), NoProgress);
        let context = JobContext::new(&cancel, &progress);
        for linear in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let options = ImportOptions {
                tessellation: Some(TessellationSettings {
                    linear,
                    angular: 0.1,
                }),
                ..ImportOptions::default()
            };
            let err = importer.import(b"x", &options, &context).unwrap_err();
            assert!(matches!(err, ImportError::Invalid { .. }), "{err}");
        }
    }

    #[test]
    fn req_nfr_07_a_cancelled_import_stops_before_the_worker_starts() {
        let importer = StepImporter::new().with_worker("/nonexistent/openchuck-step-worker");
        let (cancel, progress) = (CancelToken::new(), NoProgress);
        cancel.cancel();
        let context = JobContext::new(&cancel, &progress);
        let err = importer
            .import(b"x", &ImportOptions::default(), &context)
            .unwrap_err();
        assert!(matches!(err, ImportError::Cancelled), "{err}");
    }

    #[test]
    fn req_nfr_17_helper_programs_start_without_a_console_window() {
        // The real check (no window flashes) needs Windows: OI-580.
        let expected = if cfg!(windows) { 0x0800_0000 } else { 0 };
        assert_eq!(helper_creation_flags(), expected);
        let mut command = Command::new("openchuck-step-worker");
        hide_window(&mut command);
    }

    #[test]
    fn req_nfr_07_worker_messages_reach_the_error() {
        let dir = std::env::temp_dir().join(format!("oc-wm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(worker_messages(&dir), "");
        std::fs::write(dir.join(STDERR_FILE), "  \n").unwrap();
        assert_eq!(worker_messages(&dir), "");
        std::fs::write(dir.join(STDERR_FILE), "bad entity\n").unwrap();
        assert_eq!(worker_messages(&dir), " (the reader said: bad entity)");
        // A long log gives its end, not its beginning.
        let long = format!("{}final reason\n", "early noise\n".repeat(500));
        std::fs::write(dir.join(STDERR_FILE), long).unwrap();
        let said = worker_messages(&dir);
        assert!(said.contains("final reason"), "{said}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn req_imp_03_file_units_map_to_known_units() {
        assert_eq!(unit_from_millimetres(25.4), Some(LengthUnit::Inch));
        assert_eq!(unit_from_millimetres(1.0), Some(LengthUnit::Millimetre));
        assert_eq!(unit_from_millimetres(1000.0), Some(LengthUnit::Metre));
        assert_eq!(unit_from_millimetres(304.8), None);
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! What the export dialog shows and decides (GC-02, GC-04, GC-05, GC-15;
//! specification section 11.6; decision record 0071): the posts to choose
//! from, the files a grouping would write, the checks before writing, and
//! writing the files. The dialog itself, in `openchuck-ui`, only draws these.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use openchuck_post::{ADirection, FeedMode, UnwindMode, builtin_ids};
use openchuck_project::Project;
use openchuck_project::id::OperationId;

use crate::cache::{ModelFiles, operation_key};
use crate::error::AppError;
use crate::pipeline::Regeneration;
use crate::posting::PostSelection;

/// What a post says about its output, for the dialog's read-only lines
/// (GC-04). Units, the A letter and its wrap come from the post, never from
/// the dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostSummary {
    /// The selection that gives this post.
    pub selection: PostSelection,
    /// The post's display name.
    pub name: String,
    /// The program's units, as words.
    pub units: String,
    /// The rotary axis letter.
    pub a_letter: String,
    /// How positive A is written and how whole turns are removed.
    pub a_wrap: String,
    /// The feed word mode, as words.
    pub feed_mode: String,
    /// The program file extension, without the dot.
    pub file_extension: String,
}

/// Reads what `selection` says about its output. A post folder that cannot be
/// read is an error naming the problem (NFR-07).
pub fn post_summary(selection: &PostSelection) -> Result<PostSummary, AppError> {
    let definition = selection.load()?;
    let options = &definition.options;
    let direction = match options.a_direction {
        ADirection::Same => "positive A as the machine profile defines it",
        ADirection::Reversed => "positive A written reversed",
    };
    let wrap = match options.unwind {
        UnwindMode::Rotate => "whole turns removed by a rotation of A",
        UnwindMode::DropTurns => "whole turns dropped without motion",
    };
    Ok(PostSummary {
        selection: selection.clone(),
        name: definition.name.clone(),
        units: "millimetres".to_owned(),
        a_letter: options.a_letter.clone(),
        a_wrap: format!("{direction}; {wrap}"),
        feed_mode: match options.feed_mode {
            FeedMode::G94 => "units per minute (G94)".to_owned(),
            FeedMode::G93 => "inverse time (G93)".to_owned(),
        },
        file_extension: options.file_extension.clone(),
    })
}

/// The posts that ship with the application, in a fixed order (GC-02).
pub fn builtin_post_summaries() -> Vec<PostSummary> {
    builtin_ids()
        .filter_map(|id| post_summary(&PostSelection::Builtin(id.to_owned())).ok())
        .collect()
}

/// How serious a line of the checks is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckLevel {
    /// Writing is blocked until it is fixed (GC-15).
    Blocking,
    /// Writing is allowed; the user should look at it (GC-15).
    Warning,
}

/// One line of the checks before writing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// Whether it blocks writing.
    pub level: CheckLevel,
    /// What is wrong, why, and what to do.
    pub text: String,
}

/// The checks before writing (GC-15; specification section 11.6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExportChecks {
    /// Every blocking check and warning, blocking first, in project order.
    pub checks: Vec<Check>,
    /// Disabled operations, listed as not posted.
    pub not_posted: Vec<String>,
}

/// The fixed safety notice shown at every export (NFR-12, GC-16;
/// specification section 11.6).
pub const EXPORT_NOTICE: &str = "Verify this G-code before you run it. It was produced from your project and has not been tested on your machine. Check the origin, axis height, tool lengths, A direction and unwinding at the controller, and run an air cut first.";

/// The label of the acknowledgement tick box (GC-16).
pub const EXPORT_ACKNOWLEDGE_LABEL: &str =
    "I will verify this G-code before running it on a machine";

/// The warning at export while the project skips the stock simulation
/// (NFR-12; issue 214, decision record 0168).
pub const EXPORT_STOCK_SKIPPED: &str = "Stock simulation is skipped for this project, so the cut has not been simulated. Untick \"Skip stock simulation\" in Simulate to check it.";

/// The tick box that acknowledges, before writing, that the project skips
/// the stock simulation (OI-602, the owner's option C; decision record 0168).
pub const EXPORT_STOCK_SKIPPED_LABEL: &str =
    "I know the cut has not been simulated for this project";

/// The sentence that states the rule, shown under the checks.
pub const EXPORT_RULE: &str = "G-code is written only when every enabled operation is up to date and the files pass their checks. Operations that were not simulated, and machine values not confirmed on the machine, give warnings. Disabled operations are not written.";

impl ExportChecks {
    /// Whether nothing blocks writing.
    pub fn can_write(&self) -> bool {
        !self.checks.iter().any(|c| c.level == CheckLevel::Blocking)
    }

    /// The first blocking check, for the primary button's tooltip.
    pub fn first_blocker(&self) -> Option<&str> {
        self.checks
            .iter()
            .find(|c| c.level == CheckLevel::Blocking)
            .map(|c| c.text.as_str())
    }
}

/// Operations of `project` whose result in `run` is out of date: the project
/// has changed, in the operation or in anything it reads or in an operation
/// before it, since the run was made. This is the same key the pipeline cache
/// uses (architecture section 6), so a change that would regenerate an
/// operation marks it here. Model file contents are not compared.
/// The cache keys of `project`'s enabled operations, in machining order.
fn keys(project: &Project) -> Result<Vec<(OperationId, crate::cache::Key)>, AppError> {
    let files = ModelFiles::default();
    let mut previous = None;
    let mut out = Vec::new();
    for operation in project.operations().iter().filter(|o| o.enabled) {
        let key = operation_key(project, operation, previous, &files)?;
        out.push((operation.id, key));
        previous = Some(key);
    }
    Ok(out)
}

/// Whether every operation `run` holds is still in `project`, enabled, and
/// up to date: nothing it, or an operation before it, reads has changed
/// since the run was made (the key the pipeline cache and the tree use).
/// The operations the run did not make do not count, so a run up to an
/// operation (decision record 0117) can be current. A run that is not
/// current is not shown again by the playback panel (OI-697).
pub fn run_is_current(project: &Project, run: &Regeneration) -> bool {
    let (Ok(then), Ok(now)) = (keys(&run.project), keys(project)) else {
        return false;
    };
    !run.groups.is_empty()
        && run.groups.iter().all(|g| {
            let key = |list: &[(OperationId, crate::cache::Key)]| {
                list.iter()
                    .find(|(id, _)| *id == g.operation)
                    .map(|(_, k)| *k)
            };
            key(&now).is_some() && key(&now) == key(&then)
        })
}

fn stale_operations(
    project: &Project,
    run: &Regeneration,
) -> Result<BTreeSet<OperationId>, AppError> {
    let then = keys(&run.project)?;
    let now = keys(project)?;
    // An operation the run did not make (a run up to an earlier operation,
    // decision record 0117) is stale too: it is not in the toolpath.
    let made = |id: &OperationId| run.groups.iter().any(|g| g.operation == *id);
    let mut stale: BTreeSet<OperationId> = now
        .iter()
        .filter(|(id, key)| {
            !made(id) || then.iter().find(|(i, _)| i == id).map(|(_, k)| k) != Some(key)
        })
        .map(|(id, _)| *id)
        .collect();
    // The run holds the path of an operation that is gone or turned off since
    // (issue 196): that path would be written to the G-code and played back
    // as if it were current (NFR-12).
    stale.extend(
        run.groups
            .iter()
            .map(|g| g.operation)
            .filter(|id| !now.iter().any(|(n, _)| n == id)),
    );
    Ok(stale)
}

/// Works out the checks before writing G-code (GC-15).
///
/// `run` is the latest regeneration the user has, if any; `posted` a run made
/// with the chosen post, whose G-code files and their checks are looked at;
/// `last_error` the message of a later run that failed. An enabled operation is blocking when
/// there is no run, the run is older than its inputs (stale), or the last
/// generation failed (error). An operation the run did not simulate, and
/// each machine value not confirmed on the machine, are warnings. A G-code
/// file that failed its own checks (gates 1 and 2) blocks too: the user
/// must not be offered a file the application itself knows disagrees with
/// the toolpath (decision record 0071 item 4).
pub fn export_checks(
    project: &Project,
    run: Option<&Regeneration>,
    posted: Option<&Regeneration>,
    last_error: Option<&str>,
) -> ExportChecks {
    let mut out = ExportChecks::default();
    let mut push = |level, text: String| out.checks.push(Check { level, text });
    let enabled: Vec<_> = project.operations().iter().filter(|o| o.enabled).collect();
    if enabled.is_empty() {
        push(
            CheckLevel::Blocking,
            "No operation is enabled, so there is nothing to write. Add or enable an operation."
                .to_owned(),
        );
    }
    if let Some(error) = last_error {
        push(
            CheckLevel::Blocking,
            format!("The last generation failed: {error} Fix it, then generate again."),
        );
    }
    match run {
        None if !enabled.is_empty() && last_error.is_none() => push(
            CheckLevel::Blocking,
            "Nothing has been generated yet. Generate the operations first.".to_owned(),
        ),
        None => {}
        Some(run) => {
            match stale_operations(project, run) {
                Ok(stale) => {
                    for operation in &enabled {
                        if stale.contains(&operation.id) {
                            push(
                                CheckLevel::Blocking,
                                format!(
                                    "Operation '{}' is out of date: the project changed after it was generated. Generate it again.",
                                    operation.name
                                ),
                            );
                        }
                    }
                    // Made in the run, but removed or turned off since.
                    for id in &stale {
                        if enabled.iter().any(|o| o.id == *id) {
                            continue;
                        }
                        let name = run
                            .project
                            .operation(*id)
                            .map_or("an operation", |o| o.name.as_str());
                        let why = if project.operation(*id).is_some() {
                            "turned off"
                        } else {
                            "removed"
                        };
                        push(
                            CheckLevel::Blocking,
                            format!(
                                "Operation '{name}' is out of date: it was {why} after the toolpath was generated, but the toolpath still holds it. Generate again."
                            ),
                        );
                    }
                }
                Err(e) => push(
                    CheckLevel::Blocking,
                    format!("Could not tell whether the operations are up to date: {e}"),
                ),
            }
            for report in posted.map_or(&[][..], |p| &p.reports[..]) {
                if !report.is_clean() {
                    push(
                        CheckLevel::Blocking,
                        format!(
                            "{} failed its checks ({}). Look at the checks in the messages and fix the operation.",
                            report.file,
                            report
                                .findings
                                .first()
                                .map_or("the replay does not match", String::as_str)
                        ),
                    );
                }
            }
            let simulated: BTreeSet<&str> =
                run.simulation.as_ref().map_or_else(BTreeSet::new, |s| {
                    s.setups
                        .iter()
                        .flat_map(|setup| setup.operations.iter())
                        .map(|o| o.operation.as_str())
                        .collect()
                });
            if project.skip_stock_simulation() {
                // One line naming the cause, not one per operation telling
                // the user to run what they chose to skip; writing then
                // waits for its acknowledgement (decision record 0168,
                // OI-602).
                push(CheckLevel::Warning, EXPORT_STOCK_SKIPPED.to_owned());
            }
            for operation in enabled.iter().filter(|_| !project.skip_stock_simulation()) {
                // The simulation names an operation as it was called when the
                // run was made; a rename since then is not a missing run
                // (issue 209).
                let simulated_as = run
                    .project
                    .operation(operation.id)
                    .map_or(operation.name.as_str(), |o| o.name.as_str());
                if !simulated.contains(simulated_as) {
                    push(
                        CheckLevel::Warning,
                        format!(
                            "Operation '{}' has not been simulated. Run the simulation to see what it removes.",
                            operation.name
                        ),
                    );
                }
            }
        }
    }
    if let Some(machine) = project.machine() {
        let fields: Vec<&str> = machine
            .unconfirmed_fields()
            .into_iter()
            .map(crate::setup::profile_field_label)
            .collect();
        if !fields.is_empty() {
            push(
                CheckLevel::Warning,
                format!(
                    "These machine values are not confirmed on the machine: {}. Check them against the controller.",
                    fields.join(", ")
                ),
            );
        }
    }
    out.not_posted = project
        .operations()
        .iter()
        .filter(|o| !o.enabled)
        .map(|o| o.name.clone())
        .collect();
    out.checks.sort_by_key(|c| c.level != CheckLevel::Blocking);
    out
}

/// Writes `files` (name, text) into `folder`, creating it if needed, and
/// returns the paths written. A file is written whole or not at all: it goes
/// to a temporary name first. A file of the same name is replaced.
pub fn write_gcode_files(
    folder: &Path,
    files: &[(String, String)],
) -> Result<Vec<PathBuf>, AppError> {
    let bytes: Vec<(String, Vec<u8>)> = files
        .iter()
        .map(|(name, text)| (name.clone(), text.as_bytes().to_vec()))
        .collect();
    write_bytes(folder, &bytes)
}

pub(crate) fn write_bytes(
    folder: &Path,
    files: &[(String, Vec<u8>)],
) -> Result<Vec<PathBuf>, AppError> {
    std::fs::create_dir_all(folder).map_err(|source| AppError::Io {
        path: folder.to_path_buf(),
        source,
    })?;
    let mut written = Vec::new();
    for (name, bytes) in files {
        let path = folder.join(name);
        let temp = folder.join(format!("{name}.part"));
        std::fs::write(&temp, bytes)
            .and_then(|()| std::fs::rename(&temp, &path))
            .map_err(|source| AppError::Io {
                path: path.clone(),
                source,
            })?;
        written.push(path);
    }
    Ok(written)
}

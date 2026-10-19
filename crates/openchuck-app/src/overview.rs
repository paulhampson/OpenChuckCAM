// SPDX-License-Identifier: AGPL-3.0-or-later
//! The state of the job as the shell shows it: the workflow steps with their
//! availability and reasons (UI-10), the next required step as the primary
//! action (specification section 5), the state of each item in the project
//! tree (UI-11), the messages panel (UI-12) and the setup checks (SET-06).
//! Slice 22 lane X2; decision record 0117.
//!
//! It is split in two so nothing slow runs on the interface's thread:
//!
//! - [`Application::start_checks`] is a job that reads what needs the files:
//!   whether each model file still matches the project, the mesh checks of
//!   each model, the setup checks against the placed part, and the
//!   selections that reach a held region or a tab. It reuses the results of
//!   the previous [`Checks`] whose inputs have not changed, so the job after
//!   an ordinary edit reads no model file.
//! - [`Application::overview`] is quick and pure: it combines the project,
//!   the latest [`Checks`], what generation has recorded and the
//!   [`JobLog`] into an [`Overview`]. A front end calls it when
//!   [`Application::revision`], the checks or the job log change.
//!
//! An operation is up to date when the latest successful generation made it
//! from inputs that hash the same as its inputs now. The hash is the
//! pipeline cache's key (decision record 0030 item 1) with the model files
//! taken at their recorded hashes; it chains from the operations before, so
//! an edit to operation N makes N and every later operation stale.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use openchuck_project::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_project::{ModelCheck, ModelRef, Project};
use openchuck_types::job::JobContext;
use openchuck_types::machine::ProfileField;
use openchuck_types::setup::{PartExtent, SetupWarning, SetupWarningKind};

use crate::cache::{Key, StatusKey, status_keys};
use crate::jobs::{JobLog, JobOutcome};
use crate::{AppError, Application, JobHandle, SelectionSummary};

/// The five workflow steps, in pipeline order (UI-10; specification
/// section 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    /// Import, check units and mesh, place on the rotary axis.
    Model,
    /// Machine, stock, part position, work offset, clearance, workholding.
    Setup,
    /// Select geometry, add operations, generate.
    Operations,
    /// Play the cut and compare stock with model.
    Simulate,
    /// Write G-code and the setup sheet.
    Export,
}

impl Step {
    /// Every step, in order.
    pub const ALL: [Self; 5] = [
        Self::Model,
        Self::Setup,
        Self::Operations,
        Self::Simulate,
        Self::Export,
    ];

    /// The step's number, from 1 (Alt 1 to Alt 5 open them).
    pub fn number(self) -> usize {
        self as usize + 1
    }

    /// The step's name as the workflow bar shows it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Setup => "Setup",
            Self::Operations => "Operations",
            Self::Simulate => "Simulate",
            Self::Export => "Post and export",
        }
    }

    /// The step after this one, if any.
    pub fn next(self) -> Option<Self> {
        Self::ALL.get(self.number()).copied()
    }
}

/// Whether a step can be opened, and whether its outputs are complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepState {
    /// The step.
    pub step: Step,
    /// Why it cannot be opened yet ("Import a model first"); `None` when it
    /// is available.
    pub unavailable: Option<String>,
    /// Whether its outputs are complete (the green ring round its number).
    pub complete: bool,
}

impl StepState {
    /// Whether the step can be opened.
    pub fn is_available(&self) -> bool {
        self.unavailable.is_none()
    }
}

/// A part of a setup that is a row of its own under the setup in the tree
/// and a section of the setup inspector (specification sections 6, 11.3 and
/// 11.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SetupPart {
    /// The machine profile.
    Machine,
    /// The stock.
    Stock,
    /// The part's position.
    PartPosition,
    /// The work offset and the clearance.
    WorkOffset,
    /// The held regions and the retaining tabs.
    Workholding,
}

impl SetupPart {
    /// Every part, in the tree's order.
    pub const ALL: [Self; 5] = [
        Self::Machine,
        Self::Stock,
        Self::PartPosition,
        Self::WorkOffset,
        Self::Workholding,
    ];

    /// The row's and the section's name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Machine => "Machine profile",
            Self::Stock => "Stock",
            Self::PartPosition => "Part position",
            Self::WorkOffset => "Work offset and clearance",
            Self::Workholding => "Workholding",
        }
    }
}

/// An item of the project tree that can carry a state or be a message's
/// source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemRef {
    /// A model.
    Model(ModelId),
    /// A setup.
    Setup(SetupId),
    /// A part of a setup (its child row in the tree).
    SetupPart(SetupId, SetupPart),
    /// A selection set.
    SelectionSet(SelectionSetId),
    /// A project tool.
    Tool(ToolId),
    /// An operation.
    Operation(OperationId),
}

/// The state an item shows in the tree, by shape as well as colour
/// (UI-11; specification section 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemState {
    /// Generated, and nothing it reads has changed since: a filled green
    /// circle.
    UpToDate,
    /// Something it reads changed, or it was never generated: an amber
    /// diamond.
    Stale,
    /// A job is working on it: a blue ring with a gap.
    Running,
    /// It can be used, with a caveat: an amber triangle outline.
    Warning,
    /// It cannot be used: a red triangle.
    Error,
    /// Turned off by the user: a grey ring, the name struck through.
    Off,
}

impl ItemState {
    /// Every state, in the legend's order.
    pub const ALL: [Self; 6] = [
        Self::UpToDate,
        Self::Stale,
        Self::Running,
        Self::Warning,
        Self::Error,
        Self::Off,
    ];

    /// The state's name, as the legend and the inspector's chip say it.
    pub fn name(self) -> &'static str {
        match self {
            Self::UpToDate => "Up to date",
            Self::Stale => "Stale",
            Self::Running => "Running",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Off => "Off",
        }
    }
}

/// An item's state and its cause, for the mark's tooltip ("Stale: operation
/// 2 changed").
#[derive(Debug, Clone, PartialEq)]
pub struct ItemStatus {
    /// The state.
    pub state: ItemState,
    /// The cause, starting with the state's name.
    pub cause: String,
    /// The share of the running job done, 0 to 1, when it is known.
    pub progress: Option<f32>,
}

impl ItemStatus {
    fn new(state: ItemState, cause: impl Into<String>) -> Self {
        Self {
            state,
            cause: format!("{}: {}", state.name(), cause.into()),
            progress: None,
        }
    }
}

/// How serious a message is. Errors sort first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Something cannot be done until it is fixed: a red triangle.
    Error,
    /// Something can be done, with a caveat: an amber triangle.
    Warning,
    /// Something to know: a blue "i".
    Info,
}

impl Severity {
    /// Every severity, in order.
    pub const ALL: [Self; 3] = [Self::Error, Self::Warning, Self::Info];

    /// The severity's name, for the filter chips.
    pub fn name(self) -> &'static str {
        match self {
            Self::Error => "Errors",
            Self::Warning => "Warnings",
            Self::Info => "Information",
        }
    }
}

/// What a message's action link does. The front end carries it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageAction {
    /// Select the item, in the tree and the view.
    Show(ItemRef),
    /// Pick the selection set again on the part.
    Reselect(SelectionSetId),
    /// Open the Setup step on this setup.
    ReviewSetup(SetupId),
    /// Open the setup's workholding section.
    EditWorkholding(SetupId),
    /// Generate the stale operations.
    Generate,
}

impl MessageAction {
    /// The link's words.
    pub fn label(self) -> &'static str {
        match self {
            Self::Show(_) => "Show",
            Self::Reselect(_) => "Reselect",
            Self::ReviewSetup(_) => "Review setup",
            Self::EditWorkholding(_) => "Edit workholding",
            Self::Generate => "Generate",
        }
    }
}

/// One row of the messages panel (UI-12; specification section 9).
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    /// How serious it is.
    pub severity: Severity,
    /// One or two plain sentences: what happened and why.
    pub text: String,
    /// The item it is about, if any; choosing it selects the item.
    pub source: Option<ItemRef>,
    /// The source item's name, for the link.
    pub source_name: String,
    /// The action that fixes it, if one does.
    pub action: Option<MessageAction>,
}

impl Message {
    /// A message about `source`.
    pub fn new(
        severity: Severity,
        text: impl Into<String>,
        source: Option<(ItemRef, String)>,
        action: Option<MessageAction>,
    ) -> Self {
        let (source, source_name) = match source {
            Some((item, name)) => (Some(item), name),
            None => (None, String::new()),
        };
        Self {
            severity,
            text: sentence(&text.into()),
            source,
            source_name,
            action,
        }
    }
}

/// One row of the Setup checks tab: a green tick or an amber triangle
/// (SET-06; specification section 9).
#[derive(Debug, Clone, PartialEq)]
pub struct SetupCheck {
    /// The setup checked.
    pub setup: SetupId,
    /// True for a passed check (a tick), false for a warning.
    pub passed: bool,
    /// What was checked or found.
    pub text: String,
    /// The action that fixes a warning, if one does.
    pub action: Option<MessageAction>,
}

/// What the footer's primary button does (specification section 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryKind {
    /// Open the import dialog.
    ImportModel,
    /// Open the next step.
    Continue(Step),
    /// Generate the stale operations; the number of them.
    GenerateStale(usize),
    /// Generate and simulate.
    RunSimulation,
    /// Generate and simulate when an operation was never generated; the
    /// operation's number in the list, from 1.
    GenerateAndSimulate(usize),
    /// Open the export dialog.
    OpenExport,
}

/// The footer's primary action: the next required step.
#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryAction {
    /// What it does.
    pub kind: PrimaryKind,
    /// The button's words.
    pub label: String,
    /// Why it is disabled, for its tooltip and the footer's status; `None`
    /// when it is enabled.
    pub blocked: Option<String>,
    /// The message to highlight when it is disabled by an error, as an index
    /// into [`Overview::messages`].
    pub message: Option<usize>,
}

/// Everything the shell shows about the state of the job, worked out at one
/// [`Application::revision`].
#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
    /// The project revision it was worked out at.
    pub revision: u64,
    /// The five steps, in order.
    pub steps: Vec<StepState>,
    /// The state of each item that has one. Tools, and models and setups
    /// with nothing to report, have none.
    pub items: BTreeMap<ItemRef, ItemStatus>,
    /// The messages, errors first, then warnings, then information.
    pub messages: Vec<Message>,
    /// The Setup checks tab's rows.
    pub setup_checks: Vec<SetupCheck>,
    /// The enabled operations that are stale, in machining order.
    pub stale: Vec<OperationId>,
    /// The machine's values not confirmed on the machine, for the footer
    /// ("Axis height unconfirmed").
    pub machine_warnings: Vec<String>,
    /// Whether the checks it used are from the current revision; false
    /// while they are being worked out again.
    pub checks_current: bool,
    simulated_all: bool,
    first_unsimulated_unmade: Option<usize>,
    any_enabled: bool,
    models: usize,
    /// Selection sets in error that an enabled operation uses.
    blocked_sets: Vec<ItemRef>,
}

impl Overview {
    /// The overview with no project open: only the Model step, and its
    /// primary action, are available (specification section 11.1).
    pub fn no_project() -> Self {
        let steps = Step::ALL
            .into_iter()
            .map(|step| StepState {
                step,
                unavailable: (step != Step::Model).then(|| "Open or start a project first".into()),
                complete: false,
            })
            .collect();
        Self {
            revision: 0,
            steps,
            items: BTreeMap::new(),
            messages: Vec::new(),
            setup_checks: Vec::new(),
            stale: Vec::new(),
            machine_warnings: Vec::new(),
            checks_current: true,
            simulated_all: false,
            first_unsimulated_unmade: None,
            any_enabled: false,
            models: 0,
            blocked_sets: Vec::new(),
        }
    }

    /// The state of `step`.
    pub fn step(&self, step: Step) -> StepState {
        self.steps
            .iter()
            .find(|s| s.step == step)
            .cloned()
            // `steps` always holds the five; this is never reached.
            .unwrap_or(StepState {
                step,
                unavailable: Some("Open or start a project first".to_owned()),
                complete: false,
            })
    }

    /// The state of `item`, if it has one.
    pub fn status(&self, item: ItemRef) -> Option<&ItemStatus> {
        self.items.get(&item)
    }

    /// How many messages there are of `severity`, for the filter chips.
    pub fn count(&self, severity: Severity) -> usize {
        self.messages
            .iter()
            .filter(|m| m.severity == severity)
            .count()
    }

    /// Adds a message the application cannot see, such as a note from the
    /// view, keeping the order.
    pub fn push(&mut self, message: Message) {
        let at = self
            .messages
            .iter()
            .position(|m| m.severity > message.severity)
            .unwrap_or(self.messages.len());
        self.messages.insert(at, message);
    }

    /// The primary action of the footer on `step`, by the rule of
    /// specification section 5:
    ///
    /// 1. an enabled item with an error that blocks the next step disables
    ///    it, naming the problem;
    /// 2. stale enabled operations make it "Generate stale (n)";
    /// 3. on Simulate, an operation not simulated makes it "Run simulation",
    ///    or "Generate and simulate op N" when it was not generated either
    ///    (this is checked before rule 2 there, since simulating generates;
    ///    decision record 0117, OI-350);
    /// 4. otherwise it continues to the next step.
    ///
    /// Model offers "Import model…" with no model, Setup always continues,
    /// and Post and export opens the export dialog (section 4).
    pub fn primary(&self, step: Step) -> PrimaryAction {
        let action = |kind: PrimaryKind| {
            let label = match kind {
                PrimaryKind::ImportModel => "Import model…".to_owned(),
                PrimaryKind::Continue(next) => match next {
                    Step::Export => "Continue to post and export".to_owned(),
                    other => format!("Continue to {}", other.name().to_lowercase()),
                },
                PrimaryKind::GenerateStale(n) => format!("Generate stale ({n})"),
                PrimaryKind::RunSimulation => "Run simulation".to_owned(),
                PrimaryKind::GenerateAndSimulate(n) => format!("Generate and simulate op {n}"),
                PrimaryKind::OpenExport => "Post and export…".to_owned(),
            };
            PrimaryAction {
                kind,
                label,
                blocked: None,
                message: None,
            }
        };
        let continue_to = |next: Step| {
            let mut a = action(PrimaryKind::Continue(next));
            a.blocked = self.step(next).unavailable;
            a
        };
        let blocked = |mut a: PrimaryAction| {
            if let Some((index, m)) = self.blocking_error(step) {
                a.blocked = Some(m.text.clone());
                a.message = Some(index);
            }
            a
        };
        match step {
            Step::Model if self.models == 0 => action(PrimaryKind::ImportModel),
            Step::Model => blocked(continue_to(Step::Setup)),
            Step::Setup => continue_to(Step::Operations),
            Step::Operations if !self.stale.is_empty() => {
                blocked(action(PrimaryKind::GenerateStale(self.stale.len())))
            }
            Step::Operations => blocked(continue_to(Step::Simulate)),
            Step::Simulate if self.any_enabled && !self.simulated_all => {
                blocked(match self.first_unsimulated_unmade {
                    Some(n) => action(PrimaryKind::GenerateAndSimulate(n)),
                    None => action(PrimaryKind::RunSimulation),
                })
            }
            Step::Simulate => blocked(continue_to(Step::Export)),
            Step::Export => {
                let mut a = blocked(action(PrimaryKind::OpenExport));
                if a.blocked.is_none() {
                    a.blocked = self.step(Step::Export).unavailable;
                }
                a
            }
        }
    }

    /// The first error message that blocks the step after `step`: a model in
    /// error blocks Setup; an enabled operation in error, or a selection set
    /// an enabled operation uses, blocks generating, simulating and export.
    fn blocking_error(&self, step: Step) -> Option<(usize, &Message)> {
        self.messages.iter().enumerate().find(|(_, m)| {
            m.severity == Severity::Error
                && match (step, m.source) {
                    (Step::Model, Some(ItemRef::Model(_))) => true,
                    (Step::Model | Step::Setup, _) => false,
                    (_, Some(ItemRef::Operation(op))) => self
                        .items
                        .get(&ItemRef::Operation(op))
                        .is_some_and(|s| s.state == ItemState::Error),
                    (_, Some(set @ ItemRef::SelectionSet(_))) => self.blocked_sets.contains(&set),
                    _ => false,
                }
        })
    }
}

/// What a model file was found to be on disk.
#[derive(Debug, Clone, PartialEq)]
enum FileState {
    Unchanged,
    Changed,
    Missing,
    Unreadable(String),
}

/// The mesh check of one model, kept while its reference and file are
/// unchanged.
#[derive(Debug, Clone)]
struct ModelChecked {
    reference: ModelRef,
    stamp: Option<(u64, SystemTime)>,
    file: FileState,
    findings: Result<ModelFindings, String>,
}

#[derive(Debug, Clone, PartialEq)]
struct ModelFindings {
    problems: Vec<String>,
    unit_question: Option<String>,
    notes: Vec<String>,
    extent: Option<PartExtent>,
}

/// One setup's checks.
#[derive(Debug, Clone)]
struct SetupChecked {
    setup: SetupId,
    warnings: Vec<SetupWarning>,
    notes: Vec<String>,
    /// Why the stored workholding cannot be built, if it cannot.
    unbuildable: Option<String>,
    has_machine: bool,
}

/// What the checks job found: the model files, the mesh checks, the setup
/// checks and the selections that reach held regions or tabs. Made by
/// [`Application::start_checks`]; read by [`Application::overview`].
#[derive(Debug, Clone, Default)]
pub struct Checks {
    revision: u64,
    models: BTreeMap<ModelId, ModelChecked>,
    setups: Vec<SetupChecked>,
    /// Reach notes by operation, with the operation's own status key they
    /// were made for.
    reach: BTreeMap<OperationId, (Key, Vec<String>)>,
}

impl Checks {
    /// The project revision the checks were made at.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

/// What generation has recorded, shared with the pipeline jobs.
#[derive(Debug, Default)]
pub(crate) struct GenerationRecord(Mutex<RecordState>);

#[derive(Debug, Default, Clone)]
struct RecordState {
    generated: BTreeMap<OperationId, Made>,
    simulated: BTreeMap<OperationId, Key>,
    exported: BTreeMap<OperationId, Key>,
    failure: Option<Failure>,
    current: Option<OperationId>,
}

/// What an operation was made from: its keys, and the own keys of the
/// enabled operations before it then, which say which earlier operation made
/// it stale.
#[derive(Debug, Clone)]
struct Made {
    key: StatusKey,
    before: Vec<(OperationId, Key)>,
}

#[derive(Debug, Clone)]
struct Failure {
    operation: Option<(OperationId, Option<Key>)>,
    message: String,
}

impl GenerationRecord {
    fn with<T>(&self, f: impl FnOnce(&mut RecordState) -> T) -> Option<T> {
        self.0.lock().ok().map(|mut state| f(&mut state))
    }

    /// The pipeline is generating `operation` now, or none.
    pub(crate) fn working_on(&self, operation: Option<OperationId>) {
        self.with(|s| s.current = operation);
    }

    /// A pipeline run on `project` succeeded with `run`. Only the operations
    /// in it are recorded: a run up to an operation leaves the later ones as
    /// they were.
    pub(crate) fn succeeded(&self, project: &Project, simulated: bool, run: &crate::Regeneration) {
        let keys = status_keys(project);
        self.with(|s| {
            for (index, (id, key)) in keys.iter().enumerate() {
                if !run.groups.iter().any(|g| g.operation == *id) {
                    continue;
                }
                let before = keys
                    .get(..index)
                    .unwrap_or_default()
                    .iter()
                    .map(|(id, k)| (*id, k.own))
                    .collect();
                s.generated.insert(*id, Made { key: *key, before });
                if simulated {
                    s.simulated.insert(*id, key.chained);
                }
            }
            s.failure = None;
            s.current = None;
        });
    }

    /// A pipeline run on `project` failed with `message`, on the operation
    /// it was working on, if any.
    pub(crate) fn failed(&self, project: &Project, message: String) {
        let keys = status_keys(project);
        self.with(|s| {
            let operation = s.current.take().map(|id| {
                let key = keys.iter().find(|(o, _)| *o == id).map(|(_, k)| k.chained);
                (id, key)
            });
            s.failure = Some(Failure { operation, message });
        });
    }

    /// A pipeline run was cancelled: nothing it did is kept.
    pub(crate) fn cancelled(&self) {
        self.with(|s| s.current = None);
    }

    /// G-code was written from `project`'s operations.
    pub(crate) fn exported(&self, project: &Project) {
        let keys = status_keys(project);
        self.with(|s| {
            s.exported = keys.into_iter().map(|(id, k)| (id, k.chained)).collect();
        });
    }

    fn snapshot(&self) -> RecordState {
        self.with(|s| s.clone()).unwrap_or_default()
    }
}

/// The modification stamp of a file, to tell whether it needs reading again.
fn stamp(path: &Path) -> Option<(u64, SystemTime)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()?))
}

/// Reads what the overview needs from the files (see the module notes).
pub(crate) fn run_checks(
    location: &Path,
    project: &Project,
    revision: u64,
    previous: Option<&Checks>,
    context: &JobContext<'_>,
) -> Result<Checks, AppError> {
    let mut checks = Checks {
        revision,
        ..Checks::default()
    };
    for reference in project.models() {
        context.check_cancelled()?;
        let path = reference.path.resolve(location);
        let now = stamp(&path);
        let reused = previous
            .and_then(|p| p.models.get(&reference.id))
            .filter(|m| m.reference == *reference && m.stamp.is_some() && m.stamp == now);
        let checked = match reused {
            Some(m) => m.clone(),
            None => {
                let file = match reference.check(location) {
                    Ok(ModelCheck::Unchanged) => FileState::Unchanged,
                    Ok(ModelCheck::Missing) => FileState::Missing,
                    Ok(_) => FileState::Changed,
                    Err(e) => FileState::Unreadable(e.to_string()),
                };
                let findings = match file {
                    FileState::Missing | FileState::Unreadable(_) => {
                        Err("the file cannot be read".to_owned())
                    }
                    _ => match crate::model::inspect_model(location, reference, context) {
                        Ok(i) => Ok(ModelFindings {
                            problems: i.report.problems(),
                            unit_question: i.unit_question.map(|q| q.message()),
                            notes: i.notes,
                            extent: i.extent,
                        }),
                        Err(e) if e.is_cancelled() => return Err(e),
                        Err(e) => Err(e.to_string()),
                    },
                };
                ModelChecked {
                    reference: reference.clone(),
                    stamp: now,
                    file,
                    findings,
                }
            }
        };
        checks.models.insert(reference.id, checked);
    }
    for s in project.setups() {
        context.check_cancelled()?;
        let part = s
            .part
            .and_then(|id| checks.models.get(&id))
            .and_then(|m| m.findings.as_ref().ok())
            .and_then(|f| f.extent);
        let mut notes = Vec::new();
        let warnings = match project.machine() {
            Some(profile) => {
                openchuck_types::setup::check_setup(&openchuck_types::setup::SetupCheck {
                    profile,
                    frame: &s.frame,
                    stock: s.stock.as_ref(),
                    clearance: s.clearance.as_ref(),
                    part: part.as_ref(),
                })
            }
            None => Vec::new(),
        };
        let mut warnings = warnings;
        warnings.extend(
            openchuck_types::workholding::check_tab_zones(
                &s.workholding,
                s.stock.as_ref(),
                part.as_ref(),
            )
            .into_iter()
            .map(|kind| SetupWarning {
                kind,
                unconfirmed: Vec::new(),
            }),
        );
        notes.extend(crate::workholding::narrow_tab_notes(project, s));
        let unbuildable = match (&part, s.workholding.is_empty()) {
            (Some(part), false) => {
                crate::workholding::labelled_solids(&s.workholding, s.stock.as_ref(), part)
                    .err()
                    .map(|e| e.to_string())
            }
            _ => None,
        };
        checks.setups.push(SetupChecked {
            setup: s.id,
            warnings,
            notes,
            unbuildable,
            has_machine: project.machine().is_some(),
        });
    }
    let keys = status_keys(project);
    for operation in project.operations() {
        context.check_cancelled()?;
        let Some(own) = keys
            .iter()
            .find(|(id, _)| *id == operation.id)
            .map(|(_, k)| k.own)
        else {
            continue;
        };
        let held = project
            .setup(operation.setup)
            .is_some_and(|s| !s.workholding.is_empty());
        if !held || operation.selections.is_empty() {
            continue;
        }
        let reused = previous
            .and_then(|p| p.reach.get(&operation.id))
            .filter(|(key, _)| *key == own);
        let notes = match reused {
            Some((_, notes)) => notes.clone(),
            None => match crate::workholding::reach_warnings_of(
                location,
                project,
                operation.id,
                context,
            ) {
                Ok(notes) => notes,
                Err(e) if e.is_cancelled() => return Err(e),
                // A part that cannot be read is reported by its model.
                Err(_) => Vec::new(),
            },
        };
        checks.reach.insert(operation.id, (own, notes));
    }
    Ok(checks)
}

/// Why an operation whose own inputs are unchanged is stale: the first
/// enabled operation before it whose own key differs from when it was made
/// ("operation 2 changed", numbered in the whole list), or a change in which
/// operations come before it.
fn earlier_change(
    keys: &[(OperationId, StatusKey)],
    operations: &[openchuck_project::Operation],
    operation: OperationId,
    before: &[(OperationId, Key)],
) -> String {
    let now: Vec<(OperationId, Key)> = keys
        .iter()
        .take_while(|(id, _)| *id != operation)
        .map(|(id, k)| (*id, k.own))
        .collect();
    let same_order =
        now.len() == before.len() && now.iter().zip(before).all(|((a, _), (b, _))| a == b);
    let changed = now
        .iter()
        .zip(before)
        .find(|((_, a), (_, b))| a != b)
        .map(|((id, _), _)| *id);
    match changed {
        Some(id) if same_order => {
            let place = operations
                .iter()
                .position(|o| o.id == id)
                .map_or(0, |i| i + 1);
            format!("operation {place} changed")
        }
        _ => "an earlier operation was added, removed, moved or turned on or off".to_owned(),
    }
}

/// "the setup has no stock" becomes "The setup has no stock."
fn sentence(text: &str) -> String {
    let text = text.trim();
    let mut chars = text.chars();
    let mut out: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => return String::new(),
    };
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

/// Which part of a setup a setup check's finding is about.
fn part_of(kind: &SetupWarningKind) -> Option<SetupPart> {
    match kind {
        SetupWarningKind::NoStock
        | SetupWarningKind::StockNeedsPart
        | SetupWarningKind::ImportedStock
        | SetupWarningKind::PartOutsideStock => Some(SetupPart::Stock),
        SetupWarningKind::ClearanceInsideStock { .. }
        | SetupWarningKind::ClearanceInsidePart { .. }
        | SetupWarningKind::OutsideTravel { .. } => Some(SetupPart::WorkOffset),
        SetupWarningKind::TabOutsideEndZones { .. } => Some(SetupPart::Workholding),
        _ => None,
    }
}

/// A setup warning in the user's words: the library's text, with the machine
/// values it used named as the machine panel names them, not by their names
/// in the profile file (specification section 15).
fn setup_warning_text(warning: &SetupWarning) -> String {
    let mut plain = warning.clone();
    plain.unconfirmed.clear();
    let mut text = plain.to_string();
    if !warning.unconfirmed.is_empty() {
        let names: Vec<&str> = warning
            .unconfirmed
            .iter()
            .map(|f| crate::setup::profile_field_label(*f))
            .collect();
        text.push_str(&format!(
            " (uses machine values not confirmed on the machine: {})",
            names.join(", ")
        ));
    }
    text
}

/// Raises `item`'s state to `status` when it is more serious (error over
/// warning over the rest).
fn raise(items: &mut BTreeMap<ItemRef, ItemStatus>, item: ItemRef, status: ItemStatus) {
    let rank = |s: ItemState| match s {
        ItemState::Error => 2,
        ItemState::Warning => 1,
        _ => 0,
    };
    match items.get(&item) {
        Some(old) if rank(old.state) >= rank(status.state) => {}
        _ => {
            items.insert(item, status);
        }
    }
}

impl Application {
    /// A number that changes whenever the project changes: each command,
    /// undo and redo. A front end works the [`Overview`] out again, and
    /// starts the checks again, when it changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Starts the checks the overview needs from the files, as a job (see
    /// the module notes). Pass the previous checks so that what has not
    /// changed is not read again.
    pub fn start_checks(&self, previous: Option<Arc<Checks>>) -> JobHandle<Checks> {
        let document = self.document.clone();
        let revision = self.revision;
        self.jobs
            .submit("project checks", self.events_tx.clone(), move |context| {
                run_checks(
                    &document.location,
                    &document.project,
                    revision,
                    previous.as_deref(),
                    context,
                )
            })
    }

    /// Records that G-code was written from `run`, which completes the Post
    /// and export step until something its operations read changes.
    pub fn record_export(&self, run: &crate::Regeneration) {
        self.record.exported(&run.project);
    }

    /// Asks the running job `id` to stop (the Jobs tab's Cancel). Returns
    /// false when no such job is running.
    pub fn cancel_job(&self, id: crate::JobId) -> bool {
        self.jobs.cancel(id)
    }

    /// The steps, item states, messages, setup checks and primary actions,
    /// from the project, the latest `checks` (if any), what generation has
    /// recorded and the `jobs` log (see the module notes). Quick: it reads
    /// no file.
    pub fn overview(&self, checks: Option<&Checks>, jobs: &JobLog) -> Overview {
        build(self, checks, jobs)
    }
}

/// A setup's child row in the tree: its words and its chip (specification
/// section 6: "Stock · cylinder Ø26 × 32", the work offset, the
/// workholding's count "1 held · 2 tabs").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupRow {
    /// The row's words.
    pub label: String,
    /// The chip after the name, if any.
    pub chip: Option<String>,
}

/// A length in millimetres with at most three places and no trailing zeros
/// ("26", "12.5").
fn short_mm(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// The row of `part` of `setup` in the tree.
pub fn setup_row(project: &Project, setup: &openchuck_project::Setup, part: SetupPart) -> SetupRow {
    use openchuck_types::stock::Stock;
    let row = |label: String, chip: Option<String>| SetupRow { label, chip };
    match part {
        SetupPart::Machine => row(
            part.name().to_owned(),
            project.machine().map(|m| m.name.clone()),
        ),
        SetupPart::Stock => {
            let what = match &setup.stock {
                None => "none".to_owned(),
                Some(Stock::Cylinder {
                    diameter, length, ..
                }) => format!(
                    "cylinder Ø{} × {}",
                    short_mm(diameter.0),
                    short_mm(length.0)
                ),
                Some(Stock::Box { bounds }) => format!(
                    "box {} × {} × {}",
                    short_mm(bounds.max.x - bounds.min.x),
                    short_mm(bounds.max.y - bounds.min.y),
                    short_mm(bounds.max.z - bounds.min.z)
                ),
                Some(Stock::OffsetBoundingBox { offset }) => {
                    format!("bounds + {} mm", short_mm(offset.0))
                }
                Some(_) => "imported".to_owned(),
            };
            row(format!("Stock · {what}"), None)
        }
        SetupPart::PartPosition => row(part.name().to_owned(), None),
        SetupPart::WorkOffset => row(
            part.name().to_owned(),
            Some(setup.frame.work_offset.register.to_string()),
        ),
        SetupPart::Workholding => {
            let w = &setup.workholding;
            let held = usize::from(w.start.is_some()) + usize::from(w.end.is_some());
            let tabs = w.tabs.len();
            let chip = match (held, tabs) {
                (0, 0) => "None".to_owned(),
                (h, 0) => format!("{h} held"),
                (0, 1) => "1 tab".to_owned(),
                (0, t) => format!("{t} tabs"),
                (h, 1) => format!("{h} held · 1 tab"),
                (h, t) => format!("{h} held · {t} tabs"),
            };
            row(part.name().to_owned(), Some(chip))
        }
    }
}

impl Application {
    /// The tolerance that applies to `operation`, in millimetres, for the
    /// footer (specification section 10); `None` when it has none.
    pub fn operation_tolerance(&self, operation: OperationId) -> Option<f64> {
        let form = self.parameter_form(operation).ok()?;
        let field = form
            .fields
            .iter()
            .find(|f| f.name == openchuck_path::params::names::TOLERANCE)?;
        match field.value.as_ref()? {
            openchuck_project::ParameterValue::Number(n) => Some(*n),
            #[allow(clippy::cast_precision_loss)]
            openchuck_project::ParameterValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }
}

/// The user's name for an operation: its number and name.
fn operation_name(index: usize, name: &str) -> String {
    format!("{} {name}", index + 1)
}

#[allow(clippy::too_many_lines)]
fn build(app: &Application, checks: Option<&Checks>, jobs: &JobLog) -> Overview {
    let project = app.project();
    let record = app.record.snapshot();
    let keys_in_order = status_keys(project);
    let keys: BTreeMap<OperationId, StatusKey> = keys_in_order.iter().copied().collect();
    let summaries: Vec<SelectionSummary> = app.selection_summaries(&file_warnings(project, checks));
    let mut items = BTreeMap::new();
    // Each source's messages, gathered apart so they can be ordered: the
    // newest kind of news first within a severity (decision record 0117).
    let mut messages: Vec<Message> = Vec::new();
    let mut setup_checks = Vec::new();
    let mut blocked_sets = Vec::new();

    // Jobs that failed (other than generation, which is recorded below).
    for name in ["build scene", "project checks", "import model"] {
        if let Some(entry) = jobs.latest_failure(name)
            && let Some(JobOutcome::Failed(e)) = &entry.outcome
        {
            messages.push(Message::new(
                Severity::Error,
                format!("{} failed: {e}", entry.label),
                None,
                None,
            ));
        }
    }

    // Generation.
    let running_pipeline = jobs
        .running()
        .iter()
        .find(|e| matches!(e.name.as_str(), "regenerate" | "simulate" | "post"));
    let ops = project.operations();
    let mut stale = Vec::new();
    let mut first_unsimulated_unmade = None;
    let mut simulated_all = true;
    let mut any_enabled = false;
    let mut any_generated = false;
    let mut exported_all = true;
    for (index, o) in ops.iter().enumerate() {
        let item = ItemRef::Operation(o.id);
        let label = operation_name(index, &o.name);
        if !o.enabled {
            items.insert(
                item,
                ItemStatus::new(
                    ItemState::Off,
                    "turned off, so it is left out of generation, simulation and G-code",
                ),
            );
            continue;
        }
        any_enabled = true;
        let key = keys.get(&o.id);
        let made = record.generated.get(&o.id);
        any_generated |= made.is_some();
        let up_to_date = matches!((key, made), (Some(k), Some(m)) if k.chained == m.key.chained);
        let simulated =
            matches!((key, record.simulated.get(&o.id)), (Some(k), Some(s)) if k.chained == *s);
        simulated_all &= simulated;
        exported_all &=
            matches!((key, record.exported.get(&o.id)), (Some(k), Some(s)) if k.chained == *s);
        if !simulated && !up_to_date && first_unsimulated_unmade.is_none() {
            first_unsimulated_unmade = Some(index + 1);
        }
        let mut status = if up_to_date {
            ItemStatus::new(
                ItemState::UpToDate,
                "generated, and nothing it reads has changed since",
            )
        } else {
            stale.push(o.id);
            let cause = match (made, key) {
                (None, _) | (_, None) => "not generated yet".to_owned(),
                (Some(m), Some(k)) if m.key.own != k.own => {
                    "it, or its setup, tool, model or selection sets, changed".to_owned()
                }
                (Some(m), Some(_)) => earlier_change(&keys_in_order, ops, o.id, &m.before),
            };
            ItemStatus::new(ItemState::Stale, cause)
        };
        if record.current == Some(o.id) && running_pipeline.is_some() {
            status = ItemStatus::new(ItemState::Running, "generating");
            status.progress = running_pipeline.and_then(|e| e.fraction());
        }
        // A selection set with a problem makes its operations' error too
        // (specification section 6, "Problems go up the tree").
        if let Some(set) = summaries
            .iter()
            .find(|s| s.problem.is_some() && s.uses.iter().any(|u| u.operation == o.id))
        {
            status = ItemStatus::new(
                ItemState::Error,
                format!(
                    "the selection set '{}' cannot be used: {}",
                    set.name,
                    set.problem.clone().unwrap_or_default()
                ),
            );
            blocked_sets.push(ItemRef::SelectionSet(set.id));
        }
        if let Some(Failure {
            operation: Some((id, key_then)),
            message,
        }) = &record.failure
            && *id == o.id
            && key_then.is_some()
            && *key_then == key.map(|k| k.chained)
        {
            status = ItemStatus::new(ItemState::Error, sentence(message));
            messages.push(Message::new(
                Severity::Error,
                message.clone(),
                Some((item, label.clone())),
                Some(MessageAction::Show(item)),
            ));
        }
        if let Some((_, notes)) = checks.and_then(|c| c.reach.get(&o.id)) {
            for note in notes {
                messages.push(Message::new(
                    Severity::Warning,
                    note.clone(),
                    Some((item, label.clone())),
                    Some(MessageAction::EditWorkholding(o.setup)),
                ));
            }
        }
        items.insert(item, status);
    }
    if let Some(Failure {
        operation: None,
        message,
    }) = &record.failure
    {
        messages.push(Message::new(
            Severity::Error,
            format!("Generating failed: {message}"),
            None,
            Some(MessageAction::Generate),
        ));
    }

    // Selection sets.
    for s in &summaries {
        if let Some(problem) = &s.problem {
            let item = ItemRef::SelectionSet(s.id);
            items.insert(
                item,
                ItemStatus::new(
                    ItemState::Error,
                    format!("{problem}. Reselect it in the view"),
                ),
            );
            messages.push(Message::new(
                Severity::Error,
                format!("The selection set '{}' cannot be used: {problem}", s.name),
                Some((item, s.name.clone())),
                Some(MessageAction::Reselect(s.id)),
            ));
        }
    }

    // Setups and the machine.
    let machine_warnings: Vec<String> = project
        .machine()
        .map(|m| {
            m.unconfirmed_fields()
                .into_iter()
                .map(|f| {
                    let name = crate::setup::profile_field_label(f);
                    let mut chars = name.chars();
                    let name: String = chars
                        .next()
                        .map(|c| c.to_uppercase().chain(chars).collect())
                        .unwrap_or_default();
                    format!("{name} unconfirmed")
                })
                .collect()
        })
        .unwrap_or_default();
    for s in project.setups() {
        let setup = ItemRef::Setup(s.id);
        let at = |part| ItemRef::SetupPart(s.id, part);
        match project.machine() {
            None => {
                let text = "The project has no machine profile, so the setup is not checked. Choose one in the setup's machine section.";
                raise(
                    &mut items,
                    at(SetupPart::Machine),
                    ItemStatus::new(ItemState::Warning, "no machine profile is chosen"),
                );
                messages.push(Message::new(
                    Severity::Warning,
                    text,
                    Some((at(SetupPart::Machine), s.name.clone())),
                    Some(MessageAction::ReviewSetup(s.id)),
                ));
                setup_checks.push(SetupCheck {
                    setup: s.id,
                    passed: false,
                    text: sentence(text),
                    action: Some(MessageAction::ReviewSetup(s.id)),
                });
            }
            Some(m) => {
                let unconfirmed = m.unconfirmed_fields();
                if !unconfirmed.is_empty() {
                    let names: Vec<&str> = unconfirmed
                        .iter()
                        .map(|f| crate::setup::profile_field_label(*f))
                        .collect();
                    raise(
                        &mut items,
                        at(SetupPart::Machine),
                        ItemStatus::new(
                            ItemState::Warning,
                            format!("not confirmed on the machine: {}", names.join(", ")),
                        ),
                    );
                }
                if unconfirmed.contains(&ProfileField::RotaryOrigin) {
                    // OI-60: the first check while the axis height is a
                    // placeholder (specification section 11.3).
                    setup_checks.push(SetupCheck {
                        setup: s.id,
                        passed: false,
                        text: "Safe Z cannot be checked against the Z travel: the machine's axis height is not confirmed on the machine.".to_owned(),
                        action: Some(MessageAction::ReviewSetup(s.id)),
                    });
                }
            }
        }
        if s.stock.is_none() {
            raise(
                &mut items,
                at(SetupPart::Stock),
                ItemStatus::new(ItemState::Warning, "the setup has no stock"),
            );
        }
        if let Some(found) = checks.and_then(|c| c.setups.iter().find(|c| c.setup == s.id)) {
            for w in &found.warnings {
                let part = part_of(&w.kind);
                let target = part.map_or(setup, at);
                let text = sentence(&setup_warning_text(w));
                raise(
                    &mut items,
                    target,
                    ItemStatus::new(ItemState::Warning, text.clone()),
                );
                let action = if part == Some(SetupPart::Workholding) {
                    MessageAction::EditWorkholding(s.id)
                } else {
                    MessageAction::ReviewSetup(s.id)
                };
                messages.push(Message::new(
                    Severity::Warning,
                    text.clone(),
                    Some((target, s.name.clone())),
                    Some(action),
                ));
                setup_checks.push(SetupCheck {
                    setup: s.id,
                    passed: false,
                    text,
                    action: Some(action),
                });
            }
            for note in &found.notes {
                let text = sentence(note);
                raise(
                    &mut items,
                    at(SetupPart::Workholding),
                    ItemStatus::new(ItemState::Warning, text.clone()),
                );
                messages.push(Message::new(
                    Severity::Warning,
                    text.clone(),
                    Some((at(SetupPart::Workholding), s.name.clone())),
                    Some(MessageAction::EditWorkholding(s.id)),
                ));
                setup_checks.push(SetupCheck {
                    setup: s.id,
                    passed: false,
                    text,
                    action: Some(MessageAction::EditWorkholding(s.id)),
                });
            }
            if let Some(reason) = &found.unbuildable {
                let text = format!(
                    "The stored workholding cannot be built, so operations of this setup cannot be generated: {reason}"
                );
                raise(
                    &mut items,
                    at(SetupPart::Workholding),
                    ItemStatus::new(ItemState::Error, sentence(reason)),
                );
                messages.push(Message::new(
                    Severity::Error,
                    text,
                    Some((at(SetupPart::Workholding), s.name.clone())),
                    Some(MessageAction::EditWorkholding(s.id)),
                ));
            }
            if found.has_machine && found.warnings.is_empty() {
                setup_checks.push(SetupCheck {
                    setup: s.id,
                    passed: true,
                    text: "The stock, the clearance and the travel raise no warnings.".to_owned(),
                    action: None,
                });
            }
        }
        // The setup row shows the worst of its rows (problems go up the
        // tree).
        let worst = SetupPart::ALL
            .into_iter()
            .filter_map(|p| items.get(&at(p)).cloned())
            .max_by_key(|status| match status.state {
                ItemState::Error => 2,
                ItemState::Warning => 1,
                _ => 0,
            });
        if let Some(worst) = worst {
            raise(&mut items, setup, worst);
        }
    }

    // Models.
    let mut model_error = false;
    for m in project.models() {
        let item = ItemRef::Model(m.id);
        let name = m.path.as_str().to_owned();
        let Some(found) = checks.and_then(|c| c.models.get(&m.id)) else {
            continue;
        };
        match &found.file {
            FileState::Missing => {
                model_error = true;
                items.insert(
                    item,
                    ItemStatus::new(ItemState::Error, "the file is missing"),
                );
                messages.push(Message::new(
                    Severity::Error,
                    format!("'{name}' is missing. Put the file back, or remove the model and import it again"),
                    Some((item, name.clone())),
                    Some(MessageAction::Show(item)),
                ));
            }
            FileState::Unreadable(e) => {
                model_error = true;
                items.insert(
                    item,
                    ItemStatus::new(ItemState::Error, format!("the file cannot be read: {e}")),
                );
                messages.push(Message::new(
                    Severity::Error,
                    format!("'{name}' cannot be read: {e}"),
                    Some((item, name.clone())),
                    Some(MessageAction::Show(item)),
                ));
            }
            FileState::Changed => {
                raise(
                    &mut items,
                    item,
                    ItemStatus::new(
                        ItemState::Warning,
                        "the file has changed since it was added",
                    ),
                );
                messages.push(Message::new(
                    Severity::Warning,
                    format!("'{name}' has changed since it was added. The toolpaths will use the file as it is now"),
                    Some((item, name.clone())),
                    Some(MessageAction::Show(item)),
                ));
            }
            FileState::Unchanged => {}
        }
        match &found.findings {
            Ok(f) => {
                if !f.problems.is_empty() {
                    raise(
                        &mut items,
                        item,
                        ItemStatus::new(
                            ItemState::Warning,
                            format!("mesh check: {}", f.problems.join("; ")),
                        ),
                    );
                    messages.push(Message::new(
                        Severity::Warning,
                        format!("The mesh check found: {}", f.problems.join("; ")),
                        Some((item, name.clone())),
                        Some(MessageAction::Show(item)),
                    ));
                }
                if let Some(q) = &f.unit_question {
                    raise(
                        &mut items,
                        item,
                        ItemStatus::new(ItemState::Warning, q.clone()),
                    );
                    messages.push(Message::new(
                        Severity::Warning,
                        q.clone(),
                        Some((item, name.clone())),
                        Some(MessageAction::Show(item)),
                    ));
                }
                // The file check above already says when the file changed.
                for note in f
                    .notes
                    .iter()
                    .filter(|n| !n.contains("has changed since it was added"))
                {
                    messages.push(Message::new(
                        Severity::Info,
                        note.clone(),
                        Some((item, name.clone())),
                        None,
                    ));
                }
            }
            Err(e) if matches!(found.file, FileState::Unchanged | FileState::Changed) => {
                model_error = true;
                items.insert(
                    item,
                    ItemStatus::new(ItemState::Error, format!("it cannot be read: {e}")),
                );
                messages.push(Message::new(
                    Severity::Error,
                    format!("'{name}' cannot be imported: {e}"),
                    Some((item, name.clone())),
                    Some(MessageAction::Show(item)),
                ));
            }
            Err(_) => {}
        }
    }
    // Running jobs on the project's items.
    if let Some(entry) = jobs.running().iter().find(|e| e.name == "build scene") {
        for m in project.models() {
            let item = ItemRef::Model(m.id);
            if !items
                .get(&item)
                .is_some_and(|s| s.state == ItemState::Error)
            {
                let mut status = ItemStatus::new(ItemState::Running, "building the view");
                status.progress = entry.fraction();
                items.insert(item, status);
            }
        }
    }

    // Order: errors, warnings, information; the sort is stable, so each
    // keeps the order above.
    messages.sort_by_key(|m| m.severity);

    // Steps (specification section 4).
    let has_model = !project.models().is_empty();
    let has_machine_and_stock =
        project.machine().is_some() && project.setups().iter().any(|s| s.stock.is_some());
    let setup_complete = project.machine().is_some()
        && !project.setups().is_empty()
        && project
            .setups()
            .iter()
            .all(|s| s.stock.is_some() && s.part.is_some())
        && checks.is_some_and(|c| c.setups.iter().all(|s| s.warnings.is_empty()));
    let generated = any_enabled && any_generated;
    let state = |step, unavailable: Option<&str>, complete| StepState {
        step,
        unavailable: unavailable.map(str::to_owned),
        complete,
    };
    let steps = vec![
        state(Step::Model, None, has_model && !model_error),
        state(
            Step::Setup,
            (!has_model).then_some("Import a model first"),
            setup_complete,
        ),
        state(
            Step::Operations,
            (!has_machine_and_stock).then_some("Set up the machine and stock first"),
            any_enabled && stale.is_empty(),
        ),
        state(
            Step::Simulate,
            (!generated).then_some("Generate an operation first"),
            any_enabled && simulated_all,
        ),
        state(
            Step::Export,
            (!generated).then_some("Generate an operation first"),
            any_enabled && exported_all,
        ),
    ];
    Overview {
        revision: app.revision,
        steps,
        items,
        messages,
        setup_checks,
        stale,
        machine_warnings,
        checks_current: checks.is_some_and(|c| c.revision == app.revision),
        simulated_all: any_enabled && simulated_all,
        first_unsimulated_unmade,
        any_enabled,
        models: project.models().len(),
        blocked_sets,
    }
}

/// The file warnings the selection summaries compare against, in the words
/// [`Application::model_file_warnings`] uses, from the checks rather than by
/// reading the files again.
fn file_warnings(project: &Project, checks: Option<&Checks>) -> Vec<String> {
    let Some(checks) = checks else {
        return Vec::new();
    };
    project
        .models()
        .iter()
        .filter_map(|m| {
            let path = m.path.as_str();
            match &checks.models.get(&m.id)?.file {
                FileState::Unchanged => None,
                FileState::Missing => Some(format!("'{path}' is missing")),
                FileState::Changed => Some(format!("'{path}' has changed since it was added")),
                FileState::Unreadable(e) => Some(format!("'{path}' cannot be checked: {e}")),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_11_lengths_on_rows_have_no_trailing_zeros() {
        assert_eq!(short_mm(26.0), "26");
        assert_eq!(short_mm(12.5), "12.5");
        assert_eq!(short_mm(0.1234), "0.123");
    }

    #[test]
    fn req_ui_12_messages_are_plain_sentences() {
        assert_eq!(
            sentence("the setup has no stock"),
            "The setup has no stock."
        );
        assert_eq!(sentence("Done."), "Done.");
        assert_eq!(sentence("  "), "");
    }

    #[test]
    fn req_ui_10_steps_are_numbered_in_pipeline_order() {
        let names: Vec<_> = Step::ALL.iter().map(|s| (s.number(), s.name())).collect();
        assert_eq!(
            names,
            [
                (1, "Model"),
                (2, "Setup"),
                (3, "Operations"),
                (4, "Simulate"),
                (5, "Post and export")
            ]
        );
        assert_eq!(Step::Simulate.next(), Some(Step::Export));
        assert_eq!(Step::Export.next(), None);
    }

    #[test]
    fn req_ui_12_every_action_has_its_words() {
        let set = SelectionSetId::new();
        let setup = SetupId::new();
        let labels: Vec<_> = [
            MessageAction::Show(ItemRef::Setup(setup)),
            MessageAction::Reselect(set),
            MessageAction::ReviewSetup(setup),
            MessageAction::EditWorkholding(setup),
            MessageAction::Generate,
        ]
        .iter()
        .map(|a| a.label())
        .collect();
        assert_eq!(
            labels,
            [
                "Show",
                "Reselect",
                "Review setup",
                "Edit workholding",
                "Generate"
            ]
        );
        let severities: Vec<_> = Severity::ALL.iter().map(|s| s.name()).collect();
        assert_eq!(severities, ["Errors", "Warnings", "Information"]);
        let names: Vec<_> = SetupPart::ALL.iter().map(|p| p.name()).collect();
        assert_eq!(
            names,
            [
                "Machine profile",
                "Stock",
                "Part position",
                "Work offset and clearance",
                "Workholding"
            ]
        );
    }

    #[test]
    fn req_ui_10_with_no_project_only_the_model_step_is_open() {
        let o = Overview::no_project();
        assert!(o.step(Step::Model).is_available());
        for step in &Step::ALL[1..] {
            assert!(!o.step(*step).is_available());
        }
        assert_eq!(o.primary(Step::Model).kind, PrimaryKind::ImportModel);
        assert_eq!(o.primary(Step::Model).label, "Import model…");
    }

    #[test]
    fn req_ui_12_a_pushed_message_keeps_the_severity_order() {
        let mut o = Overview::no_project();
        o.push(Message::new(Severity::Info, "a note", None, None));
        o.push(Message::new(Severity::Error, "broken", None, None));
        o.push(Message::new(Severity::Warning, "careful", None, None));
        let order: Vec<_> = o.messages.iter().map(|m| m.severity).collect();
        assert_eq!(order, [Severity::Error, Severity::Warning, Severity::Info]);
        assert_eq!(o.count(Severity::Warning), 1);
    }
}

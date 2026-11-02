// SPDX-License-Identifier: AGPL-3.0-or-later
//! Recovery presentation; storage and jobs remain in the application layer.
//!
//! Recovery is offered only for the project being opened (PRJ-08; decision
//! 0460, amending 0243 items 7 and 8). Startup discovery never opens a
//! dialog: another project's kept recovery data is one line in Messages,
//! whose action opens that project (or locates a moved one).
use super::{Next, Shell};
use eframe::egui::{self, Id};
use openchuck_app::overview::{Message, MessageAction, Severity};
use openchuck_app::project::recovery::{Catalog, Kind};
use openchuck_app::{
    Application, Threads,
    checkpoints::{self as jobs, Candidate, Discovery, Notice, Session},
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, UNIX_EPOCH};

// Keep the calendar display bounded and avoid platform-dependent SystemTime
// overflow from untrusted metadata. The raw timestamp and snapshot stay intact.
fn capture_time(milliseconds: u64) -> String {
    if milliseconds > 253_402_300_799_999 {
        return "Capture time outside supported calendar".into();
    }
    UNIX_EPOCH
        .checked_add(Duration::from_millis(milliseconds))
        .map(openchuck_app::iso8601_utc)
        .unwrap_or_else(|| "Capture time unavailable on this platform".into())
}

/// A folder as the user knows it: Windows' canonical `\\?\` prefix is an
/// implementation detail, not part of the name they chose (decision 0114).
pub(super) fn shown(folder: &Path) -> String {
    let text = folder.display().to_string();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        text
    }
}

enum Read {
    Startup(Discovery),
    Attached(u64, Candidate),
    Unopened(Key, Candidate),
    Located(Box<Locate>, Result<Candidate, String>),
    Restored(Result<jobs::Recovered, String>),
}
#[derive(Clone, Copy)]
enum Choice {
    Recover,
    Keep,
    Discard,
}
/// The open request an offer belongs to; a later open drops it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    /// The project opened, as open number `0`.
    Opened(u64),
    /// The requested project's saved files could not be loaded (open number
    /// `0`, failed request `1`): its valid checkpoints are still offered, and
    /// the project that stays open keeps its writer (decision 0460 item 2).
    Unopened(u64, u64),
}

/// The recovery choices of the project being opened.
pub(super) struct Review {
    pub(super) candidate: Candidate,
    /// The open this offer belongs to; a later open drops it.
    key: Key,
    selected: Option<String>,
    confirm_discard: bool,
}
/// Finding the new folder of a moved project, from its Messages line.
pub(super) struct Locate {
    candidate: Candidate,
    pub(super) path: String,
    pub(super) problem: Option<String>,
}

#[derive(Default)]
pub(super) struct State {
    config: Option<PathBuf>,
    readers: Vec<Receiver<Read>>,
    /// The opened project's recovery choices, waiting for a free frame.
    offer: Option<(Key, Candidate)>,
    pub(super) review: Option<Review>,
    review_session: Option<Session>,
    review_folder: Option<PathBuf>,
    install_review: Option<Review>,
    awaiting_install: bool,
    choice: Option<Choice>,
    pub(super) active: Option<Session>,
    active_folder: Option<PathBuf>,
    pending_attach: Option<PathBuf>,
    attachment: u64,
    /// Requests whose saved project could not be loaded.
    failed: u64,
    /// Canonical folder of the opened project, once inspected.
    opened: Option<PathBuf>,
    /// Other projects with kept recovery data, from startup discovery.
    pub(super) elsewhere: Vec<Candidate>,
    pub(super) locate: Option<Locate>,
    /// The Messages lines changed.
    pub(super) changed: bool,
    retiring: Vec<Session>,
    close_next: Option<Next>,
    pub(super) problem: Option<String>,
}

pub(super) enum Effect {
    Recovered(PathBuf, Box<openchuck_app::project::Project>, Vec<String>),
    Resume(Next),
}

/// Whether a project's recovery data needs the user: unexpected-session
/// choices, damaged files or a folder that cannot be found.
fn needs_attention(candidate: &Candidate) -> bool {
    candidate.problem.is_some()
        || candidate
            .catalog
            .as_ref()
            .is_some_and(|c| c.recovery_needed || !c.problems.is_empty())
}

impl State {
    pub(super) fn new(config: Option<PathBuf>) -> Self {
        let mut state = Self {
            config,
            ..Self::default()
        };
        if let Some(config) = state.config.clone() {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(Read::Startup(jobs::discover(&config)));
            });
            state.readers.push(rx);
        }
        state
    }

    pub(super) fn attach(&mut self, folder: &Path, previous_revision: u64, _minutes: u16) {
        self.attachment = self.attachment.saturating_add(1);
        self.pending_attach = None;
        // An offer belongs to the open that found it, never to a later one.
        self.offer = None;
        self.opened = None;
        if let Some(mut previous) = self.active.take() {
            match previous.close(previous_revision) {
                Ok(()) => self.retiring.push(previous),
                Err(e) => self.problem = Some(e),
            }
        }
        self.active_folder = None;
        if self.review_session.is_some() && self.review_folder.as_deref() == Some(folder) {
            self.active = self.review_session.take();
            self.review_folder = None;
            self.active_folder = Some(folder.to_owned());
            self.opened = Some(folder.to_owned());
            return;
        }
        self.review = None;
        self.choice = None;
        self.review_session = None;
        self.review_folder = None;
        if self.config.is_none() {
            return;
        }
        self.pending_attach = Some(folder.to_owned());
    }

    fn inspect(&mut self, folder: PathBuf) {
        let attachment = self.attachment;
        self.read_folder(folder, move |candidate| {
            Read::Attached(attachment, candidate)
        });
    }

    /// The requested project's saved files could not be loaded. Its recovery
    /// data is still inspected and offered for this request only; the
    /// project that is open, if any, stays open with its writer.
    pub(super) fn open_failed(&mut self, folder: &Path) {
        if self.config.is_none() {
            return;
        }
        self.failed = self.failed.saturating_add(1);
        let key = Key::Unopened(self.attachment, self.failed);
        self.offer = None;
        self.read_folder(folder.to_owned(), move |candidate| {
            Read::Unopened(key, candidate)
        });
    }

    fn current(&self, key: Key) -> bool {
        match key {
            Key::Opened(attachment) => attachment == self.attachment,
            Key::Unopened(attachment, failed) => {
                attachment == self.attachment && failed == self.failed
            }
        }
    }

    fn read_folder(
        &mut self,
        folder: PathBuf,
        read: impl FnOnce(Candidate) -> Read + Send + 'static,
    ) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let canonical = folder.canonicalize().unwrap_or(folder);
            let (catalog, problem) = match Catalog::read(&canonical) {
                Ok(c) => (Some(c), None),
                Err(e) => (None, Some(e.to_string())),
            };
            let project_id = catalog
                .as_ref()
                .and_then(|c| c.project_id.clone())
                .unwrap_or_default();
            let _ = tx.send(read(Candidate {
                folder: canonical,
                catalog,
                problem,
                project_id,
            }));
        });
        self.readers.push(rx);
    }

    fn start(&mut self, folder: PathBuf, minutes: u16) {
        let Some(config) = &self.config else {
            return;
        };
        match Session::open(folder.clone(), config.clone(), minutes) {
            Ok(session) => {
                self.active = Some(session);
                self.active_folder = Some(folder);
            }
            Err(e) => self.problem = Some(e),
        }
    }

    /// Drop another project's line once that project is the open one.
    fn forget_elsewhere(&mut self, folder: &Path) {
        let before = self.elsewhere.len();
        self.elsewhere.retain(|c| c.folder != folder);
        self.changed |= self.elsewhere.len() != before;
    }

    pub(super) fn saved(&mut self, revision: u64) {
        if let Some(session) = &self.active
            && let Err(e) = session.saved(revision)
        {
            self.problem = Some(e);
        }
    }

    pub(super) fn close(&mut self, next: Next, revision: u64) -> bool {
        self.pending_attach = None;
        self.readers.clear();
        if self.close_next.is_some() {
            return true;
        }
        if let Some(mut session) = self.active.take() {
            match session.close(revision) {
                Ok(()) => self.retiring.push(session),
                Err(e) => self.problem = Some(e),
            }
        }
        self.active_folder = None;
        if self.retiring.is_empty() {
            false
        } else {
            self.close_next = Some(next);
            true
        }
    }

    pub(super) fn blocks_input(&self) -> bool {
        self.review.is_some() || self.locate.is_some() || self.close_next.is_some()
    }

    pub(super) fn is_closing(&self) -> bool {
        self.close_next.is_some()
    }

    pub(super) fn finish_install(&mut self) {
        self.awaiting_install = false;
        self.install_review = None;
    }

    pub(super) fn cancel_install(&mut self, folder: &Path) {
        self.awaiting_install = false;
        if self.review_folder.as_deref() == Some(folder) {
            self.review_session = None;
            self.review_folder = None;
        }
        if let Some(review) = self.install_review.take()
            && self.current(review.key)
        {
            self.offer = Some((review.key, review.candidate));
        }
    }

    /// Whether a background read is still running.
    #[cfg(test)]
    pub(super) fn reading(&self) -> bool {
        !self.readers.is_empty()
    }

    /// Put a shown offer back, waiting for a free frame (tests).
    #[cfg(test)]
    pub(super) fn offer_again(&mut self, review: Review) {
        self.offer = Some((review.key, review.candidate));
    }

    /// The folder of another project's Messages line.
    pub(super) fn elsewhere_folder(&self, index: usize) -> Option<PathBuf> {
        self.elsewhere.get(index).map(|c| c.folder.clone())
    }

    /// Ask where a moved project went (its Messages line's action).
    pub(super) fn start_locate(&mut self, index: usize) {
        if let Some(c) = self.elsewhere.get(index) {
            self.locate = Some(Locate {
                candidate: Candidate {
                    folder: c.folder.clone(),
                    catalog: None,
                    problem: c.problem.clone(),
                    project_id: c.project_id.clone(),
                },
                path: String::new(),
                problem: None,
            });
        }
    }

    /// One Messages line per other project with kept recovery data. Nothing
    /// is offered from here: the action opens (or locates) that project.
    pub(super) fn messages(&self) -> Vec<Message> {
        self.elsewhere
            .iter()
            .enumerate()
            .map(|(index, c)| {
                let folder = shown(&c.folder);
                let name = c
                    .catalog
                    .as_ref()
                    .and_then(|catalog| catalog.snapshots.first())
                    .map(|e| e.project_name.clone());
                let (text, action) = match (&c.problem, &c.catalog) {
                    (Some(_), _) | (None, None) => (
                        format!(
                            "Recovery data was kept for a project last in {folder}, but it is not there now. If you moved the project, locate its new folder"
                        ),
                        MessageAction::LocateRecovery(index),
                    ),
                    (None, Some(catalog)) if catalog.recovery_needed => (
                        format!(
                            "Unsaved changes to '{}' in {folder} were kept when it closed unexpectedly. Open that project to recover or discard them",
                            name.unwrap_or_else(|| "a project".into())
                        ),
                        MessageAction::OpenRecovery(index),
                    ),
                    (None, Some(_)) => (
                        format!(
                            "Some recovery files for the project in {folder} are damaged. They were kept; open that project to see them"
                        ),
                        MessageAction::OpenRecovery(index),
                    ),
                };
                Message::new(Severity::Warning, text, None, Some(action))
            })
            .collect()
    }

    fn choose(&mut self, choice: Choice, minutes: u16) {
        let Some(review) = &self.review else {
            return;
        };
        if !self.current(review.key) {
            self.review = None;
            return;
        }
        if matches!(choice, Choice::Recover) {
            let Some(id) = review.selected.clone() else {
                return;
            };
            let folder = review.candidate.folder.clone();
            let identity = review.candidate.project_id.clone();
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(Read::Restored(jobs::recover(&folder, &id, &identity)));
            });
            self.readers.push(rx);
            self.choice = Some(choice);
            return;
        }
        if self.review_session.is_none() {
            let Some(config) = &self.config else {
                return;
            };
            match Session::open(review.candidate.folder.clone(), config.clone(), minutes) {
                Ok(session) => {
                    self.review_session = Some(session);
                    self.review_folder = Some(review.candidate.folder.clone());
                }
                Err(e) => {
                    self.problem = Some(e);
                    return;
                }
            }
        }
        let Some(session) = &self.review_session else {
            return;
        };
        let sent = if matches!(choice, Choice::Discard) {
            session.discard()
        } else {
            session.dismiss()
        };
        match sent {
            Ok(()) => self.choice = Some(choice),
            Err(e) => self.problem = Some(e),
        }
    }

    fn receive(&mut self, read: Read, minutes: u16, effects: &mut Vec<Effect>) {
        match read {
            Read::Startup(found) => {
                if !found.problems.is_empty() {
                    self.problem = Some(found.problems.join("\n"));
                }
                let opened = self.opened.clone();
                self.elsewhere = found
                    .projects
                    .into_iter()
                    .filter(|c| needs_attention(c) && Some(&c.folder) != opened.as_ref())
                    .collect();
                self.changed = true;
            }
            Read::Attached(attachment, candidate) => {
                if attachment != self.attachment {
                    return;
                }
                self.opened = Some(candidate.folder.clone());
                self.forget_elsewhere(&candidate.folder);
                let catalog = candidate.catalog.as_ref();
                if let Some(original) = catalog.and_then(|c| c.copied_from.clone()) {
                    self.problem = Some(format!(
                        "This project folder is a copy of {}. The recovery data copied with it belongs to that project, so it is not offered here; it is kept in a folder named .openchuck-recovery-copied-… inside this project folder.",
                        shown(&original)
                    ));
                    if self.active.is_none() {
                        self.start(candidate.folder, minutes);
                    }
                } else if candidate.problem.is_some() {
                    self.problem = candidate.problem;
                } else if catalog.is_some_and(|c| c.live) {
                    self.problem=Some("Another application instance owns this project's checkpoints. Recovery writing is unavailable here.".into());
                } else if catalog.is_some_and(|c| c.recovery_needed) {
                    self.offer = Some((Key::Opened(attachment), candidate));
                } else if self.active.is_none()
                    && self
                        .review
                        .as_ref()
                        .is_none_or(|r| r.candidate.folder != candidate.folder)
                {
                    if let Some(problems) = catalog.map(|c| &c.problems)
                        && !problems.is_empty()
                    {
                        self.problem = Some(problems.join("\n"));
                    }
                    self.start(candidate.folder, minutes);
                }
            }
            Read::Unopened(key, candidate) => {
                // Only this request's own valid checkpoints; nothing starts
                // writing, and the open project is left as it is.
                if self.current(key)
                    && candidate.problem.is_none()
                    && candidate
                        .catalog
                        .as_ref()
                        .is_some_and(|c| c.recovery_needed && !c.live)
                {
                    self.forget_elsewhere(&candidate.folder);
                    self.offer = Some((key, candidate));
                }
            }
            Read::Located(mut locate, result) => match result {
                Ok(found) => {
                    let old = locate.candidate;
                    self.elsewhere
                        .retain(|c| c.folder != old.folder || c.project_id != old.project_id);
                    if needs_attention(&found) && self.opened.as_ref() != Some(&found.folder) {
                        self.elsewhere.push(found);
                    }
                    self.changed = true;
                }
                Err(e) => {
                    locate.problem = Some(e);
                    self.locate = Some(*locate);
                }
            },
            Read::Restored(result) => {
                self.choice = None;
                let current = self.review.as_ref().is_some_and(|r| self.current(r.key));
                match result {
                    Ok(recovered) if current => {
                        self.install_review = self.review.take();
                        self.awaiting_install = true;
                        if let Some(config) = &self.config {
                            match Session::open(recovered.folder.clone(), config.clone(), minutes) {
                                Ok(session) => {
                                    self.review_session = Some(session);
                                    self.review_folder = Some(recovered.folder.clone());
                                }
                                Err(e) => self.problem = Some(e),
                            }
                        }
                        effects.push(Effect::Recovered(
                            recovered.folder,
                            recovered.project,
                            recovered.warnings,
                        ));
                    }
                    Ok(_) => self.review = None,
                    Err(e) => self.problem = Some(e),
                }
            }
        }
    }

    /// The Keep or Discard answer was stored: the opened project stays open
    /// and its writer carries on. Never open another project from here.
    fn review_answered(&mut self, notices: Vec<Notice>) {
        for notice in notices {
            match notice {
                Notice::Saved if self.choice.is_some() => {
                    self.choice = None;
                    let review = self.review.take();
                    let session = self.review_session.take();
                    self.review_folder = None;
                    // A project whose saved files could not be loaded is
                    // not open: its answer is stored, nothing else changes.
                    if let Some(review) = review
                        && review.key == Key::Opened(self.attachment)
                        && self.active.is_none()
                    {
                        self.active = session;
                        self.active_folder = Some(review.candidate.folder);
                    }
                    return;
                }
                Notice::Problem(e) => {
                    self.problem = Some(format!("Recovery checkpoint failed: {e}"));
                    if matches!(self.choice, Some(Choice::Keep | Choice::Discard)) {
                        self.review = None;
                    }
                    self.choice = None;
                    self.review_session = None;
                    self.review_folder = None;
                    return;
                }
                _ => {}
            }
        }
    }

    pub(super) fn frame(
        &mut self,
        ctx: &egui::Context,
        app: Option<&Application>,
        minutes: u16,
        allow_offer: bool,
    ) -> Vec<Effect> {
        if self.retiring.is_empty()
            && let Some(folder) = self.pending_attach.take()
        {
            self.inspect(folder);
        }
        let mut effects = Vec::new();
        let mut received = Vec::new();
        self.readers.retain(|rx| match rx.try_recv() {
            Ok(read) => {
                received.push(read);
                false
            }
            Err(mpsc::TryRecvError::Empty) => true,
            Err(mpsc::TryRecvError::Disconnected) => false,
        });
        for read in received {
            self.receive(read, minutes, &mut effects);
        }
        if self.review.is_none()
            && self.locate.is_none()
            && !self.awaiting_install
            && allow_offer
            && let Some((key, candidate)) = self.offer.take()
            && self.current(key)
        {
            self.review = Some(Review {
                candidate,
                key,
                selected: None,
                confirm_discard: false,
            });
        }
        self.retiring.retain_mut(|session| {
            let mut closed = false;
            for notice in session.poll(session.elapsed()) {
                match notice {
                    Notice::Closed { problem } => {
                        closed = true;
                        if problem.is_some() {
                            self.problem = problem;
                        }
                    }
                    Notice::Problem(e) => self.problem = Some(e),
                    _ => {}
                }
            }
            !closed
        });
        if self.retiring.is_empty()
            && let Some(next) = self.close_next.take()
        {
            effects.push(Effect::Resume(next));
        }
        if let Some(session) = &mut self.active {
            if let Err(e) = session.configure(minutes) {
                self.problem = Some(e);
            }
            for notice in session.poll(session.elapsed()) {
                if let Notice::Problem(e) = notice {
                    self.problem = Some(format!("Recovery checkpoint failed: {e}"));
                }
            }
            if let Some(app) = app {
                session.tick(
                    session.elapsed(),
                    app.project(),
                    app.revision(),
                    app.is_modified(),
                );
            }
        }
        if let Some(session) = &mut self.review_session {
            let notices = session.poll(session.elapsed());
            self.review_answered(notices);
        }
        if self.close_next.is_some() {
            egui::Modal::new(Id::unique("closing recovery")).show(ctx, |ui| {
                ui.heading("Finishing recovery checkpoints…");
                ui.spinner();
            });
        }
        let answer = self.review_dialog(ctx);
        if let Some(choice) = answer {
            self.choose(choice, minutes);
        }
        self.locate_dialog(ctx);
        if self.active.is_some()
            || !self.readers.is_empty()
            || self.review_session.is_some()
            || !self.retiring.is_empty()
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        effects
    }

    fn review_dialog(&mut self, ctx: &egui::Context) -> Option<Choice> {
        let review = self.review.as_mut()?;
        let mut answer = None;
        egui::Modal::new(Id::unique("recover project choices")).show(ctx,|ui| {
            ui.set_max_width(680.0);
            ui.heading("Recover project changes");
            ui.label(format!("Unsaved changes were kept for the project you are opening, in {}, when it last closed unexpectedly.", shown(&review.candidate.folder)));
            ui.label("Choose a checkpoint to recover. Saved project files change only after you explicitly Save.");
            if let Some(problem)=&review.candidate.problem {ui.colored_label(crate::theme::DARK.error,problem);}
            if let Some(problem)=&self.problem {ui.colored_label(crate::theme::DARK.error,problem);}
            if let Some(catalog)=&review.candidate.catalog {
                for problem in &catalog.problems {ui.colored_label(crate::theme::DARK.error,problem);}
                egui::ScrollArea::vertical().max_height(300.0).show(ui,|ui| {
                    for entry in &catalog.snapshots {
                        let kind=match entry.metadata.kind {Kind::Periodic=>"Periodic",Kind::Interim=>"Interim"};
                        let time=capture_time(entry.metadata.timestamp_ms);
                        let retired=if entry.retired {" · retained history"}else{""};
                        ui.radio_value(&mut review.selected,Some(entry.id.clone()),format!("{} · {kind} — {time}{retired}",entry.project_name));
                    }
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(review.selected.is_some() && self.choice.is_none(),egui::Button::new("Recover selected")).clicked() {answer=Some(Choice::Recover);}
                    if ui.add_enabled(self.choice.is_none(),egui::Button::new("Keep saved project")).clicked() {answer=Some(Choice::Keep);}
                    if ui.add_enabled(self.choice.is_none(),egui::Button::new("Discard recovery data…")).clicked() {review.confirm_discard=true;}
                });
                if review.confirm_discard {
                    ui.label("Delete the valid recovery checkpoints for this project? This cannot be undone. Damaged and partial files remain preserved.");
                    if ui.button("Delete checkpoints").clicked() {answer=Some(Choice::Discard);}
                    if ui.button("Cancel deletion").clicked() {review.confirm_discard=false;}
                }
            }
            if self.choice.is_some() {ui.spinner();}
        });
        answer
    }

    fn locate_dialog(&mut self, ctx: &egui::Context) {
        let Some(locate) = &mut self.locate else {
            return;
        };
        let mut chosen = None;
        let mut cancel = false;
        egui::Modal::new(Id::unique("locate moved recovery project")).show(ctx, |ui| {
            ui.set_max_width(680.0);
            ui.heading("Locate moved project");
            ui.label(format!(
                "Recovery data was kept for a project last in {}. Enter the folder it was moved to. Nothing is opened or changed until you open that project.",
                shown(&locate.candidate.folder)
            ));
            if let Some(problem) = &locate.problem {
                ui.colored_label(crate::theme::DARK.error, problem);
            }
            ui.label("Moved project folder:");
            ui.text_edit_singleline(&mut locate.path);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!locate.path.trim().is_empty(), egui::Button::new("Locate"))
                    .clicked()
                {
                    chosen = Some(PathBuf::from(locate.path.trim()));
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if cancel {
            self.locate = None;
            return;
        }
        if let Some(folder) = chosen
            && let (Some(config), Some(locate)) = (self.config.clone(), self.locate.take())
        {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let result = jobs::locate(&config, &locate.candidate, &folder);
                let _ = tx.send(Read::Located(Box::new(locate), result));
            });
            self.readers.push(rx);
        }
    }
}

impl Shell {
    pub(super) fn checkpoint_frame(&mut self, ctx: &egui::Context) {
        let previous = self.recovery.problem.clone();
        let allow_offer = self.dialog.is_none()
            && !self.edit_guard.active()
            && self.recovery_project.is_none()
            && self.app.as_ref().is_none_or(|app| !app.is_modified())
            && crate::pending_edits::entries(ctx).is_empty();
        let effects = self.recovery.frame(
            ctx,
            self.app.as_ref(),
            self.settings.checkpoint_minutes,
            allow_offer,
        );
        if self.recovery.problem != previous || std::mem::take(&mut self.recovery.changed) {
            self.overview_dirty = true;
        }
        for effect in effects {
            match effect {
                Effect::Recovered(folder, project, warnings) => {
                    match Application::from_checkpoint(&folder, *project, Threads::All) {
                        Ok(app) => {
                            self.recovery_project = Some((app, warnings, true));
                            self.request(Next::Recovery, ctx);
                        }
                        Err(e) => {
                            self.recovery.cancel_install(&folder);
                            self.dialog = Some(super::Dialog::Error(e.to_string()));
                        }
                    }
                }
                Effect::Resume(next) => self.proceed(next, ctx),
            }
        }
    }

    /// The recent projects that still exist, newest first. Folders that are
    /// gone are forgotten and the list is kept (issue 381).
    pub(super) fn recent_projects(&mut self) -> Vec<PathBuf> {
        if self.settings.forget_missing() {
            self.settings_touched();
        }
        self.settings.recent.clone()
    }

    /// Opens a recent project through the ordinary Open command: the
    /// unsaved-changes question first, then the Open dialog on that folder
    /// (the same route as a recovery line, decision 0460).
    pub(super) fn open_recent(&mut self, folder: &Path, ctx: &egui::Context) {
        self.open_hint = Some(shown(folder));
        self.request(Next::Open, ctx);
    }

    /// A recovery line's action: open that project through the ordinary
    /// Open dialog (with its unsaved-changes question), or locate it.
    pub(super) fn recovery_action(&mut self, action: MessageAction) {
        match action {
            MessageAction::OpenRecovery(index) => {
                if let Some(folder) = self.recovery.elsewhere_folder(index)
                    && let Some(ctx) = self.edit_context.clone()
                {
                    self.open_hint = Some(shown(&folder));
                    self.request(Next::Open, &ctx);
                }
            }
            MessageAction::LocateRecovery(index) => self.recovery.start_locate(index),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::super::checkpoint_tests::{click, ready_project};
    use super::*;
    use std::time::Instant;

    #[test]
    fn req_prj_08_req_nfr_04_headless_frames_remain_responsive_during_checkpoint_publication() {
        use openchuck_app::project::{Item, Placement, id::OperationId};
        use openchuck_app::settings::SettingsFile;
        use std::io::Write;
        let config = super::super::frame_tests::temp("checkpoint-frame-measure");
        let mut rig = super::super::frame_tests::Rig::cube(
            "checkpoint-frame-project",
            Some(SettingsFile::in_folder(&config)),
        );
        rig.dirs.push(config);
        let original = rig.shell.app.as_ref().unwrap().project().operations()[0].clone();
        for n in 0..200 {
            let mut operation = original.clone();
            operation.id = OperationId::from_u128(10000 + n);
            operation.name = format!("Measured operation {n}");
            rig.shell
                .app
                .as_mut()
                .unwrap()
                .execute(openchuck_app::Command::Add {
                    item: Item::Operation(operation),
                    placement: Placement::End,
                })
                .unwrap();
        }
        rig.settle();
        let deadline = Instant::now() + Duration::from_secs(10);
        while rig
            .shell
            .recovery
            .active
            .as_ref()
            .is_none_or(|s| !s.is_ready())
        {
            rig.frame(Vec::new());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut baseline = Duration::ZERO;
        for _ in 0..10 {
            let start = Instant::now();
            rig.frame(Vec::new());
            baseline = baseline.max(start.elapsed());
        }
        let folder = rig.shell.app.as_ref().unwrap().location().to_owned();
        {
            let app = rig.shell.app.as_ref().unwrap();
            rig.shell.recovery.active.as_mut().unwrap().tick(
                Duration::from_secs(180),
                app.project(),
                app.revision(),
                true,
            );
        }
        let mut during = Duration::ZERO;
        let mut frames = 0;
        loop {
            let start = Instant::now();
            rig.frame(Vec::new());
            during = during.max(start.elapsed());
            frames += 1;
            if !Catalog::read(&folder).unwrap().snapshots.is_empty() {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        writeln!(std::io::stdout(),"checkpoint_headless_baseline_max_ms={:.3},during_max_ms={:.3},frames={frames},operations=201",baseline.as_secs_f64()*1000.0,during.as_secs_f64()*1000.0).unwrap();
    }

    #[test]
    fn req_prj_08_folder_names_leave_out_the_windows_verbatim_prefix() {
        // Decision 0460 item 3: the user's folder, not `\\?\` (decision 0114).
        assert_eq!(
            shown(Path::new(r"\\?\C:\Projects\Part")),
            r"C:\Projects\Part"
        );
        assert_eq!(
            shown(Path::new(r"\\?\UNC\server\share\Part")),
            r"\\server\share\Part"
        );
        assert_eq!(shown(Path::new("/home/user/Part")), "/home/user/Part");
    }

    #[test]
    fn req_prj_08_late_folder_inspection_cannot_attach_a_writer_to_the_wrong_project() {
        let root = super::super::frame_tests::temp("checkpoint-obsolete-inspection");
        let old = root.join("old");
        let new = root.join("new");
        let config = root.join("config");
        std::fs::create_dir_all(&old).unwrap();
        let app = Application::create(&new, "new", Threads::Exactly(1)).unwrap();
        let mut state = State::new(Some(config));
        state.readers.clear();
        let (tx, rx) = mpsc::channel();
        state.readers.push(rx);
        state.attach(&new, 0, 3);
        tx.send(Read::Attached(
            0,
            Candidate {
                folder: old.clone(),
                catalog: Some(Catalog::default()),
                problem: None,
                project_id: String::new(),
            },
        ))
        .unwrap();
        state.frame(&egui::Context::default(), Some(&app), 3, true);
        assert_ne!(state.active_folder.as_deref(), Some(old.as_path()));
        drop(state);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn req_prj_08_reopening_the_same_project_starts_a_new_revision_namespace() {
        let mut rig = ready_project("checkpoint-reopen");
        click(&mut rig, "Keep saved project");
        let deadline = Instant::now() + Duration::from_secs(5);
        while rig
            .shell
            .recovery
            .active
            .as_ref()
            .is_none_or(|s| !s.is_ready())
        {
            rig.frame(Vec::new());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        rig.shell
            .app
            .as_mut()
            .unwrap()
            .execute(openchuck_app::Command::RenameProject(
                "first session".into(),
            ))
            .unwrap();
        {
            let app = rig.shell.app.as_ref().unwrap();
            let session = rig.shell.recovery.active.as_mut().unwrap();
            session.tick(Duration::ZERO, app.project(), app.revision(), true);
            session.tick(
                Duration::from_secs(180),
                app.project(),
                app.revision(),
                true,
            );
        }
        let folder = rig.shell.app.as_ref().unwrap().location().to_owned();
        loop {
            rig.frame(Vec::new());
            if Catalog::read(&folder)
                .unwrap()
                .snapshots
                .first()
                .is_some_and(|s| s.metadata.revision == 1)
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        let old_session = Catalog::read(&folder).unwrap().snapshots[0]
            .metadata
            .session_id
            .clone();
        assert!(rig.shell.save());
        while !Catalog::read(&folder).unwrap().snapshots[0].retired {
            rig.frame(Vec::new());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        rig.shell.open(folder.clone());
        // Reopening drops the old session and starts the new one only after a
        // background read of the folder lands on a later frame, so wait for
        // that state (its own deadline: a loaded runner is slow here).
        let reopen_deadline = Instant::now() + Duration::from_secs(30);
        while rig
            .shell
            .recovery
            .active
            .as_ref()
            .is_none_or(|s| !s.is_ready())
        {
            rig.frame(Vec::new());
            assert!(
                Instant::now() < reopen_deadline,
                "the reopened project never started a recovery session"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        rig.shell
            .app
            .as_mut()
            .unwrap()
            .execute(openchuck_app::Command::RenameProject(
                "second session".into(),
            ))
            .unwrap();
        {
            let app = rig.shell.app.as_ref().unwrap();
            let session = rig.shell.recovery.active.as_mut().unwrap();
            session.tick(Duration::ZERO, app.project(), app.revision(), true);
            session.tick(
                Duration::from_secs(360),
                app.project(),
                app.revision(),
                true,
            );
        }
        let publish_deadline = Instant::now() + Duration::from_secs(30);
        loop {
            rig.frame(Vec::new());
            let catalog = Catalog::read(&folder).unwrap();
            if catalog.snapshots[0].metadata.session_id != old_session {
                assert!(!catalog.snapshots[0].retired);
                break;
            }
            assert!(
                Instant::now() < publish_deadline,
                "reopened project reused an old revision namespace"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn req_prj_08_req_nfr_07_extreme_capture_time_is_reported_without_losing_recovery_choices() {
        assert_eq!(capture_time(0), "1970-01-01T00:00:00Z");
        assert_eq!(capture_time(253_402_300_799_999), "9999-12-31T23:59:59Z");
        let mut rig = ready_project("checkpoint-extreme-time");
        rig.shell
            .recovery
            .review
            .as_mut()
            .unwrap()
            .candidate
            .catalog
            .as_mut()
            .unwrap()
            .snapshots[0]
            .metadata
            .timestamp_ms = i64::MAX as u64;
        rig.frame(Vec::new());
        assert!(rig.shows("Capture time outside supported calendar"));
        assert!(rig.shows("Recover selected"));
        assert!(rig.shows("periodic state"));
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Recovery presentation; storage and jobs remain in the application layer.
use super::{Next, Shell};
use eframe::egui::{self, Id};
use openchuck_app::project::recovery::{Catalog, Kind};
use openchuck_app::{
    Application, Threads,
    checkpoints::{self as jobs, Candidate, Discovery, Notice, Session},
};
use std::collections::{BTreeSet, VecDeque};
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

enum Read {
    Startup(Discovery),
    Attached(u64, Candidate),
    Located(Box<Review>, Result<Candidate, String>),
    Restored(Result<jobs::Recovered, String>),
}
#[derive(Clone, Copy)]
enum Choice {
    Recover,
    Keep,
    Discard,
}
struct Review {
    candidate: Candidate,
    selected: Option<String>,
    confirm_discard: bool,
    locate_path: String,
}

#[derive(Default)]
pub(super) struct State {
    config: Option<PathBuf>,
    readers: Vec<Receiver<Read>>,
    queued: VecDeque<Candidate>,
    shown: BTreeSet<(PathBuf, String)>,
    review: Option<Review>,
    review_session: Option<Session>,
    review_folder: Option<PathBuf>,
    install_review: Option<Review>,
    awaiting_install: bool,
    choice: Option<Choice>,
    active: Option<Session>,
    active_folder: Option<PathBuf>,
    pending_attach: Option<PathBuf>,
    attachment: u64,
    retiring: Vec<Session>,
    close_next: Option<Next>,
    pub(super) problem: Option<String>,
}

pub(super) enum Effect {
    Recovered(PathBuf, Box<openchuck_app::project::Project>, Vec<String>),
    Open(PathBuf),
    Resume(Next),
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
            return;
        }
        self.review_session = None;
        self.review_folder = None;
        if self.config.is_none() {
            return;
        }
        self.pending_attach = Some(folder.to_owned());
    }

    fn inspect(&mut self, folder: PathBuf) {
        let attachment = self.attachment;
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
            let _ = tx.send(Read::Attached(
                attachment,
                Candidate {
                    folder: canonical,
                    catalog,
                    problem,
                    project_id,
                },
            ));
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
        self.review.is_some() || self.close_next.is_some()
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
        if let Some(review) = self.install_review.take() {
            self.shown.remove(&(
                review.candidate.folder.clone(),
                review.candidate.project_id.clone(),
            ));
            self.queued.push_front(review.candidate);
        }
    }

    fn choose(&mut self, choice: Choice, minutes: u16) {
        let Some(review) = &self.review else {
            return;
        };
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
            match read {
                Read::Startup(found) => {
                    if !found.problems.is_empty() {
                        self.problem = Some(found.problems.join("\n"));
                    }
                    for candidate in found.projects {
                        if candidate.problem.is_some()
                            || candidate
                                .catalog
                                .as_ref()
                                .is_some_and(|c| c.recovery_needed || !c.problems.is_empty())
                        {
                            self.queued.push_back(candidate);
                        }
                    }
                }
                Read::Attached(attachment, candidate) => {
                    if attachment != self.attachment {
                        continue;
                    }
                    if candidate.problem.is_some() {
                        self.problem = candidate.problem;
                    } else if candidate.catalog.as_ref().is_some_and(|c| c.live) {
                        self.problem=Some("Another application instance owns this project's checkpoints. Recovery writing is unavailable here.".into());
                    } else if candidate
                        .catalog
                        .as_ref()
                        .is_some_and(|c| c.recovery_needed)
                    {
                        self.queued.push_back(candidate);
                    } else if self
                        .review
                        .as_ref()
                        .is_none_or(|r| r.candidate.folder != candidate.folder)
                        && self.active.is_none()
                    {
                        self.start(candidate.folder, minutes);
                    }
                }
                Read::Located(original, result) => match result {
                    Ok(candidate) => {
                        self.problem = None;
                        self.review = Some(Review {
                            candidate,
                            selected: None,
                            confirm_discard: false,
                            locate_path: String::new(),
                        })
                    }
                    Err(e) => {
                        self.problem = Some(e);
                        self.review = Some(*original);
                    }
                },
                Read::Restored(result) => {
                    self.choice = None;
                    match result {
                        Ok(recovered) => {
                            self.install_review = self.review.take();
                            self.awaiting_install = true;
                            if let Some(config) = &self.config {
                                match Session::open(
                                    recovered.folder.clone(),
                                    config.clone(),
                                    minutes,
                                ) {
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
                        Err(e) => self.problem = Some(e),
                    }
                }
            }
        }
        if self.review.is_none() && !self.awaiting_install && allow_offer {
            while let Some(candidate) = self.queued.pop_front() {
                if self
                    .shown
                    .insert((candidate.folder.clone(), candidate.project_id.clone()))
                {
                    self.review = Some(Review {
                        candidate,
                        selected: None,
                        confirm_discard: false,
                        locate_path: String::new(),
                    });
                    break;
                }
            }
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
            for notice in session.poll(session.elapsed()) {
                match notice {
                    Notice::Saved if self.choice.is_some() => {
                        if let Some(review) = self.review.take() {
                            effects.push(Effect::Open(review.candidate.folder.clone()));
                            self.install_review = Some(review);
                            self.awaiting_install = true;
                            self.choice = None;
                        }
                    }
                    Notice::Problem(e) => {
                        self.problem = Some(format!("Recovery checkpoint failed: {e}"));
                        if matches!(self.choice, Some(Choice::Keep))
                            && let Some(review) = self.review.take()
                        {
                            effects.push(Effect::Open(review.candidate.folder.clone()));
                            self.install_review = Some(review);
                            self.awaiting_install = true;
                        }
                        self.choice = None;
                        self.review_session = None;
                        self.review_folder = None;
                        break;
                    }
                    _ => {}
                }
            }
        }
        if self.close_next.is_some() {
            egui::Modal::new(Id::new("closing recovery")).show(ctx, |ui| {
                ui.heading("Finishing recovery checkpoints…");
                ui.spinner();
            });
        }
        let mut answer = None;
        let mut located = None;
        if let Some(review) = &mut self.review {
            egui::Modal::new(Id::new("recover project choices")).show(ctx,|ui| {
                ui.set_max_width(680.0);
                ui.heading("Recover project changes");
                ui.label(review.candidate.folder.display().to_string());
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
                if review.candidate.problem.is_some() {
                    ui.label("Moved project folder:");
                    ui.text_edit_singleline(&mut review.locate_path);
                    if ui.add_enabled(!review.locate_path.trim().is_empty(),egui::Button::new("Locate")).clicked() {located=Some(PathBuf::from(review.locate_path.trim()));}
                }
                if self.choice.is_some() {ui.spinner();}
            });
        }
        if let Some(choice) = answer {
            self.choose(choice, minutes);
        }
        if let Some(folder) = located
            && let (Some(config), Some(review)) = (self.config.clone(), self.review.take())
        {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let result = jobs::locate(&config, &review.candidate, &folder);
                let _ = tx.send(Read::Located(Box::new(review), result));
            });
            self.readers.push(rx);
        }
        if self.active.is_some()
            || !self.readers.is_empty()
            || self.review_session.is_some()
            || !self.retiring.is_empty()
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        effects
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
        if self.recovery.problem != previous {
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
                Effect::Open(folder) => match Application::open(&folder, Threads::All) {
                    Ok(app) => {
                        self.recovery_project = Some((app, Vec::new(), false));
                        self.request(Next::Recovery, ctx);
                    }
                    Err(e) => {
                        self.recovery.cancel_install(&folder);
                        self.dialog = Some(super::Dialog::Error(e.to_string()));
                    }
                },
                Effect::Resume(next) => self.proceed(next, ctx),
            }
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
    fn req_prj_08_failed_locate_preserves_the_choice_and_allows_retry() {
        let mut rig = ready_project("checkpoint-locate-retry");
        let review = rig.shell.recovery.review.as_mut().unwrap();
        let original = review.candidate.folder.clone();
        review.candidate.problem = Some("Project folder moved".into());
        review.locate_path = original.join("does-not-exist").display().to_string();
        rig.settle();
        rig.frame(Vec::new());
        click(&mut rig, "Locate");
        rig.frame(Vec::new());
        let deadline = Instant::now() + Duration::from_secs(5);
        while !rig.shell.recovery.readers.is_empty() {
            rig.frame(Vec::new());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            rig.shell
                .recovery
                .review
                .as_ref()
                .map(|r| &r.candidate.folder),
            Some(&original),
            "a failed Locate must retain the original recovery choice"
        );
        assert!(rig.shell.recovery.problem.is_some());
        assert!(rig.shows("Locate"));
        rig.shell.recovery.review.as_mut().unwrap().locate_path = original.display().to_string();
        rig.frame(Vec::new());
        rig.frame(Vec::new());
        click(&mut rig, "Locate");
        rig.frame(Vec::new());
        while !rig.shell.recovery.readers.is_empty() {
            rig.frame(Vec::new());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(rig.shell.recovery.problem.is_none());
        assert!(
            rig.shell
                .recovery
                .review
                .as_ref()
                .unwrap()
                .candidate
                .catalog
                .is_some()
        );
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
        while rig.shell.app.is_none() {
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

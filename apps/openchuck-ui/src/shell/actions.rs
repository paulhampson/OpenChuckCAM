// SPDX-License-Identifier: AGPL-3.0-or-later
//! Recording the user's actions in the diagnostic log (requirement NFR-18;
//! decision record 0157).
//!
//! One place gathers them each frame ([`Shell::record_actions_frame`]):
//!
//! - the commands, undos and redos the application ran
//!   ([`openchuck_app::Application::drain_actions`]), which name the item
//!   and the parameter but never a value;
//! - the buttons and menu items clicked, by their label, read from egui's
//!   output events;
//! - the dialogs and windows that opened and closed;
//! - the jobs that started and finished.
//!
//! A line never holds file contents, a project's geometry, a parameter's
//! value or a typed text. The switch is `UserSettings::record_actions`; the
//! lines go to the log file only ([`crate::logging::action`]).

use eframe::egui::{self, output::OutputEvent};
use openchuck_app::{Event, JobId, job_label};

use std::path::PathBuf;

use super::{Dialog, Shell};
use crate::logging;

/// The longest label logged, in characters.
const LABEL_LIMIT: usize = 60;

/// What the recorder remembers between frames.
#[derive(Debug, Default)]
pub(super) struct Recorder {
    /// The dialogs and windows open at the end of the last frame.
    windows: Vec<&'static str>,
    /// The listed jobs started and not yet finished.
    jobs: Vec<(JobId, &'static str)>,
}

impl Dialog {
    /// The dialog's name in the log.
    pub(super) fn log_name(&self) -> &'static str {
        match self {
            Self::Folder { create: true, .. } => "New project",
            Self::Folder { create: false, .. } => "Open project",
            Self::OfferCreate { .. } => "Create project offer",
            Self::FolderExists { .. } => "Folder already exists",
            Self::Unsaved(_) => "Unsaved changes",
            Self::Rename { .. } => "Rename",
            Self::Keys => "Keyboard shortcuts",
            Self::About => "About",
            Self::Settings => "Settings",
            Self::Error(_) => "Error message",
            Self::CopyLinked(_) => "Copy linked models",
            Self::AdaptiveHeight { .. } => "Turn on adaptive spacing",
        }
    }
}

/// The log text for an egui output event: a button or a menu item (or a
/// link, or a check box) clicked, by its label. `None` for anything else:
/// typed text, a choice among values, a drag value or a slider would show a
/// value (NFR-18).
pub(super) fn click_text(event: &OutputEvent) -> Option<String> {
    use egui::Role::{Button, CheckBox, Link};
    let OutputEvent::Clicked(info) = event else {
        return None;
    };
    if !matches!(info.role, Button | CheckBox | Link) {
        return None;
    }
    click_line(info.label.as_deref()?)
}

/// The log text for a click on the control labelled `label`.
fn click_line(label: &str) -> Option<String> {
    let label = label.split_whitespace().collect::<Vec<_>>().join(" ");
    if label.is_empty() {
        return None;
    }
    let shown: String = label.chars().take(LABEL_LIMIT).collect();
    let more = if label.chars().count() > LABEL_LIMIT {
        "…"
    } else {
        ""
    };
    Some(format!("click: '{shown}{more}'"))
}

/// The click lines of a frame with a click on a recent project (labelled with
/// its folder's name, which is the user's) logged as the command alone.
pub(super) fn without_folder_names(lines: Vec<String>, folders: &[PathBuf]) -> Vec<String> {
    let private: Vec<String> = folders
        .iter()
        .filter_map(|f| click_line(&super::frame::recent_label(f)))
        .collect();
    lines
        .into_iter()
        .map(|line| {
            if private.contains(&line) {
                "click: 'Open recent project'".to_owned()
            } else {
                line
            }
        })
        .collect()
}

impl Recorder {
    /// The lines for the dialogs and windows that opened or closed since the
    /// last call, given those open now.
    pub(super) fn window_lines(&mut self, now: Vec<&'static str>) -> Vec<String> {
        let mut lines = Vec::new();
        for name in &now {
            if !self.windows.contains(name) {
                lines.push(format!("window opened: {name}"));
            }
        }
        for name in &self.windows {
            if !now.contains(name) {
                lines.push(format!("window closed: {name}"));
            }
        }
        self.windows = now;
        lines
    }

    /// The lines for the jobs in `events` the Jobs list shows: started, and
    /// how they ended. Quick jobs that run most frames are left out.
    pub(super) fn job_lines(&mut self, events: &[Event]) -> Vec<String> {
        let mut lines = Vec::new();
        for event in events {
            match event {
                Event::JobStarted { job, name } => {
                    if let Some((label, true)) = job_label(name) {
                        self.jobs.push((*job, label));
                        lines.push(format!("job started: {label}"));
                    }
                }
                Event::JobFinished {
                    job,
                    error,
                    cancelled,
                } => {
                    if let Some(at) = self.jobs.iter().position(|(id, _)| id == job) {
                        let (_, label) = self.jobs.remove(at);
                        let how = if *cancelled {
                            "cancelled"
                        } else if error.is_some() {
                            "failed"
                        } else {
                            "done"
                        };
                        lines.push(format!("job {how}: {label}"));
                    }
                }
                _ => {}
            }
        }
        lines
    }
}

impl Recorder {
    /// Writes the job lines for `events`, when the switch is on.
    pub(super) fn record_jobs(&mut self, events: &[Event]) {
        if logging::recording() {
            for line in self.job_lines(events) {
                logging::action(&line);
            }
        }
    }
}

#[cfg(test)]
impl Shell {
    /// The dialogs and windows the recorder saw open at the last frame.
    pub(super) fn recorder_windows(&self) -> &[&'static str] {
        &self.recorder.windows
    }
}

impl Shell {
    /// Writes the user's actions of this frame to the log when the switch is
    /// on (NFR-18). Call it once, at the end of the frame, so the clicks of
    /// the frame are all in egui's output.
    pub(super) fn record_actions_frame(&mut self, ctx: &egui::Context) {
        let lines = self
            .app
            .as_mut()
            .map(openchuck_app::Application::drain_actions);
        if !logging::recording() {
            return;
        }
        for line in lines.into_iter().flatten() {
            logging::action(&line);
        }
        let clicks = ctx.output(|o| o.events.iter().filter_map(click_text).collect::<Vec<_>>());
        for line in without_folder_names(clicks, &self.settings.recent) {
            logging::action(&line);
        }
        let mut open = Vec::new();
        if let Some(dialog) = &self.dialog {
            open.push(dialog.log_name());
        }
        if self.import.is_some() {
            open.push("Import model");
        }
        if self.export.is_open() {
            open.push("Post and export");
        }
        for line in self.recorder.window_lines(open) {
            logging::action(&line);
        }
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Role, WidgetInfo};

    use super::*;

    fn clicked(role: Role, label: &str) -> OutputEvent {
        OutputEvent::Clicked(WidgetInfo::labeled(role, true, label))
    }

    #[test]
    fn req_nfr_18_a_clicked_button_is_logged_by_its_label() {
        assert_eq!(
            click_text(&clicked(Role::Button, "Undo")).as_deref(),
            Some("click: 'Undo'")
        );
        assert_eq!(
            click_text(&clicked(Role::Button, "  Open\n project… ")).as_deref(),
            Some("click: 'Open project…'")
        );
        let long = "x".repeat(100);
        let text = click_text(&clicked(Role::Link, &long)).unwrap();
        assert_eq!(text.chars().filter(|c| *c == 'x').count(), LABEL_LIMIT);
        assert!(text.ends_with("…'"));
    }

    #[test]
    fn req_nfr_18_a_selectable_button_is_logged_by_its_label_as_before() {
        // egui 0.36 and 0.37 both give `Button::selectable` the role `Button`
        // with a selected state (`SelectableLabel` was no longer produced).
        for chosen in [false, true] {
            let step = WidgetInfo::selected(Role::Button, true, chosen, "Top");
            assert_eq!(
                click_text(&OutputEvent::Clicked(step)).as_deref(),
                Some("click: 'Top'")
            );
        }
    }

    #[test]
    fn req_nfr_18_values_and_typed_text_are_never_logged() {
        for role in [
            Role::TextInput,
            Role::SpinButton,
            Role::Slider,
            Role::RadioButton,
            Role::ComboBox,
        ] {
            assert_eq!(click_text(&clicked(role, "1234.5")), None, "{role:?}");
        }
        let mut typed = WidgetInfo::text_edit(true, "old", "secret text", "");
        typed.label = Some("secret text".into());
        assert_eq!(click_text(&OutputEvent::ValueChanged(typed)), None);
        // A button with no text, and a click that is not a click.
        let mut icon = WidgetInfo::new(Role::Button);
        icon.label = None;
        assert_eq!(click_text(&OutputEvent::Clicked(icon)), None);
        assert_eq!(click_text(&clicked(Role::Button, "  ")), None);
        let focus = OutputEvent::FocusGained(WidgetInfo::labeled(Role::Button, true, "Save"));
        assert_eq!(click_text(&focus), None);
    }

    #[test]
    fn req_nfr_18_dialogs_opening_and_closing_are_logged_once() {
        let mut recorder = Recorder::default();
        assert!(recorder.window_lines(vec![]).is_empty());
        assert_eq!(
            recorder.window_lines(vec!["About"]),
            ["window opened: About"]
        );
        assert!(recorder.window_lines(vec!["About"]).is_empty());
        assert_eq!(
            recorder.window_lines(vec!["Import model"]),
            ["window opened: Import model", "window closed: About"]
        );
    }

    #[test]
    fn req_nfr_18_listed_jobs_are_logged_when_they_start_and_end() {
        let mut recorder = Recorder::default();
        let events = [
            Event::JobStarted {
                job: JobId(1),
                name: "regenerate".to_owned(),
            },
            // A quick job the Jobs list leaves out.
            Event::JobStarted {
                job: JobId(2),
                name: "stock at move".to_owned(),
            },
            Event::JobFinished {
                job: JobId(2),
                error: None,
                cancelled: false,
            },
            Event::JobFinished {
                job: JobId(1),
                error: Some("a path /home/u/x".to_owned()),
                cancelled: false,
            },
        ];
        let lines = recorder.job_lines(&events);
        assert_eq!(lines, ["job started: Generating", "job failed: Generating"]);
        // The error text itself is not logged.
        assert!(lines.iter().all(|l| !l.contains("/home")));
        let more = [
            Event::JobStarted {
                job: JobId(3),
                name: "post".to_owned(),
            },
            Event::JobFinished {
                job: JobId(3),
                error: None,
                cancelled: true,
            },
            Event::JobStarted {
                job: JobId(4),
                name: "post".to_owned(),
            },
            Event::JobFinished {
                job: JobId(4),
                error: None,
                cancelled: false,
            },
        ];
        assert_eq!(
            recorder.job_lines(&more),
            [
                "job started: Posting G-code",
                "job cancelled: Posting G-code",
                "job started: Posting G-code",
                "job done: Posting G-code"
            ]
        );
    }

    #[test]
    fn req_nfr_18_every_dialog_has_a_name() {
        let names = [
            Dialog::Keys.log_name(),
            Dialog::About.log_name(),
            Dialog::Settings.log_name(),
            Dialog::Error("x".into()).log_name(),
            Dialog::Unsaved(super::super::Next::Quit).log_name(),
            Dialog::OfferCreate { folder: "f".into() }.log_name(),
        ];
        assert!(names.iter().all(|n| !n.is_empty()));
        let new = Dialog::Folder {
            create: true,
            folder: String::new(),
            name: String::new(),
        };
        assert_eq!(new.log_name(), "New project");
    }
}

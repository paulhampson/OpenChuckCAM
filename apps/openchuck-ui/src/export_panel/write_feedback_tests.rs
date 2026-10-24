// SPDX-License-Identifier: AGPL-3.0-or-later
//! Actual write feedback and explicit overwrite consent (UI-12, GC-05/12/15,
//! NFR-07/12); temporary destinations only.
#![allow(clippy::unwrap_used)]

use super::*;
use crate::testing::cube_project;

struct Rig {
    panel: ExportPanel,
    app: Application,
    run: Arc<Regeneration>,
    ctx: egui::Context,
    dir: PathBuf,
    texts: Vec<(String, egui::Rect)>,
    copied: Option<String>,
}

impl Rig {
    fn new(name: &str) -> Self {
        let (dir, mut app, _, _) = cube_project(name);
        let mut panel = ExportPanel::new(None);
        app.set_post(panel.selection());
        let run = Arc::new(app.regenerate().unwrap());
        panel.open(&app);
        panel.folder = dir.join("out").display().to_string();
        panel.ticked = true;
        let key = panel.key(&app).unwrap();
        let plan = app
            .planned_files(&run.toolpath, &key.post, key.grouping)
            .unwrap();
        panel.files = Some(Files {
            key,
            run: Arc::clone(&run),
            plan,
        });
        let ctx = egui::Context::default();
        crate::theme::install_style(&ctx, crate::theme::Theme::Dark);
        let mut rig = Self {
            panel,
            app,
            run,
            ctx,
            dir,
            texts: Vec::new(),
            copied: None,
        };
        rig.settle();
        rig
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        fn texts(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_owned(),
                    egui::Rect::from_min_size(t.pos, t.galley.size()),
                )),
                egui::Shape::Vec(shapes) => {
                    for s in shapes {
                        texts(s, out);
                    }
                }
                _ => {}
            }
        }
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1366.0, 768.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                self.panel
                    .frame(ui.ctx(), &mut self.app, Some(&self.run), None);
            },
        );
        output.textures_delta.clear();
        for command in &output.platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = command {
                self.copied = Some(text.clone());
            }
        }
        self.texts.clear();
        for s in output.shapes {
            texts(&s.shape, &mut self.texts);
        }
    }

    fn settle(&mut self) {
        for _ in 0..5 {
            self.frame(Vec::new());
        }
    }

    fn click(&mut self, name: &str) {
        let at = self
            .texts
            .iter()
            .rev()
            .find(|(text, _)| text == name)
            .unwrap_or_else(|| panic!("{name} not drawn: {:?}", self.texts))
            .1
            .center();
        self.frame(vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.frame(vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        self.settle();
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn req_ui_12_req_gc_12_success_stays_until_dismissed_and_is_retained_in_messages() {
    let mut rig = Rig::new("persistent-export");
    rig.click("Write G-code");
    assert!(rig.panel.result.as_ref().unwrap().is_complete());
    let paths = rig.panel.result.as_ref().unwrap().written.clone();
    assert_eq!(paths.len(), rig.run.gcode.len() + 2);
    rig.panel.open = false;
    rig.settle();
    assert!(rig.texts.iter().any(|(text, _)| text == "Export result"));
    assert!(
        rig.panel.result.is_some(),
        "closing export is not dismissing its result"
    );
    rig.click("Copy paths");
    assert_eq!(
        rig.copied.as_deref(),
        Some(
            paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n")
                .as_str()
        )
    );
    rig.click("Dismiss");
    assert!(rig.panel.result.is_none());
    let overview = rig.app.overview(None, &openchuck_app::JobLog::default());
    let message = overview
        .messages
        .iter()
        .find(|m| m.action.is_some_and(|a| a.label() == "Show result"))
        .unwrap();
    assert!(
        paths
            .iter()
            .all(|p| message.text.contains(&p.display().to_string()))
    );
    rig.panel.show_report(rig.app.output_report(0).unwrap());
    rig.settle();
    assert!(rig.panel.result.is_some());
    rig.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(
        rig.panel.result.is_none(),
        "Esc explicitly dismisses a result"
    );
    assert!(rig.app.output_report(0).is_some(), "Esc retains history");
}

#[test]
fn req_gc_05_req_nfr_07_overwrite_cancel_preserves_every_existing_file() {
    let mut rig = Rig::new("export-overwrite-cancel");
    let out = rig.dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let nc = out.join(&rig.run.gcode[0].0);
    let sheet = out.join(&rig.app.setup_sheet_file_names()[0]);
    std::fs::write(&nc, b"keep NC").unwrap();
    std::fs::write(&sheet, b"keep HTML").unwrap();
    rig.click("Write G-code");
    assert!(
        rig.texts
            .iter()
            .any(|(t, _)| t == "Replace existing files?")
    );
    for path in [&nc, &sheet] {
        assert!(
            rig.texts
                .iter()
                .any(|(t, _)| t == &path.display().to_string())
        );
    }
    // Esc has the same zero-write meaning as Cancel.
    rig.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    rig.settle();
    assert!(rig.panel.overwrite.is_none());
    assert!(rig.app.output_report(0).is_none());
    assert_eq!(std::fs::read(&nc).unwrap(), b"keep NC");
    rig.click("Write G-code");
    rig.click("Cancel");
    assert_eq!(std::fs::read(&nc).unwrap(), b"keep NC");
    assert_eq!(std::fs::read(&sheet).unwrap(), b"keep HTML");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 2);
    assert!(
        rig.app.output_report(0).is_none(),
        "Cancel is not a completed write attempt"
    );
    rig.click("Write G-code");
    rig.click("Replace files");
    assert!(rig.panel.result.as_ref().unwrap().is_complete());
    assert_eq!(std::fs::read(&nc).unwrap(), rig.run.gcode[0].1.as_bytes());
}

#[test]
fn req_gc_15_req_nfr_07_overwrite_rechecks_inputs_and_reports_failure_persistently() {
    let mut rig = Rig::new("export-overwrite-stale");
    let out = rig.dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let nc = out.join(&rig.run.gcode[0].0);
    std::fs::write(&nc, b"keep").unwrap();
    rig.click("Write G-code");
    let op = rig.app.project().operations()[0].id;
    rig.app
        .set_parameter(
            op,
            "stepover",
            Some(openchuck_app::project::ParameterValue::Number(0.7)),
        )
        .unwrap();
    rig.click("Replace files");
    let result = rig.panel.result.as_ref().unwrap();
    assert!(result.written.is_empty());
    assert!(result.failed.is_some());
    assert_eq!(std::fs::read(&nc).unwrap(), b"keep");
    rig.panel.open = false;
    rig.settle();
    assert!(rig.texts.iter().any(|(text, _)| text == "Export result"));
}

#[test]
fn req_gc_05_req_nfr_07_changed_folder_never_reuses_overwrite_approval() {
    let mut rig = Rig::new("export-overwrite-folder");
    let out = rig.dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let nc = out.join(&rig.run.gcode[0].0);
    std::fs::write(&nc, b"keep").unwrap();
    rig.click("Write G-code");
    let other = rig.dir.join("different-output");
    rig.panel.folder = other.display().to_string();
    rig.click("Replace files");
    assert!(rig.panel.result.as_ref().unwrap().written.is_empty());
    assert_eq!(std::fs::read(&nc).unwrap(), b"keep");
    assert!(!other.exists());
}

#[test]
fn req_ui_12_req_nfr_07_preflight_failure_lists_the_file_and_never_claims_success() {
    let mut rig = Rig::new("export-preflight-failure");
    let out = rig.dir.join("out");
    let failed = out.join(&rig.run.gcode[0].0);
    std::fs::create_dir_all(&failed).unwrap();
    std::fs::write(failed.join("keep"), b"keep directory contents").unwrap();
    rig.click("Write G-code");
    let report = rig.panel.result.as_ref().unwrap();
    assert!(report.written.is_empty());
    assert_eq!(report.failed.as_ref().unwrap().path, failed);
    assert!(report.failed.as_ref().unwrap().reason.contains("directory"));
    assert_eq!(
        report.not_attempted.len(),
        2,
        "both setup sheets were not attempted"
    );
    assert_eq!(
        std::fs::read(failed.join("keep")).unwrap(),
        b"keep directory contents"
    );
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);
    rig.click("Copy paths");
    assert!(
        rig.copied.is_none(),
        "failed paths are not copied as written"
    );
    rig.settle();
    assert!(rig.panel.result.is_some());
    rig.click("Dismiss");
    assert!(rig.app.output_report(0).unwrap().failed.is_some());
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! The read-only G-code preview of the export dialog (issue 312, decision
//! 0327): its keys, its file picker, its bytes and its invalidation.

#![allow(clippy::unwrap_used)]

use super::*;
use crate::testing::cube_project;

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A dialog with the files of a run, for `grouping`.
fn dialog(name: &str, grouping: OutputGrouping) -> (Cleanup, Application, ExportPanel) {
    let (dir, mut app, _, _) = cube_project(name);
    if grouping == OutputGrouping::Operation {
        // A second operation gives a second file.
        let mut second = app.project().operations()[0].clone();
        second.id = openchuck_app::project::id::OperationId::new();
        second.name = "Second".into();
        app.execute(openchuck_app::Command::Add {
            item: openchuck_app::project::Item::Operation(second),
            placement: openchuck_app::project::Placement::End,
        })
        .unwrap();
    }
    let mut panel = ExportPanel::new(None);
    panel.grouping = grouping;
    panel.open(&app);
    app.set_post(panel.selection());
    app.set_output_grouping(grouping);
    let run = Arc::new(app.regenerate().unwrap());
    let key = panel.key(&app).unwrap();
    let plan = app.planned_files(&run, &key.post, key.grouping).unwrap();
    panel.files = Some(Files { key, run, plan });
    (Cleanup(dir), app, panel)
}

fn frame(
    ctx: &egui::Context,
    panel: &mut ExportPanel,
    app: &Application,
    events: Vec<egui::Event>,
    clock: &mut f64,
) -> Vec<String> {
    *clock += 0.1;
    let checks = app.export_checks(None, panel.files.as_ref().map(|f| &*f.run), None);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1000.0),
            )),
            time: Some(*clock),
            events,
            ..Default::default()
        },
        |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let current = panel.files_current(app).is_some();
                panel.right(ui, app, current);
            });
            let _ = &checks;
        },
    );
    output.textures_delta.clear();
    let mut texts = Vec::new();
    fn collect(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            _ => {}
        }
    }
    for clipped in output.shapes {
        collect(&clipped.shape, &mut texts);
    }
    texts
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn settle(ctx: &egui::Context, panel: &mut ExportPanel, app: &Application, clock: &mut f64) {
    for _ in 0..4 {
        frame(ctx, panel, app, Vec::new(), clock);
    }
}

fn viewer() -> egui::Context {
    let ctx = egui::Context::default();
    crate::theme::install_style(&ctx, crate::theme::Theme::Dark);
    ctx
}

/// Focuses the preview viewer by clicking in it; returns after the click.
fn focus_viewer(ctx: &egui::Context, panel: &mut ExportPanel, app: &Application, clock: &mut f64) {
    settle(ctx, panel, app, clock);
    // The viewer is the only widget that takes the keyboard on a click and
    // is tall; find it by clicking down the column until a key moves it.
    for y in (100..900).step_by(20) {
        let at = egui::pos2(300.0, y as f32);
        for pressed in [true, false] {
            frame(
                ctx,
                panel,
                app,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                clock,
            );
        }
        frame(
            ctx,
            panel,
            app,
            vec![key(egui::Key::PageDown, egui::Modifiers::NONE)],
            clock,
        );
        if panel.preview_view.scroll.top > 0.0 {
            frame(
                ctx,
                panel,
                app,
                vec![key(egui::Key::Home, egui::Modifiers::NONE)],
                clock,
            );
            return;
        }
    }
    panic!("the preview never took the keyboard");
}

#[test]
fn req_nfr_15_the_preview_scrolls_with_page_keys_and_home_and_end() {
    let (_clean, app, mut panel) = dialog("preview-keys", OutputGrouping::Setup);
    let (ctx, mut clock) = (viewer(), 0.0);
    focus_viewer(&ctx, &mut panel, &app, &mut clock);
    let lines = panel
        .output_preview
        .as_ref()
        .unwrap()
        .3
        .as_ref()
        .unwrap()
        .files()[0]
        .line_count();
    assert!(lines > preview::VISIBLE_ROWS * 2, "{lines} lines");
    assert_eq!(panel.preview_view.scroll.first_row(), 0);
    frame(
        &ctx,
        &mut panel,
        &app,
        vec![key(egui::Key::PageDown, egui::Modifiers::NONE)],
        &mut clock,
    );
    assert_eq!(
        panel.preview_view.scroll.first_row(),
        preview::VISIBLE_ROWS - 1
    );
    frame(
        &ctx,
        &mut panel,
        &app,
        vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
        &mut clock,
    );
    assert_eq!(panel.preview_view.scroll.first_row(), preview::VISIBLE_ROWS);
    frame(
        &ctx,
        &mut panel,
        &app,
        vec![key(egui::Key::PageUp, egui::Modifiers::NONE)],
        &mut clock,
    );
    assert_eq!(panel.preview_view.scroll.first_row(), 1);
    frame(
        &ctx,
        &mut panel,
        &app,
        vec![key(egui::Key::End, egui::Modifiers::NONE)],
        &mut clock,
    );
    assert_eq!(
        panel.preview_view.scroll.first_row(),
        lines - preview::VISIBLE_ROWS
    );
    frame(
        &ctx,
        &mut panel,
        &app,
        vec![key(egui::Key::Home, egui::Modifiers::NONE)],
        &mut clock,
    );
    assert_eq!(panel.preview_view.scroll.first_row(), 0);
}

#[test]
fn req_nfr_15_ctrl_g_moves_the_keyboard_to_go_to_line() {
    let (_clean, app, mut panel) = dialog("preview-goto", OutputGrouping::Setup);
    let (ctx, mut clock) = (viewer(), 0.0);
    focus_viewer(&ctx, &mut panel, &app, &mut clock);
    let before = ctx.memory(|m| m.focused());
    frame(
        &ctx,
        &mut panel,
        &app,
        vec![key(egui::Key::G, egui::Modifiers::COMMAND)],
        &mut clock,
    );
    settle(&ctx, &mut panel, &app, &mut clock);
    assert_ne!(
        ctx.memory(|m| m.focused()),
        before,
        "the go-to box took the keyboard"
    );
}

#[test]
fn req_gc_07_the_dialogs_preview_holds_the_bytes_the_write_makes() {
    let (_clean, app, mut panel) = dialog("preview-bytes", OutputGrouping::Setup);
    let (ctx, mut clock) = (viewer(), 0.0);
    settle(&ctx, &mut panel, &app, &mut clock);
    let run = Arc::clone(&panel.files.as_ref().unwrap().run);
    let out = std::env::temp_dir().join(format!("openchuck-preview-ui-{}", std::process::id()));
    let _out = Cleanup(out.clone());
    let plan = app.prepare_output(&out, &run).unwrap();
    assert!(app.write_output(&plan, false).is_complete());
    let shown = panel.output_preview.as_ref().unwrap().3.as_ref().unwrap();
    assert!(!shown.files().is_empty());
    for file in shown.files() {
        assert_eq!(std::fs::read(out.join(file.name())).unwrap(), file.bytes());
    }
}

#[test]
fn req_gc_05_a_split_output_has_a_file_picker_and_one_file_does_not() {
    let (_one, app, mut panel) = dialog("preview-one", OutputGrouping::Project);
    let (ctx, mut clock) = (viewer(), 0.0);
    let texts = frame(&ctx, &mut panel, &app, Vec::new(), &mut clock);
    assert!(texts.iter().any(|t| t == "Preview"), "{texts:?}");
    assert_eq!(files_labels(&texts), 1, "one file needs no picker");
    let (_many, app, mut panel) = dialog("preview-many", OutputGrouping::Operation);
    let count = panel.files.as_ref().unwrap().plan.len();
    let texts = frame(&ctx, &mut panel, &app, Vec::new(), &mut clock);
    assert!(count > 1, "one file per operation gives several files");
    assert_eq!(files_labels(&texts), 2, "{texts:?}");
}

#[test]
fn req_gc_15_an_edit_takes_the_preview_away_until_the_files_are_made_again() {
    let (_clean, mut app, mut panel) = dialog("preview-stale", OutputGrouping::Setup);
    let (ctx, mut clock) = (viewer(), 0.0);
    let texts = frame(&ctx, &mut panel, &app, Vec::new(), &mut clock);
    assert!(texts.iter().any(|t| t == "Preview"));
    let first = app.project().operations()[0].id;
    app.execute(openchuck_app::Command::SetOperationEnabled {
        operation: first,
        enabled: false,
    })
    .unwrap();
    let texts = frame(&ctx, &mut panel, &app, Vec::new(), &mut clock);
    assert!(!texts.iter().any(|t| t == "Preview"), "{texts:?}");
    assert!(
        !texts.iter().any(|t| t.starts_with("SHA-256")),
        "no hash of files that are not the ones to write"
    );
}

#[test]
fn req_nfr_12_the_preview_says_what_it_is_and_never_calls_itself_a_check() {
    let (_clean, app, mut panel) = dialog("preview-words", OutputGrouping::Setup);
    let (ctx, mut clock) = (viewer(), 0.0);
    let texts = frame(&ctx, &mut panel, &app, Vec::new(), &mut clock).join("\n");
    assert!(texts.contains("Read-only"));
    for word in ["safe", "verified", "valid", "approved", "checked"] {
        assert!(!texts.to_lowercase().contains(word), "{word}");
    }
}

/// "File" heads the files table; the picker's label is a second one.
fn files_labels(texts: &[String]) -> usize {
    texts.iter().filter(|t| *t == "File").count()
}

#[test]
fn req_gc_07_search_marks_the_line_and_brings_it_into_view() {
    let (_clean, app, mut panel) = dialog("preview-search", OutputGrouping::Setup);
    let (ctx, mut clock) = (viewer(), 0.0);
    settle(&ctx, &mut panel, &app, &mut clock);
    let preview = app
        .output_preview(&panel.files.as_ref().unwrap().run)
        .unwrap();
    let file = &preview.files()[0];
    let view = &mut panel.preview_view;
    view.needle = "M6".into();
    view.search(file, true);
    let line = view.hit.expect("the program changes tool");
    assert!(file.line(line).unwrap().contains("M6"));
    let first = view.scroll.first_row();
    assert!((first..first + preview::VISIBLE_ROWS).contains(&line));
    view.needle = "no such text anywhere".into();
    view.search(file, true);
    assert_eq!(view.hit, None);
    assert!(
        view.problem
            .as_deref()
            .unwrap()
            .contains("not in this file")
    );
    view.needle.clear();
    view.search(file, false);
    assert!(view.problem.as_deref().unwrap().contains("Type the text"));
}

/// Measurement for decision 0327 (OI-1300): not a check. Run with
/// `cargo test -p openchuck-ui --release -- --ignored --nocapture measure_`.
#[test]
#[ignore = "measurement, not a check"]
#[allow(clippy::print_stderr)]
fn measure_a_million_line_program_in_the_dialog() {
    use std::fmt::Write;
    use std::time::Instant;
    let (dir, mut app, _, _) = cube_project("preview-million");
    let _clean = Cleanup(dir);
    let mut panel = ExportPanel::new(None);
    panel.open(&app);
    app.set_post(panel.selection());
    let mut run = app.regenerate().unwrap();
    let mut text = String::new();
    for i in 0..1_000_000 {
        let _ = writeln!(
            text,
            "G1 X{:.3} Y{:.3} Z-1.500 A{} F800",
            i as f64 * 0.001,
            (i % 977) as f64 * 0.01,
            i % 360
        );
    }
    run.gcode[0].1 = text;
    let run = Arc::new(run);
    let key = panel.key(&app).unwrap();
    let plan = app.planned_files(&run, &key.post, key.grouping).unwrap();
    panel.files = Some(Files { key, run, plan });
    let (ctx, mut clock) = (viewer(), 0.0);
    let t = Instant::now();
    frame(&ctx, &mut panel, &app, Vec::new(), &mut clock);
    let first = t.elapsed();
    let time = |label: &str, panel: &mut ExportPanel, ctx: &egui::Context, clock: &mut f64| {
        let t = Instant::now();
        for _ in 0..50 {
            frame(ctx, panel, &app, Vec::new(), clock);
        }
        eprintln!("{label}: {:?} per frame", t.elapsed() / 50);
    };
    time("top", &mut panel, &ctx, &mut clock);
    panel.preview_view.scroll.top = 500_000.5;
    time("middle", &mut panel, &ctx, &mut clock);
    panel.preview_view.scroll.top = 999_984.0;
    time("end", &mut panel, &ctx, &mut clock);
    eprintln!("first frame (preview made, 1M lines): {first:?}");
}

#[test]
fn req_gc_15_a_preview_refused_for_a_changed_project_is_made_again_when_it_is_back() {
    let (_clean, mut app, mut panel) = dialog("preview-skip", OutputGrouping::Setup);
    let (ctx, mut clock) = (viewer(), 0.0);
    frame(&ctx, &mut panel, &app, Vec::new(), &mut clock);
    assert!(panel.output_preview.as_ref().unwrap().3.is_ok());
    // The stock simulation switch is not part of the output's key, so the
    // list stays current while the preview's run no longer matches.
    app.execute(openchuck_app::Command::SetSkipStockSimulation(true))
        .unwrap();
    frame(&ctx, &mut panel, &app, Vec::new(), &mut clock);
    assert!(panel.output_preview.as_ref().unwrap().3.is_err());
    app.execute(openchuck_app::Command::SetSkipStockSimulation(false))
        .unwrap();
    frame(&ctx, &mut panel, &app, Vec::new(), &mut clock);
    assert!(
        panel.output_preview.as_ref().unwrap().3.is_ok(),
        "the refusal is not kept once the project is as the run was"
    );
}

/// Where the painted text `label` starts.
fn text_at(
    ctx: &egui::Context,
    panel: &mut ExportPanel,
    app: &Application,
    clock: &mut f64,
    label: &str,
) -> egui::Pos2 {
    *clock += 0.1;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1000.0),
            )),
            time: Some(*clock),
            ..Default::default()
        },
        |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let current = panel.files_current(app).is_some();
                panel.right(ui, app, current);
            });
        },
    );
    output.textures_delta.clear();
    fn find(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Vec(v) => v.iter().find_map(|s| find(s, label)),
            egui::Shape::Text(t) if t.galley.text() == label => Some(t.pos),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|c| find(&c.shape, label))
        .unwrap_or_else(|| panic!("{label} is painted"))
}

#[test]
fn req_nfr_15_enter_in_find_goes_on_to_the_next_line_without_clicking_the_box_again() {
    let (_clean, app, mut panel) = dialog("preview-enter", OutputGrouping::Setup);
    let (ctx, mut clock) = (viewer(), 0.0);
    settle(&ctx, &mut panel, &app, &mut clock);
    let label = text_at(&ctx, &mut panel, &app, &mut clock, "Find");
    let at = label + egui::vec2(60.0, 6.0);
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut panel,
            &app,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            &mut clock,
        );
    }
    frame(
        &ctx,
        &mut panel,
        &app,
        vec![egui::Event::Text("X0".into())],
        &mut clock,
    );
    assert_eq!(
        panel.preview_view.needle, "X0",
        "the Find box has the keyboard"
    );
    // Enter keeps the Find box, so each further Enter finds the next line.
    let mut hits = Vec::new();
    for _ in 0..4 {
        frame(
            &ctx,
            &mut panel,
            &app,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            &mut clock,
        );
        hits.push(panel.preview_view.hit.expect("Enter finds a line"));
    }
    assert!(
        hits.windows(2).all(|w| w[1] > w[0]),
        "every Enter goes on to a later line: {hits:?}"
    );
}

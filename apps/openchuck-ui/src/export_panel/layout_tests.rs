// SPDX-License-Identifier: AGPL-3.0-or-later
//! UI-16, issue 311: test the complete export window, its painted content,
//! reserved scrollbar space and fixed actions, including overflow states.

#![allow(clippy::unwrap_used)]

use super::*;
use crate::testing::cube_project;

struct Snapshot {
    dialog: egui::Rect,
    texts: Vec<(String, egui::Rect, egui::Rect)>,
    bars: Vec<egui::Rect>,
    controls: Vec<(egui::Rect, egui::Rect)>,
}

fn paint(shape: &egui::Shape, clip: egui::Rect, snapshot: &mut Snapshot) {
    match shape {
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                paint(shape, clip, snapshot);
            }
        }
        egui::Shape::Text(text) => {
            for row in &text.galley.rows {
                let whole = row
                    .rect_without_leading_space()
                    .translate(text.pos.to_vec2());
                let seen = whole.intersect(clip);
                if seen.is_positive() {
                    snapshot
                        .texts
                        .push((text.galley.text().to_owned(), whole, seen));
                }
            }
        }
        egui::Shape::Rect(rect) if rect.rect.width() < 14.0 && rect.rect.height() > 40.0 => {
            snapshot.bars.push(rect.rect.intersect(clip));
        }
        egui::Shape::Rect(rect)
            if (14.0..=40.0).contains(&rect.rect.height()) && rect.rect.width() >= 14.0 =>
        {
            snapshot
                .controls
                .push((rect.rect, rect.rect.intersect(clip)));
        }
        _ => {}
    }
}

fn draw(
    panel: &mut ExportPanel,
    app: &Application,
    ctx: &egui::Context,
    screen: egui::Vec2,
    size: [f32; 2],
    bottom: bool,
    clock: &mut f64,
) -> Snapshot {
    let mut snapshot = Snapshot {
        dialog: egui::Rect::NOTHING,
        texts: Vec::new(),
        bars: Vec::new(),
        controls: Vec::new(),
    };
    let checks = app.export_checks(None, panel.files.as_ref().map(|f| &*f.run), None);
    // Repeated frames settle window sizing and scrollbar animation. Real
    // pointer/wheel events scroll the body; the footer must not move.
    for pass in 0..10 {
        *clock += 0.1;
        let pointer = ctx
            .memory(|m| m.area_rect(egui::Id::new(("dialog", "export dialog"))))
            .map_or(screen.to_pos2() / 2.0, |rect| rect.center());
        let mut events = vec![egui::Event::PointerMoved(pointer)];
        if bottom && pass == 2 {
            events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(0.0, -10000.0),
                modifiers: egui::Modifiers::NONE,
            });
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)),
                time: Some(*clock),
                events,
                ..Default::default()
            },
            |ui| {
                floating_unscrolled(ui.ctx(), "export dialog", "Post and export", size, |ui| {
                    panel.draw(
                        ui,
                        app,
                        &checks,
                        Some("Choose an output folder before writing."),
                        &mut Vec::new(),
                    )
                });
            },
        );
        output.textures_delta.clear();
        snapshot.texts.clear();
        snapshot.bars.clear();
        snapshot.controls.clear();
        for clipped in output.shapes {
            paint(&clipped.shape, clipped.clip_rect, &mut snapshot);
        }
    }
    snapshot.dialog = ctx
        .memory(|m| m.area_rect(egui::Id::new(("dialog", "export dialog"))))
        .expect("export window drawn");
    snapshot
}

fn button(snapshot: &Snapshot, name: &str) -> egui::Rect {
    let (_, whole, seen) = snapshot
        .texts
        .iter()
        .find(|(text, _, _)| text == name)
        .unwrap_or_else(|| panic!("missing {name}"));
    assert!(seen.expand(0.5).contains_rect(*whole), "{name} clipped");
    assert!(
        snapshot.dialog.contains_rect(*whole),
        "{name} outside window"
    );
    let (control, visible) = snapshot
        .controls
        .iter()
        .filter(|(rect, _)| rect.contains_rect(*whole))
        .min_by(|(a, _), (b, _)| a.area().total_cmp(&b.area()))
        .unwrap_or_else(|| panic!("missing {name} button frame"));
    assert!(
        visible.expand(0.5).contains_rect(*control),
        "{name} button clipped"
    );
    assert!(
        snapshot.dialog.contains_rect(*control),
        "{name} button outside window"
    );
    *control
}

fn check(snapshot: &Snapshot, screen: egui::Vec2, case: &str) {
    check_actions(snapshot, screen, case, ["Cancel", "Write G-code"]);
}

fn check_actions(snapshot: &Snapshot, screen: egui::Vec2, case: &str, names: [&str; 2]) {
    assert!(
        egui::Rect::from_min_size(egui::Pos2::ZERO, screen)
            .expand(0.5)
            .contains_rect(snapshot.dialog),
        "{case}: window outside screen: {:?}",
        snapshot.dialog
    );
    assert!(
        !snapshot.bars.is_empty(),
        "{case}: overflowing content needs a visible scrollbar"
    );
    let actions = [button(snapshot, names[0]), button(snapshot, names[1])];
    for bar in &snapshot.bars {
        assert!(
            snapshot.dialog.expand(0.5).contains_rect(*bar),
            "{case}: bar outside window"
        );
        for (text, _, seen) in &snapshot.texts {
            assert!(
                !bar.intersects(seen.shrink(0.5)),
                "{case}: scrollbar {bar:?} overlaps {text:?} {seen:?}"
            );
        }
        for (control, _) in &snapshot.controls {
            assert!(
                !bar.intersects(control.shrink(0.5)),
                "{case}: scrollbar {bar:?} overlaps a control {control:?}"
            );
        }
        assert!(
            bar.bottom() <= actions[0].top(),
            "{case}: body runs into action strip"
        );
    }
    for action in actions {
        for (text, _, seen) in &snapshot.texts {
            if text != names[0] && text != names[1] {
                assert!(
                    !action.shrink(0.5).intersects(seen.shrink(0.5)),
                    "{case}: action overlaps {text:?}"
                );
            }
        }
    }
}

fn layout(files: bool) {
    let (dir, mut app, _, _) = cube_project(if files {
        "export-layout-files"
    } else {
        "export-layout-errors"
    });
    // Clean controlled temporary projects even when a layout assertion fails.
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(dir);
    // Use the dialog's actual post/grouping for both the generated files
    // and their plan, so this case exercises the existing preview too.
    let choices = ExportPanel::new(None);
    app.set_post(choices.selection());
    app.set_output_grouping(choices.grouping);
    let run = files.then(|| Arc::new(app.regenerate().unwrap()));
    for (width, height) in [(1366.0, 768.0), (1920.0, 1080.0)] {
        for scale in [1.0, 1.2] {
            for size in [[980.0, 700.0], [720.0, 460.0]] {
                let ctx = egui::Context::default();
                crate::theme::install_style(&ctx, crate::theme::Theme::Dark);
                ctx.set_pixels_per_point(scale);
                let screen = egui::vec2(width / scale, height / scale);
                let mut panel = ExportPanel::new(None);
                panel.open(&app);
                panel.folder = format!("/exports/{}/part", "long_project_directory/".repeat(10));
                panel.number_choice = NumberChoice::ByHand;
                panel.by_hand[0].1 = "not a tool number".to_owned();
                if let Some(run) = &run {
                    let key = panel.key(&app).unwrap();
                    let plan = app.planned_files(run, &key.post, key.grouping).unwrap();
                    assert!(
                        plan.iter()
                            .all(|file| run.gcode.iter().any(|(name, _)| name == &file.name)),
                        "fixture file plan matches the preview's generated files"
                    );
                    panel.files = Some(Files {
                        key,
                        run: Arc::clone(run),
                        plan,
                    });
                } else {
                    panel.files_error = Some(format!(
                        "Cannot write {}: permission denied.\n{}",
                        panel.folder,
                        "The affected file could not be prepared.\n".repeat(12)
                    ));
                }
                let case = format!("{width}x{height} {scale} {size:?} files={files}");
                let mut clock = 0.0;
                let top = draw(&mut panel, &app, &ctx, screen, size, false, &mut clock);
                check(&top, screen, &case);
                let bottom = draw(&mut panel, &app, &ctx, screen, size, true, &mut clock);
                check(&bottom, screen, &case);
                for action in ["Cancel", "Write G-code"] {
                    assert!(
                        (button(&top, action).top() - button(&bottom, action).top()).abs() < 0.5,
                        "{case}: {action} moved while scrolling"
                    );
                }
                assert!(
                    bottom
                        .texts
                        .iter()
                        .any(|(text, _, _)| text.contains("Verify this G-code")),
                    "{case}: can scroll to verification notice"
                );
            }
        }
    }
}

#[test]
fn req_ui_16_export_errors_reserve_scrollbar_space_and_keep_actions_fixed() {
    layout(false);
}

#[test]
fn req_ui_16_export_files_reserve_scrollbar_space_and_keep_actions_fixed() {
    layout(true);
}

#[test]
fn req_ui_12_req_ui_16_confirmations_and_results_keep_bounds_and_actions_while_scrolling() {
    use openchuck_app::OutputFailure;
    let path = |i| {
        PathBuf::from(format!(
            "/exports/{}/output-{i}.nc",
            "long_directory/".repeat(10)
        ))
    };
    // A read-only confirmation fixture with many actual existing destinations.
    let (dir, app, _, _) = cube_project("overwrite-layout");
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());
    let folder = dir.join("long_directory/".repeat(10));
    std::fs::create_dir_all(&folder).unwrap();
    let mut run = app.regenerate().unwrap();
    run.gcode = (0..24)
        .map(|i| (format!("output-{i}.nc"), "NC".into()))
        .collect();
    for (name, _) in &run.gcode {
        std::fs::write(folder.join(name), b"existing NC").unwrap();
    }
    let run = Arc::new(run);
    // Complete success, total failure, partial success and overwrite review.
    // Actual I/O/result wiring is covered in write_feedback_tests.
    for state in 0..4 {
        for (w, h) in [(1366.0, 768.0), (1920.0, 1080.0), (720.0, 480.0)] {
            for scale in [1.0, 1.2] {
                let ctx = egui::Context::default();
                crate::theme::install_style(&ctx, crate::theme::Theme::Dark);
                ctx.set_pixels_per_point(scale);
                let screen = egui::vec2(w / scale, h / scale);
                let mut panel = ExportPanel::new(None);
                let (key, actions) = if state == 3 {
                    panel.overwrite = Some(PendingOutput {
                        plan: app.prepare_output(&folder, &run).unwrap(),
                        key: panel.key(&app).unwrap(),
                        folder: folder.display().to_string(),
                    });
                    ("export overwrite", ["Cancel", "Replace files"])
                } else {
                    panel.show_report(OutputReport {
                        written: if state == 1 {
                            Vec::new()
                        } else {
                            (0..20).map(path).collect()
                        },
                        failed: (state != 0).then(|| OutputFailure {
                            path: path(20),
                            reason: "Disk full. The file could not be written.".to_owned(),
                        }),
                        not_attempted: if state == 0 {
                            Vec::new()
                        } else {
                            (21..40).map(path).collect()
                        },
                    });
                    ("export result", ["Copy paths", "Dismiss"])
                };
                let mut before = None;
                for bottom in [false, true] {
                    let mut snapshot = Snapshot {
                        dialog: egui::Rect::NOTHING,
                        texts: Vec::new(),
                        bars: Vec::new(),
                        controls: Vec::new(),
                    };
                    for pass in 0..10 {
                        let mut events = vec![egui::Event::PointerMoved(screen.to_pos2() / 2.0)];
                        if bottom && pass == 2 {
                            events.push(egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                phase: egui::TouchPhase::Move,
                                delta: egui::vec2(0.0, -10000.0),
                                modifiers: egui::Modifiers::NONE,
                            });
                        }
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    screen,
                                )),
                                time: Some(f64::from(pass) * 0.1 + if bottom { 2.0 } else { 0.0 }),
                                events,
                                ..Default::default()
                            },
                            |ui| panel.feedback_frame(ui.ctx(), &mut Vec::new()),
                        );
                        output.textures_delta.clear();
                        snapshot.texts.clear();
                        snapshot.bars.clear();
                        snapshot.controls.clear();
                        for shape in output.shapes {
                            paint(&shape.shape, shape.clip_rect, &mut snapshot);
                        }
                    }
                    snapshot.dialog = ctx.memory(|m| m.area_rect(egui::Id::new(key))).unwrap();
                    let case =
                        format!("result state {state}, {w}x{h}, scale {scale}, bottom {bottom}");
                    check_actions(&snapshot, screen, &case, actions);
                    let actions = [button(&snapshot, actions[0]), button(&snapshot, actions[1])];
                    if let Some(before) = before {
                        let before: [egui::Rect; 2] = before;
                        for (a, b) in before.iter().zip(actions) {
                            assert!((a.top() - b.top()).abs() < 0.5, "{case}: action moved");
                        }
                    } else {
                        before = Some(actions);
                    }
                    assert!(
                        panel.result.is_some() || panel.overwrite.is_some(),
                        "scrolling never dismisses feedback"
                    );
                }
            }
        }
    }
}

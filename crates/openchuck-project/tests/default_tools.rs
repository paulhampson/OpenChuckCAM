// SPDX-License-Identifier: AGPL-3.0-or-later
//! The default tool library (slice 23; TL-12, TL-02, TL-03, NFR-09; decision
//! record 0081).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use openchuck_project::tools::{LEDGER_FILE, ToolLibrary, default_tools};
use openchuck_types::tool::ToolShape;

fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("openchuck-defaults-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The closed-form lower envelope of the shape at radius `r` (decision 0012).
fn closed_form(shape: &ToolShape, diameter: f64, r: f64) -> f64 {
    let radius = diameter / 2.0;
    match shape {
        ToolShape::FlatEnd => 0.0,
        ToolShape::BallEnd => radius - (radius * radius - r * r).sqrt(),
        ToolShape::BullNose { corner_radius } => {
            let c = corner_radius.0;
            let flat = radius - c;
            if r <= flat {
                0.0
            } else {
                c - (c * c - (r - flat) * (r - flat)).sqrt()
            }
        }
        ToolShape::TaperedBall {
            tip_radius,
            taper_half_angle,
        } => {
            let (rt, a) = (tip_radius.0, taper_half_angle.0.to_radians());
            if r <= rt * a.cos() {
                rt - (rt * rt - r * r).sqrt()
            } else {
                rt - rt * a.sin() + (r - rt * a.cos()) / a.tan()
            }
        }
        ToolShape::VBit {
            included_angle,
            tip_diameter,
        }
        | ToolShape::Chamfer {
            included_angle,
            tip_diameter,
        } => {
            let t = tip_diameter.0 / 2.0;
            if r <= t {
                0.0
            } else {
                (r - t) / (included_angle.0.to_radians() / 2.0).tan()
            }
        }
        ToolShape::Drill { point_angle } => r / (point_angle.0.to_radians() / 2.0).tan(),
        _ => panic!("a default tool of an unexpected shape"),
    }
}

#[test]
fn req_tl_12_every_default_validates_and_has_a_stable_unique_id() {
    let tools = default_tools().unwrap();
    assert_eq!(tools.len(), 23);
    let mut ids: Vec<_> = tools.iter().map(|t| t.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), tools.len(), "an ID is used twice");
    for t in &tools {
        t.validate().unwrap_or_else(|e| panic!("{}: {e}", t.name));
        assert!(t.cutting_data.is_empty(), "{} ships cutting data", t.name);
    }
}

#[test]
fn req_tl_12_req_tl_02_req_tl_03_defaults_cover_every_standard_type() {
    let tools = default_tools().unwrap();
    let has = |f: fn(&ToolShape) -> bool| tools.iter().any(|t| f(&t.tool.shape));
    assert!(has(|s| matches!(s, ToolShape::FlatEnd)));
    assert!(has(|s| matches!(s, ToolShape::BallEnd)));
    assert!(has(|s| matches!(s, ToolShape::BullNose { .. })));
    assert!(has(|s| matches!(s, ToolShape::TaperedBall { .. })));
    assert!(has(|s| matches!(s, ToolShape::VBit { .. })));
    assert!(has(|s| matches!(s, ToolShape::Chamfer { .. })));
    assert!(has(|s| matches!(s, ToolShape::Drill { .. })));
}

#[test]
fn req_tl_12_req_tl_02_req_tl_03_default_shapes_match_their_closed_form_silhouette() {
    for t in default_tools().unwrap() {
        let profile = t.tool.profile().unwrap();
        let d = t.tool.diameter.0;
        for i in 0..=300 {
            let r = d / 2.0 * f64::from(i) / 300.0;
            let got = profile.height_at(r).unwrap();
            let want = closed_form(&t.tool.shape, d, r);
            assert!(
                (got - want).abs() < 1e-9,
                "{} at r={r}: {got} against {want}",
                t.name
            );
        }
    }
}

#[test]
fn req_tl_12_first_run_installs_into_an_empty_library_and_a_second_run_installs_nothing() {
    let dir = temp_dir("first-run").join("tools");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let report = lib.install_defaults().unwrap();
    assert_eq!(report.installed.len(), 23);
    assert!(dir.join(LEDGER_FILE).is_file());

    let reopened = ToolLibrary::open(&dir).unwrap();
    assert_eq!(reopened.tools().count(), 23);
    assert!(reopened.problems().is_empty(), "{:?}", reopened.problems());

    let mut again = reopened;
    let report = again.install_defaults().unwrap();
    assert!(report.installed.is_empty());
    assert_eq!(report.already_installed, 23);
    assert_eq!(again.tools().count(), 23);
}

#[test]
fn req_tl_12_an_edited_default_is_never_overwritten() {
    let dir = temp_dir("edited").join("tools");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    lib.install_defaults().unwrap();
    let mut tool = lib.tools().next().unwrap().clone();
    tool.name = "My own name".to_owned();
    tool.tool.flute_count = 4;
    lib.save_tool(tool.clone()).unwrap();

    let mut reopened = ToolLibrary::open(&dir).unwrap();
    reopened.install_defaults().unwrap();
    assert_eq!(reopened.get(tool.id), Some(&tool));
}

#[test]
fn req_tl_12_a_deleted_default_is_not_restored() {
    let dir = temp_dir("deleted").join("tools");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    lib.install_defaults().unwrap();
    let gone = lib.tools().next().unwrap().id;
    lib.remove_tool(gone).unwrap();

    let mut reopened = ToolLibrary::open(&dir).unwrap();
    let report = reopened.install_defaults().unwrap();
    assert!(report.installed.is_empty());
    assert!(reopened.get(gone).is_none());
    assert_eq!(reopened.tools().count(), 22);
}

#[test]
fn req_tl_12_a_default_the_ledger_does_not_know_is_installed_and_a_user_tool_is_kept() {
    let dir = temp_dir("new-default").join("tools");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    lib.install_defaults().unwrap();
    // As if a later release had added the last default: forget it.
    let ledger = std::fs::read_to_string(dir.join(LEDGER_FILE)).unwrap();
    let mut lines: Vec<&str> = ledger.lines().collect();
    let forgotten = lines.pop().unwrap().to_owned();
    std::fs::write(dir.join(LEDGER_FILE), lines.join("\n")).unwrap();
    let id = forgotten.parse().unwrap();
    lib.remove_tool(id).unwrap();

    let mut reopened = ToolLibrary::open(&dir).unwrap();
    let report = reopened.install_defaults().unwrap();
    assert_eq!(report.installed, vec![id]);
    assert_eq!(reopened.tools().count(), 23);
}

#[test]
fn req_tl_12_a_damaged_ledger_never_overwrites_a_tool() {
    let dir = temp_dir("damaged-ledger").join("tools");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    lib.install_defaults().unwrap();
    let mut tool = lib.tools().next().unwrap().clone();
    tool.name = "Edited".to_owned();
    lib.save_tool(tool.clone()).unwrap();
    std::fs::write(dir.join(LEDGER_FILE), "garbage\n").unwrap();

    let mut reopened = ToolLibrary::open(&dir).unwrap();
    let report = reopened.install_defaults().unwrap();
    assert!(report.installed.is_empty());
    assert_eq!(report.already_present, 23);
    assert_eq!(reopened.get(tool.id), Some(&tool));
}

#[test]
fn req_nfr_09_req_tl_12_the_defaults_are_embedded_not_read_from_the_source_tree() {
    // The data is `include_str!`ed, so installing works with no source tree
    // and no network: run it from a folder that holds nothing else.
    let dir = temp_dir("offline").join("tools");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    assert_eq!(lib.install_defaults().unwrap().installed.len(), 23);
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tutorial's turned piece (the public baluster example) with ramp
//! entry on both its operations, in Rings and as a spiral: every ramp
//! follows the toolpath and the program generates (decision 0543, proposed;
//! issue #639; OP-11, NFR-07, NFR-12). A public stand-in for the owner's
//! own projects, which are not in the repository.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use openchuck_app::{Application, Threads};

const ROUGH: &str = "operations/1a280b59-8da1-4ee1-97e9-1e2b40c356f5.toml";
const FINISH: &str = "operations/34e3aa5e-36cc-4614-be6d-ec39cf22f5f8.toml";

/// A copy of the example project with ramp entry at `angle` degrees,
/// Allow single-point plunge on, and `shape` as the pass shape of both
/// operations.
fn example(shape: &str, angle: f64) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/examples/baluster/baluster-project");
    let dir = std::env::temp_dir().join(format!(
        "openchuck-ramp-example-{shape}-{angle}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    copy(&from, &dir);
    for op in [ROUGH, FINISH] {
        let path = dir.join(op);
        let text = std::fs::read_to_string(&path).unwrap();
        let mut out = String::new();
        let mut done = false;
        for line in text.lines() {
            let drop = [
                "entry =",
                "pass_shape =",
                "allow_single_point_plunge =",
                "ramp_angle =",
            ]
            .iter()
            .any(|k| line.trim_start().starts_with(k));
            if drop {
                continue;
            }
            out.push_str(line);
            out.push('\n');
            if line.trim() == "[parameters]" && !done {
                out.push_str(&format!(
                    "entry = \"ramp\"\npass_shape = \"{shape}\"\nallow_single_point_plunge = true\nramp_angle = {angle:?}\n"
                ));
                done = true;
            }
        }
        assert!(done, "{op} has a [parameters] table");
        std::fs::write(&path, out).unwrap();
    }
    dir
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// The ramp moves of the example regenerated, or the refusal.
fn ramps(shape: &str, angle: f64) -> Result<usize, String> {
    let dir = example(shape, angle);
    let app = Application::open(&dir, Threads::Exactly(4)).unwrap();
    let out = app.regenerate().map_err(|e| e.to_string()).map(|run| {
        run.toolpath
            .groups
            .iter()
            .flat_map(|g| &g.segments)
            .filter(|s| s.kind == openchuck_types::ir::MoveKind::LeadIn)
            .count()
    });
    let _ = std::fs::remove_dir_all(&dir);
    out
}

#[test]
fn req_op_11_req_set_09_the_tutorial_piece_ramps_round_its_rings() {
    // Every ramp goes round its ring, or back and forth over a short step,
    // and both operations generate at the default 3 degrees.
    let rings = ramps("rings", 3.0).unwrap();
    assert!(rings > 2, "{rings} ramp moves");
}

#[test]
fn req_op_11_the_tutorial_piece_ramps_round_its_spiral() {
    // Roughing's last layers turn within a millimetre of the axis, where 3
    // degrees would take more than the 10 turns allowed (decision 0543 D4;
    // the refusal is tested in openchuck-path); at 10 degrees it generates.
    let spiral = ramps("helix", 10.0).unwrap();
    assert!(spiral > 2, "{spiral} ramp moves");
}

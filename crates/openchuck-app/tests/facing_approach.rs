// SPDX-License-Identifier: AGPL-3.0-or-later
//! End facing on the Baluster example, as the owner set it up (issue #499):
//! box stock 72.4 x 35 x 35 mm, the model scaled by 1.15, the start end held
//! for 8 mm, and End facing with a 3.175 mm flat tool and 1 mm stock to
//! leave. The tests run the real application and both built-in posts.
//!
//! The G-code these tests produce is not checked on a machine: generated
//! G-code is the user's to verify before it is run (NFR-12).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::panic
)]

use std::path::{Path, PathBuf};

use openchuck_app::{Application, OutputGrouping, PostSelection, Regeneration, Threads};
use openchuck_types::ir::{MoveKind, ToolpathGroup};

const SETUP: &str = "57ad6798-2544-4dfb-90a0-caf67ec69484";
const FACING: &str = "99e9173f-5bc8-4b43-99a9-9030e097e6b5";
/// The approach distance the operation uses (the default).
const APPROACH: f64 = 1.;
/// The tool's radius (Z1 O-flute 3.175).
const RADIUS: f64 = 3.175 / 2.;
/// The far end of the stock and the top of the box, model frame.
const STOCK_END: f64 = 71.4;
const STOCK_TOP: f64 = 17.5;

/// A copy of the Baluster example with the owner's End facing set-up, in a
/// folder of its own. `extra` adds lines to the operation's parameters.
fn project(name: &str, extra: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "openchuck-face-approach-{name}-{unique}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/examples/baluster/baluster-project");
    copy(&example, &dir);
    let project = dir.join("project.toml");
    let text = std::fs::read_to_string(&project).unwrap();
    std::fs::write(
        &project,
        format!("{text}\n[[models.transform]]\nkind = \"scale\"\nfactor = 1.15\n"),
    )
    .unwrap();
    let setup = dir.join(format!("setups/{SETUP}.toml"));
    let text = std::fs::read_to_string(&setup).unwrap();
    let stock = "[stock]\ntype = \"cylinder\"\ndiameter = 24.000000238343276\nlength = 63.0\nx_start = -1.0\n";
    assert!(text.contains(stock), "the example's stock has changed");
    std::fs::write(
        &setup,
        text.replace(
            stock,
            "[stock]\ntype = \"box\"\n\n[stock.bounds.min]\nx = -1.0\ny = -17.5\nz = -17.5\n\n\
             [stock.bounds.max]\nx = 71.4\ny = 17.5\nz = 17.5\n",
        ),
    )
    .unwrap();
    // Only the facing operation runs: the example's own operations are off.
    for entry in std::fs::read_dir(dir.join("operations")).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            text.replacen("\n[parameters]", "enabled = false\n\n[parameters]", 1),
        )
        .unwrap();
    }
    std::fs::write(
        dir.join(format!("operations/{FACING}.toml")),
        format!(
            "schema = 25\nid = \"{FACING}\"\norder = \"U\"\nname = \"End facing\"\n\
             setup = \"{SETUP}\"\nstrategy = \"facing\"\n\
             tool = \"c0de0001-0000-4000-8000-000000000104\"\nselections = []\n\
             spindle_speed = 10000.0\n\n[parameters]\nextend = \"no\"\nfeed = 1000.0\n\
             plunge_feed = 300.0\nshape = \"end\"\nstepdown = 2.0\nstepover = 2.2225\n\
             stock_to_leave = 1.0\n{extra}"
        ),
    )
    .unwrap();
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
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn post(dir: &Path, post: &str) -> Regeneration {
    let mut app = Application::open(dir, Threads::Exactly(2)).unwrap();
    app.start_regenerate_with_post(PostSelection::Builtin(post.into()), OutputGrouping::Setup)
        .wait()
        .unwrap_or_else(|e| panic!("{post}: {e}"))
}

/// The facing group of a run.
fn facing(run: &Regeneration) -> &ToolpathGroup {
    assert_eq!(run.toolpath.groups.len(), 1);
    &run.toolpath.groups[0]
}

/// How many times the tool goes back up to the retract height between the
/// first and the last cut.
fn retracts(group: &ToolpathGroup) -> usize {
    let safe = group.start.tip.z;
    let first = group
        .segments
        .iter()
        .position(|s| s.kind == MoveKind::Cutting)
        .unwrap();
    let last = group
        .segments
        .iter()
        .rposition(|s| s.kind == MoveKind::Cutting)
        .unwrap();
    group.segments[first..last]
        .iter()
        .filter(|s| s.kind == MoveKind::Rapid && s.end.tip.z >= safe - 1e-9)
        .count()
}

/// Issue #499: the owner's Baluster End facing posts through both posts.
/// It was refused with "the actual end sweeps leave an unfinished required
/// region in an axial layer": the written motion read back put the lowest
/// band 7e-15 mm above the stock's bottom edge.
#[test]
fn req_op_24_req_gc_08_baluster_end_facing_posts_through_both_posts() {
    let dir = project("posts", "");
    for name in ["makera-z1", "linuxcnc"] {
        let run = post(&dir, name);
        assert_eq!(run.gcode.len(), 1, "{name}");
        assert!(run.gcode[0].1.contains("G1"), "{name}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// Issue #499: a rapid that goes down beside the end face keeps the
/// approach distance from it, as a rapid above the stock keeps it from the
/// top (OP-14). It was 2.5 um, so stock slightly longer than modelled would
/// have been hit at rapid. Every fed move into the end starts from there.
#[test]
fn req_op_14_req_nfr_12_baluster_end_rapids_keep_the_approach_distance_from_the_end_face() {
    let dir = project("air", "");
    let run = post(&dir, "makera-z1");
    let group = facing(&run);
    let mut previous = group.start;
    for segment in &group.segments {
        if segment.kind == MoveKind::Rapid
            && previous.tip.z.min(segment.end.tip.z) < STOCK_TOP + APPROACH - 1e-9
        {
            let near = previous.tip.x.min(segment.end.tip.x) - RADIUS;
            assert!(
                near >= STOCK_END + APPROACH - 1e-9,
                "a rapid below the stock top + approach passes {:.6} mm from the end face",
                near - STOCK_END
            );
        }
        previous = segment.end;
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// Decision D1 of issue #499 (the owner's choice): Facing End offers the
/// "Links between passes" choice of decision 0496, retract by default. A
/// saved operation without it, and one set to retract, make the same path,
/// going up to the retract height between bands as before (GC-07).
#[test]
fn req_op_14_req_gc_07_end_facing_links_retract_by_default() {
    let saved = project("saved", "");
    let retract = project("retract", "link_height = \"retract\"\n");
    let a = post(&saved, "linuxcnc");
    let b = post(&retract, "linuxcnc");
    assert_eq!(a.canonical_toolpath, b.canonical_toolpath);
    assert_eq!(a.gcode, b.gcode);
    assert!(retracts(facing(&a)) >= 15, "{}", retracts(facing(&a)));
    for dir in [saved, retract] {
        std::fs::remove_dir_all(dir).unwrap();
    }
}

/// With "Links between passes" low, End facing links each band to the next
/// beside the end, in the air the approach distance clear of the face, and
/// never goes back up to the retract height between bands (OP-14, decision
/// 0496's rule). The held start end is far away. Both posts accept it.
#[test]
fn req_op_14_req_set_09_end_facing_low_links_stay_beside_the_end() {
    let dir = project("low", "link_height = \"low\"\n");
    for name in ["makera-z1", "linuxcnc"] {
        let run = post(&dir, name);
        let group = facing(&run);
        assert_eq!(retracts(group), 0, "{name}");
        let mut previous = group.start;
        for segment in &group.segments {
            if !segment.kind.is_fed() && previous.tip.z.min(segment.end.tip.z) < STOCK_TOP {
                assert!(
                    previous.tip.x.min(segment.end.tip.x) - RADIUS >= STOCK_END + APPROACH - 1e-9,
                    "{name}: a low link passes over the stock"
                );
            }
            previous = segment.end;
        }
        assert!(
            !run.notes.iter().any(|n| n.contains("could not be lowered")),
            "{name}: {:?}",
            run.notes
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// The owner's decision F1 on issue #499: the Baluster's end is 35 mm deep
/// across the tool and the tool has 25 mm of flute, so End facing faces it
/// from A 0 and A 180, never cutting more than the flute length below the
/// stock's top, and both posts accept the program.
#[test]
fn req_op_24_req_nfr_12_baluster_end_is_faced_from_both_sides_within_the_flutes() {
    let dir = project("flutes", "");
    for name in ["makera-z1", "linuxcnc"] {
        let run = post(&dir, name);
        let group = facing(&run);
        let mut angles = Vec::new();
        for segment in &group.segments {
            if segment.kind == MoveKind::Cutting {
                let a = segment.end.a.0.rem_euclid(360.);
                if !angles.contains(&a) {
                    angles.push(a);
                }
                // The tip's height above the axis along the tool, at this A.
                let (sin, cos) = segment.end.a.0.to_radians().sin_cos();
                let height = segment.end.tip.z * cos + segment.end.tip.y * sin;
                assert!(
                    STOCK_TOP - height <= 25. + 1e-6,
                    "{name}: a cut {:.3} mm below the stock top at A {a}",
                    STOCK_TOP - height
                );
            }
        }
        angles.sort_by(f64::total_cmp);
        assert_eq!(angles.len(), 2, "{name}: {angles:?}");
        assert!(
            (angles[1] - angles[0] - 180.).abs() < 1e-6,
            "{name}: {angles:?}"
        );
        assert_eq!(run.gcode.len(), 1, "{name}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! The golden file audit: checks the committed G-code files against things
//! that do not come from the post or the path library, so a person does not
//! need to read them line by line (work breakdown slice 6 review checklist;
//! decision record 0020 item 14). Run it with `cargo xtask golden`.
//!
//! For every golden program it reads only the file and then:
//!
//! 1. **Conventions.** The start and end sequences, the codes that must and
//!    must not appear, ASCII, no line numbers, no `-0` and no `NaN`.
//! 2. **Gate 1.** The static checks against the Z1 profile's limits.
//! 3. **Geometry** (reference jobs). Each position is mapped back to the
//!    tool by the independent kinematic oracle and held against the exact
//!    analytic part: no cut goes deeper than 0.02 mm into the part, the ball
//!    rides on the surface, and A is continuous.
//! 4. **Feed** (reference jobs). The contact point's speed over the exact
//!    surface, from the file's own F words under the controller's feed rule,
//!    is the programmed 480 mm/min (12000 rpm x 2 flutes x 0.02 mm).
//! 5. **Agreement.** The LinuxCNC and Z1 programs of one job reach the same
//!    tool positions.
//!
//! The small part's files are checked against its closed forms.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::print_stdout
)]

use std::path::{Path, PathBuf};

use openchuck_app::gcode::{
    CheckConfig, Dialect, FeedLimits, Motion, Move, read, replay, static_check,
};
use openchuck_app::setup::{ARange, builtin};
use openchuck_testkit::gouge::{Outline, Pose, max_gouge, min_clearance};
use openchuck_testkit::oracle::{OracleMachine, model_tip};
use openchuck_testkit::parts::{Cone, HelicalGroove, LobedVase, RadialPart, Sphere};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;

fn golden(path: &str) -> (PathBuf, String) {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/golden")
        .join(path);
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    (p, text)
}

#[derive(Clone, Copy, PartialEq)]
enum Post {
    Z1,
    LinuxCnc,
}

impl Post {
    fn dialect(self) -> Dialect {
        if self == Self::Z1 {
            Dialect::MakeraZ1
        } else {
            Dialect::Linuxcnc
        }
    }
}

/// 1. Conventions: what each post's files must look like.
fn audit_conventions(name: &str, text: &str, post: Post) {
    assert!(text.is_ascii(), "{name}: not ASCII");
    assert!(!text.contains('\r'), "{name}: carriage returns");
    assert!(
        !text.contains("NaN") && !text.contains("inf"),
        "{name}: NaN or inf"
    );
    assert!(!text.contains("-0.000"), "{name}: negative zero");
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines.iter().all(|l| !l.starts_with('N')),
        "{name}: line numbers"
    );
    assert!(
        lines
            .iter()
            .all(|l| !l.contains("G93") && !l.contains("G43")),
        "{name}: G93 or G43"
    );
    // The header: title, optional revision, then the post's lines, then the work offset.
    let body: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| !l.starts_with('('))
        .collect();
    let tail_expected: &[&str] = if post == Post::Z1 {
        &["M5", "G53 G0 Z-3", "G28", "M2"]
    } else {
        &["M5", "M30"]
    };
    assert_eq!(
        &body[body.len() - tail_expected.len()..],
        tail_expected,
        "{name}: ending"
    );
    let head_expected: &[&str] = if post == Post::Z1 {
        &["G90 G21 G17", "G54", "M5", "T1 M6"]
    } else {
        &["G90 G21 G17", "G61", "G54", "M5", "T1 M6"]
    };
    assert_eq!(&body[..head_expected.len()], head_expected, "{name}: start");
    assert!(
        body[head_expected.len()].starts_with('S') && body[head_expected.len()].ends_with(" M3"),
        "{name}: spindle"
    );
    // Comments stand alone and hold no nested parentheses.
    for l in lines.iter().filter(|l| l.starts_with('(')) {
        assert!(
            l.ends_with(')') && l[1..l.len() - 1].find(['(', ')']).is_none(),
            "{name}: comment `{l}`"
        );
    }
    let drops = lines.iter().filter(|l| **l == "G92.4 A0 S0").count();
    if post == Post::Z1 {
        // A non-moving boundary reset is useful only beyond one complete
        // turn. In particular the firmware keeps exactly +/-360 unchanged.
        let parsed = read(text).unwrap();
        let played = replay(&parsed, Dialect::MakeraZ1).unwrap();
        let last = played.moves().filter(|m| !m.machine).last().unwrap();
        assert_eq!(
            drops,
            usize::from(last.to[3].abs() > 360.0),
            "{name}: reset only when the final physical counter needs it"
        );
    } else {
        assert_eq!(drops, 0, "{name}: LinuxCNC must not use G92.4");
    }
    // One spindle start and one tool change per program here.
    assert_eq!(
        lines.iter().filter(|l| l.contains("M6")).count(),
        1,
        "{name}: tool changes"
    );
}

fn z1_check_config() -> CheckConfig {
    let machine = builtin("makera-z1").unwrap().unwrap();
    let zero = openchuck_types::setup::expected_work_offset(
        &machine,
        &openchuck_types::setup::SetupFrame::default(),
    );
    CheckConfig {
        travel: [
            (machine.travel.min.x - zero.x, machine.travel.max.x - zero.x),
            (machine.travel.min.y - zero.y, machine.travel.max.y - zero.y),
            (machine.travel.min.z - zero.z, machine.travel.max.z - zero.z),
        ],
        a_range: match machine.a_range {
            ARange::Continuous => None,
            ARange::Limited { min, max } => Some((min.0, max.0)),
        },
        max_feed: FeedLimits {
            linear: machine.max_feeds.linear.0,
            z: machine.max_feeds.z.0,
            rotary: machine.max_feeds.rotary.0,
        },
        safe_z: machine.clearance.safe_z.0,
        spindle_off_at_tool_change: true,
        require_work_offset: true,
        allow_inverse_time: false,
    }
}

/// 2. Gate 1 on the file.
fn audit_static(name: &str, text: &str, post: Post) -> openchuck_app::gcode::Replay {
    let program = read(text).unwrap_or_else(|e| panic!("{name}: {}", e[0]));
    let replayed = replay(&program, post.dialect()).unwrap_or_else(|e| panic!("{name}: {e}"));
    let findings = static_check(&replayed, &z1_check_config());
    assert!(
        findings.is_empty(),
        "{name}: {:?}",
        findings
            .iter()
            .map(|f| format!("line {}: {}", f.line, f.message))
            .take(5)
            .collect::<Vec<_>>()
    );
    replayed
}

/// The controller's time for a fed move, by the file's own F.
fn move_seconds(m: &Move, post: Post) -> f64 {
    let f = m.feed.unwrap();
    let length = ((m.to[0] - m.from[0]).powi(2)
        + (m.to[1] - m.from[1]).powi(2)
        + (m.to[2] - m.from[2]).powi(2))
    .sqrt();
    let da = (m.to[3] - m.from[3]).abs();
    let amount = match post {
        Post::LinuxCnc => {
            if length > 1e-9 {
                length
            } else {
                da
            }
        }
        Post::Z1 => {
            let r = m.to[1].hypot(m.to[2]).max(1.0);
            let p = 2.0 * std::f64::consts::PI * r + 30.0;
            if length > 1e-9 {
                length.max(da * p / 360.0)
            } else {
                da * p.min(360.0) / 360.0
            }
        }
    };
    amount * 60.0 / f
}

/// The model-frame tip and tool axis of a position, by the oracle (positive A is counter-clockwise).
fn model_pose(m: &[f64; 4]) -> (Point3, Vec3) {
    let machine = OracleMachine {
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: 0.0,
        sense: 1.0,
    };
    let t = model_tip(&machine, m[0], m[1], m[2], m[3]);
    // The tool axis is machine +Z, polar angle 90 degrees; in the model it turns by -A.
    let phi = std::f64::consts::FRAC_PI_2 - m[3].to_radians();
    (
        Point3::new(t[0], t[1], t[2]),
        Vec3::new(0.0, phi.cos(), phi.sin()),
    )
}

/// Cutting moves: fed (G1), with motion in X and A (not the plunges and exits, which are along Z).
fn cutting_moves(r: &openchuck_app::gcode::Replay) -> Vec<Move> {
    r.moves()
        .filter(|m| m.motion == Motion::Linear && (m.to[3] - m.from[3]).abs() > 1e-9)
        .cloned()
        .collect()
}

/// The point of the exact part nearest to `c`, by a local grid search with refinement.
fn nearest_surface_point(part: &dyn RadialPart, c: Point3) -> Point3 {
    let (x0, x1) = part.x_range();
    let angle0 = c.z.atan2(c.y);
    let (mut best, mut best_d) = (Point3::ORIGIN, f64::INFINITY);
    let (mut cx, mut ca, mut wx, mut wa) = (c.x, angle0, 4.0, 0.9);
    for _ in 0..6 {
        for i in 0..=24 {
            for j in 0..=24 {
                let x = (cx - wx + 2.0 * wx * f64::from(i) / 24.0).clamp(x0, x1);
                let a = ca - wa + 2.0 * wa * f64::from(j) / 24.0;
                let rho = part.radius_at(x, a);
                let p = Point3::new(x, rho * a.cos(), rho * a.sin());
                let d = (p.x - c.x).powi(2) + (p.y - c.y).powi(2) + (p.z - c.z).powi(2);
                if d < best_d {
                    best_d = d;
                    best = p;
                    cx = x;
                    ca = a;
                }
            }
        }
        wx /= 5.0;
        wa /= 5.0;
    }
    best
}

fn dist(a: Point3, b: Point3) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

struct Job {
    name: &'static str,
    part: Box<dyn RadialPart>,
    /// The largest A step of a cutting move: the job's 5 degree angle step
    /// for helical finishing, where A is the drive angle; more for
    /// simultaneous finishing, where A follows the normal and turns faster
    /// than the drive point on a lobe's flank (decision record 0036).
    max_a_step: f64,
    /// Where the part's outline has a concave kink (model X): within
    /// [`KINK_ZONE`] of these the contact feed is not judged, because the
    /// ball's contact slides along the valley, as at the part's ends.
    kinks: &'static [f64],
}

/// Half-width of the zone round a concave kink where the contact feed is not
/// judged (mm).
const KINK_ZONE: f64 = 2.5;

/// The `cone_held_tabs` reference job's target: stock of diameter 20 from X
/// -12 to 42; the cylinder tab and held region at the start (radius 10), the
/// cone from X 0 to 30 (radius 10 to 5), the frustum tab from X 30 to 40
/// (radius 5 to 10) and the held region to X 42 (radius 10).
struct ConeHeld;

impl RadialPart for ConeHeld {
    fn name(&self) -> &'static str {
        "cone with held regions and tabs"
    }
    fn x_range(&self) -> (f64, f64) {
        (-12.0, 42.0)
    }
    fn radius_at(&self, x: f64, _angle: f64) -> f64 {
        if x <= 0.0 {
            10.0
        } else if x <= 30.0 {
            10.0 - 5.0 * x / 30.0
        } else if x <= 40.0 {
            5.0 + 5.0 * (x - 30.0) / 10.0
        } else {
            10.0
        }
    }
}

fn jobs() -> Vec<Job> {
    vec![
        Job {
            name: "groove",
            part: Box::new(HelicalGroove {
                radius: 12.0,
                length: 30.0,
                pitch: 12.0,
                depth: 1.5,
            }),
            max_a_step: 5.0,
            kinks: &[],
        },
        Job {
            name: "cone",
            part: Box::new(Cone {
                start_radius: 10.0,
                end_radius: 5.0,
                length: 30.0,
            }),
            max_a_step: 5.0,
            kinks: &[],
        },
        // Slice 21 (decision 0035): the cone with its held regions and tabs
        // as one surface of revolution, the exact target the audit holds the
        // tool to.
        Job {
            name: "cone_held_tabs",
            part: Box::new(ConeHeld),
            max_a_step: 5.0,
            kinks: &[30.0],
        },
        Job {
            name: "sphere",
            part: Box::new(Sphere { radius: 10.0 }),
            max_a_step: 5.0,
            kinks: &[],
        },
        Job {
            name: "lobed",
            part: Box::new(LobedVase {
                base: 10.0,
                wave: 1.5,
                wave_length: 40.0,
                lobe: 1.0,
                lobes: 3,
                length: 20.0,
            }),
            max_a_step: 10.0,
            kinks: &[],
        },
    ]
}

const BALL_RADIUS: f64 = 1.5;
const CONTACT_FEED: f64 = 12000.0 * 2.0 * 0.02;
/// Distance from either end of the part inside which the contact feed is not judged (mm).
const END_ZONE: f64 = 4.0;

/// 3 and 4. Geometry and feed against the exact part.
fn audit_geometry(
    name: &str,
    replayed: &openchuck_app::gcode::Replay,
    post: Post,
    part: &dyn RadialPart,
    max_a_step: f64,
    kinks: &[f64],
) -> Vec<Point3> {
    let ball = Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(2.0 * BALL_RADIUS),
        flute_length: Millimetres(8.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(3.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    };
    let outline = Outline::new(&ball.profile().unwrap());
    let cuts = cutting_moves(replayed);
    assert!(
        cuts.len() > 150,
        "{name}: only {} cutting moves",
        cuts.len()
    );
    // A is continuous: no step over what the job allows, never reversing.
    let mut direction = 0.0_f64;
    for m in &cuts {
        let da = m.to[3] - m.from[3];
        assert!(
            da.abs() <= max_a_step + 1e-6,
            "{name} line {}: A steps {da}",
            m.line
        );
        if direction == 0.0 {
            direction = da.signum();
        }
        assert!(da * direction > 0.0, "{name} line {}: A reverses", m.line);
    }
    let poses: Vec<Pose> = cuts
        .iter()
        .map(|m| {
            let (tip, axis) = model_pose(&m.to);
            Pose { tip, axis }
        })
        .collect();
    // No cut goes into the part.
    let gouge = max_gouge(&outline, part, &poses, 0.25);
    assert!(
        gouge.depth <= 0.02,
        "{name}: gouge {:.4} mm at pose {} ({:?})",
        gouge.depth,
        gouge.pose,
        gouge.at
    );
    // The ball rides on the surface: clearance about zero (no stock to leave).
    let mut clearances: Vec<f64> = poses
        .iter()
        .step_by(4)
        .map(|p| min_clearance(&outline, part, p, 0.25))
        .collect();
    clearances.sort_by(f64::total_cmp);
    let median = clearances[clearances.len() / 2];
    assert!(
        median.abs() <= 0.02,
        "{name}: median clearance {median:.4} mm"
    );
    assert!(
        clearances[0] >= -0.02,
        "{name}: minimum clearance {:.4}",
        clearances[0]
    );
    // Contact speed over the exact surface from the file's own F words.
    let rotary_limit = builtin("makera-z1").unwrap().unwrap().max_feeds.rotary.0;
    let contacts: Vec<Point3> = poses
        .iter()
        .map(|p| {
            let c = Point3::new(
                p.tip.x + p.axis.x * BALL_RADIUS,
                p.tip.y + p.axis.y * BALL_RADIUS,
                p.tip.z + p.axis.z * BALL_RADIUS,
            );
            nearest_surface_point(part, c)
        })
        .collect();
    let mut errors = Vec::new();
    for (k, m) in cuts.iter().enumerate().skip(1) {
        let before = nearest_start(&cuts, k, part);
        let length = dist(before, contacts[k]);
        let seconds = move_seconds(m, post);
        // Away from the part's ends, as in the pathing tests (OI-77): near a rim or a pole the
        // contact slides round the end and its speed is not what the feed is defined on.
        let (x0, x1) = part.x_range();
        let inside = contacts[k].x > x0 + END_ZONE
            && contacts[k].x < x1 - END_ZONE
            && kinks.iter().all(|c| (contacts[k].x - c).abs() > KINK_ZONE);
        if seconds > 0.0 && length > 0.0 && inside {
            let e = (length / seconds * 60.0) / CONTACT_FEED - 1.0;
            if std::env::var("AUDIT_DEBUG").is_ok() && k < 40 {
                println!(
                    "    move {k}: line {} contact {:.4} mm, {:.4} s, error {:+.2} %",
                    m.line,
                    length,
                    seconds,
                    e * 100.0
                );
            }
            if std::env::var("AUDIT_DEBUG").is_ok() && e.abs() > 0.05 {
                println!(
                    "    big: line {} x {:.2} contact {:.4} mm {:.4} s error {:+.1} %",
                    m.line,
                    contacts[k].x,
                    length,
                    seconds,
                    e * 100.0
                );
            }
            // A move may be slower than programmed only because an axis is at its feed limit:
            // then its time is what the limit allows (the path library lengthens such moves
            // and says so, decision 0018 item 8).
            if e < -0.02 {
                let limit_seconds = (m.to[3] - m.from[3]).abs() / (rotary_limit / 60.0);
                assert!(
                    (seconds - limit_seconds).abs() / limit_seconds <= 0.01,
                    "{name} line {}: the move is {:.1} % slower than programmed but takes {seconds:.4} s, not the {limit_seconds:.4} s the rotary limit allows",
                    m.line,
                    -e * 100.0
                );
                continue;
            }
            errors.push(e.abs());
        }
    }
    errors.sort_by(f64::total_cmp);
    let median = errors[errors.len() / 2];
    let p90 = errors[errors.len() * 9 / 10];
    let worst = errors[errors.len() - 1];
    assert!(
        median <= 0.02,
        "{name}: median contact feed error {:.2} %",
        median * 100.0
    );
    assert!(
        worst <= 0.025,
        "{name}: worst contact feed error {:.2} % away from the ends",
        worst * 100.0
    );
    println!(
        "  {name}: {} cutting moves, deepest gouge {:.4} mm, median clearance {median_c:.4} mm, contact feed error median {:.2} % / 90th {:.2} % / worst {:.2} % over {} moves away from the ends",
        cuts.len(),
        gouge.depth,
        errors[errors.len() / 2] * 100.0,
        p90 * 100.0,
        worst * 100.0,
        errors.len(),
        median_c = clearances[clearances.len() / 2]
    );
    // Contact points are what the two posts must agree on.
    contacts
}

/// The contact point at the start of move `k` (the end of move `k - 1`, or of the move's own `from`).
fn nearest_start(cuts: &[Move], k: usize, part: &dyn RadialPart) -> Point3 {
    let (tip, axis) = model_pose(&cuts[k].from);
    let c = Point3::new(
        tip.x + axis.x * BALL_RADIUS,
        tip.y + axis.y * BALL_RADIUS,
        tip.z + axis.z * BALL_RADIUS,
    );
    nearest_surface_point(part, c)
}

#[test]
fn req_gc_09_req_gc_10_req_nfr_08_reference_job_golden_files_pass_the_audit() {
    for job in jobs() {
        let mut contacts = Vec::new();
        for (post, file) in [
            (Post::Z1, format!("reference/{}_makera-z1.nc", job.name)),
            (
                Post::LinuxCnc,
                format!("reference/{}_linuxcnc.ngc", job.name),
            ),
        ] {
            let (_, text) = golden(&file);
            audit_conventions(&file, &text, post);
            let replayed = audit_static(&file, &text, post);
            println!("{file}:");
            contacts.push(audit_geometry(
                &file,
                &replayed,
                post,
                job.part.as_ref(),
                job.max_a_step,
                job.kinks,
            ));
        }
        // Agreement: the two posts reach the same tool positions on the exact surface.
        assert_eq!(
            contacts[0].len(),
            contacts[1].len(),
            "{}: different numbers of cutting moves",
            job.name
        );
        let worst = contacts[0]
            .iter()
            .zip(&contacts[1])
            .map(|(a, b)| dist(*a, *b))
            .fold(0.0, f64::max);
        assert!(
            worst <= 0.005,
            "{}: the posts' contact points differ by {worst:.4} mm",
            job.name
        );
    }
}

#[test]
fn req_gc_09_req_gc_10_req_nfr_08_small_part_golden_files_match_their_closed_forms() {
    for (post, file) in [
        (Post::Z1, "post_makera_z1_small_part.ngc"),
        (Post::LinuxCnc, "post_linuxcnc_small_part.ngc"),
    ] {
        let (_, text) = golden(file);
        audit_conventions(file, &text, post);
        let replayed = audit_static(file, &text, post);
        let cuts = cutting_moves(&replayed);
        assert_eq!(cuts.len(), 24, "{file}");
        for (i, m) in cuts.iter().enumerate() {
            let step = (i + 1) as f64;
            // Closed form: the tip rides the top of a 10 mm radius at X = 0.25 step, A = 45 step.
            let (tip, _) = model_pose(&m.to);
            assert!(
                (tip.x - 0.25 * step).abs() < 1e-6,
                "{file} move {}: X {}",
                i + 1,
                tip.x
            );
            assert!(
                (m.to[3] - 45.0 * step).abs() < 1e-6,
                "{file} move {}: A {}",
                i + 1,
                m.to[3]
            );
            assert!(
                (tip.y.hypot(tip.z) - 10.0).abs() < 0.001,
                "{file} move {}: radius {}",
                i + 1,
                tip.y.hypot(tip.z)
            );
            assert!(
                tip.y.abs() < 0.001 && (tip.z - 10.0).abs() < 0.001,
                "{file} move {}: the tip leaves the top ({}, {})",
                i + 1,
                tip.y,
                tip.z
            );
            // The IR said every move takes 1 s; the controller's time from F is that, within F's rounding.
            let seconds = move_seconds(m, post);
            assert!(
                (seconds - 1.0).abs() < 0.005,
                "{file} move {}: {seconds:.4} s",
                i + 1
            );
        }
        println!(
            "{file}: 24 cutting moves match their closed forms; every move takes 1 s under the controller's rule"
        );
    }
}

/// The baseline finishing jobs of slice 12 (decision 0034), on the cone of
/// the `cone` job.
const BASELINE_JOBS: [&str; 3] = ["cone_axial", "cone_indexed", "cone_axial_lead"];

/// Fed moves that run along X: the cuts of an axial or indexed pass, and its
/// lead arcs.
fn x_moves(r: &openchuck_app::gcode::Replay) -> Vec<Move> {
    r.moves()
        .filter(|m| {
            m.motion != Motion::Rapid && m.feed.is_some() && (m.to[0] - m.from[0]).abs() > 1e-9
        })
        .cloned()
        .collect()
}

#[test]
fn req_op_04_baseline_finishing_golden_files_pass_the_audit() {
    let part = Cone {
        start_radius: 10.0,
        end_radius: 5.0,
        length: 30.0,
    };
    let ball = Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(2.0 * BALL_RADIUS),
        flute_length: Millimetres(8.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(3.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    };
    let outline = Outline::new(&ball.profile().unwrap());
    for name in BASELINE_JOBS {
        let mut ends = Vec::new();
        for (post, file) in [
            (Post::Z1, format!("reference/{name}_makera-z1.nc")),
            (Post::LinuxCnc, format!("reference/{name}_linuxcnc.ngc")),
        ] {
            let (_, text) = golden(&file);
            audit_conventions(&file, &text, post);
            let replayed = audit_static(&file, &text, post);
            let moves = x_moves(&replayed);
            assert!(moves.len() > 30, "{file}: only {} moves", moves.len());
            // Within an axial or indexed pass A does not move.
            for m in &moves {
                if name != "cone_axial_lead" {
                    assert!(
                        (m.to[3] - m.from[3]).abs() < 1e-6,
                        "{file} line {}: A moves in a pass",
                        m.line
                    );
                }
            }
            let poses: Vec<Pose> = moves
                .iter()
                .map(|m| {
                    let (tip, axis) = model_pose(&m.to);
                    Pose { tip, axis }
                })
                .collect();
            let gouge = max_gouge(&outline, &part, &poses, 0.25);
            assert!(gouge.depth <= 0.02, "{file}: gouge {:.4} mm", gouge.depth);
            let mut clearances: Vec<f64> = poses
                .iter()
                .step_by(4)
                .map(|p| min_clearance(&outline, &part, p, 0.25))
                .collect();
            clearances.sort_by(f64::total_cmp);
            assert!(
                clearances[0] >= -0.02,
                "{file}: clearance {:.4}",
                clearances[0]
            );
            println!(
                "{file}: {} fed moves along X, deepest gouge {:.4} mm, minimum clearance {:.4} mm",
                moves.len(),
                gouge.depth,
                clearances[0]
            );
            ends.push(poses);
        }
        // The two posts reach the same tool positions.
        assert_eq!(ends[0].len(), ends[1].len(), "{name}");
        let worst = ends[0]
            .iter()
            .zip(&ends[1])
            .map(|(a, b)| dist(a.tip, b.tip))
            .fold(0.0, f64::max);
        assert!(worst <= 0.005, "{name}: the posts differ by {worst:.4} mm");
    }
}

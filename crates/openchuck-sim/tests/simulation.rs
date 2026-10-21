// SPDX-License-Identifier: AGPL-3.0-or-later
//! Slice 7 review checklist: material removal against independent
//! references (SIM-03, SIM-05, SIM-06, SIM-07, NFR-06). The references are
//! closed forms and quadratures in `support`, not the simulator's code.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod support;

use std::f64::consts::PI;
use std::sync::Mutex;

use openchuck_sim::{
    DeviationOptions, DexelStock, Pose, SampleOn, Simulator, ToolSolid, compare,
    compare_cancellable, compare_classified, cut_spans, stock_mesh, triangle_values,
};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::Error;
use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::ir::{MoveKind, ToolpathGroup};
use openchuck_types::job::{CancelToken, JobContext, NoProgress, Progress, ProgressSink};
use openchuck_types::simulation::SimulationSettings;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{ProfilePoint, ProfileSegment, ToolProfile, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};

use support::*;

fn run(
    stock: DexelStock,
    groups: &[(&ToolpathGroup, &openchuck_types::tool::RevolvedProfile)],
) -> (Simulator, Vec<openchuck_sim::GroupResult>) {
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let mut sim = Simulator::new(stock, &machine(), None);
    let results = groups
        .iter()
        .map(|(g, p)| sim.run_group(g, p, &ctx).unwrap())
        .collect();
    (sim, results)
}

struct TurnedCase {
    name: &'static str,
    part: Box<dyn Turned>,
    zone: (f64, f64),
}

fn turned_cases() -> Vec<TurnedCase> {
    vec![
        TurnedCase {
            name: "cylinder",
            part: Box::new(Cyl(10.0)),
            zone: (-8.0, 8.0),
        },
        TurnedCase {
            name: "cone",
            part: Box::new(ConeT(8.0, 0.25)),
            zone: (-8.0, 8.0),
        },
        TurnedCase {
            name: "sphere",
            part: Box::new(Sph(11.0)),
            zone: (-7.0, 7.0),
        },
    ]
}

/// Turns `case` at `cell` and returns (volume error, bound, worst radial
/// error, stock memory in bytes, poses).
fn turn(case: &TurnedCase, cell: f64, dx: f64, da: f64) -> (f64, f64, f64, usize, usize) {
    let m = machine();
    let profile = ball(4.0);
    let poses = turning_poses(&m, case.part.as_ref(), 2.0, case.zone, dx, da);
    let g = group(case.name, &poses, MoveKind::Cutting);
    let stock = DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, cell).unwrap();
    let (sim, results) = run(stock, &[(&g, &profile)]);
    let stock = sim.stock();
    // Inner region, clear of the zone's ends where the ball rounds off.
    let (xa, xb) = (case.zone.0 + 3.0, case.zone.1 - 3.0);
    let region = Aabb {
        min: Point3::new(xa, -20.0, -20.0),
        max: Point3::new(xb, 20.0, 20.0),
    };
    // Columns count by their centre: snap the region to whole columns.
    let (cx, _) = stock.cell();
    let x0 = stock.bounds().min.x;
    let (ia, ib) = (((xa - x0) / cx).round(), ((xb - x0) / cx).round());
    let (sa, sb) = (x0 + ia * cx, x0 + ib * cx);
    let expected = integrate(|x| PI * case.part.radius(x).powi(2), sa, sb);
    let lateral = integrate(
        |x| 2.0 * PI * case.part.radius(x) * (1.0 + case.part.slope(x).powi(2)).sqrt(),
        sa,
        sb,
    );
    let got = stock.region_volume(&region);
    // Radial probes from outside the stock towards the axis.
    let mut worst: f64 = 0.0;
    for i in 0..40 {
        let x = sa + (sb - sa) * (i as f64 + 0.5) / 40.0;
        for k in 0..36 {
            let t = (k as f64) * 10.0_f64.to_radians();
            let (c, s) = (t.cos(), t.sin());
            let start = Point3::new(x, 16.0 * c, 16.0 * s);
            let hit = stock
                .first_material(
                    start,
                    openchuck_types::geometry::Vec3::new(0.0, -c, -s),
                    16.0,
                )
                .unwrap_or(16.0);
            let r = 16.0 - hit;
            worst = worst.max((r - case.part.radius(x)).abs());
        }
    }
    (
        got - expected,
        lateral * cell,
        worst,
        stock.memory_bytes(),
        results[0].poses,
    )
}

#[test]
fn req_sim_03_turned_parts_match_the_analytic_volume_within_one_cell() {
    for case in turned_cases() {
        let (err, bound, radial, _, _) = turn(&case, 0.1, 0.5, 2.0);
        println!(
            "{}: volume error {err:+.3} mm3 (bound {bound:.3}), worst radial error {radial:.4} mm",
            case.name
        );
        assert!(err.abs() <= bound, "{}: {err} beyond {bound}", case.name);
        // Within one cell, plus the ball's scallop between 0.5 mm stations
        // (0.5^2 / 16 = 0.016 mm).
        assert!(
            radial <= 0.1 + 0.016,
            "{}: radial error {radial}",
            case.name
        );
    }
}

/// The groove cross-section of a tool dragged along X with its tip `depth`
/// below the top of a flat stock: the area between the stock top and the
/// tool's lower envelope over its width.
fn groove_area(h: impl Fn(f64) -> f64, radius: f64, depth: f64) -> f64 {
    integrate(|y| (depth - h(y.abs())).max(0.0), -radius, radius)
}

#[test]
fn req_sim_03_straight_pass_by_tool_type_matches_the_closed_form() {
    let r = 3.0;
    let depth = 2.0;
    let half_v = 45f64.to_radians();
    let custom = ToolProfile {
        segments: vec![ProfileSegment::Arc {
            to: ProfilePoint::new(3.0, 3.0),
            centre: ProfilePoint::new(0.0, 3.0),
        }],
    };
    type Envelope = Box<dyn Fn(f64) -> f64>;
    let cases: Vec<(&str, ToolShape, Envelope)> = vec![
        ("flat", ToolShape::FlatEnd, Box::new(|_| 0.0)),
        (
            "ball",
            ToolShape::BallEnd,
            Box::new(move |q: f64| r - (r * r - q * q).max(0.0).sqrt()),
        ),
        (
            "bull nose",
            ToolShape::BullNose {
                corner_radius: Millimetres(1.0),
            },
            Box::new(|q: f64| {
                if q <= 2.0 {
                    0.0
                } else {
                    1.0 - (1.0 - (q - 2.0).powi(2)).max(0.0).sqrt()
                }
            }),
        ),
        (
            "v-bit",
            ToolShape::VBit {
                included_angle: Degrees(90.0),
                tip_diameter: Millimetres(0.0),
            },
            Box::new(move |q: f64| q / half_v.tan()),
        ),
        (
            "chamfer",
            ToolShape::Chamfer {
                included_angle: Degrees(90.0),
                tip_diameter: Millimetres(1.0),
            },
            Box::new(move |q: f64| (q - 0.5).max(0.0) / half_v.tan()),
        ),
        (
            "drill",
            ToolShape::Drill {
                point_angle: Degrees(118.0),
            },
            Box::new(|q: f64| q / 59f64.to_radians().tan()),
        ),
        (
            "custom (a ball as an arc)",
            ToolShape::Custom { profile: custom },
            Box::new(move |q: f64| r - (r * r - q * q).max(0.0).sqrt()),
        ),
    ];
    let cell = 0.1;
    for (name, shape, h) in cases {
        let profile = tool(shape, 2.0 * r).profile().unwrap();
        let stock = Stock::Box {
            bounds: Aabb {
                min: Point3::new(0.0, -6.0, -5.0),
                max: Point3::new(20.0, 6.0, 0.0),
            },
        };
        let stock = DexelStock::from_stock(&stock, None, cell).unwrap();
        let before = stock.volume();
        // A pass along X at A = 0 (tool along model +Z), tip `depth` below the top.
        let at = |x: f64| openchuck_types::ir::ToolPose {
            tip: Point3::new(x, 0.0, -depth),
            axis: openchuck_types::geometry::UnitVec3::Z,
            a: Degrees(0.0),
        };
        let g = group(name, &[at(-5.0), at(25.0)], MoveKind::Cutting);
        let (sim, _) = run(stock, &[(&g, &profile)]);
        let removed = before - sim.stock().volume();
        let area = groove_area(&h, r, depth);
        let expected = area * 20.0;
        // One cell of error over the groove's wetted outline, along its length.
        let outline = 2.0 * r + 2.0 * depth;
        let bound = outline * cell * 20.0;
        println!("{name}: removed {removed:.3} mm3, expected {expected:.3} mm3, bound {bound:.3}");
        assert!(
            (removed - expected).abs() <= bound,
            "{name}: {removed} vs {expected}"
        );
    }
}

#[test]
fn req_sim_03_a_custom_profile_cuts_like_the_equivalent_standard_tool() {
    let ball_profile = ball(6.0);
    let custom = tool(
        ToolShape::Custom {
            profile: ToolProfile {
                segments: vec![ProfileSegment::Arc {
                    to: ProfilePoint::new(3.0, 3.0),
                    centre: ProfilePoint::new(0.0, 3.0),
                }],
            },
        },
        6.0,
    )
    .profile()
    .unwrap();
    let m = machine();
    let poses = turning_poses(&m, &Cyl(10.0), 3.0, (-5.0, 5.0), 1.0, 5.0);
    let g = group("op", &poses, MoveKind::Cutting);
    let make = || DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, 0.2).unwrap();
    let (a, _) = run(make(), &[(&g, &ball_profile)]);
    let (b, _) = run(make(), &[(&g, &custom)]);
    assert_eq!(a.stock().state_hash(), b.stock().state_hash());
}

#[test]
fn req_sim_03_the_groove_does_not_depend_on_where_the_part_has_turned() {
    // A pass along X into cylinder stock at A = 0 and at A = 37 degrees: the
    // stock is round, so the removed volume is the same; the tool's axis
    // tilts in the model frame at 37.
    let m = machine();
    let profile = ball(4.0);
    let removed = |a: f64| {
        let poses = [turned_pose(&m, -3.0, 9.0, a), turned_pose(&m, 15.0, 9.0, a)];
        let g = group("op", &poses, MoveKind::Cutting);
        let stock = DexelStock::from_stock(&cylinder_stock(20.0, 0.0, 12.0), None, 0.1).unwrap();
        let before = stock.volume();
        let (sim, _) = run(stock, &[(&g, &profile)]);
        before - sim.stock().volume()
    };
    let (a0, a37) = (removed(0.0), removed(37.0));
    println!("A = 0: {a0:.4} mm3, A = 37: {a37:.4} mm3");
    assert!(a0 > 1.0);
    // One cell over the groove's outline (about 2 x 4 mm) along 12 mm.
    assert!((a0 - a37).abs() <= 8.0 * 0.1 * 12.0, "{a0} vs {a37}");
}

#[test]
fn req_sim_05_triangle_values_put_the_overcut_on_the_triangles_it_is_on() {
    let m = machine();
    let profile = ball(4.0);
    let target = Cylinder {
        radius: 10.0,
        length: 16.0,
    };
    let mut poses = turning_poses(&m, &Cyl(10.0), 2.0, (0.0, 16.0), 0.5, 3.0);
    for k in 0..40 {
        let p = &mut poses[16 * 120 + k];
        let a = p.a.0;
        *p = turned_pose(&m, 8.0, 9.5, a);
    }
    let g = group("gouge", &poses, MoveKind::Cutting);
    let stock = DexelStock::from_stock(&cylinder_stock(26.0, -2.0, 20.0), None, 0.1).unwrap();
    let (sim, _) = run(stock, &[(&g, &profile)]);
    let mesh = target.mesh(161, 180);
    let options = DeviationOptions::for_cell(0.1);
    let d = compare(sim.stock(), &mesh, &options).unwrap();
    let values = triangle_values(&d, &mesh, options.spacing).unwrap();
    assert_eq!(values.len(), mesh.triangles.len());
    // The triangles with the deepest overcut are the ones at the dip, and
    // the value is the worst of their samples.
    let (deepest, deepest_value) = values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.map(|v| (i, v.0)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    let [a, b, c] = mesh.triangle(deepest).unwrap();
    let x = (a.x + b.x + c.x) / 3.0;
    assert!((x - 8.0).abs() <= 2.0 + 0.1, "centroid x {x}");
    assert!((-deepest_value - 0.5).abs() <= 0.12, "{deepest_value}");
    assert!(
        (deepest_value - d.worst_overcut.unwrap().value).abs() < 1e-12,
        "the worst triangle carries the worst sample"
    );
    // Away from the dip no triangle is overcut beyond the limit.
    for (i, v) in values.iter().enumerate() {
        let [a, b, c] = mesh.triangle(i).unwrap();
        let x = (a.x + b.x + c.x) / 3.0;
        if let Some((value, _)) = v
            && !(3.0..=13.0).contains(&x)
            && x > 0.5
            && x < 15.5
        {
            assert!(-value <= options.overcut_limit, "{i} at x {x}: {value}");
        }
    }
    // A comparison made at another spacing does not fit this mesh.
    assert!(triangle_values(&d, &mesh, 0.01).is_err());
    assert!(triangle_values(&d, &target.mesh(20, 20), options.spacing).is_err());
}

#[test]
fn req_sim_05_a_gouging_path_is_reported_as_overcut_with_a_location() {
    let m = machine();
    let profile = ball(4.0);
    let target = Cylinder {
        radius: 10.0,
        length: 16.0,
    };
    // Shift the testkit cylinder (x 0..16) to the stock's frame by turning
    // from x = 0 to 16.
    let mut poses = turning_poses(&m, &Cyl(10.0), 2.0, (0.0, 16.0), 0.5, 3.0);
    // A dip: at the station x = 8, a third of a turn cuts 0.5 mm deep.
    let station = 16;
    let per_turn = 120;
    for k in 0..40 {
        let p = &mut poses[station * per_turn + k];
        let a = p.a.0;
        *p = turned_pose(&m, 8.0, 9.5, a);
    }
    let g = group("gouge", &poses, MoveKind::Cutting);
    let stock = DexelStock::from_stock(&cylinder_stock(26.0, -2.0, 20.0), None, 0.1).unwrap();
    let (sim, _) = run(stock, &[(&g, &profile)]);
    let mesh = target.mesh(161, 180);
    let options = DeviationOptions::for_cell(0.1);
    let d = compare(sim.stock(), &mesh, &options).unwrap();
    let worst = d.worst_overcut.unwrap();
    println!(
        "worst overcut {:.4} mm at {:?}; {} samples beyond {} mm; places {:?}",
        -worst.value,
        worst.point,
        d.overcut_count,
        d.overcut_limit,
        d.overcut_places.iter().map(|p| p.point).collect::<Vec<_>>()
    );
    assert!((-worst.value - 0.5).abs() <= 0.1 + 0.02, "{}", worst.value);
    assert!((worst.point.x - 8.0).abs() <= 2.0 + 0.1);
    assert!(!d.overcut_places.is_empty());
    for p in &d.overcut_places {
        assert!((p.point.x - 8.0).abs() <= 2.0 + 0.1, "{:?}", p.point);
    }

    // The same path without the dip: nothing beyond one cell, and the
    // material left on the surface is within a cell plus the scallop.
    let poses = turning_poses(&m, &Cyl(10.0), 2.0, (0.0, 16.0), 0.5, 3.0);
    let g = group("clean", &poses, MoveKind::Cutting);
    let stock = DexelStock::from_stock(&cylinder_stock(26.0, -2.0, 20.0), None, 0.1).unwrap();
    let (sim, _) = run(stock, &[(&g, &profile)]);
    let d = compare(sim.stock(), &mesh, &options).unwrap();
    println!(
        "clean: worst overcut {:?}, worst remaining {:?} mm (inner)",
        d.worst_overcut.map(|p| p.value),
        d.points
            .iter()
            .filter(|p| p.point.x > 3.0 && p.point.x < 13.0)
            .map(|p| p.value)
            .fold(f64::MIN, f64::max)
    );
    assert_eq!(d.overcut_count, 0);
    let inner_remaining = d
        .points
        .iter()
        .filter(|p| p.point.x > 3.0 && p.point.x < 13.0)
        .map(|p| p.value)
        .fold(f64::MIN, f64::max);
    assert!(inner_remaining <= 0.1 + 0.02, "{inner_remaining}");
}

#[test]
fn req_sim_06_operation_two_sees_the_stock_left_by_operation_one() {
    use openchuck_path::StockModel;
    let m = machine();
    let profile = ball(4.0);
    let rough = turning_poses(&m, &Cyl(11.0), 2.0, (-6.0, 6.0), 1.0, 5.0);
    let finish = turning_poses(&m, &Cyl(10.0), 2.0, (-6.0, 6.0), 1.0, 5.0);
    let g1 = group("rough", &rough, MoveKind::Cutting);
    let g2 = group("finish", &finish, MoveKind::Cutting);
    let make = || DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, 0.2).unwrap();
    // Finishing alone, then after roughing.
    let (_, alone) = run(make(), &[(&g2, &profile)]);
    let (sim, both) = run(make(), &[(&g1, &profile), (&g2, &profile)]);
    println!(
        "finish alone removes {:.1} mm3; after roughing {:.1} mm3 (roughing {:.1})",
        alone[0].removed_volume, both[1].removed_volume, both[0].removed_volume
    );
    assert!(both[1].removed_volume < 0.2 * alone[0].removed_volume);
    // The sum is the same material either way, within the cells it touched.
    let total = both[0].removed_volume + both[1].removed_volume;
    assert!((total - alone[0].removed_volume).abs() < 0.01 * total);
    // Later operations query the stock through the pathing port: from above
    // the part at x = 0 the material now starts near radius 10.
    let down = unit(0.0, 0.0, -1.0);
    let t = sim
        .stock()
        .ray_entry(Point3::new(0.05, 0.05, 20.0), down, 40.0)
        .unwrap();
    assert!((20.0 - t - 10.0).abs() < 0.2 + 0.05, "{t}");
}

#[test]
fn req_sim_06_jumping_to_a_move_matches_running_up_to_it() {
    let m = machine();
    let profile = ball(4.0);
    let rough = turning_poses(&m, &Cyl(11.0), 2.0, (-4.0, 4.0), 1.0, 10.0);
    let finish = turning_poses(&m, &Cyl(10.0), 2.0, (-4.0, 4.0), 1.0, 10.0);
    let g1 = group("rough", &rough, MoveKind::Cutting);
    let g2 = group("finish", &finish, MoveKind::Cutting);
    let make = || DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, 0.2).unwrap();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let mut sim = Simulator::new(make(), &m, Some(100));
    sim.run_group(&g1, &profile, &ctx).unwrap();
    sim.run_group(&g2, &profile, &ctx).unwrap();
    assert!(sim.checkpoints().len() > 3);
    let groups = [(&g1, &profile), (&g2, &profile)];
    for (group_index, moves) in [(0, 0), (0, 150), (1, 0), (1, 37), (1, 250)] {
        let jumped = sim.stock_at(group_index, moves, &groups, &ctx).unwrap();
        // The same moves run from the start.
        let mut direct = Simulator::new(make(), &m, None);
        if group_index == 1 {
            direct.run_group(&g1, &profile, &ctx).unwrap();
        }
        let g = if group_index == 0 { &g1 } else { &g2 };
        let mut partial = g.clone();
        partial.segments.truncate(moves);
        direct.run_group(&partial, &profile, &ctx).unwrap();
        assert_eq!(
            jumped.state_hash(),
            direct.stock().state_hash(),
            "group {group_index}, move {moves}"
        );
    }
}

#[test]
fn req_sim_07_finer_resolution_lowers_the_error_and_costs_more() {
    let case = &turned_cases()[1];
    assert_eq!(SimulationSettings::default().cell, Millimetres(0.1));
    let mut rows = Vec::new();
    for cell in [0.2, 0.1, 0.05] {
        let start = std::time::Instant::now();
        let (err, bound, radial, bytes, poses) = turn(case, cell, 0.5, 2.0);
        let secs = start.elapsed().as_secs_f64();
        println!(
            "cell {cell}: volume error {err:+.3} (bound {bound:.3}), radial {radial:.4} mm, \
             {:.1} MB, {poses} poses, {secs:.2} s",
            bytes as f64 / 1e6
        );
        rows.push((cell, radial, bytes, poses));
    }
    for w in rows.windows(2) {
        let (coarse, fine) = (w[0], w[1]);
        assert!(
            fine.1 < coarse.1,
            "radial error must fall: {coarse:?} -> {fine:?}"
        );
        assert!(
            fine.2 > coarse.2,
            "memory must grow: {coarse:?} -> {fine:?}"
        );
        assert!(fine.3 > coarse.3, "poses must grow: {coarse:?} -> {fine:?}");
    }
}

#[test]
fn req_sim_03_req_sim_05_finest_setting_is_within_two_hundredths_against_a_brute_force_run() {
    // Slice 7 item 7 and OI-100: a short zone at the finest supported cell,
    // against the closed form and against the default cell.
    let case = TurnedCase {
        name: "cone",
        part: Box::new(ConeT(8.0, 0.25)),
        zone: (-4.0, 4.0),
    };
    let fine = openchuck_types::simulation::FINEST_CELL;
    let (err_f, bound_f, radial_f, bytes_f, _) = turn(&case, fine, 0.25, 1.0);
    let (err_d, bound_d, radial_d, _, _) = turn(&case, 0.1, 0.25, 1.0);
    println!(
        "cell {fine}: volume error {err_f:+.3} (bound {bound_f:.3}), radial {radial_f:.4} mm, \
         {:.1} MB; cell 0.1: {err_d:+.3} (bound {bound_d:.3}), radial {radial_d:.4}",
        bytes_f as f64 / 1e6
    );
    assert!(radial_f <= 0.02, "finest cell radial error {radial_f}");
    assert!(err_f.abs() <= bound_f);
    // The default run agrees with the brute-force one within its own cell.
    assert!((err_d - err_f).abs() <= bound_d);
}

fn determinism_job() -> Vec<u8> {
    let m = machine();
    let profile = ball(4.0);
    let v = tool(
        ToolShape::VBit {
            included_angle: Degrees(60.0),
            tip_diameter: Millimetres(0.0),
        },
        4.0,
    )
    .profile()
    .unwrap();
    let a = turning_poses(&m, &ConeT(9.0, 0.2), 2.0, (-5.0, 5.0), 0.5, 4.0);
    let b = turning_poses(&m, &Cyl(9.5), 2.0, (-3.0, 3.0), 1.0, 6.0);
    let g1 = group("a", &a, MoveKind::Cutting);
    let g2 = group("b", &b, MoveKind::Cutting);
    let stock = DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, 0.1).unwrap();
    let (sim, results) = run(stock, &[(&g1, &profile), (&g2, &v)]);
    let mut out = sim.stock().state_hash().to_le_bytes().to_vec();
    for r in results {
        out.extend(r.removed_volume.to_bits().to_le_bytes());
        out.extend((r.poses as u64).to_le_bytes());
    }
    let d = compare(
        sim.stock(),
        &Cylinder {
            radius: 9.0,
            length: 10.0,
        }
        .mesh(41, 90),
        &DeviationOptions::for_cell(0.1),
    )
    .unwrap();
    for p in d.points {
        out.extend(p.value.to_bits().to_le_bytes());
    }
    let mesh = stock_mesh(sim.stock());
    out.extend((mesh.triangles.len() as u64).to_le_bytes());
    out
}

#[test]
fn req_nfr_06_simulation_is_identical_at_1_2_and_n_threads() {
    openchuck_testkit::determinism::check(determinism_job).unwrap();
}

#[derive(Default)]
struct Recorder(Mutex<Vec<Progress>>);
impl ProgressSink for Recorder {
    fn report(&self, progress: Progress) {
        self.0.lock().unwrap().push(progress);
    }
}

#[test]
fn req_nfr_04_simulation_reports_progress_and_can_be_cancelled() {
    let m = machine();
    let profile = ball(4.0);
    let poses = turning_poses(&m, &Cyl(10.0), 2.0, (-8.0, 8.0), 0.25, 1.0);
    let g = group("op", &poses, MoveKind::Cutting);
    let make = || DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, 0.2).unwrap();
    let recorder = Recorder::default();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &recorder);
    let mut sim = Simulator::new(make(), &m, None);
    let result = sim.run_group(&g, &profile, &ctx).unwrap();
    let reports = recorder.0.lock().unwrap();
    // At least one report per batch of poses, never going backwards.
    assert!(reports.len() > result.poses / openchuck_sim::BATCH_POSES);
    assert!(reports.windows(2).all(|w| w[0].done <= w[1].done));
    let last = reports.last().unwrap();
    assert_eq!(last.done, g.segments.len() as u64);
    assert_eq!(last.total, Some(g.segments.len() as u64));
    drop(reports);

    let cancelled = CancelToken::new();
    cancelled.cancel();
    let ctx = JobContext::new(&cancelled, &NoProgress);
    let mut sim = Simulator::new(make(), &m, None);
    assert_eq!(sim.run_group(&g, &profile, &ctx), Err(Error::Cancelled));
}

/// A batch of poses can take over a second to cut, so the cut watches the
/// cancel flag itself instead of waiting for the end of the batch, and so
/// does the comparison with the part (UI-09, NFR-04).
#[test]
#[allow(clippy::float_cmp)] // the same code path must give the same bits
fn req_nfr_04_cutting_and_comparing_stop_when_cancelled() {
    let profile = ball(4.0);
    let tool = ToolSolid::new(&profile);
    let poses: Vec<Pose> = (0..256)
        .map(|i| Pose {
            tip: Point3::new(-10.0 + i as f64 * 0.08, 0.0, 0.0),
            axis: Vec3::new(0.0, 0.0, 1.0),
        })
        .collect();
    let make = || DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, 0.2).unwrap();

    // Not cancelled, it cuts exactly as `cut` does.
    let free = CancelToken::new();
    let (mut watched, mut plain) = (make(), make());
    let removed = watched.cut_cancellable(&tool, &poses, &free).unwrap();
    assert!(removed > 0.0);
    assert_eq!(removed, plain.cut(&tool, &poses));
    assert_eq!(watched.volume(), plain.volume());

    // Cancelled before it starts, it gives up at once and cuts nothing.
    let cancelled = CancelToken::new();
    cancelled.cancel();
    let mut stopped = make();
    assert_eq!(
        stopped.cut_cancellable(&tool, &poses, &cancelled),
        Err(Error::Cancelled)
    );
    assert_eq!(stopped.volume(), make().volume());

    // The comparison with the part stops too.
    let target = Cylinder {
        radius: 10.0,
        length: 16.0,
    };
    let mesh = target.mesh(41, 60);
    let options = DeviationOptions::for_cell(0.2);
    let ctx = JobContext::new(&cancelled, &NoProgress);
    assert_eq!(
        compare_cancellable(&watched, &mesh, &options, &ctx),
        Err(Error::Cancelled)
    );
    let ctx = JobContext::new(&free, &NoProgress);
    assert_eq!(
        compare_cancellable(&watched, &mesh, &options, &ctx).unwrap(),
        compare(&watched, &mesh, &options).unwrap()
    );
}

#[test]
fn req_sim_03_a_rapid_through_the_stock_is_reported() {
    let m = machine();
    let profile = ball(4.0);
    let mut g = group(
        "op",
        &[
            turned_pose(&m, -20.0, 20.0, 0.0),
            turned_pose(&m, -20.0, 9.0, 0.0),
        ],
        MoveKind::Rapid,
    );
    // A rapid along X through the stock at radius 9.
    g.segments
        .push(segment(MoveKind::Rapid, turned_pose(&m, 20.0, 9.0, 0.0)));
    let stock = DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, 0.2).unwrap();
    let (_, results) = run(stock, &[(&g, &profile)]);
    let cuts = &results[0].rapid_cuts;
    assert_eq!(cuts.len(), 1, "{cuts:?}");
    assert_eq!(cuts[0].segment, 1);
    assert!(cuts[0].volume > 1.0);
}

#[test]
fn req_sim_03_line_spans_follow_the_tool_tilt() {
    // A cross-check of the span solver against the closed form for a ball
    // whose axis is tilted: a line through the ball's centre crosses it over
    // one diameter.
    let solid = ToolSolid::new(&ball(4.0));
    // In the tool frame the centre is at (0, 0, 2); a line through it along
    // (sin 30, 0, cos 30) from below.
    let (s, c) = 30f64.to_radians().sin_cos();
    let o = [-10.0 * s, 0.0, 2.0 - 10.0 * c];
    let spans = cut_spans(&solid, o, [s, 0.0, c], -50.0, 50.0, 0.1);
    let span = spans.as_slice()[0];
    // Enters the ball at 10 - 2; leaves through the cylinder side (r = 2) at
    // 10 + 2 / sin 30 = 14, below the flute top.
    assert!((span.0 - 8.0).abs() < 1e-6, "{span:?}");
    assert!((span.1 - 14.0).abs() < 1e-6, "{span:?}");
}

#[test]
fn req_sim_06_a_simulator_without_checkpoints_keeps_no_copies() {
    let m = machine();
    let profile = ball(4.0);
    let poses = turning_poses(&m, &Cyl(10.0), 2.0, (-4.0, 4.0), 1.0, 10.0);
    let g = group("op", &poses, MoveKind::Cutting);
    let make = || DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, 0.2).unwrap();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let mut lean = Simulator::without_checkpoints(make(), &m);
    lean.run_group(&g, &profile, &ctx).unwrap();
    lean.run_group(&g, &profile, &ctx).unwrap();
    assert!(lean.checkpoints().is_empty());
    assert!(lean.stock_at(0, 1, &[(&g, &profile)], &ctx).is_err());
    let (full, _) = run(make(), &[(&g, &profile), (&g, &profile)]);
    assert_eq!(full.checkpoints().len(), 3);
    assert_eq!(lean.into_stock().state_hash(), full.stock().state_hash());
}

#[test]
fn req_nfr_06_a_shrunk_stock_holds_the_same_material_and_memory_as_a_copy() {
    // The memory a stock reports follows its vectors' capacities, so the
    // same stock reached by cutting or by copying a cut one (the pipeline
    // cache, decision record 0190) reports the same only once shrunk.
    let m = machine();
    let profile = ball(4.0);
    let poses = turning_poses(&m, &Cyl(10.0), 2.0, (-4.0, 4.0), 1.0, 10.0);
    let g = group("op", &poses, MoveKind::Cutting);
    let stock = DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, 0.2).unwrap();
    let (sim, _) = run(stock, &[(&g, &profile)]);
    let mut cut = sim.into_stock();
    let copy = cut.clone();
    let before = cut.state_hash();
    cut.shrink_to_fit();
    assert_eq!(cut.state_hash(), before);
    assert_eq!(cut, copy);
    assert_eq!(cut.memory_bytes(), copy.memory_bytes());
    let mut again = copy.clone();
    again.shrink_to_fit();
    assert_eq!(again.memory_bytes(), copy.memory_bytes());
}

/// The stock's extent is kept between rays (issue #252: every slanted ray
/// read every column for it), so it must follow every cut: a cut that takes
/// the top off lowers it, and a copy made before the cut keeps its own.
#[test]
fn req_sim_03_the_stock_extent_follows_each_cut() {
    let profile = ball(4.0);
    let tool = ToolSolid::new(&profile);
    let mut stock = DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, 0.2).unwrap();
    let before = stock.bounds();
    assert!((before.max.z - 15.0).abs() < 0.2, "{before:?}");
    let copy = stock.clone();
    // A ball 2 mm deep across the whole top, in rows 1 mm apart.
    let poses: Vec<Pose> = (0..=30)
        .flat_map(|j| {
            (0..=180).map(move |i| Pose {
                tip: Point3::new(-12.0 + f64::from(i) * 0.2, -15.0 + f64::from(j), 13.0),
                axis: Vec3::new(0.0, 0.0, 1.0),
            })
        })
        .collect();
    // A slanted ray asks for the extent, as an entry move's rays do.
    let slant = UnitVec3::new(Vec3::new(0.3, 0.0, -1.0)).unwrap();
    assert!(
        stock
            .first_material(Point3::new(0.0, 0.0, 40.0), slant.get(), 60.0)
            .is_some()
    );
    stock.cut(&tool, &poses);
    let after = stock.bounds();
    assert!(after.max.z < 13.0 + 0.2, "{after:?}");
    assert!(after.max.z > 13.0 - 0.2, "{after:?}");
    assert_eq!(copy.bounds(), before);
    // Equal stocks compare equal whether or not the extent was asked for.
    assert_eq!(
        copy,
        DexelStock::from_stock(&cylinder_stock(30.0, -12.0, 24.0), None, 0.2).unwrap()
    );
}

/// What each sample lies on decides whether it counts (SIM-05, OP-21,
/// SET-09; decision record 0194): a sample inside the finished part (under a
/// tab) never counts, one on a held region or tab counts only where it is
/// overcut, and one on the part always counts, right up to the others.
#[test]
fn req_sim_05_req_op_21_req_set_09_only_the_samples_that_count_make_the_figures() {
    let stock = DexelStock::from_stock(&cylinder_stock(20.0, 0.0, 30.0), None, 0.2).unwrap();
    let options = DeviationOptions::for_cell(0.2);
    let never = CancelToken::new();
    let context = JobContext::new(&never, &NoProgress);
    // Below X 10 inside, from 10 to 15 a held region's or tab's face, from
    // 15 the part.
    let on = |_: usize, p: Point3, _: Vec3| {
        if p.x < 10.0 {
            SampleOn::Inside
        } else if p.x < 15.0 {
            SampleOn::Workholding
        } else {
            SampleOn::Part
        }
    };

    // A target inside the stock: material left everywhere, more of it to
    // the left of X 10 (the radius there is smaller).
    let left =
        openchuck_testkit::parts::revolve(&[(1.0, 4.0), (10.0, 4.0), (10.0, 8.0), (29.0, 8.0)], 48);
    let d = compare_classified(&stock, &left, &options, &on, &context).unwrap();
    let worst = d.worst_remaining.unwrap();
    assert_eq!(worst.on, SampleOn::Part, "{worst:?}");
    assert!(worst.value < 2.5, "{worst:?}");
    assert!(
        d.points
            .iter()
            .any(|p| p.on == SampleOn::Inside && p.value > 3.0 - 1e-9)
    );
    assert!(
        d.points
            .iter()
            .any(|p| p.on == SampleOn::Workholding && p.value > 1.0)
    );
    let counted = d.points.iter().filter(|p| p.counts()).count();
    assert_eq!(d.leftover(0.0, 0.1).samples, counted);
    assert!(counted > 0 && counted < d.points.len());
    // A triangle wholly inside takes no colour.
    let values = triangle_values(&d, &left, options.spacing).unwrap();
    let mut inside = 0;
    for (t, v) in values.iter().enumerate() {
        if let Some([a, b, c]) = left.triangle(t)
            && [a, b, c].iter().all(|p| p.x < 9.0)
            && a != b
        {
            inside += 1;
            assert_eq!(*v, None, "triangle {t}");
        }
    }
    assert!(inside > 0);

    // A target outside the stock: overcut everywhere. The overcut on the
    // workholding counts (a cut into it), the one inside does not.
    let over = openchuck_testkit::parts::revolve(&[(1.0, 11.0), (29.0, 11.0)], 48);
    let d = compare_classified(&stock, &over, &options, &on, &context).unwrap();
    let expected = d
        .points
        .iter()
        .filter(|p| p.on != SampleOn::Inside && -p.value > options.overcut_limit)
        .count();
    assert!(
        d.points
            .iter()
            .any(|p| p.on == SampleOn::Workholding && p.value < -0.5)
    );
    assert_eq!(d.overcut_count, expected);
    assert!(
        d.points
            .iter()
            .any(|p| p.on == SampleOn::Inside && p.value < -0.5)
    );
    assert!(
        expected
            < d.points
                .iter()
                .filter(|p| -p.value > options.overcut_limit)
                .count()
    );

    // With every sample on the part, it is the comparison as it was.
    let plain = compare_cancellable(&stock, &over, &options, &context).unwrap();
    assert!(
        plain
            .points
            .iter()
            .all(|p| p.on == SampleOn::Part && p.counts())
    );
}

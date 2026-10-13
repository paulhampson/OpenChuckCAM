// SPDX-License-Identifier: AGPL-3.0-or-later
//! SP-11 tests, named after the requirement they bear on (OP-07, OP-12, NFR-05, NFR-06, NFR-07).

use sp11_swarf::cases::{Case, TOL, cases};
use sp11_swarf::detect::{Family, detect};
use sp11_swarf::place::*;
use sp11_swarf::surface::*;
use sp11_swarf::vec3::{V3, v3};

fn case(id: &str) -> Case {
    cases().into_iter().find(|c| c.id == id).unwrap()
}

fn run(c: &Case, n: usize, mode: Mode) -> (Vec<Pose>, Metrics) {
    let det = detect(c.surface.as_ref(), TOL, 9, 17);
    let fam = det.best().family;
    let plan = plan_for(c.surface.as_ref(), c.tool, fam, c.side, 17);
    let p = toolpath(c.surface.as_ref(), c.tool, plan, n, mode, 17).unwrap();
    let m = measure(c.surface.as_ref(), c.tool, &p, 60, 30);
    (p, m)
}

#[test]
fn req_op_07_exact_ruled_surfaces_are_detected() {
    for id in [
        "cylinder",
        "cone, matching taper",
        "helicoid p=1.5",
        "coaxial cylinder",
        "plane",
    ] {
        let c = case(id);
        let d = detect(c.surface.as_ref(), TOL, 9, 17);
        assert!(d.chosen.is_some(), "{id}");
        assert!(d.best().residual < 1e-9, "{id}: {}", d.best().residual);
    }
}

#[test]
fn req_op_07_ruling_family_follows_the_parameters() {
    let c = case("cylinder");
    assert_eq!(
        detect(c.surface.as_ref(), TOL, 9, 17)
            .chosen
            .unwrap()
            .family,
        Family::AlongV
    );
    struct Swapped(Box<dyn Surface>);
    impl Surface for Swapped {
        fn name(&self) -> String {
            self.0.name()
        }
        fn domain(&self) -> (f64, f64, f64, f64) {
            let (a, b, c, d) = self.0.domain();
            (c, d, a, b)
        }
        fn point(&self, u: f64, v: f64) -> V3 {
            self.0.point(v, u)
        }
    }
    let sw = Swapped(case("cylinder").surface);
    let d = detect(&sw, TOL, 9, 17);
    assert_eq!(d.chosen.unwrap().family, Family::AlongU);
    assert!(d.best().residual < 1e-9);
}

#[test]
fn req_op_07_non_ruled_surfaces_are_refused_with_a_message() {
    for id in ["sphere", "torus"] {
        let c = case(id);
        let d = detect(c.surface.as_ref(), TOL, 9, 17);
        assert!(d.chosen.is_none(), "{id}");
        let msg = d.refusal().unwrap();
        assert!(msg.contains("not a ruled surface"), "{msg}");
        assert!(d.best().residual > 100.0 * TOL);
    }
}

#[test]
fn req_op_07_near_ruled_spline_threshold() {
    // A bump b on the two inner control points bends each v line by 0.75 b at the middle, so
    // the best line misses by between 0.25 b and 0.75 b.
    for (b, accepted) in [(0.004, true), (0.05, false)] {
        let s = near_ruled(b);
        let d = detect(&s, TOL, 9, 17);
        let r = d.best().residual;
        assert!(r > 0.25 * b && r < 0.75 * b, "bump {b}: {r}");
        assert_eq!(d.chosen.is_some(), accepted, "bump {b}");
    }
}

#[test]
fn req_op_07_decision_is_stable_across_sampling_density() {
    for c in cases() {
        let verdicts: Vec<bool> = [(5, 9), (9, 17), (17, 33), (33, 65)]
            .iter()
            .map(|&(l, s)| detect(c.surface.as_ref(), TOL, l, s).chosen.is_some())
            .collect();
        assert!(
            verdicts.iter().all(|v| *v == verdicts[0]),
            "{}: {verdicts:?}",
            c.id
        );
    }
}

#[test]
fn req_op_07_normal_by_differences_matches_the_analytic_normal() {
    let cyl = RadialCylinder {
        x0: 10.0,
        r: 8.0,
        u: (0.4, 2.7),
        v: (15.0, 30.0),
    };
    for u in [0.5, 1.2, 2.5] {
        let n = normal(&cyl, u, 20.0).unwrap();
        let want = v3(-u.cos(), 0.0, -u.sin());
        assert!((n - want).len() < 1e-6);
    }
}

#[test]
fn req_op_07_developable_surfaces_cut_within_tolerance_with_a_vertical_tool() {
    // Cylinder, and cone with a matching taper: the 4-axis pose equals the ideal pose.
    for id in ["cylinder", "cone, matching taper", "plane"] {
        let c = case(id);
        let (pi, mi) = run(&c, 161, Mode::Ideal);
        let (p4, m4) = run(&c, 161, Mode::Axis4);
        assert!(pi.iter().chain(&p4).all(|p| p.tilt_deg < 1e-6), "{id}");
        assert!(m4.gouge < 1e-6, "{id}: gouge {}", m4.gouge);
        assert!(m4.left < TOL, "{id}: left {}", m4.left);
        assert!(
            (m4.left - mi.left).abs() < 1e-9 && (m4.gouge - mi.gouge).abs() < 1e-9,
            "{id}"
        );
    }
}

#[test]
fn req_op_07_ideal_placement_is_exact_even_where_four_axes_cannot_follow() {
    for id in ["cone, taper 4 deg", "cone, straight tool"] {
        let c = case(id);
        let (_, m) = run(&c, 161, Mode::Ideal);
        assert!(m.gouge < 1e-6 && m.left < TOL, "{id}: {m:?}");
    }
}

#[test]
fn req_op_07_taper_mismatch_is_caught_by_the_deviation_check() {
    // Tilt = asin(sin(beta - alpha) cos u) at the worst u; 6 deg cone, 4 deg tool, u from 0.4.
    let (_, m) = run(&case("cone, taper 4 deg"), 161, Mode::Axis4);
    let (p, _) = run(&case("cone, taper 4 deg"), 161, Mode::Axis4);
    let tilt = p.iter().map(|x| x.tilt_deg).fold(0.0, f64::max);
    let want = (2f64.to_radians().sin() * 0.4f64.cos()).asin().to_degrees();
    assert!((tilt - want).abs() < 0.01, "{tilt} vs {want}");
    assert!(m.gouge > 10.0 * TOL && m.left > 10.0 * TOL, "{m:?}");
    let (_, straight) = run(&case("cone, straight tool"), 161, Mode::Axis4);
    assert!(straight.gouge > m.gouge);
}

#[test]
fn req_op_07_axis_along_a_is_refused_not_a_crash() {
    let c = case("coaxial cylinder");
    let plan = plan_for(c.surface.as_ref(), c.tool, Family::AlongV, c.side, 17);
    assert!(matches!(
        toolpath(c.surface.as_ref(), c.tool, plan, 10, Mode::Axis4, 17),
        Err(PlaceError::AxisAlongA)
    ));
    assert!(toolpath(c.surface.as_ref(), c.tool, plan, 10, Mode::Ideal, 17).is_ok());
}

#[test]
fn req_op_07_ruled_but_twisted_surface_is_caught_by_the_deviation_check() {
    // The helicoid is exactly ruled (residual 0) but not developable: gouge grows with twist.
    let tool = Tool {
        radius: 3.0,
        half_angle: 0.0,
    };
    let mut last = -1.0;
    let mut gouges = vec![];
    for pitch in [0.3, 1.5, 3.0, 6.0] {
        let h = Helicoid {
            pitch,
            u: (0.2, 2.9),
            v: (12.0, 30.0),
        };
        assert!(detect(&h, TOL, 9, 17).chosen.is_some());
        let plan = plan_for(&h, tool, Family::AlongV, 1.0, 17);
        let p = toolpath(&h, tool, plan, 321, Mode::Axis4, 17).unwrap();
        let g = measure(&h, tool, &p, 60, 30).gouge;
        assert!(g > last, "gouge must grow with pitch");
        last = g;
        gouges.push(g);
    }
    assert!(gouges[0] < TOL && gouges[1] < TOL, "{gouges:?}");
    assert!(gouges[3] > TOL, "{gouges:?}");
}

#[test]
fn req_op_07_pose_spacing_governs_material_left() {
    let c = case("cylinder");
    let mut prev = f64::MAX;
    for n in [11, 21, 41, 81, 161] {
        let (_, m) = run(&c, n, Mode::Axis4);
        assert!(m.left < prev);
        prev = m.left;
    }
    assert!(prev < TOL / 5.0);
}

#[test]
fn req_op_05_machine_frame_check_agrees_with_the_part_frame() {
    for c in cases() {
        let det = detect(c.surface.as_ref(), TOL, 9, 17);
        let plan = plan_for(c.surface.as_ref(), c.tool, det.best().family, c.side, 17);
        let Ok(poses) = toolpath(c.surface.as_ref(), c.tool, plan, 31, Mode::Axis4, 17) else {
            continue;
        };
        let (u0, u1, v0, v1) = c.surface.domain();
        for p in &poses {
            let z = p.axis.rot_x(p.a);
            assert!(
                (z - v3(0.0, 0.0, 1.0)).len() < 1e-12,
                "{}: axis not vertical",
                c.id
            );
            let q = c.surface.point(0.5 * (u0 + u1), 0.5 * (v0 + v1));
            let d = signed_dev(c.tool, p, q) - signed_dev_machine(c.tool, p, q);
            assert!(d.abs() < 1e-9, "{}: {d}", c.id);
        }
    }
}

#[test]
fn req_op_15_a_is_continuous_along_the_path() {
    for id in ["helicoid p=1.5", "cone, straight tool", "cylinder"] {
        let c = case(id);
        let (p, _) = run(&c, 91, Mode::Axis4);
        let (_, step) = a_travel(&p);
        assert!(step < 5.0, "{id}: A step {step} deg");
    }
}

#[test]
fn req_nfr_06_results_are_bit_identical_on_repeat() {
    let report = || {
        cases()
            .iter()
            .map(|c| {
                let (p, m) = run(c, 41, Mode::Ideal);
                format!(
                    "{:?} {:?}",
                    p.iter().map(|x| x.a.to_bits()).collect::<Vec<_>>(),
                    (m.gouge.to_bits(), m.left.to_bits())
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(report(), report());
}

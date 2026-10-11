// SPDX-License-Identifier: AGPL-3.0-or-later
//! SP-13 pass criteria as tests. Requirement IDs: TL-02, TL-03, OP-12 (gouge),
//! SIM-03 (sweep), GC-07 and NFR-06 (determinism), NFR-05 (tolerance).

use sp13_tool_profiles::drop::{Drop, drop_mesh};
use sp13_tool_profiles::gouge::max_gouge;
use sp13_tool_profiles::mesh::XyIndex;
use sp13_tool_profiles::normal::tip_from_contact;
use sp13_tool_profiles::par;
use sp13_tool_profiles::parts::{Part, divisions_for};
use sp13_tool_profiles::profile::{Profile, Pt, Segment};
use sp13_tool_profiles::shapes;
use sp13_tool_profiles::sweep::{Heightfield, RadialGrid, RotaryPose, sample_line};

const CHORD: f64 = 0.0005;
const D: f64 = 6.0;
const R: f64 = 3.0;
const FLUTE: f64 = 12.0;

fn index(part: &Part) -> (XyIndex, f64) {
    let rmax = part.max_radius();
    let n = divisions_for(rmax, CHORD);
    let sag = rmax * (1.0 - (std::f64::consts::PI / n as f64).cos());
    // Latitude-longitude triangles on the sphere sit up to about twice the
    // edge sagitta inside; cylinder and cone quads are planar.
    let bound = match part {
        Part::Sphere { .. } => 2.5 * sag,
        _ => sag,
    };
    (XyIndex::new(part.mesh(n, 1.0), 1.0), bound)
}

fn drop_full(p: &Profile, idx: &XyIndex, x: f64, y: f64) -> Drop {
    drop_mesh(p, idx, x, y, &mut Vec::new()).expect("tool over the part")
}

fn drop_at(p: &Profile, idx: &XyIndex, x: f64, y: f64) -> f64 {
    drop_mesh(p, idx, x, y, &mut Vec::new())
        .expect("tool over the part")
        .tip_z
}

// ---- closed forms -----------------------------------------------------------

/// Drop onto a circle of radius `rs` at horizontal distance `d` from its
/// centre (cylinder cross-section, or the sphere's plane through the axis).
mod closed {
    pub fn flat(rs: f64, r: f64, d: f64) -> f64 {
        if d <= r {
            rs
        } else {
            (rs * rs - (d - r).powi(2)).sqrt()
        }
    }
    pub fn ball(rs: f64, rb: f64, d: f64) -> f64 {
        ((rs + rb).powi(2) - d * d).sqrt() - rb
    }
    pub fn bull(rs: f64, r: f64, rc: f64, d: f64) -> f64 {
        let k = r - rc;
        if d <= k {
            rs
        } else {
            ((rs + rc).powi(2) - (d - k).powi(2)).sqrt() - rc
        }
    }
    /// V-bit or chamfer: tip flat radius `rt`, half angle `a` (radians), cutting radius `r`.
    pub fn v(rs: f64, r: f64, rt: f64, a: f64, d: f64) -> f64 {
        if d <= rt {
            return rs;
        }
        let e = d - rt;
        if e <= rs * a.cos() {
            (rs * rs - e * e).sqrt()
        } else {
            // Flank contact at horizontal position rs cos(a); it must be on the tool.
            assert!(
                d - rs * a.cos() <= r,
                "closed form needs the flank contact on the tool"
            );
            rs / a.sin() - e / a.tan()
        }
    }
}

// ---- profile mapping and validation (TL-02, TL-03) -----------------------------

#[test]
fn req_tl_02_tl_03_standard_silhouettes_match_closed_form() {
    let h45 = 45f64.to_radians();
    let cases: Vec<(&str, Profile, Box<dyn Fn(f64) -> f64>)> = vec![
        ("flat", shapes::flat(D, FLUTE).unwrap(), Box::new(|_| 0.0)),
        (
            "ball",
            shapes::ball(D, FLUTE).unwrap(),
            Box::new(|r: f64| R - (R * R - r * r).sqrt()),
        ),
        (
            "bull",
            shapes::bull_nose(D, 1.0, FLUTE).unwrap(),
            Box::new(|r: f64| {
                if r <= 2.0 {
                    0.0
                } else {
                    1.0 - (1.0 - (r - 2.0).powi(2)).sqrt()
                }
            }),
        ),
        (
            "v90",
            shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap(),
            Box::new(move |r: f64| r / h45.tan()),
        ),
        (
            "chamfer90",
            shapes::chamfer(D, 90.0, 1.0, FLUTE).unwrap(),
            Box::new(move |r: f64| ((r - 0.5) / h45.tan()).max(0.0)),
        ),
        (
            "drill118",
            shapes::drill(D, 118.0, FLUTE).unwrap(),
            Box::new(|r: f64| r / 59f64.to_radians().tan()),
        ),
        (
            "tapered",
            shapes::tapered_ball(D, 0.5, 15.0, FLUTE).unwrap(),
            Box::new(|r: f64| {
                let a = 15f64.to_radians();
                let rt = 0.5;
                if r <= rt * a.cos() {
                    rt - (rt * rt - r * r).sqrt()
                } else {
                    rt * (1.0 - a.sin()) + (r - rt * a.cos()) / a.tan()
                }
            }),
        ),
    ];
    for (name, p, h) in &cases {
        assert!((p.r_max() - R).abs() < 1e-12, "{name} radius");
        assert!((p.z_end() - FLUTE).abs() < 1e-12, "{name} flute length");
        for k in 0..=300 {
            let r = R * k as f64 / 300.0;
            let got = p.height_at(r).unwrap();
            assert!(
                (got - h(r)).abs() < 1e-12,
                "{name} at r={r}: {got} vs {}",
                h(r)
            );
        }
        assert!(p.height_at(R + 0.01).is_none());
    }
}

#[test]
fn req_tl_03_impossible_values_rejected_with_a_message() {
    let msg =
        |r: Result<Profile, sp13_tool_profiles::profile::ProfileError>| r.unwrap_err().to_string();
    assert!(msg(shapes::bull_nose(D, 3.5, FLUTE)).contains("corner radius"));
    assert!(msg(shapes::v_bit(D, 0.0, 0.0, FLUTE)).contains("included angle"));
    assert!(msg(shapes::v_bit(D, 180.0, 0.0, FLUTE)).contains("included angle"));
    assert!(msg(shapes::drill(D, 20.0, 5.0)).contains("above the flute length"));
    assert!(msg(shapes::tapered_ball(D, 0.5, 1.0, 12.0)).contains("above the flute length"));
    assert!(msg(shapes::tapered_ball(D, 4.0, 10.0, 12.0)).contains("no taper"));
    assert!(msg(shapes::flat(-1.0, FLUTE)).contains("diameter"));
    assert!(msg(shapes::flat(f64::NAN, FLUTE)).contains("diameter"));
}

#[test]
fn req_tl_03_custom_profile_validation() {
    let l = |r, z| Segment::Line { to: Pt::new(r, z) };
    let e = |s: &[Segment]| Profile::new(s).unwrap_err().to_string();
    // Turns back on itself (self-intersecting or undercut).
    assert!(
        e(&[l(3.0, 0.0), l(3.0, 5.0), l(1.0, 6.0), l(4.0, 7.0)])
            .contains("segment 3: the profile turns back")
    );
    // Downward step.
    assert!(e(&[l(2.0, 1.0), l(3.0, 0.5)]).contains("turns back"));
    // Arc whose ends are not on one circle.
    assert!(
        e(&[Segment::Arc {
            to: Pt::new(3.0, 3.1),
            centre: Pt::new(0.0, 3.0)
        }])
        .contains("from its centre")
    );
    // Arc spanning more than a quarter turn (not monotone).
    assert!(
        e(&[
            l(1.0, 0.0),
            Segment::Arc {
                to: Pt::new(1.0, 2.0),
                centre: Pt::new(1.0, 1.0)
            }
        ])
        .contains("not monotone")
    );
    // Runs up the axis first; zero length; empty; not finite.
    assert!(e(&[l(0.0, 2.0), l(3.0, 3.0)]).contains("up the tool axis"));
    assert!(e(&[l(0.0, 0.0)]).contains("zero length"));
    assert!(e(&[]).contains("at least one"));
    assert!(e(&[l(f64::INFINITY, 1.0)]).contains("finite"));
    // A concave (corner-rounding style) profile is valid but not convex.
    let p = Profile::new(&bell()).unwrap();
    assert!(!p.is_convex());
    assert!(p.contact_for_normal(-0.3).is_none());
    for p in [
        shapes::flat(D, FLUTE),
        shapes::ball(D, FLUTE),
        shapes::v_bit(D, 60.0, 0.2, FLUTE),
    ] {
        assert!(p.unwrap().is_convex());
    }
}

/// A non-convex custom tool: flat bottom, concave arc, straight side.
fn bell() -> Vec<Segment> {
    vec![
        Segment::Line {
            to: Pt::new(1.0, 0.0),
        },
        Segment::Arc {
            to: Pt::new(3.0, 2.0),
            centre: Pt::new(3.0, 0.0),
        },
        Segment::Line {
            to: Pt::new(3.0, FLUTE),
        },
    ]
}

// ---- drop-cutter against closed form (pass criterion 1) --------------------------

/// `sag` is the radial mesh error; the vertical error it allows grows as the
/// surface tilts, so it is divided by the normal's Z at the contact.
fn check(name: &str, part: &Part, got: Drop, want: f64, sag: f64) {
    let nz = part.normal(got.contact)[2].max(0.05);
    let bound = sag / nz + 1e-9;
    let got = got.tip_z;
    let err = want - got;
    // The mesh is inscribed, so the dropped tip is never above the exact one.
    assert!(
        err >= -1e-9 && err <= bound,
        "{name}: got {got}, want {want}, err {err}, bound {bound}"
    );
}

#[test]
fn req_tl_03_drop_on_cylinder_matches_closed_form() {
    let rc = 20.0;
    let part = Part::Cylinder {
        r: rc,
        x0: -30.0,
        x1: 30.0,
    };
    let (idx, bound) = index(&part);
    let flat = shapes::flat(D, FLUTE).unwrap();
    let ball = shapes::ball(D, FLUTE).unwrap();
    let bull = shapes::bull_nose(D, 1.0, FLUTE).unwrap();
    let v90 = shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap();
    let v60 = shapes::v_bit(D, 60.0, 0.0, FLUTE).unwrap();
    let ch = shapes::chamfer(D, 90.0, 1.0, FLUTE).unwrap();
    let a45 = 45f64.to_radians();
    let a30 = 30f64.to_radians();
    for y in [0.0, 0.7, 1.5, 2.9, 3.1, 5.0, 8.0, 12.0, -6.5] {
        let d = f64::abs(y);
        let x = 1.234;
        check(
            "flat",
            &part,
            drop_full(&flat, &idx, x, y),
            closed::flat(rc, R, d),
            bound,
        );
        check(
            "ball",
            &part,
            drop_full(&ball, &idx, x, y),
            closed::ball(rc, R, d),
            bound,
        );
        check(
            "bull",
            &part,
            drop_full(&bull, &idx, x, y),
            closed::bull(rc, R, 1.0, d),
            bound,
        );
        if d - rc * a45.cos() <= R {
            check(
                "v90",
                &part,
                drop_full(&v90, &idx, x, y),
                closed::v(rc, R, 0.0, a45, d),
                bound,
            );
            check(
                "chamfer",
                &part,
                drop_full(&ch, &idx, x, y),
                closed::v(rc, R, 0.5, a45, d),
                bound,
            );
        }
        if d - rc * a30.cos() <= R {
            check(
                "v60",
                &part,
                drop_full(&v60, &idx, x, y),
                closed::v(rc, R, 0.0, a30, d),
                bound,
            );
        }
    }
    // Flank contact needs |y| > rc cos(a): use a 30 degree V near the side.
    for y in [17.5, 18.5, 19.5] {
        check(
            "v60 flank",
            &part,
            drop_full(&v60, &idx, 0.0, y),
            closed::v(rc, R, 0.0, a30, y),
            bound,
        );
    }
}

#[test]
fn req_tl_03_drop_on_sphere_matches_closed_form() {
    let rs = 15.0;
    let part = Part::Sphere { r: rs };
    let (idx, bound) = index(&part);
    let flat = shapes::flat(D, FLUTE).unwrap();
    let ball = shapes::ball(D, FLUTE).unwrap();
    let bull = shapes::bull_nose(D, 1.0, FLUTE).unwrap();
    let v90 = shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap();
    let a45 = 45f64.to_radians();
    for (x, y) in [
        (0.0, 0.0),
        (1.0, 0.5),
        (2.5, -2.0),
        (-4.0, 3.0),
        (6.0, 6.0),
        (0.3, -9.0),
    ] {
        let d = f64::hypot(x, y);
        check(
            "flat",
            &part,
            drop_full(&flat, &idx, x, y),
            closed::flat(rs, R, d),
            bound,
        );
        check(
            "ball",
            &part,
            drop_full(&ball, &idx, x, y),
            closed::ball(rs, R, d),
            bound,
        );
        check(
            "bull",
            &part,
            drop_full(&bull, &idx, x, y),
            closed::bull(rs, R, 1.0, d),
            bound,
        );
        check(
            "v90",
            &part,
            drop_full(&v90, &idx, x, y),
            closed::v(rs, R, 0.0, a45, d),
            bound,
        );
    }
    // Flank contact on the sphere.
    for (x, y) in [(0.0, 11.5), (8.5, 8.5)] {
        let d = f64::hypot(x, y);
        check(
            "v90 flank",
            &part,
            drop_full(&v90, &idx, x, y),
            closed::v(rs, R, 0.0, a45, d),
            bound,
        );
    }
}

#[test]
fn req_tl_03_drop_on_cone_matches_closed_form() {
    let flat = shapes::flat(D, FLUTE).unwrap();
    let ball = shapes::ball(D, FLUTE).unwrap();
    let v90 = shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap();
    for beta in [20.0f64, 60.0] {
        let part = Part::Cone {
            r0: 10.0,
            half_angle_deg: beta,
            x0: -5.0,
            x1: 5.0,
        };
        let (idx, bound) = index(&part);
        let t = beta.to_radians().tan();
        let rad = |x: f64| 10.0 + x * t;
        for x in [-1.5, 0.0, 0.8, 1.9] {
            check(
                "cone flat",
                &part,
                drop_full(&flat, &idx, x, 0.0),
                rad(x + R),
                bound,
            );
            let b = beta.to_radians();
            check(
                "cone ball",
                &part,
                drop_full(&ball, &idx, x, 0.0),
                rad(x) + R / b.cos() - R,
                bound,
            );
            // V: the apex touches when the cone is shallower than the flank,
            // otherwise the top edge of the flank touches.
            let want = if beta <= 45.0 { rad(x) } else { rad(x + R) - R };
            check(
                "cone v90",
                &part,
                drop_full(&v90, &idx, x, 0.0),
                want,
                bound,
            );
        }
    }
}

// ---- custom profile agrees with the equivalent standard tool (criterion 2) -------

fn equivalents() -> Vec<(&'static str, Profile, Profile)> {
    let s = std::f64::consts::FRAC_1_SQRT_2;
    let l = |r, z| Segment::Line { to: Pt::new(r, z) };
    vec![
        (
            "ball as two arcs",
            shapes::ball(D, FLUTE).unwrap(),
            Profile::new(&[
                Segment::Arc {
                    to: Pt::new(R * s, R - R * s),
                    centre: Pt::new(0.0, R),
                },
                Segment::Arc {
                    to: Pt::new(R, R),
                    centre: Pt::new(0.0, R),
                },
                l(R, FLUTE),
            ])
            .unwrap(),
        ),
        (
            "flat with a split bottom",
            shapes::flat(D, FLUTE).unwrap(),
            Profile::new(&[l(1.0, 0.0), l(R, 0.0), l(R, 5.0), l(R, FLUTE)]).unwrap(),
        ),
        (
            "v90 with a split flank",
            shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap(),
            Profile::new(&[l(1.2, 1.2), l(R, R), l(R, FLUTE)]).unwrap(),
        ),
        (
            "bull-nose typed in",
            shapes::bull_nose(D, 1.0, FLUTE).unwrap(),
            Profile::new(&[
                l(2.0, 0.0),
                Segment::Arc {
                    to: Pt::new(R, 1.0),
                    centre: Pt::new(2.0, 1.0),
                },
                l(R, FLUTE),
            ])
            .unwrap(),
        ),
        (
            "tapered ball typed in",
            shapes::tapered_ball(D, 0.5, 15.0, FLUTE).unwrap(),
            {
                let a = 15f64.to_radians();
                let t = Pt::new(0.5 * a.cos(), 0.5 * (1.0 - a.sin()));
                let z = t.z + (R - t.r) / a.tan();
                Profile::new(&[
                    Segment::Arc {
                        to: t,
                        centre: Pt::new(0.0, 0.5),
                    },
                    l(R, z),
                    l(R, FLUTE),
                ])
                .unwrap()
            },
        ),
    ]
}

#[test]
fn req_tl_03_custom_profile_agrees_with_standard_tool() {
    let parts = [
        Part::Sphere { r: 15.0 },
        Part::Cylinder {
            r: 20.0,
            x0: -30.0,
            x1: 30.0,
        },
        Part::Cone {
            r0: 10.0,
            half_angle_deg: 30.0,
            x0: -5.0,
            x1: 5.0,
        },
    ];
    for part in &parts {
        let (idx, _) = index(part);
        for (name, std_tool, custom) in equivalents() {
            let mut worst = 0f64;
            for i in -6..=6 {
                for j in -6..=6 {
                    let (x, y) = (0.37 * i as f64, 0.91 * j as f64);
                    let a = drop_at(&std_tool, &idx, x, y);
                    let b = drop_at(&custom, &idx, x, y);
                    worst = worst.max((a - b).abs());
                }
            }
            assert!(
                worst <= 1e-9,
                "{name} on {part:?}: worst difference {worst}"
            );
        }
    }
}

// ---- normal-offset solving agrees with drop-cutter ------------------------------

#[test]
fn req_tl_03_normal_offset_agrees_with_drop() {
    let part = Part::Sphere { r: 15.0 };
    let (idx, bound) = index(&part);
    let tools = [
        shapes::flat(D, FLUTE).unwrap(),
        shapes::ball(D, FLUTE).unwrap(),
        shapes::bull_nose(D, 1.0, FLUTE).unwrap(),
        shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap(),
        shapes::tapered_ball(D, 0.5, 15.0, FLUTE).unwrap(),
    ];
    for tool in &tools {
        for (x, y) in [(0.0, 0.0), (1.0, 2.0), (-3.0, 4.0), (5.0, -6.0)] {
            let z = part.top(x, y).unwrap();
            let p = [x, y, z];
            let tip = tip_from_contact(tool, p, part.normal(p), [0.0, 0.0, 1.0]).unwrap();
            let dropped = drop_full(tool, &idx, tip[0], tip[1]);
            check("normal vs drop", &part, dropped, tip[2], bound);
            let g = max_gouge(tool, &part, tip, 0.02);
            assert!(g.depth < 1e-6, "normal-offset pose gouges by {}", g.depth);
        }
    }
}

// ---- independent gouge checker (OP-12) --------------------------------------------

#[test]
fn req_op_12_gouge_checker_passes_drops_and_catches_seeded_faults() {
    let parts = [
        Part::Sphere { r: 15.0 },
        Part::Cylinder {
            r: 20.0,
            x0: -30.0,
            x1: 30.0,
        },
    ];
    let mut tools: Vec<(&str, Profile)> = vec![
        ("flat", shapes::flat(D, FLUTE).unwrap()),
        ("ball", shapes::ball(D, FLUTE).unwrap()),
        ("bull", shapes::bull_nose(D, 1.0, FLUTE).unwrap()),
        (
            "tapered",
            shapes::tapered_ball(D, 0.5, 15.0, FLUTE).unwrap(),
        ),
        ("v90", shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap()),
        ("chamfer", shapes::chamfer(D, 90.0, 1.0, FLUTE).unwrap()),
        ("drill", shapes::drill(D, 118.0, FLUTE).unwrap()),
    ];
    tools.push(("bell", Profile::new(&bell()).unwrap()));
    for part in &parts {
        let (idx, bound) = index(part);
        for (name, tool) in &tools {
            for (x, y) in [(0.0, 0.0), (1.3, 2.2), (-2.0, 6.0)] {
                let tip_z = drop_at(tool, &idx, x, y);
                let ok = max_gouge(tool, part, [x, y, tip_z], 0.01);
                assert!(
                    ok.depth <= bound + 1e-9,
                    "{name} {part:?} at ({x},{y}): {:?}",
                    ok
                );
                let seeded = max_gouge(tool, part, [x, y, tip_z - 0.05], 0.01);
                assert!(
                    seeded.depth > 0.01,
                    "{name} seeded fault missed: {:?}",
                    seeded
                );
            }
        }
    }
}

/// Brute-force reference for any profile: the highest value of
/// z - h(d) over a fine grid of the exact surface.
fn brute_drop(tool: &Profile, part: &Part, x: f64, y: f64, step: f64) -> f64 {
    let n = (tool.r_max() / step).ceil() as i64;
    let mut best = f64::MIN;
    for i in -n..=n {
        for j in -n..=n {
            let (dx, dy) = (i as f64 * step, j as f64 * step);
            if let (Some(h), Some(z)) = (tool.height_at(dx.hypot(dy)), part.top(x + dx, y + dy)) {
                best = best.max(z - h);
            }
        }
    }
    best
}

#[test]
fn req_tl_03_non_convex_custom_drop_matches_brute_force() {
    let tool = Profile::new(&bell()).unwrap();
    for part in [
        Part::Sphere { r: 15.0 },
        Part::Cylinder {
            r: 20.0,
            x0: -30.0,
            x1: 30.0,
        },
    ] {
        let (idx, bound) = index(&part);
        for (x, y) in [(0.0, 0.0), (1.3, 2.2), (-2.0, 6.0), (3.0, 9.0)] {
            let got = drop_at(&tool, &idx, x, y);
            let want = brute_drop(&tool, &part, x, y, 0.005);
            // The brute force undershoots by its grid step's sagitta; the mesh by its own.
            assert!(
                (got - want).abs() <= bound + 2e-4,
                "bell {part:?} ({x},{y}): {got} vs {want}"
            );
        }
    }
}

// ---- sweep (SIM-03 stand-ins) ----------------------------------------------------------

/// Closed-form groove depth below the cut level at lateral offset `u` (the profile itself).
fn groove_profiles() -> Vec<(&'static str, Profile, Box<dyn Fn(f64) -> f64>)> {
    vec![
        ("flat", shapes::flat(D, FLUTE).unwrap(), Box::new(|_| 0.0)),
        (
            "ball",
            shapes::ball(D, FLUTE).unwrap(),
            Box::new(|u: f64| R - (R * R - u * u).sqrt()),
        ),
        (
            "v90",
            shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap(),
            Box::new(|u: f64| u),
        ),
    ]
}

#[test]
fn req_sim_03_dexel_groove_matches_profile() {
    let cell = 0.05;
    let stock = Heightfield::block(-10.0, -5.0, cell, 401, 201, 0.0);
    for (name, tool, h) in groove_profiles() {
        let depth = 2.0;
        let poses = sample_line([-8.0, 0.0, -depth], [8.0, 0.0, -depth], cell / 4.0);
        let cut = stock.swept(&tool, &poses, 1);
        for j in 0..cut.ny {
            let y = cut.y0 + j as f64 * cut.cell;
            let i = 200; // x = 0, mid-pass
            let want = if y.abs() <= R {
                (-depth + h(y.abs())).min(0.0)
            } else {
                0.0
            };
            let got = cut.top[j * cut.nx + i];
            assert!((got - want).abs() < 1e-9, "{name} y={y}: {got} vs {want}");
        }
    }
}

#[test]
fn req_sim_03_rotary_groove_matches_profile() {
    let rc = 20.0;
    let nt = 360;
    let stock = RadialGrid::cylinder(-4.0, 0.05, 161, nt, rc);
    for (name, tool, h) in groove_profiles() {
        let depth = 1.5;
        let poses: Vec<RotaryPose> = (0..2 * nt)
            .map(|k| RotaryPose {
                tip: [0.0, 0.0, rc - depth],
                a: std::f64::consts::TAU * k as f64 / (2 * nt) as f64,
            })
            .collect();
        let cut = stock.swept(&tool, &poses, 1);
        for i in 0..cut.nx {
            let x = cut.x0 + i as f64 * cut.dx;
            let want = if x.abs() <= R {
                (rc - depth + h(x.abs())).min(rc)
            } else {
                rc
            };
            for j in 0..nt {
                let got = cut.rho[i * nt + j];
                assert!(
                    (got - want).abs() < 1e-9,
                    "{name} x={x} theta {j}: {got} vs {want}"
                );
            }
        }
    }
}

#[test]
fn req_sim_03_custom_equivalent_sweeps_identically() {
    let stock = Heightfield::block(-10.0, -5.0, 0.05, 401, 201, 0.0);
    let poses = sample_line([-8.0, -1.0, -2.0], [8.0, 1.5, -1.0], 0.0125);
    for (name, a, b) in equivalents() {
        let ca = stock.swept(&a, &poses, 1);
        let cb = stock.swept(&b, &poses, 1);
        let worst = ca
            .top
            .iter()
            .zip(&cb.top)
            .map(|(p, q)| (p - q).abs())
            .fold(0.0, f64::max);
        assert!(worst <= 1e-12, "{name}: {worst}");
    }
}

// ---- determinism (criterion 4; GC-07, NFR-06) -----------------------------------------

#[test]
fn req_nfr_06_identical_results_at_1_and_n_threads() {
    let n = par::threads().max(3);
    let part = Part::Sphere { r: 15.0 };
    let (idx, _) = index(&part);
    let tool = Profile::new(&bell()).unwrap();
    let grid = |threads| {
        par::map_items(41 * 41, threads, |k| {
            let (i, j) = ((k % 41) as f64, (k / 41) as f64);
            drop_mesh(
                &tool,
                &idx,
                -10.0 + 0.5 * i,
                -10.0 + 0.5 * j,
                &mut Vec::new(),
            )
            .map(|d| d.tip_z.to_bits())
        })
    };
    assert_eq!(grid(1), grid(2));
    assert_eq!(grid(1), grid(n));

    let stock = Heightfield::block(-10.0, -5.0, 0.05, 401, 201, 0.0);
    let poses = sample_line([-8.0, -1.0, -2.0], [8.0, 1.5, -1.0], 0.0125);
    let bits = |h: Heightfield| h.top.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(
        bits(stock.swept(&tool, &poses, 1)),
        bits(stock.swept(&tool, &poses, n))
    );

    let rg = RadialGrid::cylinder(-4.0, 0.1, 81, 180, 20.0);
    let rposes: Vec<RotaryPose> = (0..90)
        .map(|k| RotaryPose {
            tip: [-2.0 + 0.05 * k as f64, 0.7, 18.5],
            a: 0.07 * k as f64,
        })
        .collect();
    let rbits = |g: RadialGrid| g.rho.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(
        rbits(rg.swept(&tool, &rposes, 1)),
        rbits(rg.swept(&tool, &rposes, n))
    );
}

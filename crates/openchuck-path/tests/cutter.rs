// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tool-centre solving and the surface port against closed forms and the
//! independent gouge checker (OP-12; decision records 0012 and 0015).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use openchuck_path::{MeshSurface, RotaryMapping, SurfaceSource, ToolLine};
use openchuck_testkit::gouge::{Outline, Pose, max_gouge};
use openchuck_testkit::parts::{Cylinder, RadialPart, Sphere};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::ToolShape;
use openchuck_types::units::{Degrees, Millimetres};
use support::{ball, machine, tool};

const R: f64 = 15.0;

fn cylinder() -> (Cylinder, MeshSurface, f64) {
    let part = Cylinder {
        radius: R,
        length: 40.0,
    };
    let around = 1440;
    // An inscribed mesh: its error is the chord sagitta.
    let sagitta = R * (1.0 - (std::f64::consts::PI / f64::from(around)).cos());
    let surface = MeshSurface::new(&part.mesh(3, around)).unwrap();
    (part, surface, sagitta)
}

fn vertical(x: f64, y: f64) -> ToolLine {
    ToolLine {
        origin: Point3::new(x, y, 0.0),
        axis: UnitVec3::Z,
    }
}

#[test]
fn req_op_12_drop_matches_closed_form_on_a_cylinder() {
    let (_, surface, sagitta) = cylinder();
    let rho = 3.0;
    let ball = ball(2.0 * rho).profile().unwrap();
    let flat = tool(ToolShape::FlatEnd, 2.0 * rho).profile().unwrap();
    for i in 0..=40 {
        let d = -10.0 + 0.5 * f64::from(i);
        // Ball: the centre is R + rho from the axis.
        let exact = ((R + rho).powi(2) - d * d).sqrt() - rho;
        let got = surface.drop_tool(&ball, &vertical(20.0, d)).unwrap();
        let err = exact - got.tip.z;
        assert!(
            (-1e-9..=2.0 * sagitta).contains(&err),
            "ball at y {d}: exact {exact}, got {}, error {err}",
            got.tip.z
        );
        // The contact lies on the surface, one ball radius from the centre.
        let centre = got.tip + Vec3::new(0.0, 0.0, rho);
        assert!((got.contact.distance(centre) - rho).abs() < 1e-6);
        // Flat: the bottom disc touches at the point nearest the axis.
        let y = 0.0_f64.clamp(d - rho, d + rho);
        let exact = (R * R - y * y).sqrt();
        let got = surface.drop_tool(&flat, &vertical(20.0, d)).unwrap();
        let err = exact - got.tip.z;
        assert!(
            (-1e-9..=2.0 * sagitta).contains(&err),
            "flat at y {d}: exact {exact}, got {}, error {err}",
            got.tip.z
        );
    }
}

#[test]
fn req_op_12_drop_matches_closed_form_on_a_sphere_at_any_a() {
    let part = Sphere { radius: R };
    let surface = MeshSurface::new(&part.mesh(721, 720)).unwrap();
    let rho = 3.0;
    let ball = ball(2.0 * rho).profile().unwrap();
    let rotary = RotaryMapping::of_profile(&machine());
    for a in [-725.0, -90.0, 0.0, 37.5, 180.0, 359.0] {
        for i in 0..=10 {
            let x = -10.0 + 2.0 * f64::from(i);
            let line = rotary.tool_line(Millimetres(x), Degrees(a), Millimetres(0.0));
            let got = surface.drop_tool(&ball, &line).unwrap();
            let exact = ((R + rho).powi(2) - x * x).sqrt() - rho;
            // The tip lies on the line, `exact` from the axis.
            let tip_radius = got.tip.y.hypot(got.tip.z);
            let err = exact - tip_radius;
            assert!(
                (-1e-9..=0.01).contains(&err),
                "a {a} x {x}: exact {exact}, got {tip_radius}"
            );
            assert!((got.tip.x - x).abs() < 1e-9);
        }
    }
}

#[test]
fn req_op_12_independent_checker_finds_no_gouge_and_catches_a_seeded_fault() {
    let (part, surface, sagitta) = cylinder();
    let shapes = [
        ToolShape::BallEnd,
        ToolShape::FlatEnd,
        ToolShape::BullNose {
            corner_radius: Millimetres(1.0),
        },
        ToolShape::VBit {
            included_angle: Degrees(90.0),
            tip_diameter: Millimetres(0.0),
        },
    ];
    let rotary = RotaryMapping::of_profile(&machine());
    for shape in shapes {
        let t = tool(shape.clone(), 6.0);
        let profile = t.profile().unwrap();
        let outline = Outline::new(&profile);
        let mut poses = Vec::new();
        for i in 0..=8 {
            for k in 0..6 {
                let a = 60.0 * f64::from(k) + 7.0;
                let across = -4.0 + f64::from(i);
                let line = rotary.tool_line(Millimetres(20.0), Degrees(a), Millimetres(across));
                let c = surface.drop_tool(&profile, &line).unwrap();
                poses.push(Pose {
                    tip: c.tip,
                    axis: line.axis.get(),
                });
            }
        }
        let g = max_gouge(&outline, &part, &poses, 0.02);
        assert!(
            g.depth <= 2.0 * sagitta + 1e-9,
            "{shape:?}: gouge {} at {:?}",
            g.depth,
            g.at
        );
        let lowered: Vec<Pose> = poses
            .iter()
            .map(|p| Pose {
                tip: p.tip + p.axis * -0.05,
                axis: p.axis,
            })
            .collect();
        let g = max_gouge(&outline, &part, &lowered, 0.02);
        assert!(
            g.depth > 0.02,
            "{shape:?}: seeded fault not found ({})",
            g.depth
        );
    }
}

#[test]
fn req_op_12_custom_profile_agrees_with_the_standard_tool() {
    use openchuck_types::tool::{ProfilePoint, ProfileSegment, ToolProfile};
    let (_, surface, _) = cylinder();
    let standard = ball(6.0).profile().unwrap();
    // A ball typed as two arcs.
    let custom = tool(
        ToolShape::Custom {
            profile: ToolProfile {
                segments: vec![
                    ProfileSegment::Arc {
                        to: ProfilePoint::new(
                            3.0 * (std::f64::consts::FRAC_PI_4).sin(),
                            3.0 - 3.0 * (std::f64::consts::FRAC_PI_4).cos(),
                        ),
                        centre: ProfilePoint::new(0.0, 3.0),
                    },
                    ProfileSegment::Arc {
                        to: ProfilePoint::new(3.0, 3.0),
                        centre: ProfilePoint::new(0.0, 3.0),
                    },
                ],
            },
        },
        6.0,
    )
    .profile()
    .unwrap();
    for i in 0..=20 {
        let line = vertical(10.0, -8.0 + 0.8 * f64::from(i));
        let a = surface.drop_tool(&standard, &line).unwrap();
        let b = surface.drop_tool(&custom, &line).unwrap();
        assert!((a.tip.z - b.tip.z).abs() < 1e-9, "{a:?} {b:?}");
    }
}

#[test]
fn req_op_12_surface_queries_agree_with_the_exact_cylinder() {
    let (_, surface, sagitta) = cylinder();
    let hit = surface
        .cast_ray(
            Point3::new(10.0, 0.0, 50.0),
            UnitVec3::new(Vec3::new(0.0, 0.0, -1.0)).unwrap(),
            100.0,
        )
        .unwrap();
    assert!((hit.point.z - R).abs() <= sagitta + 1e-9);
    assert!(hit.normal.get().z > 0.999);
    assert!((hit.distance - (50.0 - hit.point.z)).abs() < 1e-9);
    assert!(
        surface
            .cast_ray(Point3::new(10.0, 0.0, 50.0), UnitVec3::Z, 100.0)
            .is_none()
    );
    let p = surface.project(Point3::new(12.0, 0.0, 30.0)).unwrap();
    assert!((p.point.z - R).abs() <= sagitta + 1e-9);
    assert!(p.normal.get().z > 0.999);
    let d = surface.distance(Point3::new(12.0, 30.0, 0.0)).unwrap();
    assert!((d - 15.0).abs() <= sagitta + 1e-9);
}

#[test]
fn req_nfr_07_bad_meshes_are_refused_with_a_message() {
    let empty = TriangleMesh {
        vertices: vec![],
        triangles: vec![],
    };
    assert!(MeshSurface::new(&empty).is_err());
    let out_of_range = TriangleMesh {
        vertices: vec![Point3::ORIGIN],
        triangles: vec![[0, 1, 2]],
    };
    assert!(MeshSurface::new(&out_of_range).is_err());
    let nan = TriangleMesh {
        vertices: vec![
            Point3::ORIGIN,
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, f64::NAN, 0.0),
        ],
        triangles: vec![[0, 1, 2]],
    };
    let e = MeshSurface::new(&nan).unwrap_err().to_string();
    assert!(e.contains("finite"), "{e}");
}

#[test]
fn req_nfr_07_degenerate_triangles_do_not_break_queries() {
    let mesh = TriangleMesh {
        vertices: vec![
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(2.0, 0.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ],
        triangles: vec![[0, 1, 2], [0, 1, 3]],
    };
    let surface = MeshSurface::new(&mesh).unwrap();
    let profile = ball(2.0).profile().unwrap();
    let c = surface.drop_tool(&profile, &vertical(0.2, 0.2)).unwrap();
    assert!(c.tip.is_finite());
    assert!(surface.project(Point3::new(1.5, -1.0, 3.0)).is_some());
}

/// Points along the profile of a tool, one every `step` mm of length.
fn profile_points(profile: &openchuck_types::tool::RevolvedProfile, step: f64) -> Vec<(f64, f64)> {
    use openchuck_types::tool::profile::ProfilePiece;
    let mut out = Vec::new();
    for piece in profile.pieces() {
        match *piece {
            ProfilePiece::Line { start, end } => {
                let (r0, z0, r1, z1) = (start.radius.0, start.height.0, end.radius.0, end.height.0);
                let n = ((r1 - r0).hypot(z1 - z0) / step).ceil().max(1.0) as u32;
                for i in 0..=n {
                    let t = f64::from(i) / f64::from(n);
                    out.push((r0 + t * (r1 - r0), z0 + t * (z1 - z0)));
                }
            }
            ProfilePiece::Arc {
                start,
                end,
                centre,
                radius,
                ..
            } => {
                let a0 = (start.height.0 - centre.height.0).atan2(start.radius.0 - centre.radius.0);
                let a1 = (end.height.0 - centre.height.0).atan2(end.radius.0 - centre.radius.0);
                let n = ((a1 - a0).abs() * radius / step).ceil().max(1.0) as u32;
                for i in 0..=n {
                    let a = a0 + (a1 - a0) * f64::from(i) / f64::from(n);
                    out.push((
                        centre.radius.0 + radius * a.cos(),
                        centre.height.0 + radius * a.sin(),
                    ));
                }
            }
        }
    }
    out
}

#[test]
fn req_op_11_the_grown_tool_is_the_tool_plus_a_ball() {
    // The lower envelope of the tool grown by s is, at each radius, the
    // lowest point of the union of balls of radius s centred on the tool's
    // outline. Checked against that definition by brute force for every
    // standard shape; the grown tool's tip is s below the original tip.
    let s = 0.4;
    let shapes = [
        ToolShape::BallEnd,
        ToolShape::FlatEnd,
        ToolShape::BullNose {
            corner_radius: Millimetres(1.0),
        },
        ToolShape::TaperedBall {
            tip_radius: Millimetres(1.0),
            taper_half_angle: Degrees(15.0),
        },
        ToolShape::VBit {
            included_angle: Degrees(60.0),
            tip_diameter: Millimetres(0.0),
        },
        ToolShape::VBit {
            included_angle: Degrees(90.0),
            tip_diameter: Millimetres(1.0),
        },
        ToolShape::Drill {
            point_angle: Degrees(118.0),
        },
    ];
    for shape in shapes {
        let t = tool(shape.clone(), 6.0);
        let original = t.profile().unwrap();
        let grown = openchuck_path::cutter::grow_profile(&original, s).unwrap();
        assert!((grown.cutting_radius() - (original.cutting_radius() + s)).abs() < 1e-9);
        let pts = profile_points(&original, 2e-4);
        let mut worst = 0.0_f64;
        for i in 0..=400 {
            let r = (original.cutting_radius() + s) * f64::from(i) / 400.0;
            let brute = pts
                .iter()
                .filter(|(rp, _)| (r - rp).abs() <= s)
                .map(|(rp, zp)| zp - (s * s - (r - rp).powi(2)).sqrt())
                .fold(f64::INFINITY, f64::min);
            let got = grown.height_at(r).unwrap() - s;
            worst = worst.max((brute - got).abs());
        }
        assert!(worst < 2e-4, "{shape:?}: {worst}");
        assert_eq!(grown.height_at(0.0).unwrap(), 0.0);
        // Zero stock is the tool itself.
        assert_eq!(
            openchuck_path::cutter::grow_profile(&original, 0.0).unwrap(),
            original
        );
    }
}

#[test]
fn req_op_11_the_cutter_contact_is_on_the_tool_stock_away_from_the_surface_point() {
    use openchuck_path::cutter::cutter_contact;
    let rho = 3.0;
    let s = 0.3;
    let ball = ball(2.0 * rho).profile().unwrap();
    let tip = Point3::ORIGIN;
    // Straight below the tip: the contact is the tip.
    let c = cutter_contact(&ball, tip, UnitVec3::Z, Point3::new(0.0, 0.0, -s));
    assert!(c.distance(tip) < 1e-12);
    // To the side and below: on the ball, in line with the ball's centre.
    let dir = UnitVec3::new(Vec3::new(1.0, 0.0, -1.0)).unwrap().get();
    let centre = Point3::new(0.0, 0.0, rho);
    let surface_point = centre + dir * (rho + s);
    let c = cutter_contact(&ball, tip, UnitVec3::Z, surface_point);
    assert!(c.distance(centre + dir * rho) < 1e-9, "{c:?}");
    assert!((c.distance(surface_point) - s).abs() < 1e-9);
    // Tilted tool: the same, in the tool's own frame.
    let axis = UnitVec3::new(Vec3::new(0.3, 0.2, 0.9)).unwrap();
    let c = cutter_contact(&ball, tip, axis, tip + axis.get() * rho + dir * (rho + s));
    assert!((c.distance(tip + axis.get() * rho) - rho).abs() < 1e-9);
}

/// Path fitting measures a move's deviation along the tool's contact normal
/// (decision record 0112, closing OI-319): for a ball that is the line from
/// the contact to the ball's centre, for a flat end it is the tool axis under
/// the bottom, and a point off the tool's lower envelope has none.
#[test]
fn req_op_12_req_nfr_05_the_contact_normal_is_the_tools_normal_at_the_contact() {
    use openchuck_path::cutter::contact_normal;
    let rho = 1.5;
    let ball = ball(2.0 * rho).profile().unwrap();
    let tip = Point3::new(3.0, -2.0, 7.0);
    let axis = UnitVec3::new(Vec3::new(0.0, 0.6, 0.8)).unwrap();
    let centre = tip + axis.get() * rho;
    let side = UnitVec3::new(Vec3::new(1.0, 0.0, 0.0)).unwrap().get();
    for k in 0..=8 {
        // From straight under the tip (k = 0) round to the equator (k = 8).
        let theta = f64::from(k) * std::f64::consts::FRAC_PI_2 / 8.0;
        let outward = side * theta.sin() - axis.get() * theta.cos();
        let contact = centre + outward * rho;
        let n = contact_normal(&ball, tip, axis, contact).unwrap().get();
        // The sign is immaterial to the fit; the line is what matters.
        assert!(
            (n - outward).length() < 1e-9 || (n + outward).length() < 1e-9,
            "k {k}: {n:?}"
        );
    }
    // A flat end's lower envelope is its bottom disc: under it, and at its
    // rim, the normal is the tool axis.
    let flat = tool(ToolShape::FlatEnd, 2.0 * rho).profile().unwrap();
    for r in [0.0, 0.7, rho] {
        let under = contact_normal(&flat, tip, axis, tip + side * r)
            .unwrap()
            .get();
        assert!(under.cross(axis.get()).length() < 1e-9, "r {r}: {under:?}");
    }
    // A point well off the tool is not on its lower envelope.
    assert!(contact_normal(&ball, tip, axis, tip + side * 4.0).is_none());
}

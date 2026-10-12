// SPDX-License-Identifier: AGPL-3.0-or-later
//! SP-4 tests, named after the requirements they bear on.

use sp4_ray_cast_sweep::part::Part;
use sp4_ray_cast_sweep::sweep::*;
use sp4_ray_cast_sweep::tool::Tool;
use sp4_ray_cast_sweep::{Vector, exact, mesh, to_machine, u_dir};

const TOL: f64 = 0.01; // NFR-05 default toolpath tolerance
const BALL: Tool = Tool::Ball { radius: 3.0 };
const FLAT: Tool = Tool::Flat { radius: 3.0 };

fn grid(n: usize, lo: f64, hi: f64) -> Vec<f64> {
    (0..n).map(|i| lo + (hi - lo) * (i as f64 + 0.37) / n as f64).collect()
}

struct Setup {
    part: Part,
    mesh: parry3d_f64::shape::TriMesh,
}
fn setup(part: Part) -> Setup {
    let (nx, nth) = mesh::steps(&part, 0.001);
    Setup { part, mesh: mesh::build(&part, nx, nth) }
}

fn run(s: &Setup, tool: Tool, method: Method, normal: bool, threads: usize) -> (Vec<Option<Pose>>, Vec<f64>, Vec<f64>) {
    let c = Caster { mesh: &s.mesh, far: 40.0 };
    let xs = grid(12, 4.0, 76.0);
    let ths = grid(30, 0.0, std::f64::consts::TAU);
    let coarse = Stencil::new(&tool, 0.75, 0.0);
    let refined = Stencil::new(&tool, 0.75, 0.375);
    let poses = if normal {
        sweep_normal_following(&c, &tool, method, &coarse, &refined, &xs, &ths, threads)
    } else {
        sweep_through_axis(&c, &tool, method, &coarse, &refined, &xs, &ths, threads)
    };
    (poses, xs, ths)
}

fn check_accuracy(part: Part, tool: Tool, method: Method, normal: bool) {
    let s = setup(part);
    let (poses, _, _) = run(&s, tool, method, normal, 2);
    assert!(poses.iter().all(|p| p.is_some()), "every station hits the part");
    for p in poses.iter().flatten() {
        let ex = exact::drop(&part, &tool, p.b, p.psi).unwrap();
        let e = p.z - ex;
        assert!(e.abs() <= TOL, "{:?} {} error {e}", method, tool.name());
    }
}

#[test]
fn req_op_05_ball_accuracy_vase_and_lobed() {
    for part in [Part::vase(), Part::lobed()] {
        for method in [Method::StencilRefined, Method::Climb, Method::Distance] {
            check_accuracy(part, BALL, method, false);
            check_accuracy(part, BALL, method, true);
        }
    }
}

#[test]
fn req_op_05_flat_accuracy_vase_and_lobed() {
    for part in [Part::vase(), Part::lobed()] {
        for method in [Method::StencilRefined, Method::Climb] {
            check_accuracy(part, FLAT, method, false);
            check_accuracy(part, FLAT, method, true);
        }
    }
}

#[test]
fn req_op_12_coarse_stencil_alone_is_not_enough() {
    // The 0.75 mm ring stencil without refinement misses the contact by more than the tolerance
    // on a 3 mm ball and a 3 mm flat tool: refinement is required, not optional.
    let s = setup(Part::lobed());
    let mut worst: f64 = 0.0;
    for tool in [BALL, FLAT] {
        let (poses, _, _) = run(&s, tool, Method::StencilCoarse, false, 2);
        for p in poses.iter().flatten() {
            let ex = exact::drop(&s.part, &tool, p.b, p.psi).unwrap();
            worst = worst.max((p.z - ex).abs());
        }
    }
    assert!(worst > TOL, "coarse stencil worst {worst}");
}

#[test]
fn req_op_06_normal_offset_places_the_ball_centre_off_the_axis_line() {
    // Centre = hit point + radius * facet normal. It touches the surface within tolerance, but
    // it is not on the in-plane tool-axis line because the normal has an X component that A
    // cannot follow, so the tip height along the line is wrong by far more than the tolerance.
    let s = setup(Part::lobed());
    let (poses, _, _) = run(&s, BALL, Method::NormalOffset, true, 2);
    let mut along: f64 = 0.0;
    for p in poses.iter().flatten().step_by(7) {
        let c = p.b + p.normal * BALL.radius();
        let dev = exact::distance(&s.part, c, 3.5) - BALL.radius();
        assert!(dev.abs() <= TOL, "centre deviation {dev}");
        along = along.max((p.z - exact::drop(&s.part, &BALL, p.b, p.psi).unwrap()).abs());
    }
    assert!(along > TOL, "tip along the line differs by {along}");
}

#[test]
fn req_op_06_tool_angle_follows_the_in_plane_normal() {
    let s = setup(Part::lobed());
    let (poses, xs, ths) = run(&s, FLAT, Method::StencilCoarse, true, 2);
    let mut worst: f64 = 0.0;
    let mut moved: f64 = 0.0;
    for (k, p) in poses.iter().enumerate() {
        let p = p.unwrap();
        let (x, th) = (xs[k / ths.len()], ths[k % ths.len()]);
        let exact_psi = th - (s.part.r_th(th) / s.part.r(x, th)).atan();
        let d = (p.psi_n - exact_psi + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
        worst = worst.max(d.abs().to_degrees());
        moved = moved.max((exact_psi - th).abs().to_degrees());
    }
    assert!(worst < 1.0, "tool angle error {worst} deg");
    assert!(moved > 5.0, "the lobes move the normal angle away from the radial angle ({moved} deg)");
}

#[test]
fn req_op_15_machine_frame_keeps_the_tool_vertical() {
    for psi in [-3.0, -1.0, 0.0, 0.7, 2.5] {
        let u = to_machine(u_dir(psi), psi);
        assert!(u.x.abs() < 1e-15 && u.y.abs() < 1e-15 && (u.z - 1.0).abs() < 1e-15);
    }
    let tip = to_machine(Vector::new(3.0, 2.0, 5.0), 0.0);
    assert_eq!((tip.x, tip.y, tip.z), (3.0, 2.0, 5.0));
}

#[test]
fn req_nfr_06_identical_at_1_and_n_threads() {
    let s = setup(Part::lobed());
    for (tool, method) in [(BALL, Method::StencilRefined), (BALL, Method::Distance), (FLAT, Method::Climb)] {
        for normal in [false, true] {
            let (a, _, _) = run(&s, tool, method, normal, 1);
            for th in [2usize, 3, 4, 7] {
                let (b, _, _) = run(&s, tool, method, normal, th);
                assert_eq!(a.len(), b.len());
                for (p, q) in a.iter().zip(&b) {
                    let (p, q) = (p.unwrap(), q.unwrap());
                    assert_eq!(p.z.to_bits(), q.z.to_bits());
                    assert_eq!(p.psi.to_bits(), q.psi.to_bits());
                    assert_eq!(p.b.y.to_bits(), q.b.y.to_bits());
                }
            }
        }
    }
}

#[test]
fn req_gc_07_repeat_runs_are_identical() {
    let s = setup(Part::lobed());
    let (a, _, _) = run(&s, BALL, Method::Climb, true, 4);
    let (b, _, _) = run(&s, BALL, Method::Climb, true, 4);
    assert!(a.iter().zip(&b).all(|(p, q)| p.unwrap().z.to_bits() == q.unwrap().z.to_bits()));
}

#[test]
fn req_nfr_07_a_line_that_misses_the_part_is_none_not_a_panic() {
    let s = setup(Part::vase());
    let c = Caster { mesh: &s.mesh, far: 40.0 };
    let coarse = Stencil::new(&BALL, 0.75, 0.0);
    // Past the end of the part in X, by more than the tool radius.
    let b = Vector::new(100.0, 0.0, 0.0);
    for m in [Method::StencilRefined, Method::Climb, Method::Distance, Method::NormalOffset] {
        assert!(solve(&c, &BALL, m, &coarse, &coarse, b, 0.3).is_none());
    }
}

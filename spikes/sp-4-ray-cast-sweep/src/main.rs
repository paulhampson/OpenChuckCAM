// SPDX-License-Identifier: AGPL-3.0-or-later
//! Report binary: `cargo run --release` writes `out/report.md` and prints it.

use sp4_ray_cast_sweep::part::Part;
use sp4_ray_cast_sweep::sweep::*;
use sp4_ray_cast_sweep::tool::Tool;
use sp4_ray_cast_sweep::{Vector, exact, mesh, par, to_machine, u_dir};
use std::fmt::Write;
use std::time::Instant;

struct Stats {
    lo: f64,
    hi: f64,
    sum_abs: f64,
    n: usize,
}
impl Stats {
    fn new() -> Self {
        Stats { lo: f64::MAX, hi: f64::MIN, sum_abs: 0.0, n: 0 }
    }
    fn add(&mut self, v: f64) {
        self.lo = self.lo.min(v);
        self.hi = self.hi.max(v);
        self.sum_abs += v.abs();
        self.n += 1;
    }
    fn max_abs(&self) -> f64 {
        self.lo.abs().max(self.hi.abs())
    }
}

fn fnv(poses: &[Option<Pose>]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for p in poses {
        let bits = match p {
            Some(p) => [p.z.to_bits(), p.psi.to_bits(), p.b.x.to_bits(), p.b.y.to_bits(), p.b.z.to_bits()],
            None => [u64::MAX; 5],
        };
        for w in bits {
            for byte in w.to_le_bytes() {
                h ^= byte as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
    }
    h
}

fn grid(n: usize, lo: f64, hi: f64) -> Vec<f64> {
    (0..n).map(|i| lo + (hi - lo) * (i as f64 + 0.37) / n as f64).collect()
}

fn main() {
    let mut out = String::new();
    let threads = par::threads();
    writeln!(out, "# SP-4 report\n\nRelease build, {threads} threads available. Units mm and degrees.\n").unwrap();

    let tool_ball = Tool::Ball { radius: 3.0 };
    let tool_flat = Tool::Flat { radius: 3.0 };
    let methods_ball = [
        Method::StencilCoarse,
        Method::StencilRefined,
        Method::Climb,
        Method::Distance,
    ];
    let methods_flat = [Method::StencilCoarse, Method::StencilRefined, Method::Climb];

    // 1. Mesh error against tolerance.
    writeln!(out, "## 1. Mesh error against the chord tolerance\n").unwrap();
    writeln!(out, "Lobed part, 2000 through-axis rays. Error = mesh hit radius minus exact radius (inscribed mesh, so never positive).\n").unwrap();
    writeln!(out, "| Chord tol | Triangles | Build (ms) | Mesh error min | Max ray time (us) |\n| --- | --- | --- | --- | --- |").unwrap();
    let part = Part::lobed();
    for tol in [0.004, 0.002, 0.001, 0.0005, 0.00015] {
        let (nx, nth) = mesh::steps(&part, tol);
        let t = Instant::now();
        let m = mesh::build(&part, nx, nth);
        let build = t.elapsed();
        let c = Caster { mesh: &m, far: 40.0 };
        let xs = grid(50, 3.0, 77.0);
        let ths = grid(40, 0.0, std::f64::consts::TAU);
        let mut st = Stats::new();
        let t = Instant::now();
        let mut cnt = 0;
        for &x in &xs {
            for &th in &ths {
                let b = Vector::new(x, 0.0, 0.0);
                let hit = c.cast(b, th, 0.0, 0.0).unwrap();
                st.add(hit.z - part.r(x, th));
                cnt += 1;
            }
        }
        let per = t.elapsed().as_secs_f64() * 1e6 / cnt as f64;
        writeln!(out, "| {tol} | {} | {:.0} | {:.5} | {:.2} |", m.indices().len(), build.as_secs_f64() * 1e3, st.lo, per).unwrap();
    }

    // 2. Accuracy of the methods against the exact surface.
    let tol = 0.001;
    writeln!(out, "\n## 2. Accuracy against the exact surface (mesh chord tol {tol})\n").unwrap();
    writeln!(out, "Error = tip height from the mesh method minus the exact drop on the analytic surface along the same line (negative = the tool sits deeper than the exact surface, a gouge). Stencil rings at 0.75 mm; refinement is a pattern search down to 0.02 mm.\n").unwrap();
    writeln!(out, "| Part | Tool | Mode | Method | Poses | Err min | Err max | Mean rays | us per pose (1 thread) |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- |").unwrap();
    for (pname, part) in [("vase", Part::vase()), ("lobed", Part::lobed())] {
        let (nx, nth) = mesh::steps(&part, tol);
        let m = mesh::build(&part, nx, nth);
        let c = Caster { mesh: &m, far: 40.0 };
        let xs = grid(25, 4.0, 76.0);
        let ths = grid(40, 0.0, std::f64::consts::TAU);
        for (tool, methods) in [(tool_ball, &methods_ball[..]), (tool_flat, &methods_flat[..])] {
            let coarse = Stencil::new(&tool, 0.75, 0.0);
            let refined = Stencil::new(&tool, 0.75, 0.375);
            for mode in ["through-axis", "normal-following"] {
                let mut ms: Vec<Method> = methods.to_vec();
                if mode == "normal-following" && matches!(tool, Tool::Ball { .. }) {
                    ms.insert(0, Method::NormalOffset);
                }
                for method in ms {
                    let t = Instant::now();
                    let poses = if mode == "through-axis" {
                        sweep_through_axis(&c, &tool, method, &coarse, &refined, &xs, &ths, 1)
                    } else {
                        sweep_normal_following(&c, &tool, method, &coarse, &refined, &xs, &ths, 1)
                    };
                    let el = t.elapsed();
                    let refs = par::map_items(poses.len(), threads, |k| {
                        poses[k].and_then(|p| exact::drop(&part, &tool, p.b, p.psi))
                    });
                    let mut st = Stats::new();
                    let mut rays = 0usize;
                    for (p, r) in poses.iter().zip(&refs) {
                        if let (Some(p), Some(r)) = (p, r) {
                            st.add(p.z - r);
                            rays += p.rays;
                        }
                    }
                    writeln!(
                        out,
                        "| {pname} | {} | {mode} | {:?} | {} | {:.5} | {:.5} | {:.0} | {:.0} |",
                        tool.name(), method, st.n, st.lo, st.hi, rays as f64 / st.n as f64,
                        el.as_secs_f64() * 1e6 / poses.len() as f64
                    )
                    .unwrap();
                }
            }
        }
    }

    // 3. Tool angle in normal-following mode.
    writeln!(out, "\n## 3. Tool angle (normal-following, lobed part)\n").unwrap();
    {
        let part = Part::lobed();
        let (nx, nth) = mesh::steps(&part, tol);
        let m = mesh::build(&part, nx, nth);
        let c = Caster { mesh: &m, far: 40.0 };
        let xs = grid(40, 4.0, 76.0);
        let ths = grid(120, 0.0, std::f64::consts::TAU);
        let coarse = Stencil::new(&tool_flat, 0.75, 0.0);
        let poses = sweep_normal_following(&c, &tool_flat, Method::StencilCoarse, &coarse, &coarse, &xs, &ths, threads);
        let mut ang = Stats::new();
        let mut off_plane = Stats::new();
        let mut radial = Stats::new();
        for (k, p) in poses.iter().enumerate() {
            let p = p.unwrap();
            let (x, th) = (xs[k / ths.len()], ths[k % ths.len()]);
            let exact_psi = th - (part.r_th(th) / part.r(x, th)).atan();
            let wrap = |d: f64| (d + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
            ang.add(wrap(p.psi_n - exact_psi).to_degrees());
            radial.add(wrap(exact_psi - th).to_degrees());
            // Remaining angle between the exact normal and the tool axis (X slope; A cannot remove it).
            let n = part.normal(x, th);
            off_plane.add(n.dot(u_dir(exact_psi)).clamp(-1.0, 1.0).acos().to_degrees());
        }
        writeln!(out, "| Quantity | Min | Max |\n| --- | --- | --- |").unwrap();
        writeln!(out, "| Mesh tool angle minus exact in-plane normal angle (deg) | {:.3} | {:.3} |", ang.lo, ang.hi).unwrap();
        writeln!(out, "| Normal-following angle minus through-axis angle (deg) | {:.2} | {:.2} |", radial.lo, radial.hi).unwrap();
        writeln!(out, "| Angle left between exact normal and tool axis, from the X slope (deg) | {:.2} | {:.2} |", off_plane.lo, off_plane.hi).unwrap();
        let p = poses[10 * ths.len() + 7].unwrap();
        let a = p.psi;
        let um = to_machine(u_dir(a), a);
        let tm = to_machine(p.tip(), a);
        writeln!(out, "\nMachine frame check at one pose: tool direction after A = psi is ({:.1e}, {:.1e}, {:.6}); tip at machine X {:.3}, Y {:.3}, Z {:.3}.", um.x, um.y, um.z, tm.x, tm.y, tm.z).unwrap();
    }

    // 4. Runtime on the reference part.
    writeln!(out, "\n## 4. Runtime on the reference part (80 mm long, 0.1 mm X stations, 0.5 degree A steps = 720 x 800)\n").unwrap();
    writeln!(out, "| Mesh tol | Triangles | Tool | Mode | Method | Poses | 1 thread (s) | {threads} threads (s) | Mean rays |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- |").unwrap();
    for tol in [0.001, 0.00015] {
        let part = Part::lobed();
        let (nx, nth) = mesh::steps(&part, tol);
        let m = mesh::build(&part, nx, nth);
        let c = Caster { mesh: &m, far: 40.0 };
        let xs: Vec<f64> = (0..800).map(|i| 0.5 + 0.1 * i as f64).collect();
        let ths: Vec<f64> = (0..720).map(|i| std::f64::consts::TAU * i as f64 / 720.0).collect();
        for (tool, method, mode) in [
            (tool_ball, Method::StencilRefined, "through-axis"),
            (tool_ball, Method::Climb, "through-axis"),
            (tool_ball, Method::Distance, "through-axis"),
            (tool_flat, Method::Climb, "through-axis"),
            (tool_flat, Method::StencilRefined, "normal-following"),
        ] {
            if tol < 0.001 && !matches!(method, Method::Climb) {
                continue;
            }
            let coarse = Stencil::new(&tool, 0.75, 0.0);
            let refined = Stencil::new(&tool, 0.75, 0.375);
            let run = |th: usize| {
                let t = Instant::now();
                let poses = if mode == "through-axis" {
                    sweep_through_axis(&c, &tool, method, &coarse, &refined, &xs, &ths, th)
                } else {
                    sweep_normal_following(&c, &tool, method, &coarse, &refined, &xs, &ths, th)
                };
                (poses, t.elapsed().as_secs_f64())
            };
            let (p1, t1) = run(1);
            let (pn, tn) = run(threads);
            assert_eq!(fnv(&p1), fnv(&pn));
            let rays: usize = p1.iter().flatten().map(|p| p.rays).sum();
            writeln!(
                out,
                "| {tol} | {} | {} | {mode} | {:?} | {} | {:.1} | {:.1} | {:.0} |",
                m.indices().len(), tool.name(), method, p1.iter().flatten().count(), t1, tn,
                rays as f64 / p1.len() as f64
            )
            .unwrap();
        }
    }

    // 5. Determinism.
    writeln!(out, "\n## 5. Determinism\n").unwrap();
    {
        let part = Part::lobed();
        let (nx, nth) = mesh::steps(&part, 0.001);
        let m = mesh::build(&part, nx, nth);
        let c = Caster { mesh: &m, far: 40.0 };
        let xs = grid(60, 2.0, 78.0);
        let ths = grid(90, 0.0, std::f64::consts::TAU);
        let coarse = Stencil::new(&tool_ball, 0.75, 0.0);
        let refined = Stencil::new(&tool_ball, 0.75, 0.375);
        writeln!(out, "FNV-1a hash of every pose's bits, lobed part, ball tool:\n\n| Mode | Method | 1 thread | 2 threads | 3 threads | 4 threads | 7 threads |\n| --- | --- | --- | --- | --- | --- | --- |").unwrap();
        for mode in ["through-axis", "normal-following"] {
            for method in [Method::StencilRefined, Method::Climb, Method::Distance] {
                let hs: Vec<u64> = [1usize, 2, 3, 4, 7]
                    .iter()
                    .map(|&th| {
                        fnv(&if mode == "through-axis" {
                            sweep_through_axis(&c, &tool_ball, method, &coarse, &refined, &xs, &ths, th)
                        } else {
                            sweep_normal_following(&c, &tool_ball, method, &coarse, &refined, &xs, &ths, th)
                        })
                    })
                    .collect();
                let same = hs.iter().all(|&h| h == hs[0]);
                writeln!(out, "| {mode} | {:?} | {} |", method, hs.iter().map(|h| format!("{h:016x}")).collect::<Vec<_>>().join(" | ")).unwrap();
                assert!(same, "thread count changed the result");
            }
        }
    }

    println!("{out}");
    std::fs::create_dir_all("out").unwrap();
    std::fs::write("out/report.md", &out).unwrap();
}

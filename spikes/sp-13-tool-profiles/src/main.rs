// SPDX-License-Identifier: AGPL-3.0-or-later
//! SP-13 report: accuracy against closed form, custom-versus-standard
//! agreement, gouge checks, timings and the 1-versus-N thread comparison.
//! Run with `cargo run --release`.

use std::f64::consts::PI;
use std::time::Instant;

use sp13_tool_profiles::drop::drop_mesh;
use sp13_tool_profiles::gouge::max_gouge;
use sp13_tool_profiles::mesh::XyIndex;
use sp13_tool_profiles::normal::tip_from_contact;
use sp13_tool_profiles::par;
use sp13_tool_profiles::parts::{Part, divisions_for};
use sp13_tool_profiles::profile::{Profile, Pt, Segment};
use sp13_tool_profiles::shapes;
use sp13_tool_profiles::sweep::{Heightfield, RadialGrid, RotaryPose, sample_line};

const D: f64 = 6.0;
const R: f64 = 3.0;
const FLUTE: f64 = 12.0;
const CHORD: f64 = 0.0005;

fn tools() -> Vec<(&'static str, Profile)> {
    vec![
        ("flat D6", shapes::flat(D, FLUTE).unwrap()),
        ("ball D6", shapes::ball(D, FLUTE).unwrap()),
        (
            "bull-nose D6 rc1",
            shapes::bull_nose(D, 1.0, FLUTE).unwrap(),
        ),
        (
            "tapered ball D6 r0.5 15deg",
            shapes::tapered_ball(D, 0.5, 15.0, FLUTE).unwrap(),
        ),
        (
            "V-bit D6 90deg",
            shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap(),
        ),
        (
            "chamfer D6 90deg tip1",
            shapes::chamfer(D, 90.0, 1.0, FLUTE).unwrap(),
        ),
        ("drill D6 118deg", shapes::drill(D, 118.0, FLUTE).unwrap()),
        (
            "custom bell (concave arc)",
            Profile::new(&[
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
            ])
            .unwrap(),
        ),
    ]
}

fn index(part: &Part) -> (XyIndex, f64, usize) {
    let rmax = part.max_radius();
    let n = divisions_for(rmax, CHORD);
    let sag = rmax * (1.0 - (PI / n as f64).cos());
    (XyIndex::new(part.mesh(n, 1.0), 1.0), sag, n)
}

fn closed_ball(rs: f64, d: f64) -> f64 {
    ((rs + R).powi(2) - d * d).sqrt() - R
}
fn closed_flat(rs: f64, d: f64) -> f64 {
    if d <= R {
        rs
    } else {
        (rs * rs - (d - R).powi(2)).sqrt()
    }
}
fn closed_v90(rs: f64, d: f64) -> f64 {
    let a = PI / 4.0;
    if d <= rs * a.cos() {
        (rs * rs - d * d).sqrt()
    } else {
        rs / a.sin() - d / a.tan()
    }
}

fn main() {
    let threads = par::threads();
    println!("# SP-13 report\n");
    println!(
        "Threads available: {threads}. Tool D = {D} mm, flute {FLUTE} mm. Mesh chord {CHORD} mm.\n"
    );

    let sphere = Part::Sphere { r: 15.0 };
    let cyl = Part::Cylinder {
        r: 20.0,
        x0: -30.0,
        x1: 30.0,
    };
    let cone = Part::Cone {
        r0: 10.0,
        half_angle_deg: 20.0,
        x0: -5.0,
        x1: 5.0,
    };

    // 1. Closed-form accuracy over a dense grid of positions.
    println!(
        "## 1. Drop-cutter against closed form (mesh is inscribed, so error = exact - mesh >= 0)\n"
    );
    println!(
        "Error along the surface normal is the vertical error times the normal's Z at the contact.\n"
    );
    println!(
        "| Part | Tool | Positions | Min error (mm) | Max error (mm) | Max error along the normal (mm) | Mesh edge sagitta (mm) |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for (pname, part, rs) in [("sphere r15", &sphere, 15.0), ("cylinder r20", &cyl, 20.0)] {
        let (idx, sag, _) = index(part);
        let cases: [(&str, Profile, fn(f64, f64) -> f64); 3] = [
            ("flat", shapes::flat(D, FLUTE).unwrap(), closed_flat),
            ("ball", shapes::ball(D, FLUTE).unwrap(), closed_ball),
            (
                "V 90",
                shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap(),
                closed_v90,
            ),
        ];
        for (tname, tool, f) in &cases {
            let (mut lo, mut hi, mut hn, mut count) = (f64::MAX, f64::MIN, 0f64, 0);
            let mut scratch = Vec::new();
            for i in 0..=40 {
                for j in 0..=40 {
                    let (x, y) = (-5.0 + 0.25 * i as f64, -10.0 + 0.5 * j as f64);
                    let d = if matches!(part, Part::Sphere { .. }) {
                        x.hypot(y)
                    } else {
                        y.abs()
                    };
                    if *tname == "V 90" && d - rs * (PI / 4.0).cos() > R {
                        continue;
                    }
                    let dr = drop_mesh(tool, &idx, x, y, &mut scratch).unwrap();
                    let e = f(rs, d) - dr.tip_z;
                    hn = hn.max(e * part.normal(dr.contact)[2]);
                    lo = lo.min(e);
                    hi = hi.max(e);
                    count += 1;
                }
            }
            println!(
                "| {pname} | {tname} | {count} | {lo:.2e} | {hi:.2e} | {hn:.2e} | {sag:.2e} |"
            );
        }
    }
    {
        let (idx, sag, _) = index(&cone);
        let t = 20f64.to_radians();
        let rad = |x: f64| 10.0 + x * t.tan();
        let mut scratch = Vec::new();
        for (tname, tool, f) in [
            (
                "flat",
                shapes::flat(D, FLUTE).unwrap(),
                Box::new(move |x: f64| rad(x + R)) as Box<dyn Fn(f64) -> f64>,
            ),
            (
                "ball",
                shapes::ball(D, FLUTE).unwrap(),
                Box::new(move |x: f64| rad(x) + R / t.cos() - R),
            ),
            (
                "V 90",
                shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap(),
                Box::new(rad),
            ),
        ] {
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for i in 0..=40 {
                let x = -1.9 + 0.095 * i as f64;
                let e = f(x) - drop_mesh(&tool, &idx, x, 0.0, &mut scratch).unwrap().tip_z;
                lo = lo.min(e);
                hi = hi.max(e);
            }
            println!(
                "| cone r10 20deg (y=0) | {tname} | 41 | {lo:.2e} | {hi:.2e} | {hi:.2e} | {sag:.2e} |"
            );
        }
    }

    // 1b. Custom profiles against the equivalent standard tool.
    println!(
        "\n## 1b. Custom profile against the equivalent standard tool (169 positions per part)\n"
    );
    println!("| Custom profile | Sphere worst difference (mm) | Cylinder | Cone 20deg |");
    println!("| --- | --- | --- | --- |");
    let s2 = std::f64::consts::FRAC_1_SQRT_2;
    let l = |r, z| Segment::Line { to: Pt::new(r, z) };
    let a15 = 15f64.to_radians();
    let tt = Pt::new(0.5 * a15.cos(), 0.5 * (1.0 - a15.sin()));
    let equivalents = [
        (
            "ball as two arcs",
            shapes::ball(D, FLUTE).unwrap(),
            vec![
                Segment::Arc {
                    to: Pt::new(R * s2, R - R * s2),
                    centre: Pt::new(0.0, R),
                },
                Segment::Arc {
                    to: Pt::new(R, R),
                    centre: Pt::new(0.0, R),
                },
                l(R, FLUTE),
            ],
        ),
        (
            "flat, split bottom and side",
            shapes::flat(D, FLUTE).unwrap(),
            vec![l(1.0, 0.0), l(R, 0.0), l(R, 5.0), l(R, FLUTE)],
        ),
        (
            "V 90, split flank",
            shapes::v_bit(D, 90.0, 0.0, FLUTE).unwrap(),
            vec![l(1.2, 1.2), l(R, R), l(R, FLUTE)],
        ),
        (
            "bull-nose typed in",
            shapes::bull_nose(D, 1.0, FLUTE).unwrap(),
            vec![
                l(2.0, 0.0),
                Segment::Arc {
                    to: Pt::new(R, 1.0),
                    centre: Pt::new(2.0, 1.0),
                },
                l(R, FLUTE),
            ],
        ),
        (
            "tapered ball typed in",
            shapes::tapered_ball(D, 0.5, 15.0, FLUTE).unwrap(),
            vec![
                Segment::Arc {
                    to: tt,
                    centre: Pt::new(0.0, 0.5),
                },
                l(R, tt.z + (R - tt.r) / a15.tan()),
                l(R, FLUTE),
            ],
        ),
    ];
    let idxs: Vec<XyIndex> = [&sphere, &cyl, &cone].iter().map(|p| index(p).0).collect();
    for (name, std_tool, segs) in &equivalents {
        let custom = Profile::new(segs).unwrap();
        let mut cells = Vec::new();
        for idx in &idxs {
            let mut worst = 0f64;
            let mut scratch = Vec::new();
            for i in -6..=6 {
                for j in -6..=6 {
                    let (x, y) = (0.37 * i as f64, 0.91 * j as f64);
                    let a = drop_mesh(std_tool, idx, x, y, &mut scratch).unwrap().tip_z;
                    let b = drop_mesh(&custom, idx, x, y, &mut scratch).unwrap().tip_z;
                    worst = worst.max((a - b).abs());
                }
            }
            cells.push(format!("{worst:.1e}"));
        }
        println!("| {name} | {} |", cells.join(" | "));
    }
    drop(idxs);

    // 2. Gouge check at every tool's dropped pose, and with a 0.05 mm seeded fault.
    println!("\n## 2. Independent gouge checker (exact surface, 0.01 mm sampling)\n");
    println!(
        "| Part | Tool | Worst gouge at dropped pose (mm) | Smallest gouge with tip lowered 0.05 mm (mm) |"
    );
    println!("| --- | --- | --- | --- |");
    for (pname, part) in [("sphere r15", &sphere), ("cylinder r20", &cyl)] {
        let (idx, _, _) = index(part);
        for (tname, tool) in tools() {
            let (mut ok, mut seeded) = (0f64, f64::MAX);
            for (x, y) in [(0.0, 0.0), (1.3, 2.2), (-2.0, 6.0), (0.5, -9.5)] {
                let z = drop_mesh(&tool, &idx, x, y, &mut Vec::new()).unwrap().tip_z;
                ok = ok.max(max_gouge(&tool, part, [x, y, z], 0.01).depth);
                seeded = seeded.min(max_gouge(&tool, part, [x, y, z - 0.05], 0.01).depth);
            }
            println!("| {pname} | {tname} | {ok:.2e} | {seeded:.4} |");
        }
    }

    // 3. Timings.
    println!("\n## 3. Timings (release build, this container)\n");
    let (idx, _, n) = index(&sphere);
    println!(
        "Sphere mesh: {} triangles ({n} divisions around).\n",
        idx.mesh.tris.len()
    );
    println!(
        "| Tool | Profile query h(r) (ns) | Drop per position, 1 thread (us) | Normal-offset tip (ns) |"
    );
    println!("| --- | --- | --- | --- |");
    for (tname, tool) in tools() {
        let reps = 2_000_000;
        let t0 = Instant::now();
        let mut acc = 0.0;
        for k in 0..reps {
            acc += tool.height_at(R * (k % 1000) as f64 / 1000.0).unwrap();
        }
        let h_ns = t0.elapsed().as_nanos() as f64 / reps as f64;
        let mut scratch = Vec::new();
        let t0 = Instant::now();
        let mut q = 0;
        for i in 0..40 {
            for j in 0..40 {
                acc += drop_mesh(
                    &tool,
                    &idx,
                    -8.0 + 0.4 * i as f64,
                    -8.0 + 0.4 * j as f64,
                    &mut scratch,
                )
                .unwrap()
                .tip_z;
                q += 1;
            }
        }
        let drop_us = t0.elapsed().as_secs_f64() * 1e6 / q as f64;
        let normal = if tool.is_convex() {
            let t0 = Instant::now();
            let reps = 200_000;
            for k in 0..reps {
                let a = k as f64 * 5e-6;
                let p = [15.0 * a.sin(), 0.0, 15.0 * a.cos()];
                acc += tip_from_contact(&tool, p, [p[0] / 15.0, 0.0, p[2] / 15.0], [0.0, 0.0, 1.0])
                    .unwrap()[2];
            }
            format!("{:.0}", t0.elapsed().as_nanos() as f64 / reps as f64)
        } else {
            "n/a (not convex)".into()
        };
        std::hint::black_box(acc);
        println!("| {tname} | {h_ns:.1} | {drop_us:.1} | {normal} |");
    }

    // Sweeps.
    println!();
    println!(
        "| Sweep | Tool | Cells x poses | 1 thread (s) | {threads} threads (s) | ns per cell-pose (1 thread) | Identical bits |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    let stock = Heightfield::block(-10.0, -5.0, 0.05, 401, 201, 0.0);
    let poses = sample_line([-8.0, -1.0, -2.0], [8.0, 1.5, -1.0], 0.0125);
    for (tname, tool) in tools() {
        let t0 = Instant::now();
        let a = stock.swept(&tool, &poses, 1);
        let t1 = t0.elapsed().as_secs_f64();
        let t0 = Instant::now();
        let b = stock.swept(&tool, &poses, threads);
        let tn = t0.elapsed().as_secs_f64();
        let same = a
            .top
            .iter()
            .zip(&b.top)
            .all(|(p, q)| p.to_bits() == q.to_bits());
        // Cells touched per pose: the footprint square.
        let cells = ((2.0 * R / 0.05).ceil() + 1.0).powi(2) as usize * poses.len();
        println!(
            "| dexel 401x201, 0.05 mm, line move | {tname} | {cells} | {t1:.3} | {tn:.3} | {:.1} | {same} |",
            t1 * 1e9 / cells as f64
        );
    }
    let rg = RadialGrid::cylinder(-4.0, 0.05, 161, 360, 20.0);
    let rposes: Vec<RotaryPose> = (0..720)
        .map(|k| RotaryPose {
            tip: [-1.0 + 0.003 * k as f64, 0.4, 18.5],
            a: std::f64::consts::TAU * k as f64 / 720.0,
        })
        .collect();
    for (tname, tool) in tools() {
        let t0 = Instant::now();
        let a = rg.swept(&tool, &rposes, 1);
        let t1 = t0.elapsed().as_secs_f64();
        let t0 = Instant::now();
        let b = rg.swept(&tool, &rposes, threads);
        let tn = t0.elapsed().as_secs_f64();
        let same = a
            .rho
            .iter()
            .zip(&b.rho)
            .all(|(p, q)| p.to_bits() == q.to_bits());
        let cells = (2.0 * R / 0.05).ceil() as usize * 360 * rposes.len();
        println!(
            "| rotary grid 161x360, wrapped move, 360 deg of A | {tname} | {cells} | {t1:.3} | {tn:.3} | {:.1} | {same} |",
            t1 * 1e9 / cells as f64
        );
    }
}

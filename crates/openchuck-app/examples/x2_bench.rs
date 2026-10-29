// SPDX-License-Identifier: AGPL-3.0-or-later
//! Throwaway (lane STEPALL-X2 stage 1): times the exact ball drop against the
//! tessellation drop on helical-finishing tool lines of a placed BrepModel
//! (JSON from `x2_dump`), one thread.
#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]
use std::time::Instant;

use openchuck_path::{BrepSurface, ParametricFace, SurfaceSource, ToolLine, Uv};
use openchuck_types::brep::BrepModel;
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::tool::profile::{ProfilePoint, ProfileSegment, RevolvedProfile, ToolProfile};

fn ball(r: f64) -> RevolvedProfile {
    ToolProfile {
        segments: vec![
            ProfileSegment::Arc {
                to: ProfilePoint::new(r, r),
                centre: ProfilePoint::new(0.0, r),
            },
            ProfileSegment::Line {
                to: ProfilePoint::new(r, 4.0 * r),
            },
        ],
    }
    .validate()
    .unwrap()
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q) as usize]
}

fn brute(surface: &BrepSurface, line: &ToolLine, r: f64, n: usize) -> Option<(f64, u32, Point3)> {
    let a = line.axis.get();
    let o = line.origin + a * r;
    let mut best: Option<(f64, u32, Point3)> = None;
    for f in surface.faces() {
        let (lo, hi) = f.uv_bounds();
        for i in 0..=n {
            for j in 0..=n {
                let uv = Uv {
                    u: lo.u + (hi.u - lo.u) * i as f64 / n as f64,
                    v: lo.v + (hi.v - lo.v) * j as f64 / n as f64,
                };
                if !f.contains(uv) {
                    continue;
                }
                let Some(sp) = f.point_normal(uv) else {
                    continue;
                };
                let w = sp.point - o;
                let z = w.dot(a);
                let rho2 = w.dot(w) - z * z;
                if rho2 <= r * r {
                    let s = z + (r * r - rho2).sqrt();
                    if best.is_none_or(|b| s > b.0) {
                        best = Some((s, f.number(), sp.point));
                    }
                }
            }
        }
    }
    best
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    if path == "check" {
        let path = args.next().unwrap();
        let x: f64 = args.next().unwrap().parse().unwrap();
        let a: f64 = args.next().unwrap().parse().unwrap();
        let r: f64 = args.next().unwrap().parse().unwrap();
        let n: usize = args.next().unwrap().parse().unwrap();
        let model: BrepModel = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let surface = BrepSurface::new(&model).unwrap();
        let (s, c) = a.to_radians().sin_cos();
        let line = ToolLine {
            origin: Point3::new(x, 0.0, 0.0),
            axis: UnitVec3::new(Vec3::new(0.0, c, s)).unwrap(),
        };
        let tool = ball(r);
        println!("mesh  {:?}", surface.drop_tool(&tool, &line));
        println!("exact {:?}", surface.drop_ball(&tool, &line));
        println!("brute {:?}", brute(&surface, &line, r, n));
        for f in surface.faces() {
            let b = f.bounding_box();
            println!("face {} box {:?}", f.number(), b);
        }
        return;
    }
    let dx: f64 = args.next().map_or(0.5, |s| s.parse().unwrap());
    let da: f64 = args.next().map_or(6.0, |s| s.parse().unwrap());
    let r: f64 = args.next().map_or(1.0, |s| s.parse().unwrap());
    let model: BrepModel = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let surface = BrepSurface::new(&model).unwrap();
    let b = surface.bounds();
    let tool = ball(r);
    let (mut tm, mut te, mut diff, mut looked) = (vec![], vec![], vec![], vec![]);
    let (mut raised, mut none_e, mut none_m, mut fallback) = (0usize, 0usize, 0usize, 0usize);
    let mut worst = (0.0, Point3::ORIGIN, 0.0);
    let mut x = b.min.x - r;
    while x <= b.max.x + r {
        let mut a = 0.0_f64;
        while a < 360.0 {
            let (s, c) = a.to_radians().sin_cos();
            let line = ToolLine {
                origin: Point3::new(x, 0.0, 0.0),
                axis: UnitVec3::new(Vec3::new(0.0, c, s)).unwrap(),
            };
            let t0 = Instant::now();
            let m = surface.drop_tool(&tool, &line);
            let t1 = Instant::now();
            let e = surface.drop_ball(&tool, &line);
            let t2 = Instant::now();
            if (t2 - t1).as_secs_f64()
                > std::env::var("X2_SLOW").map_or(1.0, |v| v.parse::<f64>().unwrap())
                && std::env::var("X2_SLOW").is_ok()
            {
                println!(
                    "slow x {x:.2} a {a} {:.0} us {:?}",
                    (t2 - t1).as_secs_f64() * 1e6,
                    e.map(|e| (e.looked, e.raised, e.position.face))
                );
            }
            tm.push((t1 - t0).as_secs_f64() * 1e6);
            te.push((t2 - t1).as_secs_f64() * 1e6);
            match (m, e) {
                (Some(m), Some(e)) => {
                    let d = e.contact.offset - m.offset;
                    if d.abs() > worst.0 {
                        worst = (d.abs(), line.origin, a);
                    }
                    diff.push(d);
                    looked.push(e.looked as f64);
                    if e.raised > 0.0 {
                        raised += 1;
                    }
                }
                (None, Some(_)) => none_m += 1,
                (Some(_), None) => {
                    none_e += 1;
                }
                (None, None) => {}
            }
            if e.is_none() && m.is_some() {
                fallback += 1;
            }
            a += da;
        }
        x += dx;
    }
    let n = tm.len();
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "lines {n}; both {}; mesh-only {none_e}; exact-only {none_m}; raised {raised}; fallback {fallback}",
        diff.len()
    );
    println!(
        "mesh  us: mean {:.1} p50 {:.1} p90 {:.1} p99 {:.1} max {:.1}",
        mean(&tm),
        pct(&mut tm.clone(), 0.5),
        pct(&mut tm.clone(), 0.9),
        pct(&mut tm.clone(), 0.99),
        pct(&mut tm, 1.0)
    );
    println!(
        "exact us: mean {:.1} p50 {:.1} p90 {:.1} p99 {:.1} max {:.1}",
        mean(&te),
        pct(&mut te.clone(), 0.5),
        pct(&mut te.clone(), 0.9),
        pct(&mut te.clone(), 0.99),
        pct(&mut te, 1.0)
    );
    println!(
        "looked: mean {:.1} p99 {:.0} max {:.0}",
        mean(&looked),
        pct(&mut looked.clone(), 0.99),
        pct(&mut looked, 1.0)
    );
    let mut ad: Vec<f64> = diff.iter().map(|d| d.abs()).collect();
    println!(
        "exact - mesh mm: min {:.5} max {:.5}; |d| p50 {:.5} p99 {:.5}; worst at {:?} a {}",
        diff.iter().copied().fold(f64::INFINITY, f64::min),
        diff.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        pct(&mut ad.clone(), 0.5),
        pct(&mut ad, 0.99),
        worst.1,
        worst.2
    );
}

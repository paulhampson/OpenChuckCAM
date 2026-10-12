// SPDX-License-Identifier: AGPL-3.0-only
// SP-9 exact surfaces: what does SurfaceSource need for B-rep faces, and who evaluates NURBS?
// Compares, against OpenCASCADE as the reference: our own analytic and NURBS code, curvo, and the
// kernel tessellation. Throwaway spike code; never merged into the product.
mod analytic;
mod ffi;
mod mesh;
mod nurbs;
mod vec3;

use analytic::{Analytic, Kind};
use curvo::prelude::*;
use ffi::{BSplineData, Mesh, Model};
use nalgebra::{Point3, Point4};
use nurbs::Nurbs;
use std::collections::BTreeMap;
use std::time::Instant;
use vec3::*;

const TYPE_NAMES: [&str; 11] = ["plane", "cylinder", "cone", "sphere", "torus", "bezier", "bspline", "revolution", "extrusion", "offset", "other"];

enum Exact { An(Analytic), Nu(Nurbs), None }

impl Exact {
    fn eval(&self, u: f64, v: f64) -> Option<(V3, V3)> {
        match self { Exact::An(a) => Some(a.eval(u, v)), Exact::Nu(n) => Some(n.eval(u, v)), Exact::None => None }
    }
    fn project(&self, q: V3) -> Option<(f64, f64, V3)> {
        match self { Exact::An(a) => Some(a.project(q)), Exact::Nu(n) => Some(n.project(q)), Exact::None => None }
    }
    fn ray(&self, o: V3, d: V3, tmax: f64) -> Vec<(f64, f64, f64)> {
        match self { Exact::An(a) => a.ray(o, d, tmax), Exact::Nu(n) => n.ray(o, d, tmax), Exact::None => vec![] }
    }
}

/// Running statistics for one (surface type, operation, method) cell.
#[derive(Default, Clone)]
struct Acc { n: u64, max: f64, sum: f64, bad: u64, ns: f64, calls: u64 }
impl Acc {
    fn add(&mut self, v: f64) { self.n += 1; self.sum += v; if v > self.max { self.max = v; } }
    fn time(&mut self, t: std::time::Duration) { self.ns += t.as_nanos() as f64; self.calls += 1; }
}
type Table = BTreeMap<(String, String, String), Acc>;
fn cell<'a>(t: &'a mut Table, ty: &str, op: &str, method: &str) -> &'a mut Acc {
    t.entry((ty.to_string(), op.to_string(), method.to_string())).or_default()
}

fn build_exact(m: &Model, i: usize, ty: i32, rev: bool, force: bool) -> (Exact, Option<BSplineData>) {
    let ty = if force && ty <= 4 { 6 } else { ty };
    match ty {
        0..=4 => {
            let (f, _) = m.frame(i);
            let kind = match ty {
                0 => Kind::Plane, 1 => Kind::Cylinder, 2 => Kind::Cone { semi: f[10] }, 3 => Kind::Sphere, _ => Kind::Torus { minor: f[10] },
            };
            let r = if ty == 2 { f[9] } else { f[9] };
            (Exact::An(Analytic { kind, o: [f[0], f[1], f[2]], z: [f[3], f[4], f[5]], x: [f[6], f[7], f[8]], r, flip: rev }), None)
        }
        5 | 6 | 7 | 8 | 9 | 10 => match m.bspline(i) {
            Some(b) => (Exact::Nu(Nurbs { pu: b.pu, pv: b.pv, uk: b.uk.clone(), vk: b.vk.clone(), nu: b.nu, nv: b.nv,
                poles: b.poles.clone(), w: b.w.clone(), flip: rev }), Some(b)),
            None => (Exact::None, None),
        },
        _ => (Exact::None, None),
    }
}

/// Run a curvo call, turning a panic into None (NFR-07: invalid input must not crash).
fn guard<T>(f: impl FnOnce() -> T) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok()
}

fn curvo_surface(b: &BSplineData) -> NurbsSurface3D<f64> {
    let cps: Vec<Vec<Point4<f64>>> = (0..b.nu).map(|i| (0..b.nv).map(|j| {
        let k = i * b.nv + j; let (p, w) = (b.poles[k], b.w[k]);
        Point4::new(p[0] * w, p[1] * w, p[2] * w, w)
    }).collect()).collect();
    NurbsSurface3D::new(b.pu, b.pv, b.uk.clone(), b.vk.clone(), cps)
}

/// Even-odd point-in-polygon over all loops; u is shifted by whole periods into the loops' range.
fn in_trims(loops: &[Vec<[f64; 2]>], u: f64, v: f64, period: Option<f64>) -> bool {
    let umin = loops.iter().flatten().map(|p| p[0]).fold(f64::MAX, f64::min);
    let u = match period { Some(p) => umin + (u - umin).rem_euclid(p), None => u };
    let mut inside = false;
    for l in loops {
        let n = l.len();
        for k in 0..n {
            let (a, b) = (l[k], l[(k + 1) % n]);
            if (a[1] > v) != (b[1] > v) && u < (b[0] - a[0]) * (v - a[1]) / (b[1] - a[1]) + a[0] { inside = !inside; }
        }
    }
    inside
}

/// Rational NURBS check, parameter-free: the kernel converts an analytic face to a B-spline (rational for
/// spheres and tori), and the reference is our own analytic code (already matched to the kernel at 1e-14).
fn converted_face(m: &Model, i: usize, an: &Analytic, uv: &[f64; 4], tyname: &str, t: &mut Table) {
    let Some(b) = m.bspline(i) else { return };
    let ours = Nurbs { pu: b.pu, pv: b.pv, uk: b.uk.clone(), vk: b.vk.clone(), nu: b.nu, nv: b.nv, poles: b.poles.clone(), w: b.w.clone(), flip: false };
    let cs = curvo_surface(&b);
    let g = 12;
    for iu in 0..g { for iv in 0..g {
        let (fu, fv) = (0.05 + 0.9 * (iu as f64 + 0.31) / g as f64, 0.05 + 0.9 * (iv as f64 + 0.57) / g as f64);
        // Shape of the conversion: points of the NURBS lie on the analytic surface.
        let (ud, vd) = (ours.u_domain(), ours.v_domain());
        let (u, v) = (ud.0 + (ud.1 - ud.0) * fu, vd.0 + (vd.1 - vd.0) * fv);
        let p = ours.d1(u, v).0;
        let (_, _, q) = an.project(p);
        cell(t, tyname, "conversion: distance of NURBS point to exact surface (mm)", "rust").add(dist(p, q));
        if let Some(c) = guard(|| { let c = cs.point_at(u, v); [c.x, c.y, c.z] }) { cell(t, tyname, "conversion: distance of NURBS point to exact surface (mm)", "curvo").add(dist(c, an.project(c).2)); cell(t, tyname, "evaluator agreement, rust vs curvo (mm)", "curvo").add(dist(c, p)); }
        else { cell(t, tyname, "conversion: distance of NURBS point to exact surface (mm)", "curvo").bad += 1; }
        // Projection and rays from points near the exact surface.
        let (ue, ve) = (uv[0] + (uv[1] - uv[0]) * fu, uv[2] + (uv[3] - uv[2]) * fv);
        let (ep, en) = an.eval(ue, ve);
        for delta in [0.25, -0.25] {
            let qq = add(ep, mul(en, delta));
            let (_, _, rq) = an.project(qq);
            let (_, _, nq) = ours.project(qq);
            cell(t, tyname, "project point (mm)", "rust").add(dist(nq, rq));
            let c = guard(|| cs.find_closest_parameter(&Point3::new(qq[0], qq[1], qq[2]), None).map(|(a, b2)| { let x = cs.point_at(a, b2); [x.x, x.y, x.z] })).and_then(|r| r.ok());
            match c { Some(cp) => cell(t, tyname, "project point (mm)", "curvo").add(dist(cp, rq)), None => cell(t, tyname, "project point (mm)", "curvo").bad += 1 }
        }
        let tan = norm(if en[0].abs() < 0.9 { cross(en, [1.0, 0.0, 0.0]) } else { cross(en, [0.0, 1.0, 0.0]) });
        let d = norm(add(mul(en, -1.0), mul(tan, 0.15)));
        let o = add(ep, mul(en, 3.0));
        // Untrimmed first hit, bounded to the patch's own domain on both sides (the analytic face is the same surface).
        let first = |h: Vec<(f64, f64, f64)>| h.first().map(|x| x.0);
        let a_hit = an.ray(o, d, 20.0).first().map(|x| x.0);
        let n_hit = first(ours.ray(o, d, 20.0));
        let a = cell(t, tyname, "ray first hit (mm)", "rust");
        match (a_hit, n_hit) { (Some(x), Some(y)) => a.add((x - y).abs()), (None, None) => a.add(0.0), _ => a.bad += 1 }
    } }
}

fn run_fixture(path: &str, t: &mut Table, notes: &mut Vec<String>, force: bool) -> Result<(), String> {
    let m = Model::read(path)?;
    let name = path.rsplit('/').next().unwrap().trim_end_matches(".step").to_string();
    let levels: [(f64, f64, &str); 2] = [(0.1, 0.5, "mesh 0.1"), (0.01, 0.1, "mesh 0.01")];
    let mut meshes: Vec<Vec<Mesh>> = Vec::new();
    for (lin, ang, _) in levels {
        if !m.mesh(lin, ang) { return Err("mesh failed".into()); }
        meshes.push((0..m.faces()).map(|i| m.face_mesh(i)).collect());
    }
    for i in 0..m.faces() {
        let info = m.info(i);
        let mut tyname = TYPE_NAMES.get(info.ty as usize).copied().unwrap_or("other").to_string();
        if force { if info.ty > 4 { continue; } tyname.push_str(" as NURBS"); }
        if force {
            if let Exact::An(an) = build_exact(&m, i, info.ty, info.reversed, false).0 { converted_face(&m, i, &an, &info.uv, &tyname, t); }
            continue;
        }
        let (exact, bs) = build_exact(&m, i, info.ty, info.reversed, force);
        if let Some(b) = &bs { notes.push(format!("{name} face {i} {tyname}: NURBS degree {}x{}, {}x{} poles, rational {}", b.pu, b.pv, b.nu, b.nv, b.rational)); }
        let curvo = bs.as_ref().map(curvo_surface);
        let [u0, u1, v0, v1] = info.uv;
        let (u0, u1, v0, v1) = match &exact { Exact::Nu(n) => { let (a, b) = n.u_domain(); let (c, d) = n.v_domain(); (u0.max(a), u1.min(b), v0.max(c), v1.min(d)) } _ => (u0, u1, v0, v1) };
        let periodic = matches!(&exact, Exact::An(a) if !matches!(a.kind, Kind::Plane));
        let trims = m.trims(i, 48);
        let g = 14;
        let (mut n_samples, mut n_trim_dis, mut n_trim_pts) = (0, 0, 0);
        for iu in 0..g { for iv in 0..g {
            let fu = 0.06 + 0.88 * (iu as f64 + 0.37) / g as f64; let fv = 0.06 + 0.88 * (iv as f64 + 0.61) / g as f64;
            let (u, v) = (u0 + (u1 - u0) * fu, v0 + (v1 - v0) * fv);
            // --- trim classification, on every sample (holes included) ---
            let kin = m.contains(i, u, v);
            let pin = !trims.is_empty() && in_trims(&trims, u, v, if periodic { Some(2.0 * std::f64::consts::PI) } else { None });
            n_trim_pts += 1; if kin != pin { n_trim_dis += 1; }
            // --- evaluation ---
            let Some((kp, kn)) = m.eval(i, u, v) else { continue };
            if !kin { /* ray test below still runs on the untrimmed point; projection and mesh need a point on the face */ }
            if kin {
                n_samples += 1;
                if let Some((ep, en)) = exact.eval(u, v) {
                    let t0 = Instant::now(); let _ = exact.eval(u, v); cell(t, &tyname, "eval", "rust").time(t0.elapsed());
                    cell(t, &tyname, "eval point (mm)", "rust").add(dist(ep, kp));
                    cell(t, &tyname, "eval normal (deg)", "rust").add(angle_deg(en, kn));
                }
                let t0 = Instant::now(); let _ = m.eval(i, u, v); cell(t, &tyname, "eval", "kernel").time(t0.elapsed());
                if let Some(cs) = &curvo {
                    let t0 = Instant::now();
                    let ce = guard(|| { let cp = cs.point_at(u, v); let cn = cs.normal_at(u, v); ([cp.x, cp.y, cp.z], [cn.x, cn.y, cn.z]) });
                    cell(t, &tyname, "eval", "curvo").time(t0.elapsed());
                    match ce {
                        Some((cp, cn)) => {
                            cell(t, &tyname, "eval point (mm)", "curvo").add(dist(cp, kp));
                            cell(t, &tyname, "eval normal (deg)", "curvo").add(angle_deg(cn, kn).min(180.0 - angle_deg(cn, kn)));
                        }
                        None => { cell(t, &tyname, "eval point (mm)", "curvo").bad += 1; }
                    }
                }
                for (k, (_, _, label)) in levels.iter().enumerate() {
                    if let Some((q, fnrm, d)) = meshes[k][i].closest(kp) {
                        let _ = q; cell(t, &tyname, "eval point (mm)", label).add(d);
                        cell(t, &tyname, "eval normal (deg)", label).add(angle_deg(fnrm, kn));
                    }
                }
                // --- projection: points 0.25 mm off the surface, both sides ---
                for delta in [0.25, -0.25] {
                    let q = add(kp, mul(kn, delta));
                    let t0 = Instant::now(); let kr = m.project(i, q); cell(t, &tyname, "project", "kernel").time(t0.elapsed());
                    let Some((ku, kv, kq, kd)) = kr else { continue };
                    if !m.contains(i, ku, kv) { continue; }
                    let _ = (ku, kv);
                    let t0 = Instant::now(); let rr = exact.project(q); cell(t, &tyname, "project", "rust").time(t0.elapsed());
                    if let Some((_, _, rq)) = rr {
                        cell(t, &tyname, "project point (mm)", "rust").add(dist(rq, kq));
                        cell(t, &tyname, "project distance (mm)", "rust").add((dist(rq, q) - kd).abs());
                    }
                    if let Some(cs) = &curvo {
                        let t0 = Instant::now();
                        let cr = guard(|| cs.find_closest_parameter(&Point3::new(q[0], q[1], q[2]), None).map(|(cu, cv)| { let p = cs.point_at(cu, cv); [p.x, p.y, p.z] }));
                        cell(t, &tyname, "project", "curvo").time(t0.elapsed());
                        match cr.and_then(|r| r.ok()) {
                            Some(cp) => {
                                cell(t, &tyname, "project point (mm)", "curvo").add(dist(cp, kq));
                                cell(t, &tyname, "project distance (mm)", "curvo").add((dist(cp, q) - kd).abs()); }
                            None => cell(t, &tyname, "project point (mm)", "curvo").bad += 1,
                        }
                    }
                    let t0 = Instant::now(); let kdr = m.distance(i, q); cell(t, &tyname, "distance to trimmed face", "kernel").time(t0.elapsed());
                    let _ = kdr;
                    for (k, (_, _, label)) in levels.iter().enumerate() {
                        if let Some((mq, _, md)) = meshes[k][i].closest(q) {
                            cell(t, &tyname, "project point (mm)", label).add(dist(mq, kq));
                            cell(t, &tyname, "project distance (mm)", label).add((md - kd).abs());
                        }
                    }
                }
            }
            // --- rays: from 3 mm above the sample, along the inward normal tilted 0.15, over the whole UV box ---
            let tan = norm(if kn[0].abs() < 0.9 { cross(kn, [1.0, 0.0, 0.0]) } else { cross(kn, [0.0, 1.0, 0.0]) });
            let d = norm(add(mul(kn, -1.0), mul(tan, 0.15)));
            let o = add(kp, mul(kn, 3.0));
            let tmax = 20.0;
            let t0 = Instant::now(); let kh = m.ray(i, o, d, tmax); cell(t, &tyname, "ray", "kernel").time(t0.elapsed());
            let kfirst = kh.first().map(|h| h.0);
            let t0 = Instant::now(); let rh = exact.ray(o, d, tmax); cell(t, &tyname, "ray", "rust").time(t0.elapsed());
            let period = if periodic { Some(2.0 * std::f64::consts::PI) } else { None };
            let tau = 2.0 * std::f64::consts::PI;
            let wrap = |h: &(f64, f64, f64)| -> (f64, f64) {
                let u = match period { Some(p) => info.uv[0] + (h.1 - info.uv[0]).rem_euclid(p), None => h.1 };
                let v = if tyname.starts_with("torus") { info.uv[2] + (h.2 - info.uv[2]).rem_euclid(tau) } else { h.2 };
                (u, v)
            };
            let in_box = |h: &(f64, f64, f64)| {
                let (u, v) = wrap(h);
                u >= info.uv[0] - 1e-9 && u <= info.uv[1] + 1e-9 && v >= info.uv[2] - 1e-9 && v <= info.uv[3] + 1e-9
            };
            let first_box = rh.iter().find(|h| in_box(h)).map(|h| h.0);
            let first_poly = rh.iter().find(|h| in_box(h) && !trims.is_empty() && { let (wu, wv) = wrap(h); in_trims(&trims, wu, wv, period) }).map(|h| h.0);
            for (label, f) in [("rust, UV box only", first_box), ("rust, UV box + trim loops", first_poly)] {
                let a = cell(t, &tyname, "ray first hit (mm)", label);
                match (kfirst, f) {
                    (Some(a1), Some(b1)) => a.add((a1 - b1).abs()),
                    (None, None) => a.add(0.0),
                    _ => a.bad += 1,
                }
            }
            if let Some(cs) = &curvo {
                let end = add(o, mul(d, tmax));
                let line = NurbsCurve3D::polyline(&[Point3::new(o[0], o[1], o[2]), Point3::new(end[0], end[1], end[2])], false);
                let t0 = Instant::now(); let r = guard(|| cs.find_intersection(&line, None)).and_then(|r| r.ok()); cell(t, &tyname, "ray", "curvo").time(t0.elapsed());
                let first = r.and_then(|v| {
                    let mut ts: Vec<f64> = v.into_iter().map(|x| x.as_tuple().1 .1).collect();
                    ts.sort_by(|a, b| a.partial_cmp(b).unwrap()); ts.first().copied()
                });
                let a = cell(t, &tyname, "ray first hit (mm)", "curvo (untrimmed)");
                // Untrimmed first hit: compare with our own untrimmed result, not the trimmed kernel's.
                match (rh.first().map(|h| h.0), first) { (Some(a1), Some(b1)) => a.add((a1 - b1).abs()), (None, None) => a.add(0.0), _ => a.bad += 1 }
            }
            for (k, (_, _, label)) in levels.iter().enumerate() {
                let t0 = Instant::now(); let mh = meshes[k][i].ray(o, d, tmax); cell(t, &tyname, "ray", label).time(t0.elapsed());
                let a = cell(t, &tyname, "ray first hit (mm)", label);
                match (kfirst, mh.first().map(|h| h.0)) {
                    (Some(a1), Some(b1)) => a.add((a1 - b1).abs()),
                    (None, None) => a.add(0.0),
                    _ => a.bad += 1,
                }
            }
        } }
        notes.push(format!("{name} face {i} {tyname}: {n_samples} on-face samples, trim loops {}, UV-polygon vs kernel classification disagrees on {n_trim_dis} of {n_trim_pts}", trims.len()));
    }
    Ok(())
}

fn main() {
    ffi::quiet();
    std::panic::set_hook(Box::new(|_| {}));
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).map(String::as_str).unwrap_or("fixtures");
    let mut files: Vec<String> = std::fs::read_dir(dir).expect("fixtures dir").filter_map(|e| e.ok())
        .map(|e| e.path().to_string_lossy().to_string()).filter(|p| p.ends_with(".step")).collect();
    files.sort();
    let mut table = Table::new(); let mut notes = Vec::new();
    let force = args.get(2).map(String::as_str) == Some("nurbs");
    for f in &files {
        if let Err(e) = run_fixture(f, &mut table, &mut notes, force) { eprintln!("{f}: {e}"); }
    }
    println!("| Surface | Operation | Method | Samples | Max | Mean | Failures |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for ((ty, op, me), a) in &table {
        if a.n == 0 && a.bad == 0 { continue; }
        println!("| {ty} | {op} | {me} | {} | {:.3e} | {:.3e} | {} |", a.n, a.max, a.sum / a.n.max(1) as f64, a.bad);
    }
    println!();
    println!("| Surface | Operation | Method | ns per call |");
    println!("| --- | --- | --- | --- |");
    for ((ty, op, me), a) in &table {
        if a.calls > 0 { println!("| {ty} | {op} | {me} | {:.0} |", a.ns / a.calls as f64); }
    }
    println!();
    for n in notes { println!("{n}"); }
}

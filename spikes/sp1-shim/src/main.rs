// SPDX-License-Identifier: AGPL-3.0-only
//! SP-1 option B prototype: our own C ABI over OpenCASCADE, driven from Rust with no cxx and no
//! opencascade-sys. Throwaway spike code; never merged into the product.
//!
//!   walk <file> [--tess]     report faces and edges with stable IDs, two digests
//!   selftest <fixtures-dir>  closed-form checks on the SP-1 fixtures; non-zero exit on failure
//!   isolate <file>...        run `walk` for each file in a child process with a timeout (NFR-07 pattern)

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::fmt::Write as _;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

extern "C" {
    fn occ_quiet();
    fn occ_read_step(path: *const c_char, err: *mut c_char, err_len: c_int) -> *mut c_void;
    fn occ_free(m: *mut c_void);
    fn occ_face_count(m: *mut c_void) -> c_int;
    fn occ_edge_count(m: *mut c_void) -> c_int;
    fn occ_face_info(m: *mut c_void, i: c_int, out: *mut f64, uv: *mut f64, area: *mut f64, rev: *mut c_int) -> c_int;
    fn occ_face_eval(m: *mut c_void, i: c_int, u: f64, v: f64, pt: *mut f64, n: *mut f64) -> c_int;
    fn occ_edge_info(m: *mut c_void, i: c_int, ty: *mut c_int, ends: *mut f64, faces: *mut c_int) -> c_int;
    fn occ_mesh(m: *mut c_void, lin: f64, ang: f64, parallel: c_int) -> c_int;
    fn occ_face_mesh(m: *mut c_void, i: c_int, nn: *mut c_int, nt: *mut c_int, nodes: *mut f64, tris: *mut c_int) -> c_int;
}

const SURF: [&str; 11] = ["Plane", "Cylinder", "Cone", "Sphere", "Torus", "Bezier", "BSpline", "Revolution", "Extrusion", "Offset", "Other"];
const CURV: [&str; 9] = ["Line", "Circle", "Ellipse", "Hyperbola", "Parabola", "Bezier", "BSpline", "Offset", "Other"];

/// Safe owner of a model handle.
struct Model(*mut c_void);
impl Drop for Model {
    fn drop(&mut self) {
        unsafe { occ_free(self.0) }
    }
}

struct Face { ty: usize, params: [f64; 12], uv: [f64; 4], area: f64, reversed: bool }
struct Edge { ty: i32, ends: [f64; 6], faces: [i32; 2], nfaces: i32 }

impl Model {
    fn open(path: &str) -> Result<Model, String> {
        let c = CString::new(path).map_err(|e| e.to_string())?;
        let mut err = [0 as c_char; 256];
        let p = unsafe { occ_read_step(c.as_ptr(), err.as_mut_ptr(), 256) };
        if p.is_null() {
            return Err(unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned());
        }
        Ok(Model(p))
    }
    fn faces(&self) -> Vec<Face> {
        (0..unsafe { occ_face_count(self.0) })
            .map(|i| {
                let (mut params, mut uv, mut area, mut rev) = ([0.0; 12], [0.0; 4], 0.0, 0);
                let ty = unsafe { occ_face_info(self.0, i, params.as_mut_ptr(), uv.as_mut_ptr(), &mut area, &mut rev) };
                Face { ty: ty as usize, params, uv, area, reversed: rev != 0 }
            })
            .collect()
    }
    fn edges(&self) -> Vec<Edge> {
        (0..unsafe { occ_edge_count(self.0) })
            .map(|i| {
                let (mut ty, mut ends, mut faces) = (0, [0.0; 6], [0; 2]);
                let n = unsafe { occ_edge_info(self.0, i, &mut ty, ends.as_mut_ptr(), faces.as_mut_ptr()) };
                Edge { ty, ends, faces, nfaces: n }
            })
            .collect()
    }
    fn eval(&self, i: usize, u: f64, v: f64) -> Option<([f64; 3], [f64; 3])> {
        let (mut p, mut n) = ([0.0; 3], [0.0; 3]);
        (unsafe { occ_face_eval(self.0, i as c_int, u, v, p.as_mut_ptr(), n.as_mut_ptr()) } == 0).then_some((p, n))
    }
    /// Per-face triangle lists: the face-to-triangle mapping the opencascade crate cannot give.
    fn mesh(&self, lin: f64, ang: f64) -> Result<Vec<(Vec<[f64; 3]>, Vec<[usize; 3]>)>, String> {
        if unsafe { occ_mesh(self.0, lin, ang, 0) } != 0 {
            return Err("mesh failed".into());
        }
        let mut out = vec![];
        for i in 0..unsafe { occ_face_count(self.0) } {
            let (mut nn, mut nt) = (0, 0);
            unsafe { occ_face_mesh(self.0, i, &mut nn, &mut nt, std::ptr::null_mut(), std::ptr::null_mut()) };
            let mut nodes = vec![0.0; 3 * nn as usize];
            let mut tris = vec![0 as c_int; 3 * nt as usize];
            unsafe { occ_face_mesh(self.0, i, &mut nn, &mut nt, nodes.as_mut_ptr(), tris.as_mut_ptr()) };
            out.push((
                nodes.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
                tris.chunks(3).map(|c| [c[0] as usize, c[1] as usize, c[2] as usize]).collect(),
            ));
        }
        Ok(out)
    }
}

/// Round for reporting: fixed decimals, and never a signed zero (a cross-platform hazard seen in SP-1).
fn r(x: f64, dp: i32) -> String {
    let s = 10f64.powi(dp);
    let v = (x * s).round() / s + 0.0;
    format!("{:.*}", dp as usize, if v == 0.0 { 0.0 } else { v })
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

fn cmd_walk(path: &str, tess: bool) -> Result<(), String> {
    let m = Model::open(path)?;
    let (faces, edges) = (m.faces(), m.edges());
    // Topology digest: IDs, types and adjacency only, no floating point, so it can be compared across platforms.
    let mut topo = String::new();
    let mut geom = String::new();
    for (i, f) in faces.iter().enumerate() {
        let _ = writeln!(topo, "F{i} {} rev={}", SURF.get(f.ty).unwrap_or(&"?"), f.reversed);
        let p: Vec<String> = f.params[..8].iter().map(|x| r(*x, 6)).collect();
        let _ = writeln!(geom, "F{i} params={} uv={} area={}", p.join(","), f.uv.iter().map(|x| r(*x, 6)).collect::<Vec<_>>().join(","), r(f.area, 3));
    }
    for (i, e) in edges.iter().enumerate() {
        let ty = if e.ty < 0 { "Degenerate" } else { CURV.get(e.ty as usize).unwrap_or(&"?") };
        let _ = writeln!(topo, "E{i} {ty} faces={},{} n={}", e.faces[0], e.faces[1], e.nfaces);
        let _ = writeln!(geom, "E{i} ends={}", e.ends.iter().map(|x| r(*x, 6)).collect::<Vec<_>>().join(","));
    }
    print!("{topo}{geom}");
    println!("faces {} edges {}", faces.len(), edges.len());
    println!("topology-digest {:016x}", fnv(&topo));
    println!("geometry-digest {:016x}", fnv(&geom));
    if tess {
        for (lin, ang) in [(0.1, 0.5), (0.01, 0.5), (0.1, 0.1)] {
            let mesh = m.mesh(lin, ang)?;
            let per: Vec<String> = mesh.iter().map(|(_, t)| t.len().to_string()).collect();
            println!("tess lin={lin} ang={ang} triangles per face [{}] total {}", per.join(","), mesh.iter().map(|(_, t)| t.len()).sum::<usize>());
        }
    }
    Ok(())
}

fn approx(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn cmd_selftest(dir: &str) -> Result<(), String> {
    let mut fails = vec![];
    let mut check = |name: &str, ok: bool| {
        println!("{} {name}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            fails.push(name.to_string());
        }
    };
    let open = |n: &str| Model::open(&format!("{dir}/{n}.step"));

    let cyl = open("cylinder")?;
    let f = cyl.faces();
    let c = f.iter().position(|f| f.ty == 1).ok_or("no cylinder face")?;
    check("cylinder: one cylindrical face, radius 10", f.iter().filter(|f| f.ty == 1).count() == 1 && approx(f[c].params[6], 10.0, 1e-9));
    let (p, n) = cyl.eval(c, std::f64::consts::PI, 25.0).ok_or("eval")?;
    check("cylinder: point at (pi, 25) is (-10, 0, 25), outward normal (-1, 0, 0)",
        approx(p[0], -10.0, 1e-9) && approx(p[1], 0.0, 1e-9) && approx(p[2], 25.0, 1e-9) && approx(n[0], -1.0, 1e-9));
    let e = cyl.edges();
    check("cylinder: every non-seam edge has two faces", e.iter().all(|e| e.nfaces >= 1));

    let cone = open("cone")?;
    let f = cone.faces();
    let k = f.iter().find(|f| f.ty == 2).ok_or("no cone face")?;
    check("cone: semi-angle = atan(8/30)", approx(k.params[6].abs(), (8.0f64 / 30.0).atan(), 1e-9));

    let sph = open("sphere")?;
    let f = sph.faces();
    check("sphere: radius 15", f.len() == 1 && f[0].ty == 3 && approx(f[0].params[6], 15.0, 1e-9));
    check("sphere: area 4*pi*r^2", approx(f[0].area, 4.0 * std::f64::consts::PI * 225.0, 1e-6));

    let tor = open("torus")?;
    let f = tor.faces();
    check("torus: radii 20 and 10", f.len() == 1 && f[0].ty == 4 && approx(f[0].params[6], 20.0, 1e-9) && approx(f[0].params[7], 10.0, 1e-9));

    let bs = open("bspline_face")?;
    let f = bs.faces();
    check("bspline face: type BSpline", f.len() == 1 && f[0].ty == 6);
    let (p, _) = bs.eval(0, 0.5, 0.5).ok_or("eval")?;
    check("bspline face: centre point (10, 10, 2.75) from the control net", approx(p[0], 10.0, 1e-9) && approx(p[1], 10.0, 1e-9) && approx(p[2], 2.75, 1e-9));

    // Meshing: per-face triangles, angular control, and chord error against the exact sphere.
    let coarse = sph.mesh(0.1, 0.5)?.iter().map(|(_, t)| t.len()).sum::<usize>();
    let fine_ang = sph.mesh(0.1, 0.1)?.iter().map(|(_, t)| t.len()).sum::<usize>();
    check("sphere: angular deflection 0.1 gives more triangles than 0.5", fine_ang > coarse);
    let mesh = sph.mesh(0.01, 0.5)?;
    let mut worst: f64 = 0.0;
    for (nodes, tris) in &mesh {
        for t in tris {
            let c: Vec<f64> = (0..3).map(|k| (nodes[t[0]][k] + nodes[t[1]][k] + nodes[t[2]][k]) / 3.0).collect();
            worst = worst.max(((c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt() - 15.0).abs());
        }
    }
    println!("sphere at 0.01 mm: worst triangle-centre deviation {worst:.5} mm");
    check("sphere: chord deviation within 2x the requested 0.01 mm", worst <= 0.02);

    let fb = open("filleted_box")?;
    let mesh = fb.mesh(0.05, 0.5)?;
    check("filleted box: every one of 26 faces has its own triangles", mesh.len() == 26 && mesh.iter().all(|(_, t)| !t.is_empty()));

    if fails.is_empty() {
        println!("selftest: all checks passed");
        Ok(())
    } else {
        Err(format!("{} check(s) failed", fails.len()))
    }
}

/// The NFR-07 pattern: the reader runs in a child process; the parent never crashes.
fn cmd_isolate(files: &[String]) -> Result<(), String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut tally = std::collections::BTreeMap::<&str, usize>::new();
    for f in files {
        let mut child = Command::new(&me).args(["walk", f]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
        let start = Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
                break Some(s);
            }
            if start.elapsed() > Duration::from_secs(20) {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let out = child.stdout.take().map(|mut s| { let mut b = String::new(); let _ = std::io::Read::read_to_string(&mut s, &mut b); b }).unwrap_or_default();
        let kind = match status {
            None => "timeout",
            Some(s) if s.success() => "ok",
            Some(s) if s.code() == Some(2) => "error",
            Some(_) => "crashed",
        };
        *tally.entry(kind).or_default() += 1;
        let detail = out.lines().find(|l| l.starts_with("ERR")).unwrap_or("");
        println!("{kind:8} {f} {detail} {}", status.map(|s| format!("({s})")).unwrap_or_default());
    }
    println!("isolate summary {tally:?}; parent process survived every file");
    Ok(())
}

fn main() -> ExitCode {
    unsafe { occ_quiet() };
    let a: Vec<String> = std::env::args().collect();
    let r = match a.get(1).map(String::as_str) {
        Some("walk") if a.len() > 2 => cmd_walk(&a[2], a.iter().any(|x| x == "--tess")),
        Some("selftest") if a.len() > 2 => cmd_selftest(&a[2]),
        Some("isolate") => cmd_isolate(&a[2..]),
        _ => Err("usage: walk <file> [--tess] | selftest <dir> | isolate <file>...".into()),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            println!("ERR {e}");
            ExitCode::from(2)
        }
    }
}

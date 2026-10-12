// SPDX-License-Identifier: AGPL-3.0-only
// Safe-ish wrappers over the C ABI in cpp/shim.cpp.
use crate::vec3::V3;
use std::ffi::{c_char, c_int, c_void, CString};

extern "C" {
    fn occ_quiet();
    fn occ_read_step(path: *const c_char, err: *mut c_char, err_len: c_int) -> *mut c_void;
    fn occ_free(m: *mut c_void);
    fn occ_face_count(m: *mut c_void) -> c_int;
    fn occ_face_info(m: *mut c_void, i: c_int, out: *mut f64, uv: *mut f64, area: *mut f64, rev: *mut c_int) -> c_int;
    fn occ_face_eval(m: *mut c_void, i: c_int, u: f64, v: f64, pt: *mut f64, n: *mut f64) -> c_int;
    fn occ_mesh(m: *mut c_void, lin: f64, ang: f64, parallel: c_int) -> c_int;
    fn occ_face_mesh(m: *mut c_void, i: c_int, nn: *mut c_int, nt: *mut c_int, nodes: *mut f64, tris: *mut c_int) -> c_int;
    fn occ_face_frame(m: *mut c_void, i: c_int, out: *mut f64) -> c_int;
    fn occ_face_bspline(m: *mut c_void, i: c_int, meta: *mut c_int, uk: *mut f64, vk: *mut f64, poles: *mut f64, w: *mut f64) -> c_int;
    fn occ_face_project(m: *mut c_void, i: c_int, x: f64, y: f64, z: f64, out: *mut f64) -> c_int;
    fn occ_face_distance(m: *mut c_void, i: c_int, x: f64, y: f64, z: f64, out: *mut f64) -> c_int;
    fn occ_face_ray(m: *mut c_void, i: c_int, o: *const f64, d: *const f64, tmax: f64, hits: *mut f64) -> c_int;
    fn occ_face_contains(m: *mut c_void, i: c_int, u: f64, v: f64) -> c_int;
    fn occ_face_trims(m: *mut c_void, i: c_int, per_edge: c_int, uv: *mut f64, cap: c_int, starts: *mut c_int, nloops: *mut c_int) -> c_int;
}

pub struct Model(*mut c_void);
impl Drop for Model { fn drop(&mut self) { unsafe { occ_free(self.0) } } }

pub fn quiet() { unsafe { occ_quiet() } }

pub struct FaceInfo { pub ty: i32, pub uv: [f64; 4], pub area: f64, pub reversed: bool }
pub struct Mesh { pub tris: Vec<[V3; 3]> }
pub struct BSplineData { pub pu: usize, pub pv: usize, pub uk: Vec<f64>, pub vk: Vec<f64>, pub nu: usize, pub nv: usize,
    pub poles: Vec<V3>, pub w: Vec<f64>, pub rational: bool }

impl Model {
    pub fn read(path: &str) -> Result<Model, String> {
        let c = CString::new(path).unwrap();
        let mut err = vec![0 as c_char; 256];
        let p = unsafe { occ_read_step(c.as_ptr(), err.as_mut_ptr(), 256) };
        if p.is_null() {
            let s: String = err.iter().take_while(|c| **c != 0).map(|c| *c as u8 as char).collect();
            return Err(s);
        }
        Ok(Model(p))
    }
    pub fn faces(&self) -> usize { unsafe { occ_face_count(self.0) as usize } }
    pub fn info(&self, i: usize) -> FaceInfo {
        let (mut out, mut uv, mut area, mut rev) = ([0.0; 12], [0.0; 4], 0.0, 0);
        let ty = unsafe { occ_face_info(self.0, i as c_int, out.as_mut_ptr(), uv.as_mut_ptr(), &mut area, &mut rev) };
        FaceInfo { ty, uv, area, reversed: rev != 0 }
    }
    /// Frame: origin, axis, xdir, a, b.
    pub fn frame(&self, i: usize) -> ([f64; 11], i32) {
        let mut out = [0.0; 11];
        let ty = unsafe { occ_face_frame(self.0, i as c_int, out.as_mut_ptr()) };
        (out, ty)
    }
    pub fn eval(&self, i: usize, u: f64, v: f64) -> Option<(V3, V3)> {
        let (mut p, mut n) = ([0.0; 3], [0.0; 3]);
        let rc = unsafe { occ_face_eval(self.0, i as c_int, u, v, p.as_mut_ptr(), n.as_mut_ptr()) };
        if rc == 0 { Some((p, n)) } else { None }
    }
    pub fn mesh(&self, lin: f64, ang: f64) -> bool { unsafe { occ_mesh(self.0, lin, ang, 0) == 0 } }
    pub fn face_mesh(&self, i: usize) -> Mesh {
        let (mut nn, mut nt) = (0, 0);
        unsafe { occ_face_mesh(self.0, i as c_int, &mut nn, &mut nt, std::ptr::null_mut(), std::ptr::null_mut()) };
        let mut nodes = vec![0.0; 3 * nn as usize]; let mut tris = vec![0 as c_int; 3 * nt as usize];
        unsafe { occ_face_mesh(self.0, i as c_int, &mut nn, &mut nt, nodes.as_mut_ptr(), tris.as_mut_ptr()) };
        let node = |k: c_int| -> V3 { let k = k as usize; [nodes[3 * k], nodes[3 * k + 1], nodes[3 * k + 2]] };
        Mesh { tris: tris.chunks(3).map(|t| [node(t[0]), node(t[1]), node(t[2])]).collect() }
    }
    pub fn bspline(&self, i: usize) -> Option<BSplineData> {
        let mut meta = [0 as c_int; 9];
        let null = std::ptr::null_mut();
        if unsafe { occ_face_bspline(self.0, i as c_int, meta.as_mut_ptr(), null, null, null, null) } != 0 { return None; }
        let (nuk, nvk, nu, nv) = (meta[5] as usize, meta[6] as usize, meta[2] as usize, meta[3] as usize);
        let (mut uk, mut vk, mut poles, mut w) = (vec![0.0; nuk], vec![0.0; nvk], vec![0.0; nu * nv * 3], vec![0.0; nu * nv]);
        if unsafe { occ_face_bspline(self.0, i as c_int, meta.as_mut_ptr(), uk.as_mut_ptr(), vk.as_mut_ptr(), poles.as_mut_ptr(), w.as_mut_ptr()) } != 0 { return None; }
        Some(BSplineData { pu: meta[0] as usize, pv: meta[1] as usize, uk, vk, nu, nv,
            poles: poles.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(), w, rational: meta[4] != 0 })
    }
    /// (u, v, point, distance) on the untrimmed surface.
    pub fn project(&self, i: usize, q: V3) -> Option<(f64, f64, V3, f64)> {
        let mut o = [0.0; 6];
        if unsafe { occ_face_project(self.0, i as c_int, q[0], q[1], q[2], o.as_mut_ptr()) } != 0 { return None; }
        Some((o[0], o[1], [o[2], o[3], o[4]], o[5]))
    }
    /// (point, distance) to the trimmed face.
    pub fn distance(&self, i: usize, q: V3) -> Option<(V3, f64)> {
        let mut o = [0.0; 4];
        if unsafe { occ_face_distance(self.0, i as c_int, q[0], q[1], q[2], o.as_mut_ptr()) } != 0 { return None; }
        Some(([o[0], o[1], o[2]], o[3]))
    }
    pub fn ray(&self, i: usize, o: V3, d: V3, tmax: f64) -> Vec<(f64, f64, f64)> {
        let mut h = [0.0; 24];
        let n = unsafe { occ_face_ray(self.0, i as c_int, o.as_ptr(), d.as_ptr(), tmax, h.as_mut_ptr()) };
        (0..n.max(0) as usize).map(|k| (h[3 * k], h[3 * k + 1], h[3 * k + 2])).collect()
    }
    pub fn contains(&self, i: usize, u: f64, v: f64) -> bool { unsafe { occ_face_contains(self.0, i as c_int, u, v) == 1 } }
    /// Trim loops in (u, v), edges sampled `per_edge` times.
    pub fn trims(&self, i: usize, per_edge: usize) -> Vec<Vec<[f64; 2]>> {
        let cap = 20000;
        let mut uv = vec![0.0; 2 * cap]; let mut starts = [0 as c_int; 64]; let mut nl = 0;
        let n = unsafe { occ_face_trims(self.0, i as c_int, per_edge as c_int, uv.as_mut_ptr(), cap as c_int, starts.as_mut_ptr(), &mut nl) };
        if n < 0 { return Vec::new(); }
        let mut loops = Vec::new();
        for l in 0..nl as usize {
            let a = starts[l] as usize; let b = if l + 1 < nl as usize { starts[l + 1] as usize } else { n as usize };
            loops.push((a..b).map(|k| [uv[2 * k], uv[2 * k + 1]]).collect());
        }
        loops
    }
}

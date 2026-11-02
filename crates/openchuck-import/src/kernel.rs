// SPDX-License-Identifier: AGPL-3.0-or-later
//! The OpenCASCADE wrapper (work breakdown slice 11 item 1; decision record 0003).
//!
//! Built only with the `kernel` feature. This is the one place in the
//! workspace that calls foreign code, so it is the one place `unsafe` is
//! allowed (a recorded exception to the workspace's `unsafe_code = "deny"`,
//! decision record 0003 consequences; open item OI-19 adds Miri and sanitizer
//! checks for it). The C++ side (`cpp/occ_step.cpp`) is a plain C ABI that
//! catches every C++ exception and returns a JSON document or an error text.
//! No kernel pointer, handle or type crosses the boundary.
//!
//! **Do not call this from the application process.** The reader segfaults on
//! damaged files, which no exception handler can catch; use
//! [`StepImporter`](crate::step::StepImporter), which runs it in the
//! `openchuck-step-worker` child process.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_void};

unsafe extern "C" {
    fn occ_quiet();
    fn occ_import_step(
        path: *const c_char,
        linear: f64,
        angular: f64,
        override_mm: f64,
    ) -> *mut c_void;
    fn occ_result_error(result: *const c_void) -> *const c_char;
    fn occ_result_json(result: *const c_void, len: *mut usize) -> *const c_char;
    fn occ_result_free(result: *mut c_void);
    fn occ_oracle_open(path: *const c_char) -> *mut c_void;
    fn occ_oracle_free(oracle: *mut c_void);
    fn occ_oracle_eval(oracle: *mut c_void, face: i32, u: f64, v: f64, out: *mut f64) -> i32;
    fn occ_oracle_project(
        oracle: *mut c_void,
        face: i32,
        x: f64,
        y: f64,
        z: f64,
        out: *mut f64,
    ) -> i32;
    fn occ_oracle_ray(
        oracle: *mut c_void,
        face: i32,
        origin: *const f64,
        direction: *const f64,
        t_max: f64,
        hits: *mut f64,
    ) -> i32;
    fn occ_oracle_contains(oracle: *mut c_void, face: i32, u: f64, v: f64) -> i32;
}

/// Owns a result from the C++ side and frees it.
struct KernelResult(*mut c_void);

impl Drop for KernelResult {
    fn drop(&mut self) {
        // SAFETY: the pointer came from `occ_import_step`, is freed only here, and
        // is not used afterwards.
        unsafe { occ_result_free(self.0) }
    }
}

/// Reads the STEP file at `path` and returns the worker's JSON answer
/// (`file_unit`, `file_unit_mm`, `scale`, `model`), tessellated at `linear`
/// millimetres and `angular` radians. `override_mm` is 0 to use the file's
/// own unit, or the size in millimetres of the unit to read the file's
/// numbers in.
///
/// # Errors
///
/// The kernel's message when the file cannot be read, holds no faces, or a
/// face cannot be converted.
pub fn read_step_json(
    path: &str,
    linear: f64,
    angular: f64,
    override_mm: f64,
) -> Result<Vec<u8>, String> {
    let path = CString::new(path).map_err(|_| "the path contains a NUL byte".to_owned())?;
    // SAFETY: both calls take plain values and no pointer into Rust memory is kept.
    let result = unsafe {
        occ_quiet();
        occ_import_step(path.as_ptr(), linear, angular, override_mm)
    };
    if result.is_null() {
        return Err("the STEP kernel could not allocate a result".to_owned());
    }
    let result = KernelResult(result);
    // SAFETY: `result.0` is a live result; the returned error text, when not null, is
    // NUL-terminated and valid until the result is freed, and is copied before then.
    unsafe {
        let error = occ_result_error(result.0);
        if !error.is_null() {
            return Err(CStr::from_ptr(error).to_string_lossy().into_owned());
        }
        let mut len = 0_usize;
        let json = occ_result_json(result.0, &raw mut len);
        if json.is_null() {
            return Err("the STEP kernel returned no data".to_owned());
        }
        // SAFETY: `json` points at `len` initialised bytes owned by the result.
        Ok(std::slice::from_raw_parts(json.cast::<u8>(), len).to_vec())
    }
}

/// A point and the unit outward normal there.
pub type PointNormal = ([f64; 3], [f64; 3]);

/// A ray hit: distance along the ray, then the face's u and v.
pub type OracleHit = (f64, f64, f64);

/// The kernel as a test oracle: it numbers faces like the import does but
/// answers evaluation, projection, ray and trim questions itself, so tests
/// can compare the library's pure-Rust evaluators with OpenCASCADE (decision
/// 0017: the SP-9 comparison is kept as a regression test). Verification
/// only; the import never uses it. Like [`read_step_json`] it must not be
/// given damaged files in a process that matters.
#[derive(Debug)]
pub struct Oracle(*mut c_void);

impl Oracle {
    /// Opens the STEP file at `path` (in millimetres, as the import does).
    pub fn open(path: &str) -> Option<Self> {
        let path = CString::new(path).ok()?;
        // SAFETY: `path` is a valid NUL-terminated string for the duration of the call.
        let handle = unsafe {
            occ_quiet();
            occ_oracle_open(path.as_ptr())
        };
        (!handle.is_null()).then_some(Self(handle))
    }

    /// The point and outward unit normal of `face` at (u, v); `None` where
    /// the normal is undefined.
    pub fn eval(&self, face: u32, u: f64, v: f64) -> Option<PointNormal> {
        let mut out = [0.0_f64; 6];
        // SAFETY: `out` has the six slots the call writes; the handle is live.
        let status =
            unsafe { occ_oracle_eval(self.0, i32::try_from(face).ok()?, u, v, out.as_mut_ptr()) };
        (status == 0).then_some(([out[0], out[1], out[2]], [out[3], out[4], out[5]]))
    }

    /// The closest point of `face`'s untrimmed surface to `point`: its u, v,
    /// position and distance.
    pub fn project(&self, face: u32, point: [f64; 3]) -> Option<(f64, f64, [f64; 3], f64)> {
        let mut out = [0.0_f64; 6];
        // SAFETY: `out` has the six slots the call writes; the handle is live.
        let status = unsafe {
            occ_oracle_project(
                self.0,
                i32::try_from(face).ok()?,
                point[0],
                point[1],
                point[2],
                out.as_mut_ptr(),
            )
        };
        (status == 0).then_some((out[0], out[1], [out[2], out[3], out[4]], out[5]))
    }

    /// Hits of the ray with the trimmed `face`, nearest first (at most 8).
    pub fn ray(
        &self,
        face: u32,
        origin: [f64; 3],
        direction: [f64; 3],
        t_max: f64,
    ) -> Vec<OracleHit> {
        let mut hits = [0.0_f64; 24];
        let Ok(face) = i32::try_from(face) else {
            return Vec::new();
        };
        // SAFETY: `origin` and `direction` hold three numbers, `hits` the 24 the call
        // may write; the handle is live.
        let n = unsafe {
            occ_oracle_ray(
                self.0,
                face,
                origin.as_ptr(),
                direction.as_ptr(),
                t_max,
                hits.as_mut_ptr(),
            )
        };
        let n = usize::try_from(n).unwrap_or(0).min(8);
        hits.chunks_exact(3)
            .take(n)
            .filter_map(|h| match *h {
                [t, u, v] => Some((t, u, v)),
                _ => None,
            })
            .collect()
    }

    /// Whether (u, v) lies on the trimmed `face`, boundary included.
    pub fn contains(&self, face: u32, u: f64, v: f64) -> Option<bool> {
        // SAFETY: the handle is live; the call takes plain values.
        match unsafe { occ_oracle_contains(self.0, i32::try_from(face).ok()?, u, v) } {
            1 => Some(true),
            0 => Some(false),
            _ => None,
        }
    }
}

impl Drop for Oracle {
    fn drop(&mut self) {
        // SAFETY: the handle came from `occ_oracle_open` and is freed once, here.
        unsafe { occ_oracle_free(self.0) }
    }
}

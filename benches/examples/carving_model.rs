// SPDX-License-Identifier: AGPL-3.0-or-later
//! Writes the carving test piece of issue #382
//! (`openchuck_testkit::parts::CarvedRelief::STANDARD`) as a binary STL, with
//! vertices `SPACING` mm apart along X and round the axis (default 0.1), for
//! timing fine-stepover finishing with a V-bit through a project
//! (`openchuck-app`'s `operation_times` example).
//!
//! `cargo run --release -p openchuck-benches --example carving_model -- OUT.stl [SPACING]`

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::indexing_slicing)]

use std::io::Write;

use openchuck_testkit::parts::{CarvedRelief, RadialPart};

fn main() -> std::process::ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(out) = args.next() else {
        eprintln!("usage: carving_model OUT.stl [SPACING]");
        return std::process::ExitCode::FAILURE;
    };
    let spacing: f64 = match args.next().map(|s| s.to_string_lossy().parse()) {
        None => 0.1,
        Some(Ok(s)) if s > 0.0 => s,
        Some(_) => {
            eprintln!("SPACING must be a number above zero");
            return std::process::ExitCode::FAILURE;
        }
    };
    let part = CarvedRelief::STANDARD;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (rings, around) = (
        (part.length / spacing).round() as u32 + 1,
        (std::f64::consts::TAU * part.radius / spacing).round() as u32,
    );
    let mesh = part.mesh(rings, around);
    let mut bytes = Vec::with_capacity(84 + 50 * mesh.triangles.len());
    let mut header = [0u8; 80];
    let name = b"OpenChuckCAM carving test piece (issue 382), millimetres";
    header[..name.len()].copy_from_slice(name);
    bytes.extend_from_slice(&header);
    let count = u32::try_from(mesh.triangles.len()).unwrap_or(u32::MAX);
    bytes.extend_from_slice(&count.to_le_bytes());
    #[allow(clippy::cast_possible_truncation)]
    for t in &mesh.triangles {
        let p = t.map(|i| mesh.vertices[i as usize]);
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        let l = n.length().max(f64::MIN_POSITIVE);
        for v in [n.x / l, n.y / l, n.z / l] {
            bytes.extend_from_slice(&(v as f32).to_le_bytes());
        }
        for q in p {
            for v in [q.x, q.y, q.z] {
                bytes.extend_from_slice(&(v as f32).to_le_bytes());
            }
        }
        bytes.extend_from_slice(&[0, 0]);
    }
    match std::fs::File::create(&out).and_then(|mut f| f.write_all(&bytes)) {
        Ok(()) => {
            println!("{count} triangles written to {}", out.to_string_lossy());
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

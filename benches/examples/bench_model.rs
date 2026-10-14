// SPDX-License-Identifier: AGPL-3.0-or-later
//! Writes the NFR-03 benchmark model (`openchuck_testkit::parts::benchmark_model_1m`,
//! a cylinder of about 1,000,000 triangles) as a binary STL, for the
//! frame-time benchmark `openchuck-ui --bench` (verification plan 4.2; work
//! breakdown slice 8 item 7; decision record 0029 item 9).
//!
//! `cargo run --release -p openchuck-benches --example bench_model -- OUT.stl`

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::indexing_slicing)]

use std::io::Write;

fn main() -> std::process::ExitCode {
    let Some(out) = std::env::args_os().nth(1) else {
        eprintln!("usage: bench_model OUT.stl");
        return std::process::ExitCode::FAILURE;
    };
    let mesh = openchuck_testkit::parts::benchmark_model_1m();
    let mut bytes = Vec::with_capacity(84 + 50 * mesh.triangles.len());
    let mut header = [0u8; 80];
    let name = b"OpenChuckCAM NFR-03 benchmark model, millimetres";
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
            println!("{} triangles written to {}", count, out.to_string_lossy());
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

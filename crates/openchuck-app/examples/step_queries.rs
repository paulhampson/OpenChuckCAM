// SPDX-License-Identifier: AGPL-3.0-or-later
//! The exact face and trim queries, measured for correctness and cost (lane
//! STEPALL-X1, issue #348; decision 0211 item 3; OI-839, OI-81). It changes
//! nothing in the product. Run it with the STEP worker built:
//!
//! ```text
//! cargo build --release -p openchuck-import --features kernel --bins
//! OPENCHUCK_STEP_WORKER=target/release/openchuck-step-worker \
//!   cargo run --release -p openchuck-app --features kernel-tests --example step_queries -- \
//!   --out DIR [--parts blisk,cone,...] [--stride N] [--offset MM]
//! ```
//!
//! It uses only `ExactFace::closest` and `BrepSurface::distance`, which exist
//! before and after the lane's change, so the same source measures both. For
//! each part in `testdata/step/valid/` (imported at the default options,
//! unplaced) it asks three sets of questions, all on one thread:
//!
//! - `on`: every `--stride`-th tessellation vertex of every face, asked of its
//!   own trimmed face. The kernel puts the vertices on the face, so a
//!   distance over [`FLAG`] is a wrong answer (the OI-839 kind).
//! - `off`: the same vertices moved `--offset` mm (1, a 2 mm ball's radius)
//!   along the vertex normal of the tessellation, asked of their own face.
//! - `model`: the `off` points of every face, asked of the whole model with
//!   `BrepSurface::distance`, as a ball drop asks (decision 0211, M3 item 1).
//!
//! Every answer is written to `DIR/<part>.tsv` (set, face, kind, vertex
//! index, the point, the distance, nanoseconds), so two runs compare
//! answer by answer; a summary per part, set and face kind is printed. A run
//! given `--compare A B` instead reads two such directories and prints, per
//! part and set, how many answers differ by more than 1e-6 mm and how far,
//! counting separately the answers that got nearer and those that got
//! further (both answers are points of the face, so nearer is better).
#![allow(
    clippy::print_stdout,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::surface::ParametricFace;
use openchuck_path::{BrepSurface, SurfaceSource};
use openchuck_types::brep::BrepModel;
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::model::ModelGeometry;

/// A vertex answered further than this, mm, from its own face is wrong.
const FLAG: f64 = 1e-3;

const PARTS: &[&str] = &[
    "blisk",
    "box",
    "cylinder",
    "cone",
    "sphere",
    "torus",
    "filleted_box",
    "rotary_blank_drilled",
    "bspline_face",
    "revolved_spline",
    "revolved_spline_partial",
    "extruded_spline",
    "hex_bar",
    "shouldered_shaft",
];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/step/valid")
        .join(format!("{name}.step"))
}

fn import(name: &str) -> BrepModel {
    let bytes = std::fs::read(fixture(name)).unwrap();
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    let model = StepImporter::new()
        .import(
            &bytes,
            &ImportOptions::default(),
            &JobContext::new(&cancel, &progress),
        )
        .unwrap();
    match model.geometry {
        ModelGeometry::Brep(b) => b,
        ModelGeometry::Mesh(_) => panic!("{name} is not a B-rep"),
    }
}

/// Unit vertex normals of a face's tessellation, area weighted.
fn vertex_normals(vertices: &[Point3], triangles: &[[u32; 3]]) -> Vec<Vec3> {
    let mut n = vec![Vec3::ZERO; vertices.len()];
    for t in triangles {
        let [a, b, c] = t.map(|i| i as usize);
        let w = (vertices[b] - vertices[a]).cross(vertices[c] - vertices[a]);
        for i in [a, b, c] {
            n[i] = n[i] + w;
        }
    }
    n.into_iter()
        .map(|v| {
            let l = v.length();
            if l > 0.0 { v * (1.0 / l) } else { Vec3::ZERO }
        })
        .collect()
}

struct Row {
    set: &'static str,
    face: u32,
    kind: String,
    index: usize,
    point: Point3,
    distance: f64,
    nanos: u128,
}

/// Per set and face kind: answers, summed and largest nanoseconds, wrong
/// answers and the furthest of them.
type Tally = (usize, u128, u128, usize, f64);

fn measure(name: &str, stride: usize, offset: f64, out: &Path) {
    let started = Instant::now();
    let model = import(name);
    let import_s = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let surface = BrepSurface::new(&model).unwrap();
    let build_s = started.elapsed().as_secs_f64();
    let mut rows = Vec::new();
    let mut offs = Vec::new();
    for (face, exact) in model.faces.iter().zip(surface.faces()) {
        let kind = format!("{:?}", face.surface);
        let normals = vertex_normals(&face.tessellation.vertices, &face.tessellation.triangles);
        for (index, v) in face
            .tessellation
            .vertices
            .iter()
            .enumerate()
            .step_by(stride)
        {
            for (set, q) in [("on", *v), ("off", *v + normals[index] * offset)] {
                let t = Instant::now();
                let answer = exact.closest(q);
                let nanos = t.elapsed().as_nanos();
                let distance = answer.map_or(f64::NAN, |c| c.distance);
                rows.push(Row {
                    set,
                    face: face.id.0,
                    kind: kind.clone(),
                    index,
                    point: q,
                    distance,
                    nanos,
                });
            }
            offs.push((face.id.0, kind.clone(), index, *v + normals[index] * offset));
        }
    }
    for (face, kind, index, q) in offs {
        let t = Instant::now();
        let distance = surface.distance(q).unwrap_or(f64::NAN);
        let nanos = t.elapsed().as_nanos();
        rows.push(Row {
            set: "model",
            face,
            kind,
            index,
            point: q,
            distance,
            nanos,
        });
    }
    let mut tsv = String::from("set\tface\tkind\tindex\tx\ty\tz\tdistance\tnanos\n");
    for r in &rows {
        writeln!(
            tsv,
            "{}\t{}\t{}\t{}\t{:.12}\t{:.12}\t{:.12}\t{:.12}\t{}",
            r.set, r.face, r.kind, r.index, r.point.x, r.point.y, r.point.z, r.distance, r.nanos
        )
        .unwrap();
    }
    std::fs::write(out.join(format!("{name}.tsv")), tsv).unwrap();
    println!(
        "{name}: {} faces, {} triangles, import {import_s:.2} s, build {build_s:.3} s",
        model.faces.len(),
        model
            .faces
            .iter()
            .map(|f| f.tessellation.triangles.len())
            .sum::<usize>()
    );
    // Per set and kind: count, mean and largest time, and the wrong answers.
    let mut by: BTreeMap<(&str, &str), Tally> = BTreeMap::new();
    for r in &rows {
        let e = by.entry((r.set, r.kind.as_str())).or_default();
        e.0 += 1;
        e.1 += r.nanos;
        e.2 = e.2.max(r.nanos);
        if r.set == "on" && (r.distance > FLAG || r.distance.is_nan()) {
            e.3 += 1;
            e.4 = e.4.max(r.distance);
        }
    }
    let mut total: BTreeMap<&str, (usize, u128)> = BTreeMap::new();
    for ((set, kind), (n, sum, max, wrong, worst)) in &by {
        let t = total.entry(set).or_default();
        t.0 += n;
        t.1 += sum;
        println!(
            "  {set:<5} {kind:<9} n {n:>6}  mean {:>10.1} us  max {:>10.1} us{}",
            *sum as f64 / 1000.0 / *n as f64,
            *max as f64 / 1000.0,
            if *wrong > 0 {
                format!("  WRONG {wrong} (worst {worst:.4} mm)")
            } else {
                String::new()
            }
        );
    }
    for (set, (n, sum)) in total {
        println!(
            "  {set:<5} all       n {n:>6}  mean {:>10.1} us  total {:.3} s",
            sum as f64 / 1000.0 / n as f64,
            sum as f64 * 1e-9
        );
    }
}

fn read(path: &Path) -> BTreeMap<(String, String, String), f64> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut out = BTreeMap::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        out.insert(
            (f[0].to_owned(), f[1].to_owned(), f[3].to_owned()),
            f[7].parse::<f64>().unwrap(),
        );
    }
    out
}

fn compare(a: &Path, b: &Path) {
    let mut names: Vec<_> = std::fs::read_dir(a)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tsv"))
        .collect();
    names.sort();
    for name in names {
        let (x, y) = (read(&a.join(&name)), read(&b.join(&name)));
        let mut by: BTreeMap<String, (usize, usize, f64, usize, f64, usize)> = BTreeMap::new();
        for (key, da) in &x {
            let Some(db) = y.get(key) else { continue };
            let e = by.entry(key.0.clone()).or_default();
            e.0 += 1;
            let d = db - da;
            if d < -1e-6 || (da.is_nan() && !db.is_nan()) {
                e.1 += 1;
                e.2 = e.2.max(-d);
            } else if d > 1e-6 || (db.is_nan() && !da.is_nan()) {
                e.3 += 1;
                e.4 = e.4.max(d);
            }
            if d.abs() <= 1e-9 {
                e.5 += 1;
            }
        }
        for (set, (n, nearer, by_n, further, by_f, same)) in by {
            println!(
                "{name:<28} {set:<5} n {n:>6}  within 1e-9: {same:>6}  nearer {nearer:>5} (up to {by_n:.6} mm)  further {further:>5} (up to {by_f:.6} mm)"
            );
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut out = None;
    let mut parts: Vec<String> = PARTS.iter().map(|s| (*s).to_owned()).collect();
    let mut stride = 1;
    let mut offset = 1.0;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            "--parts" => {
                parts = args.next().unwrap().split(',').map(str::to_owned).collect();
            }
            "--stride" => stride = args.next().unwrap().parse().unwrap(),
            "--offset" => offset = args.next().unwrap().parse().unwrap(),
            "--compare" => {
                let (x, y) = (args.next().unwrap(), args.next().unwrap());
                compare(Path::new(&x), Path::new(&y));
                return;
            }
            other => panic!("unknown argument {other}"),
        }
    }
    let out = out.expect("--out DIR is needed");
    std::fs::create_dir_all(&out).unwrap();
    for p in &parts {
        measure(p, stride.max(1), offset, &out);
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pencil clean-up's creases on a STEP part, measured (lane STEPALL-P,
//! decision record 0226; decision 0209 item 2). Run on the owner's blisk:
//!
//! ```text
//! cargo build --release -p openchuck-import --features kernel --bins
//! OPENCHUCK_STEP_WORKER=target/release/openchuck-step-worker \
//!   cargo run --release -p openchuck-app --features kernel-tests --example pencil_creases -- \
//!   testdata/step/valid/blisk.step
//! ```
//!
//! The part is placed on the rotary axis from X 10, as the owner's projects
//! place `blisk.stl` and lane STEPALL-M's measurement places `blisk.step`.
//! For each crease angle it reports the creases found in the tessellation
//! (the mesh form) and on the B-rep's inside corners (`BrepEdges::concave`),
//! their length, and how much of each lies within [`NEAR`] of the other.
//! Then it runs pencil clean-up in both forms with the owner's tool (a 2 mm
//! flat end) on a 42 mm cylinder of stock with nothing cut, and judges every
//! fed pose and the quarter, middle and three-quarter points of every fed
//! move (on the machine's straight move in X, Y, Z and A) with the testkit's
//! independent checker (its tool outline only) against points on the exact
//! faces, inside their trims, `--sample` mm apart (0.05 by default). Nothing
//! measured uses the drop or the path fitter.
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
use std::time::Instant;

use openchuck_app::model::{Axis, Degrees, TransformStep};
use openchuck_geom::transform::{brep_placement, transform_brep};
use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::pencil::{Crease, concave_creases, pencil_parameter};
use openchuck_path::rest::parameter as rest_parameter;
use openchuck_path::surface::{ParametricFace, Uv};
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepEdges, BrepSurface, CurveSource, ExactFace, ParameterValue,
    PencilFinishing, RotaryMapping, StrategyInput, generate,
};
use openchuck_testkit::gouge::Outline;
use openchuck_types::brep::BrepModel;
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::ir::{PathTags, ToolpathGroup};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::Clearance;
use openchuck_types::model::ModelGeometry;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;
use rayon::prelude::*;

/// How close, mm, a crease of one form must be to one of the other to be
/// the same corner: well beyond the tessellation's 0.01 mm.
const NEAR: f64 = 0.05;

/// The grid cell, mm, the face samples are bucketed by.
const CELL: f64 = 1.0;

/// How far up the tool from its tip, mm, samples are judged: the flutes'
/// reach into a corner.
const UP: f64 = 4.0;

fn main() {
    let mut args = std::env::args().skip(1);
    let file = args
        .next()
        .expect("usage: pencil_creases <file.step> [--sample mm]");
    let mut sample = 0.05;
    while let Some(a) = args.next() {
        if a == "--sample" {
            sample = args
                .next()
                .and_then(|s| s.parse().ok())
                .expect("--sample mm");
        }
    }
    let model = placed(&std::fs::read(&file).expect("read the STEP file"));
    let surface = BrepSurface::new(&model).unwrap();
    let mesh = surface.faces().len();
    println!("part: {file}, {mesh} faces, {} edges", model.edges.len());

    // ---- the creases --------------------------------------------------------
    let t = Instant::now();
    let all = BrepEdges::new(&model).unwrap();
    println!(
        "edges found on the exact faces (as edge following does) in {:.2} s, {} kept",
        t.elapsed().as_secs_f64(),
        all.edge_ids().len()
    );
    let t = Instant::now();
    let concave = BrepEdges::concave(&model).unwrap();
    println!(
        "inside corners classified on the exact faces in {:.2} s",
        t.elapsed().as_secs_f64()
    );
    let (tessellation, _) = model.joined_tessellation();
    println!(
        "| crease angle | form | creases | length mm | within {NEAR} mm of the other form mm |"
    );
    println!("| --- | --- | --- | --- | --- |");
    for angle in [30.0, 20.0, 10.0, 5.0, 2.0] {
        let in_mesh = concave_creases(&tessellation, angle);
        let edges = concave.clone().with_crease(angle);
        let on_brep: Vec<Crease> = edges
            .chains(&surface)
            .unwrap()
            .into_iter()
            .map(|c| Crease {
                points: c.points,
                closed: c.closed,
            })
            .collect();
        let near_mesh = length_near(&in_mesh, &on_brep);
        let near_brep = length_near(&on_brep, &in_mesh);
        println!(
            "| {angle} | tessellation | {} | {:.1} | {near_mesh:.1} |",
            in_mesh.len(),
            length(&in_mesh)
        );
        println!(
            "| {angle} | B-rep | {} | {:.1} | {near_brep:.1} |",
            on_brep.len(),
            length(&on_brep)
        );
        if edges.not_inside_throughout() > 0 {
            println!(
                "  {} edges turn both ways or could not be told",
                edges.not_inside_throughout()
            );
        }
    }

    // ---- the cut -------------------------------------------------------------
    let t = Instant::now();
    let samples = Samples::of(&model, sample);
    println!(
        "{} samples on the exact faces, {sample} mm apart, in {:.1} s",
        samples.count,
        t.elapsed().as_secs_f64()
    );
    let machine = openchuck_app::setup::builtin("makera-z1").unwrap().unwrap();
    let tool = Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(2.0),
        flute_length: Millimetres(8.0),
        overall_length: Millimetres(38.0),
        shank_diameter: Millimetres(4.0),
        flute_count: 1,
        material: None,
        centre_cutting: Some(true),
    };
    let profile = tool.profile().unwrap();
    let outline = Outline::new(&profile);
    let b = surface_bounds(&model);
    let stock = AnalyticStock::Cylinder {
        radius: 21.0,
        x_start: b.min.x - 1.0,
        x_end: b.max.x + 1.0,
    };
    let rotary = RotaryMapping::of_profile(&machine);
    let tags = PathTags {
        operation: "pencil".into(),
        setup: "setup".into(),
        tool: "flat 2".into(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &profile,
        rotary,
        clearance: Clearance {
            radius: Millimetres(27.0),
            safe_z: Millimetres(37.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
        keep_out: &[],
    };
    let values: BTreeMap<String, ParameterValue> = [
        ("feed", 500.0),
        ("plunge_feed", 200.0),
        ("stepover", 1.0),
        (pencil_parameter::CREASE_ANGLE, 30.0),
        (pencil_parameter::MAX_STEP, 0.5),
        (rest_parameter::MIN_REMAINING, 0.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), ParameterValue::Number(v)))
    .collect();
    println!(
        "| form | passes | fed moves | stations | deepest into the exact faces mm | where | stations over 0.01 mm | time s |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for (form, strategy) in [
        ("tessellation", PencilFinishing::default()),
        ("B-rep", PencilFinishing::on_brep(&model).unwrap()),
    ] {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let t = Instant::now();
        let path = generate(&strategy, &input, &values, &ctx);
        let seconds = t.elapsed().as_secs_f64();
        match path {
            Ok(path) => {
                let stations = stations(&path.group, &rotary);
                let depths: Vec<(f64, Point3)> = stations
                    .par_iter()
                    .map(|(tip, axis)| samples.deepest(&outline, *tip, *axis))
                    .collect();
                let worst = depths
                    .iter()
                    .copied()
                    .fold((0.0, Point3::ORIGIN), |w, d| if d.0 > w.0 { d } else { w });
                let over = depths.iter().filter(|d| d.0 > 0.01).count();
                let fed = path
                    .group
                    .segments
                    .iter()
                    .filter(|s| s.kind.is_fed())
                    .count();
                let passes = path
                    .notes
                    .iter()
                    .find(|n| n.message.starts_with("pencil clean-up:"))
                    .map_or_else(String::new, |n| n.message.clone());
                println!(
                    "| {form} | {passes} | {fed} | {} | {:.4} | ({:.2}, {:.2}, {:.2}) | {over} | {seconds:.1} |",
                    stations.len(),
                    worst.0,
                    worst.1.x,
                    worst.1.y,
                    worst.1.z
                );
            }
            Err(e) => println!("| {form} | {e} | | | | | | {seconds:.1} |"),
        }
    }
}

/// The STEP file imported by the kernel, turned so its longest extent is
/// along X, centred on the rotary axis and moved to start at X 10.
fn placed(bytes: &[u8]) -> BrepModel {
    let cancel = CancelToken::new();
    let imported = StepImporter::new()
        .import(
            bytes,
            &ImportOptions::default(),
            &JobContext::new(&cancel, &NoProgress),
        )
        .unwrap();
    let ModelGeometry::Brep(model) = imported.geometry else {
        panic!("not a B-rep");
    };
    let b = surface_bounds(&model);
    let spans = [b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z];
    let mut steps = Vec::new();
    if spans[2] > spans[0] && spans[2] > spans[1] {
        steps.push(TransformStep::Rotate {
            axis: Axis::Y,
            angle: Degrees(90.0),
        });
    } else if spans[1] > spans[0] && spans[1] > spans[2] {
        steps.push(TransformStep::Rotate {
            axis: Axis::Z,
            angle: Degrees(-90.0),
        });
    }
    steps.push(TransformStep::CentreOnRotaryAxis);
    let turned = transform_brep(&model, &brep_placement(&model, &steps).unwrap()).unwrap();
    steps.push(TransformStep::Translate {
        x: 10.0 - surface_bounds(&turned).min.x,
        y: 0.0,
        z: 0.0,
    });
    transform_brep(&model, &brep_placement(&model, &steps).unwrap()).unwrap()
}

fn surface_bounds(model: &BrepModel) -> Aabb {
    let mut lo = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for v in model.faces.iter().flat_map(|f| &f.tessellation.vertices) {
        lo = Point3::new(lo.x.min(v.x), lo.y.min(v.y), lo.z.min(v.z));
        hi = Point3::new(hi.x.max(v.x), hi.y.max(v.y), hi.z.max(v.z));
    }
    Aabb { min: lo, max: hi }
}

fn length(creases: &[Crease]) -> f64 {
    creases
        .iter()
        .flat_map(|c| c.points.windows(2))
        .map(|w| w[0].distance(w[1]))
        .sum()
}

/// The length of `creases` whose segments' midpoints lie within [`NEAR`] of
/// a segment of `other`.
fn length_near(creases: &[Crease], other: &[Crease]) -> f64 {
    let segments: Vec<(Point3, Point3)> = other
        .iter()
        .flat_map(|c| c.points.windows(2).map(|w| (w[0], w[1])))
        .collect();
    creases
        .par_iter()
        .flat_map_iter(|c| c.points.windows(2).map(|w| (w[0], w[1])))
        .filter(|(a, b)| {
            let m = Point3::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0);
            segments.iter().any(|(p, q)| to_segment(m, *p, *q) <= NEAR)
        })
        .map(|(a, b)| a.distance(b))
        .sum()
}

fn to_segment(m: Point3, p: Point3, q: Point3) -> f64 {
    let d = q - p;
    let l = d.dot(d);
    let t = if l > 0.0 {
        ((m - p).dot(d) / l).clamp(0.0, 1.0)
    } else {
        0.0
    };
    m.distance(p + d * t)
}

/// Every fed pose of `group`, and the quarter, middle and three-quarter
/// points of every fed move on the machine's straight move in X, Y, Z and A,
/// as (tip, tool axis).
fn stations(
    group: &ToolpathGroup,
    rotary: &RotaryMapping,
) -> Vec<(Point3, openchuck_types::geometry::Vec3)> {
    let mut out = Vec::new();
    let mut previous = group.start;
    for s in &group.segments {
        if s.kind.is_fed() {
            let (p, q) = (
                rotary.to_axis(previous.tip, previous.a),
                rotary.to_axis(s.end.tip, s.end.a),
            );
            for f in [0.25, 0.5, 0.75] {
                let mix = |u: f64, v: f64| u + (v - u) * f;
                let a = openchuck_types::units::Degrees(mix(previous.a.0, s.end.a.0));
                let tip = rotary.to_model(
                    openchuck_path::rotary::AxisPosition {
                        along: mix(p.along, q.along),
                        across: mix(p.across, q.across),
                        height: mix(p.height, q.height),
                    },
                    a,
                );
                out.push((tip, rotary.tool_axis(a).get()));
            }
            out.push((s.end.tip, s.end.axis.get()));
        }
        previous = s.end;
    }
    out
}

/// Points on the exact faces, inside their trims and along their trim loops,
/// bucketed by a [`CELL`] grid.
struct Samples {
    cells: BTreeMap<[i64; 3], Vec<Point3>>,
    count: usize,
}

fn cell_of(p: Point3) -> [i64; 3] {
    [
        (p.x / CELL).floor() as i64,
        (p.y / CELL).floor() as i64,
        (p.z / CELL).floor() as i64,
    ]
}

impl Samples {
    fn of(model: &BrepModel, spacing: f64) -> Self {
        let per_face: Vec<Vec<Point3>> = model
            .faces
            .par_iter()
            .map(|face| {
                let exact = ExactFace::new(face).unwrap();
                let (lo, hi) = exact.uv_bounds();
                // The grid fine enough that neighbouring points are no more
                // than `spacing` apart: the surface's largest stretch per
                // unit of u and v, sampled on a coarse grid.
                let (mut su, mut sv) = (0.0_f64, 0.0_f64);
                let n = 24;
                for i in 0..=n {
                    for j in 0..=n {
                        let at = |di: f64, dj: f64| {
                            exact.point_normal(Uv {
                                u: lo.u + (hi.u - lo.u) * (i as f64 + di) / (n as f64 + 1.0),
                                v: lo.v + (hi.v - lo.v) * (j as f64 + dj) / (n as f64 + 1.0),
                            })
                        };
                        if let (Some(a), Some(b), Some(c)) =
                            (at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0))
                        {
                            su = su.max(a.point.distance(b.point) * (n as f64 + 1.0));
                            sv = sv.max(a.point.distance(c.point) * (n as f64 + 1.0));
                        }
                    }
                }
                let nu = ((su / spacing).ceil() as usize).clamp(2, 20_000);
                let nv = ((sv / spacing).ceil() as usize).clamp(2, 20_000);
                let mut points = Vec::new();
                for i in 0..=nu {
                    for j in 0..=nv {
                        let uv = Uv {
                            u: lo.u + (hi.u - lo.u) * i as f64 / nu as f64,
                            v: lo.v + (hi.v - lo.v) * j as f64 / nv as f64,
                        };
                        if exact.contains(uv)
                            && let Some(p) = exact.point_normal(uv)
                        {
                            points.push(p.point);
                        }
                    }
                }
                // Along the trim loops: the edges, where a corner is.
                for l in &face.trims {
                    for w in l.0.windows(2) {
                        let (a, b) = (w[0], w[1]);
                        for k in 0..8 {
                            let f = f64::from(k) / 8.0;
                            let uv = Uv {
                                u: a.u + (b.u - a.u) * f,
                                v: a.v + (b.v - a.v) * f,
                            };
                            if let Some(p) = exact.point_normal(uv) {
                                points.push(p.point);
                            }
                        }
                    }
                }
                points
            })
            .collect();
        let mut cells: BTreeMap<[i64; 3], Vec<Point3>> = BTreeMap::new();
        let mut count = 0;
        for p in per_face.into_iter().flatten() {
            cells.entry(cell_of(p)).or_default().push(p);
            count += 1;
        }
        Self { cells, count }
    }

    /// The deepest any sample lies inside the tool at this pose, and where.
    fn deepest(
        &self,
        outline: &Outline,
        tip: Point3,
        axis: openchuck_types::geometry::Vec3,
    ) -> (f64, Point3) {
        let reach = outline.radius() + CELL;
        let mut worst = (0.0, Point3::ORIGIN);
        let top = tip + axis * UP;
        let lo = Point3::new(
            tip.x.min(top.x) - reach,
            tip.y.min(top.y) - reach,
            tip.z.min(top.z) - reach,
        );
        let hi = Point3::new(
            tip.x.max(top.x) + reach,
            tip.y.max(top.y) + reach,
            tip.z.max(top.z) + reach,
        );
        let (a, b) = (cell_of(lo), cell_of(hi));
        for x in a[0]..=b[0] {
            for y in a[1]..=b[1] {
                for z in a[2]..=b[2] {
                    for p in self.cells.get(&[x, y, z]).into_iter().flatten() {
                        let off = *p - tip;
                        let h = off.dot(axis);
                        if !(-0.01..=UP).contains(&h) {
                            continue;
                        }
                        let r = (off - axis * h).length();
                        let d = outline.depth(r, h);
                        if d > worst.0 {
                            worst = (d, *p);
                        }
                    }
                }
            }
        }
        worst
    }
}

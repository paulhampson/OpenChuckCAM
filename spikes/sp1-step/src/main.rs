// SPDX-License-Identifier: AGPL-3.0-only
//! SP-1 throwaway spike: does the `opencascade` crate give us what IMP-02 / IMP-06 need?
//!
//! Subcommands:
//!   gen <dir>             write fixture STEP files built with the safe `opencascade` API
//!   walk <file> [--tess]  read a STEP file, walk the B-rep through `opencascade-sys`,
//!                         print a deterministic report and its digest
//!   safe <file>           what the safe `opencascade` API alone exposes for the same file
//!   roundtrip <file> <tmp>  read, write STEP, read again, compare topology digests
//!
//! Never merged into the product (work breakdown, spike rules).

use opencascade::primitives::Shape;
use opencascade_sys as ffi;
use std::fmt::Write as _;
use std::process::ExitCode;

type Shp = cxx::UniquePtr<ffi::topo_ds::TopoDS_Shape>;

fn load(path: &str) -> Result<Shp, String> {
    let mut reader = ffi::step_control::STEPControl_Reader_new();
    let status = ffi::step_control::read_step(reader.pin_mut(), path.to_string());
    if status != ffi::if_select::IFSelect_ReturnStatus::IFSelect_RetDone {
        return Err(format!("read_step status {:?}", status.repr));
    }
    let n = reader.pin_mut().TransferRoots(&ffi::message::Message_ProgressRange_new());
    let shape = ffi::step_control::one_shape_step(&reader);
    if shape.is_null() || shape.IsNull() {
        return Err(format!("no shape transferred (roots transferred: {n})"));
    }
    Ok(shape)
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn curve_name(t: &ffi::geom_abs::GeomAbs_CurveType) -> &'static str {
    use ffi::geom_abs::GeomAbs_CurveType as C;
    match *t {
        C::GeomAbs_Line => "Line",
        C::GeomAbs_Circle => "Circle",
        C::GeomAbs_Ellipse => "Ellipse",
        C::GeomAbs_Hyperbola => "Hyperbola",
        C::GeomAbs_Parabola => "Parabola",
        C::GeomAbs_BezierCurve => "Bezier",
        C::GeomAbs_BSplineCurve => "BSpline",
        C::GeomAbs_OffsetCurve => "Offset",
        C::GeomAbs_OtherCurve => "Other",
        _ => "?",
    }
}

fn shape_name(t: &ffi::top_abs::TopAbs_ShapeEnum) -> &'static str {
    use ffi::top_abs::TopAbs_ShapeEnum as S;
    match *t {
        S::TopAbs_COMPOUND => "COMPOUND",
        S::TopAbs_COMPSOLID => "COMPSOLID",
        S::TopAbs_SOLID => "SOLID",
        S::TopAbs_SHELL => "SHELL",
        S::TopAbs_FACE => "FACE",
        S::TopAbs_WIRE => "WIRE",
        S::TopAbs_EDGE => "EDGE",
        S::TopAbs_VERTEX => "VERTEX",
        _ => "?",
    }
}

/// Collect every sub-shape of `kind` in explorer order.
fn collect(shape: &ffi::topo_ds::TopoDS_Shape, kind: ffi::top_abs::TopAbs_ShapeEnum) -> Vec<Shp> {
    let mut out = vec![];
    let mut ex = ffi::top_exp::TopExp_Explorer_new(shape, kind);
    while ex.More() {
        out.push(ffi::top_exp::ExplorerCurrentShape(&ex));
        ex.pin_mut().Next();
    }
    out
}

struct FaceInfo {
    surface: String,
    area: f64,
    edges: Vec<String>,
    reversed: bool,
}

/// Topology walk. IDs are positions in TopExp_Explorer order.
fn topology(shape: &Shp) -> (String, Vec<FaceInfo>) {
    use ffi::top_abs::TopAbs_ShapeEnum as S;
    let mut rep = String::new();
    let _ = writeln!(rep, "root {}", shape_name(&shape.ShapeType()));
    let solids = collect(shape, S::TopAbs_SOLID);
    let faces = collect(shape, S::TopAbs_FACE);
    let _ = writeln!(rep, "solids {} faces {}", solids.len(), faces.len());
    let mut infos = vec![];
    for (i, f) in faces.iter().enumerate() {
        let face = ffi::topo_ds::TopoDS::Face(f);
        let surf = ffi::b_rep::BRep_Tool_Surface(face);
        let surface = if surf.IsNull() {
            "NULL".to_string()
        } else {
            ffi::standard::type_name(ffi::geom::DynamicType(&surf))
        };
        let mut props = ffi::g_prop::GProps_new();
        ffi::b_rep_g_prop::BRepGProp::SurfaceProperties(f, props.pin_mut(), false, false);
        let area = props.Mass();
        let mut edges = vec![];
        for e in collect(f, S::TopAbs_EDGE) {
            let edge = ffi::topo_ds::TopoDS::Edge(&e);
            let c = ffi::b_rep_adaptor::BRepAdaptor_Curve_new(edge);
            let (a, b) = (c.FirstParameter(), c.LastParameter());
            let p0 = ffi::b_rep_adaptor::BRepAdaptor_Curve_value(&c, a);
            let p1 = ffi::b_rep_adaptor::BRepAdaptor_Curve_value(&c, b);
            edges.push(format!(
                "{}({:.4},{:.4},{:.4})->({:.4},{:.4},{:.4})",
                curve_name(&c.GetType()),
                p0.X(), p0.Y(), p0.Z(), p1.X(), p1.Y(), p1.Z()
            ));
        }
        let reversed = matches!(
            f.Orientation(),
            ffi::top_abs::TopAbs_Orientation::TopAbs_REVERSED
        );
        let _ = writeln!(
            rep,
            "face {i} {surface} area={area:.6} rev={reversed} edges={}",
            edges.len()
        );
        for (j, e) in edges.iter().enumerate() {
            let _ = writeln!(rep, "  edge {i}.{j} {e}");
        }
        infos.push(FaceInfo { surface, area, edges, reversed });
    }
    (rep, infos)
}

/// Mesh with the only knob the bridge exposes (linear deflection) and report per-face counts.
fn tessellate(path: &str, tol: f64) -> Result<(String, usize, usize), String> {
    use ffi::top_abs::TopAbs_ShapeEnum as S;
    let shape = load(path)?; // fresh shape: BRepMesh would otherwise reuse a finer mesh
    let mesher = ffi::b_rep_mesh::IncrementalMesh_new(&shape, tol);
    if !mesher.IsDone() {
        return Err("IncrementalMesh not done".into());
    }
    let mut rep = String::new();
    let (mut tn, mut tt) = (0usize, 0usize);
    for (i, f) in collect(&shape, S::TopAbs_FACE).iter().enumerate() {
        let face = ffi::topo_ds::TopoDS::Face(f);
        let mut loc = ffi::top_loc::Location_new();
        let h = ffi::b_rep::BRep_Tool_Triangulation(face, loc.pin_mut());
        match ffi::poly::Handle_Poly_Triangulation_Get(&h) {
            Ok(t) => {
                let (n, m) = (t.NbNodes() as usize, t.NbTriangles() as usize);
                tn += n;
                tt += m;
                let _ = writeln!(rep, "  tol={tol} face {i} nodes={n} tris={m} uv={}", t.HasUVNodes());
            }
            Err(_) => {
                let _ = writeln!(rep, "  tol={tol} face {i} NO TRIANGULATION");
            }
        }
    }
    Ok((rep, tn, tt))
}

fn cmd_walk(path: &str, tess: bool) -> Result<(), String> {
    let shape = load(path)?;
    let (rep, _) = topology(&shape);
    print!("{rep}");
    println!("topology-digest {:016x}", fnv(&rep));
    if tess {
        for tol in [0.1, 0.01] {
            let (r, n, t) = tessellate(path, tol)?;
            print!("{r}");
            println!("tess tol={tol} total nodes={n} tris={t}");
        }
    }
    Ok(())
}

fn cmd_safe(path: &str) -> Result<(), String> {
    let shape = Shape::read_step(path).map_err(|e| e.to_string())?;
    let nfaces = shape.faces().count();
    println!("safe API: faces={nfaces}");
    for tol in [0.1, 0.01] {
        let m = shape.mesh_with_tolerance(tol).map_err(|e| e.to_string())?;
        println!(
            "safe mesh tol={tol}: vertices={} normals={} uvs={} triangles={} (per-face ranges exposed: no)",
            m.vertices.len(), m.normals.len(), m.uvs.len(), m.indices.len() / 3
        );
    }
    for (i, f) in shape.faces().enumerate().take(3) {
        println!(
            "safe face {i}: area={:.4} centre_normal={:?} edge_types={:?}",
            f.surface_area(),
            f.normal_at_center(),
            f.edges().map(|e| e.edge_type()).collect::<Vec<_>>()
        );
    }
    Ok(())
}

fn cmd_roundtrip(path: &str, tmp: &str) -> Result<(), String> {
    let a = load(path)?;
    let (ra, _) = topology(&a);
    let mut writer = ffi::step_control::STEPControl_Writer_new();
    if ffi::step_control::transfer_shape(writer.pin_mut(), &a)
        != ffi::if_select::IFSelect_ReturnStatus::IFSelect_RetDone
    {
        return Err("transfer failed".into());
    }
    if ffi::step_control::write_step(writer.pin_mut(), tmp.to_string())
        != ffi::if_select::IFSelect_ReturnStatus::IFSelect_RetDone
    {
        return Err("write failed".into());
    }
    let b = load(tmp)?;
    let (rb, _) = topology(&b);
    println!(
        "roundtrip digest a={:016x} b={:016x} same={}",
        fnv(&ra), fnv(&rb), ra == rb
    );
    Ok(())
}

fn cmd_gen(dir: &str) -> Result<(), String> {
    use glam::DVec3;
    use opencascade::primitives::Face;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let w = |name: &str, s: &Shape| -> Result<(), String> {
        s.write_step(format!("{dir}/{name}.step")).map_err(|e| e.to_string())
    };
    w("box", &Shape::box_with_dimensions(20.0, 30.0, 40.0))?;
    w("cylinder", &Shape::cylinder_radius_height(10.0, 50.0))?;
    w("cone", &Shape::cone().bottom_radius(12.0).top_radius(4.0).height(30.0).build())?;
    w("sphere", &Shape::sphere(15.0).build())?;
    w("torus", &Shape::torus().build())?;
    let boxed = Shape::box_with_dimensions(20.0, 30.0, 40.0);
    w("filleted_box", &boxed.fillet(3.0))?;
    // rotary blank: X-axis cylinder with a radial drilled hole
    let blank = Shape::cylinder_from_points(DVec3::new(0.0, 0.0, 0.0), DVec3::new(60.0, 0.0, 0.0), 12.0);
    let drilled = blank.drill_hole(DVec3::new(30.0, 0.0, -20.0), DVec3::Z, 3.0);
    w("rotary_blank_drilled", &drilled)?;
    // B-spline surface face (Bezier -> written as B_SPLINE_SURFACE)
    let surf = opencascade::primitives::Surface::bezier(vec![
        vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 0.0, 2.0), DVec3::new(20.0, 0.0, 0.0)],
        vec![DVec3::new(0.0, 10.0, 3.0), DVec3::new(10.0, 10.0, 6.0), DVec3::new(20.0, 10.0, 3.0)],
        vec![DVec3::new(0.0, 20.0, 0.0), DVec3::new(10.0, 20.0, 2.0), DVec3::new(20.0, 20.0, 0.0)],
    ]);
    let face = Face::from_surface(&surf);
    w("bspline_face", &Shape::from(face))?;
    Ok(())
}

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().collect();
    let r = match a.get(1).map(String::as_str) {
        Some("gen") => cmd_gen(&a[2]),
        Some("walk") => cmd_walk(&a[2], a.iter().any(|x| x == "--tess")),
        Some("safe") => cmd_safe(&a[2]),
        Some("roundtrip") => cmd_roundtrip(&a[2], &a[3]),
        _ => Err("usage: gen|walk|safe|roundtrip".into()),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            println!("ERR {e}");
            ExitCode::from(2)
        }
    }
}

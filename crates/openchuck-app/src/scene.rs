// SPDX-License-Identifier: AGPL-3.0-or-later
//! The scene view model: what the viewport draws for a setup, as plain data
//! (UI-02, UI-03; work breakdown slice 8 items 1 and 2; decision record 0029).
//!
//! Everything is in the model frame, whose X axis is the rotary axis. The
//! part is the setup's placed model with its feature edges; the stock,
//! the rotary axis, the direction of positive A and the work coordinate
//! system are lines. The direction of positive A comes from the shared
//! kinematics, so the viewport shows the convention the posts use (work
//! breakdown slice 3 review checklist, "visually in slice 8").

use openchuck_geom::display::{DEFAULT_CREASE_DEGREES, feature_edges};
use openchuck_path::BrepEdges;
use std::collections::BTreeSet;

use openchuck_project::id::{ModelId, SetupId};
use openchuck_project::{ModelRef, Project};
use openchuck_types::brep::{BrepModel, EdgeId, FaceId};
use openchuck_types::geometry::{Aabb, Point3, Vec3};
use openchuck_types::job::JobContext;
use openchuck_types::kinematics::{RotaryFrame, facing_at, model_up};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::setup::PartExtent;
use openchuck_types::stock::Stock;
use openchuck_types::units::Degrees;

use crate::AppError;
use crate::model::placed_part;
use crate::pick::{Picking, ScenePicker};
/// The triangle mesh type the scene holds, re-exported so front ends need no
/// engine crate of their own (architecture section 5.8).
pub use openchuck_types::mesh::TriangleMesh as Mesh;

/// What a scene item shows, so the viewport can colour it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SceneRole {
    /// The part (a model placed in the model frame).
    Part,
    /// The stock outline.
    Stock,
    /// A held region of the setup's workholding (SET-09), shaded.
    HeldRegion,
    /// A retaining tab (OP-21), shaded.
    Tab,
    /// The rotary axis.
    RotaryAxis,
    /// The arrow showing which way the part turns for positive A.
    ADirection,
    /// The work coordinate system's X axis.
    WorkX,
    /// The work coordinate system's Y axis.
    WorkY,
    /// The work coordinate system's Z axis.
    WorkZ,
    /// Triangles or edges a mesh check lists (IMP-04): non-manifold edges,
    /// flipped, degenerate or mismatched triangles.
    Problem,
    /// The edges round a hole in a mesh (IMP-04), which the view rings in
    /// amber apart from the other problems (specification 7.4; decision 0029
    /// item 13).
    Hole,
}

/// Which held region or tab of a setup's workholding a solid of the scene
/// is (UI-25). A tab around the axis is several solids with one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkholdingItem {
    /// The held region at the low-X end of the stock.
    Start,
    /// The held region at the high-X end of the stock.
    End,
    /// The tab at this index, from zero.
    Tab(usize),
}

/// A held region or tab solid drawn in the scene, for hover and selection
/// (specification 7.5).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneSolid {
    /// The index of its mesh in [`SceneView::meshes`].
    pub mesh: usize,
    /// Which held region or tab it is.
    pub item: WorkholdingItem,
    /// Its name and size, for the tooltip: "Tab 2 · frustum · X 14.000 to
    /// 16.000 mm · Ø 4.000 to Ø 8.000 mm".
    pub description: String,
}

/// A mesh to draw shaded, with the feature edges to draw over it.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneMesh {
    /// What it shows.
    pub role: SceneRole,
    /// The model the mesh shows, for the part of a scene built from the
    /// project; `None` for a preview or a check overlay.
    pub model: Option<ModelId>,
    /// The triangles, in the model frame.
    pub mesh: TriangleMesh,
    /// Feature edges as vertex index pairs (`openchuck_geom::display`). Empty
    /// for a STEP part, whose own edges are in `brep`.
    pub feature_edges: Vec<[u32; 2]>,
    /// A STEP part's faces and edges (slice 11 item 5); `None` for a mesh.
    pub brep: Option<SceneBrep>,
}

/// The faces and edges of a STEP part as the scene draws and picks them
/// (UI-04; slice 11 item 5; decision record 0066).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneBrep {
    /// The face of each triangle of the mesh: the faces' tessellations are
    /// joined in face order.
    pub faces: Vec<FaceId>,
    /// The edges, in ID order: every edge of the model but seams and edges
    /// of no length, tangent ones included, so each face's outline is drawn.
    pub edges: Vec<SceneEdge>,
}

impl SceneBrep {
    /// The faces and edges of the placed B-rep `brep`, whose joined
    /// tessellation is the scene mesh.
    pub(crate) fn of(brep: &BrepModel) -> Result<Self, AppError> {
        let faces = brep
            .faces
            .iter()
            .flat_map(|f| std::iter::repeat_n(f.id, f.tessellation.triangles.len()))
            .collect();
        let turns = BrepEdges::new(brep)?.with_crease(0.0).edge_turns();
        let edges = turns
            .into_iter()
            .filter_map(|(id, turn)| {
                let edge = brep.edges.iter().find(|e| e.id == id)?;
                let mut faces = edge.faces.clone();
                faces.sort_unstable();
                faces.dedup();
                Some(SceneEdge {
                    id,
                    faces,
                    turn,
                    points: edge.polyline.clone(),
                })
            })
            .collect();
        Ok(Self { faces, edges })
    }

    /// The triangles of the faces `faces`, for drawing them (UI-04).
    pub fn triangles_of(&self, faces: &BTreeSet<FaceId>) -> BTreeSet<u32> {
        (0u32..)
            .zip(&self.faces)
            .filter_map(|(t, f)| faces.contains(f).then_some(t))
            .collect()
    }

    /// The edge with ID `id`.
    pub fn edge(&self, id: EdgeId) -> Option<&SceneEdge> {
        self.edges
            .binary_search_by_key(&id, |e| e.id)
            .ok()
            .and_then(|i| self.edges.get(i))
    }
}

/// One edge of a STEP part.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneEdge {
    /// Its ID in the model.
    pub id: EdgeId,
    /// The faces it joins, in ID order.
    pub faces: Vec<FaceId>,
    /// The sharpest turn between its two faces, degrees; `None` for an edge
    /// of one face (a boundary) or of three or more.
    pub turn: Option<f64>,
    /// Its polyline, in the model frame.
    pub points: Vec<Point3>,
}

/// A connected line through points.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenePolyline {
    /// What it shows.
    pub role: SceneRole,
    /// The points, in order.
    pub points: Vec<Point3>,
}

/// A short text placed at a point, such as "A+" or "X".
#[derive(Debug, Clone, PartialEq)]
pub struct SceneLabel {
    /// What it labels.
    pub role: SceneRole,
    /// Where, in the model frame.
    pub at: Point3,
    /// The text.
    pub text: String,
}

/// The view model of one setup's scene.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SceneView {
    /// The setup shown, or `None` when the project has no setup and every
    /// model is shown at its placement.
    pub setup: Option<SetupId>,
    /// Shaded meshes.
    pub meshes: Vec<SceneMesh>,
    /// Lines.
    pub lines: Vec<ScenePolyline>,
    /// Labels.
    pub labels: Vec<SceneLabel>,
    /// The box holding everything in the scene, and the model origin.
    pub bounds: Option<Aabb>,
    /// The direction of positive A in words, when the project has a machine.
    pub a_direction: Option<String>,
    /// Things the user should know: no machine, a part that cannot be read.
    pub notes: Vec<String>,
    /// Ray-cast picking on the part meshes (UI-04); empty for a preview.
    pub picking: Picking,
    /// The held region and tab solids among `meshes`, in mesh order.
    pub solids: Vec<SceneSolid>,
    /// The work offset register of the setup shown ("G54"), which names the
    /// frame of the viewport's readout (specification 7.2).
    pub work_register: Option<String>,
}

/// The scene item of the placed part `model`: its triangles `mesh`, and for a
/// STEP model its placed B-rep `brep` (whose joined tessellation `mesh` is),
/// drawn with its own edges and picked by face and edge (slice 11 item 5;
/// decision record 0066). A mesh draws its feature edges. A B-rep whose faces
/// cannot be evaluated is drawn as a mesh, with a note in `notes`.
pub(crate) fn part_mesh(
    model: &ModelRef,
    mesh: TriangleMesh,
    brep: Option<&BrepModel>,
    notes: &mut Vec<String>,
) -> Result<SceneMesh, AppError> {
    let brep = match brep.map(SceneBrep::of) {
        Some(Ok(brep)) => Some(brep),
        Some(Err(e)) => {
            notes.push(format!(
                "the faces and edges of '{}' cannot be shown or selected: {e}",
                model.path.as_str()
            ));
            None
        }
        None => None,
    };
    let feature_edges = match brep {
        Some(_) => Vec::new(),
        None => feature_edges(&mesh, DEFAULT_CREASE_DEGREES)?,
    };
    Ok(SceneMesh {
        role: SceneRole::Part,
        model: Some(model.id),
        mesh,
        feature_edges,
        brep,
    })
}

/// Segments used to draw a circle.
const CIRCLE_SEGMENTS: usize = 96;

/// Builds the scene of `setup` (or the first setup, or every model when the
/// project has no setup). Reads and places the models, so it runs as a job.
pub(crate) fn build_scene(
    root: &std::path::Path,
    project: &Project,
    setup: Option<SetupId>,
    context: &JobContext<'_>,
) -> Result<SceneView, AppError> {
    let chosen = match setup {
        Some(id) => Some(
            project
                .setups()
                .iter()
                .find(|s| s.id == id)
                .ok_or_else(|| AppError::UnknownSetup(id.to_string()))?,
        ),
        None => project.setups().first(),
    };
    let mut view = SceneView {
        setup: chosen.map(|s| s.id),
        ..SceneView::default()
    };

    // The models to draw: the setup's part, or every model without a setup.
    let models: Vec<&ModelRef> = match chosen {
        Some(s) => s
            .part
            .and_then(|id| project.models().iter().find(|m| m.id == id))
            .into_iter()
            .collect(),
        None => project.models().iter().collect(),
    };
    if let Some(s) = chosen
        && s.part.is_none()
    {
        view.notes
            .push(format!("setup '{}' has no part chosen", s.name));
    }
    let mut part: Option<PartExtent> = None;
    for model in models {
        context.check_cancelled()?;
        // A STEP part brings its placed B-rep, so it is drawn with its own
        // edges and picked by face and edge (decision record 0066).
        match placed_part(root, model, context) {
            Ok(placed) => {
                part = merge_extent(part, openchuck_geom::part_extent(&placed.mesh));
                let item = part_mesh(model, placed.mesh, placed.brep.as_ref(), &mut view.notes)?;
                view.meshes.push(item);
            }
            Err(e) => view
                .notes
                .push(format!("'{}' cannot be shown: {e}", model.path.as_str())),
        }
    }

    // The hierarchy for picking is built here, in the job, so the interface
    // thread never waits for it (NFR-04; decision record 0036).
    context.check_cancelled()?;
    match ScenePicker::build(&view.meshes) {
        Ok(picker) => view.picking = Picking::new(picker),
        Err(e) => view
            .notes
            .push(format!("the part cannot be picked on: {e}")),
    }

    // The held regions and tabs, shaded beside the part (SET-09, OP-21;
    // UI-03). A setup whose workholding cannot be built is noted, not hidden.
    if let (Some(s), Some(extent)) = (chosen, part.as_ref())
        && !s.workholding.is_empty()
    {
        match crate::workholding_view::display_items(&s.workholding, s.stock.as_ref(), extent) {
            Ok((meshes, lines, solids)) => {
                let first = view.meshes.len();
                view.meshes.extend(meshes);
                view.lines.extend(lines);
                view.solids.extend(solids.into_iter().map(|mut solid| {
                    solid.mesh += first;
                    solid
                }));
            }
            Err(e) => view
                .notes
                .push(format!("the workholding cannot be drawn: {e}")),
        }
    }

    let mut bounds = part.map(|p| p.bounds);
    if let Some(stock) = chosen.and_then(|s| s.stock.as_ref()) {
        match stock_outline(stock, part.as_ref()) {
            Some((lines, stock_bounds)) => {
                view.lines.extend(lines);
                bounds = Some(union(bounds, stock_bounds));
            }
            None => view.notes.push(
                "the stock cannot be drawn yet (imported stock, or bounding-box stock without a readable part)"
                    .to_owned(),
            ),
        }
    }
    let origin = Point3::new(0.0, 0.0, 0.0);
    let work_zero = chosen.map_or(origin, |s| Point3::new(s.frame.work_offset.x.0, 0.0, 0.0));
    view.work_register = chosen.map(|s| s.frame.work_offset.register.name().to_owned());
    let mut bounds = union(
        bounds,
        Aabb {
            min: origin,
            max: origin,
        },
    );
    bounds = union(
        Some(bounds),
        Aabb {
            min: work_zero,
            max: work_zero,
        },
    );
    let size = (bounds.max - bounds.min).length().max(10.0);

    // The rotary axis, past both ends of the scene.
    let margin = 0.15 * size;
    let (x0, x1) = (bounds.min.x - margin, bounds.max.x + margin);
    view.lines.push(ScenePolyline {
        role: SceneRole::RotaryAxis,
        points: vec![Point3::new(x0, 0.0, 0.0), Point3::new(x1, 0.0, 0.0)],
    });

    // The direction of positive A, from the shared kinematics.
    match project.machine() {
        Some(profile) => {
            let frame = RotaryFrame::of_profile(profile);
            let radius = (0.5
                * bounds
                    .max
                    .y
                    .abs()
                    .max(bounds.max.z.abs())
                    .max(bounds.min.y.abs().max(bounds.min.z.abs())))
            .max(0.08 * size);
            let (arrow, label_at) = a_arrow(&frame, x0 + 0.3 * margin, radius);
            view.lines.extend(arrow);
            view.labels.push(SceneLabel {
                role: SceneRole::ADirection,
                at: label_at,
                text: "A+".to_owned(),
            });
            view.a_direction = Some(crate::setup::a_direction(profile));
        }
        None => view.notes.push(
            "the project has no machine profile, so the direction of A is not shown".to_owned(),
        ),
    }

    // The work coordinate system at the work zero.
    let length = 0.2 * size;
    for (role, direction, text) in [
        (SceneRole::WorkX, Vec3::new(1.0, 0.0, 0.0), "X"),
        (SceneRole::WorkY, Vec3::new(0.0, 1.0, 0.0), "Y"),
        (SceneRole::WorkZ, Vec3::new(0.0, 0.0, 1.0), "Z"),
    ] {
        let end = work_zero + direction * length;
        view.lines.push(ScenePolyline {
            role,
            points: vec![work_zero, end],
        });
        view.labels.push(SceneLabel {
            role,
            at: end,
            text: text.to_owned(),
        });
    }
    view.bounds = Some(bounds);
    Ok(view)
}

/// The arrow for positive A: an arc of 270 degrees around the axis at `x`,
/// traced by where the part's up side faces as A grows (the shared
/// kinematics), with a head at its end. Returns the lines and where to put
/// the "A+" label: just outside the arc an eighth of a turn from its start,
/// between the top and the side positive A turns toward. The arc's end is
/// on the other side, where the work Y axis label sits in end views.
///
/// The machine directions from `facing_at` are drawn in the model frame,
/// which equals the machine frame's orientation at A = 0 when the rotary axis
/// is X, the only axis the first release allows (SET-02 is P2).
fn a_arrow(frame: &RotaryFrame, x: f64, radius: f64) -> (Vec<ScenePolyline>, Point3) {
    let up = model_up(frame);
    let at = |degrees: f64| {
        let d = facing_at(frame, up, Degrees(degrees));
        Point3::new(x, d.y * radius, d.z * radius)
    };
    let steps = 54;
    let sweep = 270.0;
    let arc: Vec<Point3> = (0..=steps)
        .map(|i| at(sweep * f64::from(i) / f64::from(steps)))
        .collect();
    let tip = at(sweep);
    let label = {
        let p = at(45.0);
        Point3::new(p.x, p.y * 1.15, p.z * 1.15)
    };
    // The head: two short lines back from the tip, either side of the arc.
    let back = at(sweep - 15.0);
    let along = tip - back;
    let outward = Vec3::new(0.0, tip.y, tip.z) * (1.0 / radius.max(f64::MIN_POSITIVE));
    let head = 0.6;
    let left = tip + (along * -head) + outward * (0.12 * radius);
    let right = tip + (along * -head) + outward * (-0.12 * radius);
    (
        vec![
            ScenePolyline {
                role: SceneRole::ADirection,
                points: arc,
            },
            ScenePolyline {
                role: SceneRole::ADirection,
                points: vec![left, tip, right],
            },
        ],
        label,
    )
}

/// The outline of the stock as lines, and its bounds. `None` when the stock
/// cannot be measured without data this view does not have.
fn stock_outline(stock: &Stock, part: Option<&PartExtent>) -> Option<(Vec<ScenePolyline>, Aabb)> {
    let bounds = stock.bounds(part)?;
    let line = |points: Vec<Point3>| ScenePolyline {
        role: SceneRole::Stock,
        points,
    };
    let lines = match stock {
        Stock::Cylinder { diameter, .. } => {
            let r = diameter.0 / 2.0;
            let circle = |x: f64| {
                (0..=CIRCLE_SEGMENTS)
                    .map(|i| {
                        let t = std::f64::consts::TAU * i as f64 / CIRCLE_SEGMENTS as f64;
                        Point3::new(x, r * t.cos(), r * t.sin())
                    })
                    .collect::<Vec<_>>()
            };
            let mut lines = vec![line(circle(bounds.min.x)), line(circle(bounds.max.x))];
            for (y, z) in [(r, 0.0), (0.0, r), (-r, 0.0), (0.0, -r)] {
                lines.push(line(vec![
                    Point3::new(bounds.min.x, y, z),
                    Point3::new(bounds.max.x, y, z),
                ]));
            }
            lines
        }
        _ => box_edges(&bounds).into_iter().map(line).collect(),
    };
    Some((lines, bounds))
}

/// The twelve edges of a box, as two-point lines.
fn box_edges(b: &Aabb) -> Vec<Vec<Point3>> {
    let corner = |i: usize| {
        Point3::new(
            if i & 1 == 0 { b.min.x } else { b.max.x },
            if i & 2 == 0 { b.min.y } else { b.max.y },
            if i & 4 == 0 { b.min.z } else { b.max.z },
        )
    };
    let mut edges = Vec::with_capacity(12);
    for i in 0..8 {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                edges.push(vec![corner(i), corner(i | bit)]);
            }
        }
    }
    edges
}

fn union(a: Option<Aabb>, b: Aabb) -> Aabb {
    match a {
        None => b,
        Some(a) => Aabb {
            min: Point3::new(
                a.min.x.min(b.min.x),
                a.min.y.min(b.min.y),
                a.min.z.min(b.min.z),
            ),
            max: Point3::new(
                a.max.x.max(b.max.x),
                a.max.y.max(b.max.y),
                a.max.z.max(b.max.z),
            ),
        },
    }
}

fn merge_extent(a: Option<PartExtent>, b: Option<PartExtent>) -> Option<PartExtent> {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some(a), Some(b)) => Some(PartExtent {
            bounds: union(Some(a.bounds), b.bounds),
            radius: openchuck_types::units::Millimetres(a.radius.0.max(b.radius.0)),
        }),
    }
}

/// The triangles `triangles` of `mesh` as a mesh of their own, with only the
/// vertices they use, for drawing a selection or a hover over the part
/// (UI-04). Triangles the mesh does not have are left out.
pub fn triangles_mesh(mesh: &TriangleMesh, triangles: &BTreeSet<u32>) -> TriangleMesh {
    let mut index = std::collections::BTreeMap::new();
    let mut out = TriangleMesh {
        vertices: Vec::new(),
        triangles: Vec::new(),
    };
    for &t in triangles {
        let Some(corners) = mesh.triangles.get(t as usize) else {
            continue;
        };
        let mut mapped = [0u32; 3];
        let mut complete = true;
        for (slot, &vertex) in mapped.iter_mut().zip(corners) {
            let next = out.vertices.len();
            match mesh.vertices.get(vertex as usize) {
                Some(&p) => {
                    let id = *index.entry(vertex).or_insert_with(|| {
                        out.vertices.push(p);
                        next as u32
                    });
                    *slot = id;
                }
                None => complete = false,
            }
        }
        if complete {
            out.triangles.push(mapped);
        }
    }
    out
}

/// The scene of one placed model on its own, before it joins a setup: the
/// model with its feature edges, the triangles and edges `report` lists as
/// problems, and the rotary axis for orientation (OI-51; decision record
/// 0029 item 13).
pub(crate) fn model_scene(
    mesh: TriangleMesh,
    report: &openchuck_geom::check::MeshReport,
) -> Result<SceneView, AppError> {
    let mut view = SceneView::default();
    let extent = openchuck_geom::part_extent(&mesh);
    let origin = Point3::new(0.0, 0.0, 0.0);
    let bounds = union(
        extent.map(|e| e.bounds),
        Aabb {
            min: origin,
            max: origin,
        },
    );
    let size = (bounds.max - bounds.min).length().max(10.0);

    // Problem triangles as their own mesh, drawn over the model.
    let mut flagged: Vec<u32> = report
        .flipped_triangles
        .iter()
        .chain(&report.degenerate_triangles)
        .chain(&report.normal_mismatches)
        .copied()
        .collect();
    flagged.sort_unstable();
    flagged.dedup();
    if !flagged.is_empty() {
        let triangles = flagged
            .iter()
            .filter_map(|&t| mesh.triangles.get(t as usize).copied())
            .collect();
        view.meshes.push(SceneMesh {
            role: SceneRole::Problem,
            model: None,
            mesh: TriangleMesh {
                vertices: mesh.vertices.clone(),
                triangles,
            },
            feature_edges: Vec::new(),
            brep: None,
        });
    }
    // The edges round holes, and non-manifold edges, as lines.
    let holes = report.holes.iter().flat_map(|h| h.edges.iter());
    for (role, [a, b]) in holes.map(|e| (SceneRole::Hole, e)).chain(
        report
            .non_manifold_edges
            .iter()
            .map(|e| (SceneRole::Problem, e)),
    ) {
        if let (Some(&p), Some(&q)) = (
            mesh.vertices.get(*a as usize),
            mesh.vertices.get(*b as usize),
        ) {
            view.lines.push(ScenePolyline {
                role,
                points: vec![p, q],
            });
        }
    }
    let margin = 0.15 * size;
    view.lines.push(ScenePolyline {
        role: SceneRole::RotaryAxis,
        points: vec![
            Point3::new(bounds.min.x - margin, 0.0, 0.0),
            Point3::new(bounds.max.x + margin, 0.0, 0.0),
        ],
    });
    let feature_edges = feature_edges(&mesh, DEFAULT_CREASE_DEGREES)?;
    view.meshes.insert(
        0,
        SceneMesh {
            role: SceneRole::Part,
            model: None,
            mesh,
            feature_edges,
            brep: None,
        },
    );
    view.bounds = Some(bounds);
    Ok(view)
}

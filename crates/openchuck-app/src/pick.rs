// SPDX-License-Identifier: AGPL-3.0-or-later
//! Picking in the scene: which triangle, face or edge of which model is
//! under a ray, and what a pick selects (UI-04; work breakdown slice 10 item
//! 1 and slice 11 item 5; decision records 0036 and 0066).
//!
//! The scene job builds a [`ScenePicker`] with the part meshes it places, so
//! the hierarchy is built off the interface thread (NFR-04) and costs
//! nothing at the pointer. The viewport turns the pointer into a ray with
//! `Camera::ray` and asks [`ScenePicker::pick`] for the surface or
//! [`ScenePicker::pick_edge`] for an edge; it only draws the highlight, which
//! is [`crate::scene::triangles_mesh`] of the triangles chosen, or the edges'
//! polylines. What a click selects is a [`PickMode`]: on a mesh it maps the
//! pick to a [`RegionRequest`] for [`Application::select_region`]; on a STEP
//! part's faces, and on edges, [`ScenePicker::select_faces`] and
//! [`ScenePicker::select_edges`] answer at once from the scene's own data.
//!
//! A STEP part is picked by face and edge, never by triangle (slice 11 item
//! 7): a pick on it names the face of the triangle hit. A mesh's edges are
//! its feature edges as drawn, each named by the lowest triangle and side that
//! has it ([`MeshEdge`]), so a stored mesh edge selection names each edge
//! once (schema 12; OI-204).
//!
//! [`Application::select_region`]: crate::Application::select_region

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use openchuck_geom::edges::EdgeMap;
use openchuck_geom::pick::{EdgePicker, MeshPicker};
use openchuck_project::MeshEdge;
use openchuck_project::id::ModelId;
use openchuck_types::brep::{EdgeId, FaceId};
use openchuck_types::geometry::{Point3, Vec3};

use crate::scene::{SceneMesh, SceneRole};
use crate::{AppError, RegionRequest};

/// How far from the pointer's ray an edge may be and still be picked; see
/// [`openchuck_geom::pick::PickTolerance`].
pub use openchuck_geom::pick::PickTolerance;

/// A triangle under a ray.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScenePick {
    /// Index of the mesh in the scene's `meshes`.
    pub mesh: usize,
    /// The model the mesh shows, when it shows one.
    pub model: Option<ModelId>,
    /// Index of the triangle: facet `triangle` of the model file, whatever
    /// the placement (decision record 0032); for a STEP part, a triangle of
    /// its faces' joined tessellation.
    pub triangle: u32,
    /// The face of a STEP part the triangle is on; `None` on a mesh.
    pub face: Option<FaceId>,
    /// Distance from the ray's origin, millimetres.
    pub distance: f64,
    /// The point hit, in the model frame.
    pub point: Point3,
    /// The triangle's unit normal in the model frame; `None` when it has no
    /// area.
    pub normal: Option<Vec3>,
}

/// An edge of a part: a STEP part's own edge, or an edge of a mesh's
/// triangles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeRef {
    /// A B-rep edge by ID.
    Brep(EdgeId),
    /// A mesh edge by its lowest triangle and side.
    Mesh(MeshEdge),
}

/// An edge near a ray.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneEdgePick {
    /// Index of the mesh in the scene's `meshes`.
    pub mesh: usize,
    /// The model the mesh shows, when it shows one.
    pub model: Option<ModelId>,
    /// The edge.
    pub edge: EdgeRef,
    /// Index of the edge in the part's list of pickable edges, for
    /// [`ScenePicker::edge_points`] and [`ScenePicker::select_edges`].
    pub index: usize,
    /// Distance from the ray's origin to its point nearest the edge,
    /// millimetres.
    pub distance: f64,
    /// The point of the edge nearest the ray, in the model frame.
    pub point: Point3,
}

/// The ends of the part's edges, for chaining them (decision record 0066).
#[derive(Debug, Clone, PartialEq, Default)]
struct Corners {
    /// The corner at each end of each edge: `[start, end]`.
    ends: Vec<[usize; 2]>,
    /// The edges that meet at each corner, with the end that meets it (0 at
    /// the start, 1 at the end).
    at: Vec<Vec<(usize, usize)>>,
}

/// How close, in millimetres, two edge ends must be to be one corner.
const WELD: f64 = 1e-5;

impl Corners {
    fn new(lines: &[Vec<Point3>]) -> Self {
        let key = |p: Point3| [p.x, p.y, p.z].map(|v| (v / WELD).round() as i64);
        let mut ids: BTreeMap<[i64; 3], usize> = BTreeMap::new();
        let mut corners = Self::default();
        for (edge, line) in lines.iter().enumerate() {
            let (Some(&first), Some(&last)) = (line.first(), line.last()) else {
                corners.ends.push([usize::MAX; 2]);
                continue;
            };
            let mut ends = [0; 2];
            for (end, p) in [first, last].into_iter().enumerate() {
                let next = ids.len();
                let id = *ids.entry(key(p)).or_insert(next);
                if id == corners.at.len() {
                    corners.at.push(Vec::new());
                }
                if let Some(list) = corners.at.get_mut(id) {
                    list.push((edge, end));
                }
                if let Some(slot) = ends.get_mut(end) {
                    *slot = id;
                }
            }
            corners.ends.push(ends);
        }
        corners
    }
}

/// The unit direction leaving the corner at end `end` (0 start, 1 end) of
/// `line`, along it.
fn outward(line: &[Point3], end: usize) -> Option<Vec3> {
    let (a, b) = if end == 0 {
        let first = *line.first()?;
        (
            first,
            *line.iter().skip(1).find(|p| p.distance(first) > 0.0)?,
        )
    } else {
        let last = *line.last()?;
        (
            last,
            *line.iter().rev().skip(1).find(|p| p.distance(last) > 0.0)?,
        )
    };
    let v = b - a;
    let length = v.length();
    (length > 0.0).then(|| v * (1.0 / length))
}

struct PartPicker {
    mesh: usize,
    model: Option<ModelId>,
    picker: MeshPicker,
    /// The face of each triangle of a STEP part; empty for a mesh.
    faces: Vec<FaceId>,
    /// The normal of each triangle.
    normals: Vec<Option<Vec3>>,
    edges: EdgePicker,
    edge_refs: Vec<EdgeRef>,
    /// For a STEP part, the faces each edge joins and their sharpest turn
    /// there (`None` for a boundary or an edge of three or more faces).
    edge_faces: Vec<(Vec<FaceId>, Option<f64>)>,
    corners: Corners,
}

impl PartPicker {
    fn new(mesh: usize, item: &SceneMesh) -> Result<Self, AppError> {
        let picker = MeshPicker::new(&item.mesh)?;
        let normals = (0..item.mesh.triangles.len() as u32)
            .map(|t| picker.normal(t))
            .collect();
        let (faces, lines, edge_refs, edge_faces) = match &item.brep {
            Some(brep) => (
                brep.faces.clone(),
                brep.edges.iter().map(|e| e.points.clone()).collect(),
                brep.edges.iter().map(|e| EdgeRef::Brep(e.id)).collect(),
                brep.edges
                    .iter()
                    .map(|e| (e.faces.clone(), e.turn))
                    .collect(),
            ),
            None => {
                let map = EdgeMap::new(&item.mesh)?;
                let mut lines = Vec::with_capacity(item.feature_edges.len());
                let mut refs = Vec::with_capacity(item.feature_edges.len());
                for pair in &item.feature_edges {
                    let (Some(name), Some(&p), Some(&q)) = (
                        lowest_side(&map, *pair),
                        item.mesh.vertices.get(pair[0] as usize),
                        item.mesh.vertices.get(pair[1] as usize),
                    ) else {
                        continue;
                    };
                    lines.push(vec![p, q]);
                    refs.push(EdgeRef::Mesh(name));
                }
                (Vec::new(), lines, refs, Vec::new())
            }
        };
        let corners = Corners::new(&lines);
        Ok(Self {
            mesh,
            model: item.model,
            picker,
            faces,
            normals,
            edges: EdgePicker::new(lines)?,
            edge_refs,
            edge_faces,
            corners,
        })
    }

    /// The faces joined to `face` across edges that turn by at most `angle`
    /// degrees (any edge when `None`), `face` included.
    fn flood_faces(&self, face: FaceId, angle: Option<f64>) -> BTreeSet<FaceId> {
        let mut found = BTreeSet::from([face]);
        let mut todo = vec![face];
        while let Some(f) = todo.pop() {
            for (faces, turn) in &self.edge_faces {
                let crosses = match angle {
                    None => true,
                    Some(limit) => turn.is_some_and(|t| t <= limit),
                };
                if !crosses || !faces.contains(&f) {
                    continue;
                }
                for g in faces {
                    if found.insert(*g) {
                        todo.push(*g);
                    }
                }
            }
        }
        found
    }

    /// The edges chained to edge `start` at corners: where exactly two meet
    /// (`angle` `None`), or onward to the edge that turns least when that turn
    /// is at most `angle` degrees.
    fn chain(&self, start: usize, angle: Option<f64>) -> BTreeSet<usize> {
        let mut found = BTreeSet::from([start]);
        for leave in [0, 1] {
            let (mut edge, mut end) = (start, leave);
            loop {
                let Some(corner) = self.corners.ends.get(edge).and_then(|e| e.get(end)) else {
                    break;
                };
                let others: Vec<(usize, usize)> = self
                    .corners
                    .at
                    .get(*corner)
                    .map(|list| {
                        list.iter()
                            .copied()
                            .filter(|&(e, x)| (e, x) != (edge, end))
                            .collect()
                    })
                    .unwrap_or_default();
                let line = |e: usize| self.edges.polyline(e).unwrap_or_default();
                // Arriving along `edge`: the direction into the corner.
                let arriving = outward(line(edge), end).map(|v| v * -1.0);
                let next = match angle {
                    None => match others.as_slice() {
                        [only] => Some(*only),
                        _ => None,
                    },
                    Some(limit) => others
                        .iter()
                        .filter_map(|&(e, x)| {
                            let turn = arriving
                                .zip(outward(line(e), x))
                                .map(|(a, b)| a.dot(b).clamp(-1.0, 1.0).acos().to_degrees())?;
                            (turn <= limit).then_some((turn, e, x))
                        })
                        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
                        .map(|(_, e, x)| (e, x)),
                };
                let Some((e, x)) = next else { break };
                if !found.insert(e) {
                    break;
                }
                // Leave the next edge by its other end.
                (edge, end) = (e, 1 - x);
            }
        }
        found
    }
}

/// The lowest triangle and side that has the edge `pair` of `map`.
fn lowest_side(map: &EdgeMap, pair: [u32; 2]) -> Option<MeshEdge> {
    let edge = map.edges().binary_search(&pair).ok()?;
    map.uses(edge)
        .iter()
        .filter_map(|u| {
            let side = map
                .sides(u.triangle as usize)
                .iter()
                .position(|s| *s == Some(edge as u32))?;
            MeshEdge::new(u.triangle, side as u8)
        })
        .min()
}

/// The hierarchies of the scene's part meshes, with their pickable edges.
pub struct ScenePicker {
    parts: Vec<PartPicker>,
}

impl PartialEq for ScenePicker {
    fn eq(&self, other: &Self) -> bool {
        self.parts.len() == other.parts.len()
            && self.parts.iter().zip(&other.parts).all(|(a, b)| {
                a.mesh == b.mesh
                    && a.model == b.model
                    && a.picker == b.picker
                    && a.faces == b.faces
                    && a.edges == b.edges
                    && a.edge_refs == b.edge_refs
            })
    }
}

impl std::fmt::Debug for ScenePicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScenePicker")
            .field("parts", &self.parts.len())
            .finish()
    }
}

impl ScenePicker {
    /// Builds the hierarchy of every part mesh in `meshes`, and the edges a
    /// pointer can pick: a STEP part's own edges, a mesh's feature edges. A
    /// part with no triangles is left out.
    pub(crate) fn build(meshes: &[SceneMesh]) -> Result<Self, AppError> {
        let mut parts = Vec::new();
        for (index, item) in meshes.iter().enumerate() {
            if item.role == SceneRole::Part && !item.mesh.triangles.is_empty() {
                parts.push(PartPicker::new(index, item)?);
            }
        }
        Ok(Self { parts })
    }

    /// Whether nothing can be picked.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// The nearest triangle the ray from `origin` along `direction` meets in
    /// any part, with its face on a STEP part, or `None`. Fails for a ray
    /// that is not finite or has no direction.
    pub fn pick(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
    ) -> Result<Option<ScenePick>, AppError> {
        let o = Point3::new(origin[0], origin[1], origin[2]);
        let d = Vec3::new(direction[0], direction[1], direction[2]);
        let mut best: Option<ScenePick> = None;
        for part in &self.parts {
            if let Some(hit) = part.picker.pick(o, d)?
                && best.is_none_or(|b| hit.distance < b.distance)
            {
                best = Some(ScenePick {
                    mesh: part.mesh,
                    model: part.model,
                    triangle: hit.triangle,
                    face: part.faces.get(hit.triangle as usize).copied(),
                    distance: hit.distance,
                    point: hit.point,
                    normal: part.picker.normal(hit.triangle),
                });
            }
        }
        Ok(best)
    }

    /// The edge the ray from `origin` along `direction` passes nearest within
    /// `tolerance`, in any part, or `None`. An edge behind the surface the ray
    /// meets first is not picked; one on that surface's outline is (the
    /// tolerance is allowed past the hit). Fails for a ray or tolerance that
    /// is not valid.
    pub fn pick_edge(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
        tolerance: PickTolerance,
    ) -> Result<Option<SceneEdgePick>, AppError> {
        let o = Point3::new(origin[0], origin[1], origin[2]);
        let d = Vec3::new(direction[0], direction[1], direction[2]);
        let max = match self.pick(origin, direction)? {
            Some(hit) => hit.distance + 2.0 * tolerance.at(hit.distance),
            None => f64::INFINITY,
        };
        let mut best: Option<(f64, SceneEdgePick)> = None;
        for part in &self.parts {
            let Some(hit) = part.edges.pick(o, d, tolerance, max)? else {
                continue;
            };
            let Some(&edge) = part.edge_refs.get(hit.edge) else {
                continue;
            };
            let share = hit.offset / tolerance.at(hit.distance);
            if best.is_none_or(|(s, b)| share < s || (share == s && hit.distance < b.distance)) {
                best = Some((
                    share,
                    SceneEdgePick {
                        mesh: part.mesh,
                        model: part.model,
                        edge,
                        index: hit.edge,
                        distance: hit.distance,
                        point: hit.point,
                    },
                ));
            }
        }
        Ok(best.map(|(_, pick)| pick))
    }

    fn part(&self, mesh: usize) -> Option<&PartPicker> {
        self.parts.iter().find(|p| p.mesh == mesh)
    }

    /// The polyline of pickable edge `index` of the part drawn by mesh
    /// `mesh`, in the model frame.
    pub fn edge_points(&self, mesh: usize, index: usize) -> Option<&[Point3]> {
        self.part(mesh)?.edges.polyline(index)
    }

    /// The polylines of the edges `edges` of the part drawn by mesh `mesh`,
    /// for drawing them; edges it does not have are left out.
    pub fn edges_points(&self, mesh: usize, edges: &BTreeSet<EdgeRef>) -> Vec<&[Point3]> {
        let Some(part) = self.part(mesh) else {
            return Vec::new();
        };
        part.edge_refs
            .iter()
            .enumerate()
            .filter(|(_, r)| edges.contains(r))
            .filter_map(|(i, _)| part.edges.polyline(i))
            .collect()
    }

    /// The faces of a STEP part that `mode` selects from `pick` (decision
    /// record 0066): the face alone, the faces it is joined to through
    /// edges, the faces reached across edges that turn by at most the angle,
    /// or the faces all of whose triangles face within the angle of the
    /// picked triangle. `None` when the pick is not on a STEP part, or the
    /// mode needs a direction the triangle does not have.
    pub fn select_faces(&self, pick: &ScenePick, mode: PickMode) -> Option<BTreeSet<FaceId>> {
        let face = pick.face?;
        let part = self.part(pick.mesh)?;
        Some(match mode {
            PickMode::Triangle => BTreeSet::from([face]),
            PickMode::Connected => part.flood_faces(face, None),
            PickMode::SmoothPatch { max_edge_angle } => {
                part.flood_faces(face, Some(max_edge_angle))
            }
            PickMode::SameDirection { max_angle } => {
                let direction = pick.normal?;
                let cos = max_angle.clamp(0.0, 180.0).to_radians().cos();
                let mut facing: BTreeMap<FaceId, bool> = BTreeMap::new();
                for (f, n) in part.faces.iter().zip(&part.normals) {
                    let within = n.is_some_and(|n| n.dot(direction) >= cos - 1e-12);
                    let entry = facing.entry(*f).or_insert(true);
                    *entry &= within;
                }
                facing
                    .into_iter()
                    .filter_map(|(f, all)| all.then_some(f))
                    .collect()
            }
        })
    }

    /// The edges `mode` selects from `pick` (decision record 0066): the edge
    /// alone ([`PickMode::Triangle`] and [`PickMode::SameDirection`]), its
    /// chain through corners where exactly two edges meet
    /// ([`PickMode::Connected`]), or its chain onward to the edge that turns
    /// least while that turn is at most the angle ([`PickMode::SmoothPatch`]).
    pub fn select_edges(&self, pick: &SceneEdgePick, mode: PickMode) -> BTreeSet<EdgeRef> {
        let Some(part) = self.part(pick.mesh) else {
            return BTreeSet::new();
        };
        let indices = match mode {
            PickMode::Triangle | PickMode::SameDirection { .. } => BTreeSet::from([pick.index]),
            PickMode::Connected => part.chain(pick.index, None),
            PickMode::SmoothPatch { max_edge_angle } => {
                part.chain(pick.index, Some(max_edge_angle))
            }
        };
        indices
            .into_iter()
            .filter_map(|i| part.edge_refs.get(i).copied())
            .collect()
    }
}

/// A scene's picker, shared between the scene view and the interface. Two
/// handles are equal when they are the same picker or pickers built from the
/// same meshes, so two builds of one scene compare equal (NFR-06).
#[derive(Clone, Default)]
pub struct Picking(Option<Arc<ScenePicker>>);

impl Picking {
    pub(crate) fn new(picker: ScenePicker) -> Self {
        Self(Some(Arc::new(picker)))
    }

    /// The picker, when the scene has anything to pick.
    pub fn picker(&self) -> Option<&ScenePicker> {
        self.0.as_deref().filter(|p| !p.is_empty())
    }
}

impl std::fmt::Debug for Picking {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(p) => p.fmt(f),
            None => f.write_str("Picking(none)"),
        }
    }
}

impl PartialEq for Picking {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a == b,
            _ => false,
        }
    }
}

/// What a click selects (UI-04).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum PickMode {
    /// The triangle clicked, alone.
    Triangle,
    /// The whole connected shell the triangle is in.
    Connected,
    /// The smooth patch around the triangle: the connected triangles reached
    /// without turning more than `max_edge_angle` degrees at an edge.
    SmoothPatch {
        /// The sharpest edge crossed, degrees, 0 to 180.
        max_edge_angle: f64,
    },
    /// Every triangle facing within `max_angle` degrees of the clicked
    /// triangle's direction, wherever it is on the part.
    SameDirection {
        /// The widest angle from the direction, degrees, 0 to 180.
        max_angle: f64,
    },
}

impl PickMode {
    /// The request that selects what `pick` stands for, to give
    /// [`Application::select_region`](crate::Application::select_region) on
    /// the pick's model; `None` when the pick selects the triangle alone (the
    /// mode is [`Triangle`](Self::Triangle), or the mode needs the
    /// triangle's direction and it has none).
    pub fn request(&self, pick: &ScenePick) -> Option<RegionRequest> {
        match *self {
            Self::Triangle => None,
            Self::Connected => Some(RegionRequest::FloodFill {
                seeds: vec![pick.triangle],
            }),
            Self::SmoothPatch { max_edge_angle } => Some(RegionRequest::FloodFillByAngle {
                seeds: vec![pick.triangle],
                max_edge_angle,
            }),
            Self::SameDirection { max_angle } => {
                pick.normal.map(|direction| RegionRequest::ByDirection {
                    direction,
                    max_angle,
                })
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use openchuck_testkit::brep::{HoleSpec, drilled_blank};
    use openchuck_testkit::parts::revolve;

    use super::*;
    use crate::scene::SceneBrep;

    const DOWN: [f64; 3] = [0.0, 0.0, -1.0];
    const REACH: PickTolerance = PickTolerance {
        base: 0.3,
        per_distance: 0.0,
    };

    /// A 10 mm blank 40 long with a 4 mm blind hole 3 deep from the top at
    /// x = 20: faces 0 (the side), 1 (the wall) and 2 (the bottom); edges 0
    /// (the rim, wall to side) and 1 (the bottom circle, wall to bottom).
    fn blank_picker() -> ScenePicker {
        let (brep, _) = drilled_blank(
            10.0,
            40.0,
            &[HoleSpec::blind(20.0, [0.0, 0.0, 1.0], 4.0, 3.0)],
        );
        let item = SceneMesh {
            role: SceneRole::Part,
            model: Some(ModelId::from_u128(1)),
            mesh: brep.joined_tessellation().0,
            feature_edges: Vec::new(),
            brep: Some(SceneBrep::of(&brep).unwrap()),
        };
        ScenePicker::build(&[item]).unwrap()
    }

    #[test]
    fn req_ui_04_a_pick_on_a_step_part_names_its_face_and_each_mode_selects_faces() {
        let picker = blank_picker();
        let side = picker.pick([5.0, 0.0, 50.0], DOWN).unwrap().unwrap();
        assert_eq!(side.face, Some(FaceId(0)));
        let bottom = picker.pick([20.0, 0.5, 50.0], DOWN).unwrap().unwrap();
        assert_eq!(bottom.face, Some(FaceId(2)));
        assert!((bottom.point.z - 7.0).abs() < 1e-9, "{bottom:?}");

        let faces = |pick: &ScenePick, mode| {
            picker
                .select_faces(pick, mode)
                .unwrap()
                .into_iter()
                .map(|f| f.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(faces(&side, PickMode::Triangle), [0]);
        assert_eq!(faces(&side, PickMode::Connected), [0, 1, 2]);
        // The bottom circle turns by 90 degrees, the rim by more (the side
        // curves away from the wall).
        let smooth = |angle| PickMode::SmoothPatch {
            max_edge_angle: angle,
        };
        assert_eq!(faces(&bottom, smooth(30.0)), [2]);
        assert_eq!(faces(&bottom, smooth(95.0)), [1, 2]);
        assert_eq!(faces(&bottom, smooth(120.0)), [0, 1, 2]);
        // Facing up within 10 degrees: only the flat bottom, all of whose
        // triangles do; the side and the wall turn away.
        assert_eq!(
            faces(&bottom, PickMode::SameDirection { max_angle: 10.0 }),
            [2]
        );
    }

    #[test]
    fn req_ui_04_an_edge_of_a_step_part_is_picked_near_the_pointer_but_not_through_the_part() {
        let picker = blank_picker();
        // Just outside the rim (radius 2 about the hole's axis).
        let rim = picker
            .pick_edge([20.0, 2.15, 50.0], DOWN, REACH)
            .unwrap()
            .unwrap();
        assert_eq!(rim.edge, EdgeRef::Brep(EdgeId(0)));
        assert_eq!(rim.model, Some(ModelId::from_u128(1)));
        assert!((rim.point.y - 2.0).abs() < 0.01, "{rim:?}");
        assert_eq!(
            picker.select_edges(&rim, PickMode::Connected),
            BTreeSet::from([EdgeRef::Brep(EdgeId(0))])
        );
        assert!(picker.edge_points(rim.mesh, rim.index).unwrap().len() > 8);

        // From the side at the bottom's height, the bottom circle is behind
        // the blank's side: the ray meets the side first, so nothing is
        // picked; without the part in the way it would be.
        let hidden = picker
            .pick_edge([20.0, -50.0, 7.1], [0.0, 1.0, 0.0], REACH)
            .unwrap();
        assert_eq!(hidden, None);
        let seen = picker
            .pick_edge([20.0, 1.9, 50.0], DOWN, REACH)
            .unwrap()
            .unwrap();
        assert_eq!(
            seen.edge,
            EdgeRef::Brep(EdgeId(0)),
            "the rim is nearer: {seen:?}"
        );
        // Far from every edge: nothing.
        assert_eq!(
            picker.pick_edge([5.0, 0.0, 50.0], DOWN, REACH).unwrap(),
            None
        );
    }

    #[test]
    fn req_ui_04_a_mesh_edge_click_selects_one_edge_its_chain_or_its_smooth_chain() {
        // A closed cylinder of 36 sides: each end is a chain of 36 feature
        // edges meeting two by two, turning 10 degrees at each corner.
        let mesh = revolve(&[(0.0, 10.0), (20.0, 10.0)], 36);
        let feature_edges = openchuck_geom::display::feature_edges(&mesh, 30.0).unwrap();
        let item = SceneMesh {
            role: SceneRole::Part,
            model: Some(ModelId::from_u128(2)),
            mesh: mesh.clone(),
            feature_edges,
            brep: None,
        };
        let picker = ScenePicker::build(&[item]).unwrap();
        // Down onto the rim at x = 20, on top.
        let pick = picker
            .pick_edge([20.0, 0.05, 50.0], DOWN, REACH)
            .unwrap()
            .unwrap();
        let EdgeRef::Mesh(edge) = pick.edge else {
            panic!("{pick:?}");
        };
        // The edge's name is a side of a triangle with both ends on the rim.
        let corners = mesh.triangles[edge.triangle as usize];
        for c in edge.corners() {
            assert!((mesh.vertices[corners[c] as usize].x - 20.0).abs() < 1e-12);
        }
        assert_eq!(picker.select_edges(&pick, PickMode::Triangle).len(), 1);
        let chain = picker.select_edges(&pick, PickMode::Connected);
        assert_eq!(chain.len(), 36);
        let smooth = |angle| PickMode::SmoothPatch {
            max_edge_angle: angle,
        };
        assert_eq!(picker.select_edges(&pick, smooth(15.0)), chain);
        assert_eq!(picker.select_edges(&pick, smooth(5.0)).len(), 1);
        // Each edge is named once, by its lowest triangle and side.
        let drawn: Vec<&[Point3]> = picker.edges_points(pick.mesh, &chain);
        assert_eq!(drawn.len(), 36);
        // A face selection needs a STEP part.
        let surface = picker.pick([10.0, 0.0, 50.0], DOWN).unwrap().unwrap();
        assert_eq!(surface.face, None);
        assert_eq!(picker.select_faces(&surface, PickMode::Connected), None);
    }
}

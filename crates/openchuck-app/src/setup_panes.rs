// SPDX-License-Identifier: AGPL-3.0-or-later
//! The view model of the Setup step's two orthographic panes (UI-03,
//! SET-01, SET-03, SET-05, SET-06, SET-09; interface specification 11.3 and
//! 7.5; slice 22 lane X5, decision record 0129): the front view from −Y and
//! the end view from +X, as plain 2D data in millimetres.
//!
//! Both are drawn in the model frame, whose X axis is the rotary axis. In
//! the front view screen right is +X and screen up is +Z; in the end view
//! screen right is +Y and screen up is +Z. The interface draws them and
//! works out nothing of its own (specification section 1).

use openchuck_project::Project;
use openchuck_types::geometry::Point3;
use openchuck_types::kinematics::{RotaryFrame, facing_at, model_up};
use openchuck_types::machine::MachineProfile;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;
use openchuck_types::units::{Degrees, Millimetres};

use crate::scene::{SceneSolid, SceneView, SolidSection, WorkholdingItem};

/// How many strips the part's side silhouette is cut into.
const SILHOUETTE_STRIPS: usize = 160;
/// Room left beside the stock at the low-X end for the axis line and the
/// safe-Z dimension, in mm.
const LEAD: f64 = 14.0;

/// A rectangle in a pane's plane, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect2 {
    /// The lowest horizontal coordinate.
    pub h0: f64,
    /// The highest horizontal coordinate.
    pub h1: f64,
    /// The lowest vertical coordinate.
    pub v0: f64,
    /// The highest vertical coordinate.
    pub v1: f64,
}

/// A dimension: a measured span with its text (the stock length, the
/// stock diameter, the axis to the safe Z, the length held).
#[derive(Debug, Clone, PartialEq)]
pub struct Dimension {
    /// One end, as (horizontal, vertical).
    pub from: [f64; 2],
    /// The other end.
    pub to: [f64; 2],
    /// The text: "Stock length 32.000".
    pub text: String,
}

/// A held region or a tab in the front view.
#[derive(Debug, Clone, PartialEq)]
pub struct FrontSolid {
    /// Which it is.
    pub item: WorkholdingItem,
    /// Its outline as (X, Z) points: a rectangle or a trapezoid.
    pub outline: Vec<[f64; 2]>,
    /// "Tab 1" for a tab; `None` for a held region, which is dimensioned.
    pub label: Option<String>,
    /// Its tooltip (7.5).
    pub description: String,
    /// Whether the draft fails a check on it: outline only, in red (7.5).
    pub failing: bool,
}

/// The front view, orthographic from −Y (11.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FrontPane {
    /// The stock as (X, Z) bounds; `None` while it cannot be drawn.
    pub stock: Option<Rect2>,
    /// The part's silhouette: X with the lowest and highest Z there.
    pub part: Vec<[f64; 3]>,
    /// The rotary axis, from and to X.
    pub axis: (f64, f64),
    /// The work zero's X.
    pub work_zero: f64,
    /// The clearance cylinder as (X, Z) bounds, with its radius.
    pub clearance: Option<(Rect2, f64)>,
    /// The safe Z above the axis.
    pub safe_z: Option<f64>,
    /// The Z+ travel limit above the axis; `None` without a machine.
    pub travel_z: Option<f64>,
    /// The held regions, cross-hatched violet bands.
    pub held: Vec<FrontSolid>,
    /// The tabs, in sand with a dark border.
    pub tabs: Vec<FrontSolid>,
    /// The default tabs offered and not added: dashed sand outlines with no
    /// fill, the first labelled "Default tab, not added" (7.5).
    pub offered: Vec<FrontSolid>,
    /// The dimensions.
    pub dimensions: Vec<Dimension>,
}

/// The stock in the end view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EndStock {
    /// A cylinder of this radius.
    Circle(f64),
    /// A box as (Y, Z) bounds.
    Box(Rect2),
}

/// A tab in the end view: a circle at its largest diameter.
#[derive(Debug, Clone, PartialEq)]
pub struct EndTab {
    /// Which tab it is.
    pub item: WorkholdingItem,
    /// The centre of the circle as (Y, Z).
    pub centre: [f64; 2],
    /// Its radius.
    pub radius: f64,
    /// Its tooltip (7.5).
    pub description: String,
    /// Whether the draft fails a check on it.
    pub failing: bool,
}

/// The end view from +X (11.3), 270 pixels wide.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EndPane {
    /// The stock.
    pub stock: Option<EndStock>,
    /// The radius of the part: its largest distance from the axis.
    pub part_radius: Option<f64>,
    /// The clearance circle's radius.
    pub clearance_radius: Option<f64>,
    /// The tabs.
    pub tabs: Vec<EndTab>,
    /// The default tabs offered and not added, drawn dashed (7.5).
    pub offered: Vec<EndTab>,
    /// The arc the A+ arrow follows, as (Y, Z) points at the arrow's radius,
    /// ending at the arrow head; empty without a machine.
    pub a_arc: Vec<[f64; 2]>,
    /// The first tab around the axis: where its angle is dimensioned, and
    /// "Start 90.00°".
    pub start_angle: Option<(f64, String)>,
}

/// Which roles the strip under the panes lists, in order (7.5, 11.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendEntry {
    /// The rotary axis, cyan dash-dot.
    RotaryAxis,
    /// The stock, blue dashed.
    Stock,
    /// The clearance and the safe Z, green dashed.
    Clearance,
    /// The Z+ travel limit, red hatched.
    TravelLimit,
    /// The Y axis pointing into the screen.
    YIntoScreen,
    /// A held region.
    HeldRegion,
    /// A tab.
    Tab,
}

impl LegendEntry {
    /// The words of the entry.
    pub const fn name(self) -> &'static str {
        match self {
            Self::RotaryAxis => "Rotary axis",
            Self::Stock => "Stock",
            Self::Clearance => "Clearance and safe Z",
            Self::TravelLimit => "Travel limit",
            Self::YIntoScreen => "Y into the screen",
            Self::HeldRegion => "Held region",
            Self::Tab => "Tab",
        }
    }
}

/// Both panes of a setup.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SetupPanes {
    /// The front view.
    pub front: FrontPane,
    /// The end view.
    pub end: EndPane,
    /// The legend strip's entries: the roles drawn, then the held region and
    /// tab when the setup has them.
    pub legend: Vec<LegendEntry>,
}

/// The side silhouette of `mesh`: for each of a fixed number of strips along
/// X, the lowest and highest Z of the mesh there, found where the triangles'
/// edges cross the strips' boundaries. Empty for a mesh with no length.
pub fn silhouette(mesh: &TriangleMesh) -> Vec<[f64; 3]> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for v in &mesh.vertices {
        lo = lo.min(v.x);
        hi = hi.max(v.x);
    }
    let width = hi - lo;
    if !(width.is_finite() && width > 1e-9) {
        return Vec::new();
    }
    let strips = SILHOUETTE_STRIPS;
    let step = width / strips as f64;
    // The lowest and highest Z met at each strip boundary.
    let mut spans = vec![(f64::INFINITY, f64::NEG_INFINITY); strips + 1];
    let mut take = |i: usize, z: f64| {
        if let Some(span) = spans.get_mut(i.min(strips)) {
            *span = (span.0.min(z), span.1.max(z));
        }
    };
    for v in &mesh.vertices {
        // A vertex between two boundaries counts for both, so the outline
        // never cuts a corner off.
        let f = (v.x - lo) / step;
        take(f.floor() as usize, v.z);
        take(f.ceil() as usize, v.z);
    }
    for t in &mesh.triangles {
        for (&from, &to) in t.iter().zip(t.iter().cycle().skip(1)) {
            let (Some(a), Some(b)) = (
                mesh.vertices.get(from as usize),
                mesh.vertices.get(to as usize),
            ) else {
                continue;
            };
            if (b.x - a.x).abs() < 1e-12 {
                continue;
            }
            let (a, b) = if a.x < b.x { (a, b) } else { (b, a) };
            let first = ((a.x - lo) / step).ceil() as usize;
            let last = (((b.x - lo) / step).floor() as usize).min(strips);
            for i in first..=last {
                let x = lo + step * i as f64;
                take(i, a.z + (x - a.x) / (b.x - a.x) * (b.z - a.z));
            }
        }
    }
    spans
        .into_iter()
        .enumerate()
        .filter(|(_, span)| span.0.is_finite())
        .map(|(i, span)| [lo + step * i as f64, span.0, span.1])
        .collect()
}

/// Both panes for the setup `view` shows, or `None` when the project has
/// no such setup. `solids` are the held region and tab solids to draw, the
/// view's or the draft's (7.5), `failing` those of the draft that fail a
/// check, and `offered` the default tabs offered and not added.
pub fn setup_panes(
    project: &Project,
    view: &SceneView,
    solids: &[SceneSolid],
    failing: &[WorkholdingItem],
    offered: &[SceneSolid],
) -> Option<SetupPanes> {
    let setup = project.setups().iter().find(|s| Some(s.id) == view.setup)?;
    let machine = project.machine();
    let stock = setup
        .stock
        .as_ref()
        .and_then(|s| s.bounds(view.part_extent.as_ref()).map(|b| (s, b)));
    let clearance = setup.clearance.or(machine.map(|m| m.clearance));

    // The front view's X range: stock, else part, else the work zero.
    let (mut x0, mut x1) = (0.0_f64, 0.0_f64);
    let mut seen = false;
    let mut widen = |lo: f64, hi: f64| {
        if seen {
            x0 = x0.min(lo);
            x1 = x1.max(hi);
        } else {
            (x0, x1, seen) = (lo, hi, true);
        }
    };
    if let Some((_, b)) = &stock {
        widen(b.min.x, b.max.x);
    }
    if let Some(p) = view.part_extent {
        widen(p.bounds.min.x, p.bounds.max.x);
    }
    let work_zero = setup.frame.work_offset.x.0;
    widen(work_zero, work_zero);
    let margin = 0.06 * (x1 - x0).max(10.0);

    let mut front = FrontPane {
        part: view.part_silhouette.clone(),
        axis: (x0 - margin - LEAD, x1 + margin),
        work_zero,
        safe_z: clearance.map(|c| c.safe_z.0),
        travel_z: machine.map(travel_limit),
        ..FrontPane::default()
    };
    let mut end = EndPane {
        part_radius: view.part_extent.map(|p| p.radius.0),
        clearance_radius: clearance.map(|c| c.radius.0),
        ..EndPane::default()
    };
    if let Some(c) = clearance {
        let r = c.radius.0;
        front.clearance = Some((
            Rect2 {
                h0: x0 - margin,
                h1: x1 + margin,
                v0: -r,
                v1: r,
            },
            r,
        ));
    }
    if let Some((s, b)) = &stock {
        front.stock = Some(Rect2 {
            h0: b.min.x,
            h1: b.max.x,
            v0: b.min.z,
            v1: b.max.z,
        });
        let outer = b.max.z.abs().max(b.min.z.abs());
        front.dimensions.push(Dimension {
            from: [
                b.min.x,
                front.clearance.map_or(-outer, |(r, _)| r.v0) - 0.08 * outer,
            ],
            to: [
                b.max.x,
                front.clearance.map_or(-outer, |(r, _)| r.v0) - 0.08 * outer,
            ],
            text: format!("Stock length {:.3}", b.max.x - b.min.x),
        });
        front.dimensions.push(Dimension {
            from: [b.max.x + 0.5 * margin, b.min.z],
            to: [b.max.x + 0.5 * margin, b.max.z],
            text: match s {
                Stock::Cylinder { diameter, .. } => format!("Ø{:.3}", diameter.0),
                _ => format!("{:.3}", b.max.z - b.min.z),
            },
        });
        end.stock = Some(match s {
            Stock::Cylinder { diameter, .. } => EndStock::Circle(diameter.0 / 2.0),
            _ => EndStock::Box(Rect2 {
                h0: b.min.y,
                h1: b.max.y,
                v0: b.min.z,
                v1: b.max.z,
            }),
        });
    }
    if let Some(z) = front.safe_z {
        front.dimensions.push(Dimension {
            from: [x0 - margin - 0.5 * LEAD, 0.0],
            to: [x0 - margin - 0.5 * LEAD, z],
            text: format!("Axis to safe Z\n{z:.3} mm"),
        });
    }
    if let Some(m) = machine {
        end.a_arc = a_arc(m, end_arrow_radius(&end));
    }

    for solid in solids {
        let failed = failing.contains(&solid.item);
        let SolidSection { x, radii, centre } = solid.section;
        let outline = section_outline(solid.section);
        match solid.item {
            WorkholdingItem::Start | WorkholdingItem::End => {
                front.dimensions.push(Dimension {
                    from: [x.0, centre.1 + radii.0.max(radii.1) + 4.0],
                    to: [x.1, centre.1 + radii.0.max(radii.1) + 4.0],
                    text: format!("Held {:.3}", x.1 - x.0),
                });
                front.held.push(FrontSolid {
                    item: solid.item,
                    outline,
                    label: None,
                    description: solid.description.clone(),
                    failing: failed,
                });
            }
            WorkholdingItem::Tab(i) => {
                let first = !front.tabs.iter().any(|t| t.item == solid.item);
                front.tabs.push(FrontSolid {
                    item: solid.item,
                    outline,
                    label: first.then(|| format!("Tab {}", i + 1)),
                    description: solid.description.clone(),
                    failing: failed,
                });
                end.tabs.push(EndTab {
                    item: solid.item,
                    centre: [centre.0, centre.1],
                    radius: radii.0.max(radii.1),
                    description: solid.description.clone(),
                    failing: failed,
                });
            }
        }
    }
    for (k, solid) in offered.iter().enumerate() {
        let SolidSection { radii, centre, .. } = solid.section;
        front.offered.push(FrontSolid {
            item: solid.item,
            outline: section_outline(solid.section),
            label: (k == 0).then(|| "Default tab, not added".to_owned()),
            description: format!("Default tab, not added \u{b7} {}", solid.description),
            failing: false,
        });
        end.offered.push(EndTab {
            item: solid.item,
            centre: [centre.0, centre.1],
            radius: radii.0.max(radii.1),
            description: solid.description.clone(),
            failing: false,
        });
    }
    // The first tab around the axis has its angle dimensioned from +Y
    // towards +Z (7.5).
    if let Some(t) = end
        .tabs
        .iter()
        .find(|t| t.centre[0].hypot(t.centre[1]) > 1e-9)
    {
        let angle = t.centre[1]
            .atan2(t.centre[0])
            .to_degrees()
            .rem_euclid(360.0);
        end.start_angle = Some((angle, format!("Start {angle:.2}°")));
    }

    let mut legend = vec![LegendEntry::RotaryAxis];
    if front.stock.is_some() {
        legend.push(LegendEntry::Stock);
    }
    if clearance.is_some() {
        legend.push(LegendEntry::Clearance);
    }
    if front.travel_z.is_some() {
        legend.push(LegendEntry::TravelLimit);
    }
    legend.push(LegendEntry::YIntoScreen);
    if !front.held.is_empty() {
        legend.push(LegendEntry::HeldRegion);
    }
    if !front.tabs.is_empty() {
        legend.push(LegendEntry::Tab);
    }
    Some(SetupPanes { front, end, legend })
}

/// A solid's outline in the front view as (X, Z) points: a rectangle, or a
/// trapezoid for a frustum.
fn section_outline(section: SolidSection) -> Vec<[f64; 2]> {
    let SolidSection { x, radii, centre } = section;
    vec![
        [x.0, centre.1 - radii.0],
        [x.1, centre.1 - radii.1],
        [x.1, centre.1 + radii.1],
        [x.0, centre.1 + radii.0],
    ]
}

/// The Z+ travel limit above the rotary axis: the machine's highest Z minus
/// the axis's Z (11.3).
fn travel_limit(profile: &MachineProfile) -> f64 {
    let axis: Point3 = RotaryFrame::of_profile(profile).on_axis(Millimetres(0.0));
    profile.travel.max.z - axis.z
}

/// The radius the A+ arrow is drawn at: a little inside the stock or part.
fn end_arrow_radius(end: &EndPane) -> f64 {
    let outer = match end.stock {
        Some(EndStock::Circle(r)) => r,
        Some(EndStock::Box(b)) => b.v1.abs().max(b.v0.abs()),
        None => end.part_radius.unwrap_or(10.0),
    };
    1.35 * outer.max(1.0)
}

/// The arc of the A+ arrow in the end view: where the part's up side faces
/// as A grows through 270 degrees, from the shared kinematics (the
/// convention the posts use), as (Y, Z) points at `radius`.
fn a_arc(profile: &MachineProfile, radius: f64) -> Vec<[f64; 2]> {
    let frame = RotaryFrame::of_profile(profile);
    let up = model_up(&frame);
    let steps = 54;
    (0..=steps)
        .map(|i| {
            let a = 270.0 * f64::from(i) / f64::from(steps);
            let d = facing_at(&frame, up, Degrees(a));
            [d.y * radius, d.z * radius]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]
    use super::*;

    fn cube(x: f64, z: f64) -> TriangleMesh {
        // A box from (0, -1, -1) to (x, 1, z) as two triangles per face.
        let v = |a: f64, b: f64, c: f64| Point3::new(a, b, c);
        let vertices = vec![
            v(0.0, -1.0, -1.0),
            v(x, -1.0, -1.0),
            v(x, 1.0, -1.0),
            v(0.0, 1.0, -1.0),
            v(0.0, -1.0, z),
            v(x, -1.0, z),
            v(x, 1.0, z),
            v(0.0, 1.0, z),
        ];
        let triangles = vec![
            [0, 1, 2],
            [0, 2, 3],
            [4, 6, 5],
            [4, 7, 6],
            [0, 5, 1],
            [0, 4, 5],
            [3, 2, 6],
            [3, 6, 7],
            [0, 3, 7],
            [0, 7, 4],
            [1, 5, 6],
            [1, 6, 2],
        ];
        TriangleMesh {
            vertices,
            triangles,
        }
    }

    #[test]
    fn req_ui_03_silhouette_of_a_box_is_its_side_outline() {
        let s = silhouette(&cube(30.0, 4.0));
        assert_eq!(s.first().map(|p| p[0]), Some(0.0));
        assert!((s.last().unwrap()[0] - 30.0).abs() < 1e-9);
        assert!(s.iter().all(|p| p[1] == -1.0 && p[2] == 4.0), "{s:?}");
    }

    #[test]
    fn req_ui_03_silhouette_follows_a_sloped_edge() {
        // A wedge whose top rises from Z 1 at X 0 to Z 5 at X 10.
        let mesh = TriangleMesh {
            vertices: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(10.0, 0.0, 0.0),
                Point3::new(10.0, 0.0, 5.0),
                Point3::new(0.0, 0.0, 1.0),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let s = silhouette(&mesh);
        let mid = s[s.len() / 2];
        assert!(
            (mid[2] - (1.0 + 4.0 * mid[0] / 10.0)).abs() < 0.1,
            "{mid:?}"
        );
        assert_eq!(mid[1], 0.0);
    }

    #[test]
    fn req_nfr_07_silhouette_of_a_degenerate_mesh_is_empty() {
        assert!(silhouette(&TriangleMesh::default()).is_empty());
        // A flat mesh (no length along X) and a triangle naming a vertex it
        // does not have draw nothing wrong.
        let flat = TriangleMesh {
            vertices: vec![Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0)],
            triangles: vec![[0, 1, 9]],
        };
        assert!(silhouette(&flat).is_empty());
        let bad = TriangleMesh {
            vertices: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(5.0, 0.0, 1.0)],
            triangles: vec![[0, 1, 9]],
        };
        assert_eq!(silhouette(&bad).len(), SILHOUETTE_STRIPS + 1);
    }
}

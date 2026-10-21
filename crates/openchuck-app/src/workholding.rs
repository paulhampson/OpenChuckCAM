// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workholding and retaining tabs in the application (SET-09, OP-21; work
//! breakdown slice 21c; decision records 0035, 0041, 0042 and 0045).
//!
//! The setup stores explicit dimensions (decision 0041). This module turns
//! them into the analytic solids of `openchuck-path` (the held regions and
//! each tab), which join the model's surface as the composite target of every
//! operation, the simulation and the deviation report; checks what needs the
//! part and the tool (tabs only in the head and tail zones, a tab not
//! narrower than the tool); computes the default tabs of decision 0035
//! (OI-158); and builds the deviation target.
//!
//! Positions are model X, which is the rotary axis; angles about the axis run
//! from +Y towards +Z, as in the setup data.

use openchuck_path::{Solid, SurfaceSource};
use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::setup::{PartExtent, SetupWarningKind};
use openchuck_types::stock::Stock;
use openchuck_types::units::Millimetres;
use openchuck_types::workholding::{
    Tab, TabLayout, TabShape, Workholding, check_tab_zones, tab_zones,
};

use crate::AppError;

/// The largest diameter of a default tab, in millimetres (decision 0035,
/// owner's answers to OI-158).
pub const DEFAULT_TAB_MAX_DIAMETER: f64 = 10.0;

/// The large end of a default frustum is this many times its small end
/// (decision 0045 item 3), or the stock's diameter if that is less.
pub const DEFAULT_TAB_END_RATIO: f64 = 2.0;

/// Triangles within this distance of a solid's surface count as on it when
/// the deviation target drops the faces inside the union (millimetres).
const SURFACE_TOLERANCE: f64 = 1e-6;

/// Segments around each solid in the deviation target (four triangles each).
const TARGET_SEGMENTS: u32 = 96;

fn refuse(message: impl Into<String>) -> AppError {
    AppError::Engine(openchuck_types::Error::invalid("workholding", message))
}

/// The analytic solids of `workholding`, in a fixed order: the held region at
/// the start, the one at the end, then each tab (those around the axis by
/// ascending angle) (SET-09, OP-21).
///
/// A held region spans the gripped length (SET-09; decision 0160). Where a
/// holding diameter is given it is a cylinder of that diameter about the
/// rotary axis, on any stock (on stock that is not round it must fit the
/// stock's smaller side). Without one it is the stock's own section: a
/// cylinder of the stock's diameter on round stock, else the box of its
/// section (box and bounding-box stock exactly, imported stock its bounding
/// box, which holds its outline). Fails with a message naming what to change
/// when a dimension cannot make a solid.
pub fn solids(
    workholding: &Workholding,
    stock: Option<&Stock>,
    part: &PartExtent,
) -> Result<Vec<Solid>, AppError> {
    Ok(labelled_solids(workholding, stock, part)?
        .into_iter()
        .map(|(_, solid)| solid)
        .collect())
}

/// [`solids`], each with a name for messages ("the held region at the start
/// of the stock", "tab 2").
pub fn labelled_solids(
    workholding: &Workholding,
    stock: Option<&Stock>,
    part: &PartExtent,
) -> Result<Vec<(String, Solid)>, AppError> {
    if workholding.is_empty() {
        return Ok(Vec::new());
    }
    // Automatic tabs where the part, the stock and the held regions put them
    // (decision record 0202).
    let resolved = workholding.resolved(stock, Some(part));
    let workholding = &resolved;
    let stock = stock.ok_or_else(|| refuse("the setup has held regions or tabs but no stock"))?;
    let bounds = stock.bounds(Some(part)).ok_or_else(|| {
        refuse("the stock's extent is not known, so held regions and tabs cannot be placed")
    })?;
    let axis = UnitVec3::X;
    let mut out = Vec::new();
    let (start, end) = workholding.held_intervals((bounds.min.x, bounds.max.x));
    for (name, region, interval) in [
        ("start", &workholding.start, start),
        ("end", &workholding.end, end),
    ] {
        let (Some(region), Some((lo, hi))) = (region, interval) else {
            continue;
        };
        let solid = match (region.diameter, stock) {
            (Some(d), Stock::Cylinder { .. }) => {
                Solid::cylinder(Point3::new(lo, 0.0, 0.0), axis, hi - lo, d.0 / 2.0)
            }
            (None, Stock::Cylinder { diameter, .. }) => {
                Solid::cylinder(Point3::new(lo, 0.0, 0.0), axis, hi - lo, diameter.0 / 2.0)
            }
            (Some(d), _) => {
                let inscribed = (bounds.max.y - bounds.min.y).min(bounds.max.z - bounds.min.z);
                if d.0 > inscribed + 1e-9 {
                    return Err(refuse(format!(
                        "the holding diameter {:.3} mm at the {name} of the stock is wider than \
                         the stock's smaller side ({inscribed:.3} mm)",
                        d.0
                    )));
                }
                Solid::cylinder(Point3::new(lo, 0.0, 0.0), axis, hi - lo, d.0 / 2.0)
            }
            (None, _) => Solid::cuboid(openchuck_types::geometry::Aabb {
                min: Point3::new(lo, bounds.min.y, bounds.min.z),
                max: Point3::new(hi, bounds.max.y, bounds.max.z),
            }),
        };
        out.push((
            format!("the held region at the {name} of the stock"),
            solid.map_err(AppError::from)?,
        ));
    }
    for (i, tab) in workholding.tabs.iter().enumerate() {
        let (r0, r1) = match tab.shape {
            TabShape::Cylinder { diameter } => (diameter.0 / 2.0, diameter.0 / 2.0),
            TabShape::Frustum {
                start_diameter,
                end_diameter,
            } => (start_diameter.0 / 2.0, end_diameter.0 / 2.0),
            _ => {
                return Err(refuse(format!(
                    "tab {} has a shape this version lacks",
                    i + 1
                )));
            }
        };
        let centres: Vec<(f64, f64)> = match tab.layout {
            TabLayout::OnAxis => vec![(0.0, 0.0)],
            TabLayout::AroundAxis {
                count,
                start_angle,
                radial_offset,
            } => (0..count)
                .map(|k| {
                    let angle =
                        (start_angle.0 + 360.0 * f64::from(k) / f64::from(count)).to_radians();
                    (radial_offset.0 * angle.cos(), radial_offset.0 * angle.sin())
                })
                .collect(),
            _ => {
                return Err(refuse(format!(
                    "tab {} has a layout this version lacks",
                    i + 1
                )));
            }
        };
        for (y, z) in centres {
            out.push((
                format!("tab {}", i + 1),
                Solid::frustum(Point3::new(tab.x_start.0, y, z), axis, tab.length.0, r0, r1)
                    .map_err(AppError::from)?,
            ));
        }
    }
    Ok(out)
}

/// Notes for the user about an operation's selection sets (the ones it cuts
/// in) that reach a held region or a tab: a triangle with a corner inside or
/// on one. Those are never cut, so the selection cuts less than it names
/// (SET-09, OP-21; UI-03). One note per set and solid, in set order.
pub fn selection_reach_notes(
    project: &openchuck_project::Project,
    operation: &openchuck_project::Operation,
    mesh: &TriangleMesh,
    solids: &[(String, Solid)],
) -> Vec<String> {
    use openchuck_project::SelectionTarget;
    let mut notes = Vec::new();
    for id in &operation.selections {
        let Some(set) = project.selection_sets().iter().find(|s| s.id == *id) else {
            continue;
        };
        let SelectionTarget::Triangles(triangles) = &set.target else {
            continue;
        };
        for (name, solid) in solids {
            let reaches = triangles.iter().any(|&t| {
                usize::try_from(t)
                    .ok()
                    .and_then(|t| mesh.triangle(t))
                    .is_some_and(|corners| corners.iter().any(|&p| solid.contains(p)))
            });
            if reaches {
                // A tab around the axis is several solids with one name.
                let note = format!(
                    "The selection '{}' reaches {name}, which is never machined. This operation \
                     leaves it uncut there.",
                    set.name
                );
                if !notes.contains(&note) {
                    notes.push(note);
                }
            }
        }
    }
    notes
}

/// The note for the first tab narrower than a tool of `tool_diameter`, naming
/// the tool and the operation that uses it, or `None` (OI-159; interface
/// specification 11.7).
fn narrow_tab_note(
    workholding: &Workholding,
    tool_diameter: f64,
    tool_name: &str,
    operation: &str,
) -> Option<WorkholdingFinding> {
    workholding.tabs.iter().enumerate().find_map(|(i, tab)| {
        let d = narrowest(tab);
        (d < tool_diameter - 1e-9).then(|| WorkholdingFinding {
            tab: Some(i),
            text: format!(
                "Tab {} is {d:.3} mm across at its narrowest, narrower than the tool \
                 {tool_name} ({tool_diameter:.3} mm) of {operation}. Make the tab wider or use \
                 a smaller tool.",
                i + 1
            ),
        })
    })
}

/// A finding of a workholding check against the part and the tools, and the
/// tab it concerns, so the section can put it on that tab's row (interface
/// specification 11.7; OI-334 i).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkholdingFinding {
    /// The tab, from zero; `None` for a finding about the whole setup.
    pub tab: Option<usize>,
    /// The sentence, naming the tab by its number from 1.
    pub text: String,
}

impl std::fmt::Display for WorkholdingFinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

/// An operation as the messages name it: its number in machining order and
/// its name, "1 Rough" (interface specification 11.7).
pub fn operation_label(
    project: &openchuck_project::Project,
    operation: &openchuck_project::Operation,
) -> String {
    match project
        .operations()
        .iter()
        .position(|o| o.id == operation.id)
    {
        Some(i) => format!("{} {}", i + 1, operation.name),
        None => operation.name.clone(),
    }
}

/// A note for each operation of `setup` whose tool is wider than a tab of the
/// setup (OI-159). Operations without a tool, or with a profile that cannot be
/// built, are skipped; generation reports those.
pub(crate) fn narrow_tab_notes(
    project: &openchuck_project::Project,
    setup: &openchuck_project::Setup,
) -> Vec<String> {
    narrow_tab_findings(project, setup)
        .into_iter()
        .map(|f| f.text)
        .collect()
}

/// A tool of one of a setup's operations, as the workholding section compares
/// tabs with it (OI-159; interface specification 11.7).
#[derive(Debug, Clone, PartialEq)]
pub struct SetupTool {
    /// The tool with its controller number: "T2 Flat Ø6".
    pub tool: String,
    /// Its cutting diameter.
    pub diameter: f64,
    /// The operation that uses it: "1 Rough".
    pub operation: String,
}

/// The tools of `setup`'s operations, in machining order. Operations without
/// a tool, or with a profile that cannot be built, are skipped; generation
/// reports those.
pub fn setup_tools(
    project: &openchuck_project::Project,
    setup: openchuck_project::id::SetupId,
) -> Vec<SetupTool> {
    project
        .operations()
        .iter()
        .filter(|o| o.setup == setup)
        .filter_map(|operation| {
            let snapshot = operation
                .tool
                .and_then(|id| project.tools().iter().find(|t| t.id == id))?;
            let profile = snapshot.tool.profile().ok()?;
            Some(SetupTool {
                tool: format!("T{} {}", snapshot.controller.number, snapshot.name),
                diameter: 2.0 * profile.cutting_radius(),
                operation: operation_label(project, operation),
            })
        })
        .collect()
}

/// [`narrow_tab_notes`] with the tab each concerns.
fn narrow_tab_findings(
    project: &openchuck_project::Project,
    setup: &openchuck_project::Setup,
) -> Vec<WorkholdingFinding> {
    setup_tools(project, setup.id)
        .into_iter()
        .filter_map(|t| narrow_tab_note(&setup.workholding, t.diameter, &t.tool, &t.operation))
        .collect()
}

/// What is wrong with the workholding of `setup` as it would be stored: tabs
/// outside the head and tail zones (OI-180), then tabs narrower than a tool of
/// its operations (OI-159). Reads and places the part for its extent; empty
/// when the setup has no part.
pub(crate) fn findings_of(
    root: &std::path::Path,
    project: &openchuck_project::Project,
    setup: &openchuck_project::Setup,
    context: &openchuck_types::job::JobContext<'_>,
) -> Result<Vec<WorkholdingFinding>, AppError> {
    let extent = match setup
        .part
        .and_then(|id| project.models().iter().find(|m| m.id == id))
    {
        Some(model) => {
            let mesh = crate::model::setup_mesh(root, model, setup, context)?;
            openchuck_geom::part_extent(&mesh)
        }
        None => return Ok(Vec::new()),
    };
    let mut found: Vec<WorkholdingFinding> =
        check_tab_zones(&setup.workholding, setup.stock.as_ref(), extent.as_ref())
            .into_iter()
            .map(|kind| match kind {
                SetupWarningKind::TabOutsideEndZones { tab, x } => WorkholdingFinding {
                    tab: tab.checked_sub(1),
                    text: format!(
                        "Tab {tab} (X {:.3} to {:.3} mm) is not wholly in the head or the tail \
                         zone. Tabs lie between the held region (or the end of the stock) and \
                         the part, or between the part and the other end.",
                        x.0, x.1
                    ),
                },
                other => WorkholdingFinding {
                    tab: None,
                    text: openchuck_types::setup::SetupWarning {
                        kind: other,
                        unconfirmed: Vec::new(),
                    }
                    .to_string(),
                },
            })
            .collect();
    found.extend(narrow_tab_findings(project, setup));
    Ok(found)
}

/// [`selection_reach_notes`] for `operation` of `project`, reading and placing
/// its part.
pub(crate) fn reach_warnings_of(
    root: &std::path::Path,
    project: &openchuck_project::Project,
    operation: openchuck_project::id::OperationId,
    context: &openchuck_types::job::JobContext<'_>,
) -> Result<Vec<String>, AppError> {
    let operation = project
        .operation(operation)
        .ok_or_else(|| AppError::UnknownOperation(operation.to_string()))?;
    let Some(setup) = project.setup(operation.setup) else {
        return Ok(Vec::new());
    };
    if setup.workholding.is_empty() || operation.selections.is_empty() {
        return Ok(Vec::new());
    }
    let Some(model) = setup
        .part
        .and_then(|id| project.models().iter().find(|m| m.id == id))
    else {
        return Ok(Vec::new());
    };
    let mesh = crate::model::setup_mesh(root, model, setup, context)?;
    let part = openchuck_geom::part_extent(&mesh)
        .ok_or_else(|| refuse("the setup's part has no extent"))?;
    let solids = labelled_solids(&setup.workholding, setup.stock.as_ref(), &part)?;
    Ok(selection_reach_notes(project, operation, &mesh, &solids))
}

/// The smallest diameter of a tab's shape.
fn narrowest(tab: &Tab) -> f64 {
    match tab.shape {
        TabShape::Cylinder { diameter } => diameter.0,
        TabShape::Frustum {
            start_diameter,
            end_diameter,
        } => start_diameter.0.min(end_diameter.0),
        _ => f64::INFINITY,
    }
}

/// The checks that need the part and the tool, made before generating
/// (decision 0035, owner's answers; OI-159 and OI-180): every tab lies wholly
/// in the head zone or the tail zone, and none is narrower than the tool.
/// The message names the tab and what to change.
pub fn check_for_tool(
    workholding: &Workholding,
    stock: Option<&Stock>,
    part: &PartExtent,
    tool_diameter: f64,
    tool_name: &str,
) -> Result<(), AppError> {
    if let Some(SetupWarningKind::TabOutsideEndZones { tab, x }) =
        check_tab_zones(workholding, stock, Some(part))
            .into_iter()
            .next()
    {
        return Err(refuse(format!(
            "tab {tab} (X {:.3} to {:.3} mm) is not wholly in the head or the tail zone; tabs \
             lie between the held region (or the end of the stock) and the part, or between the \
             part and the other held region (or end of the stock)",
            x.0, x.1
        )));
    }
    for (i, tab) in workholding.tabs.iter().enumerate() {
        let d = narrowest(tab);
        if d < tool_diameter - 1e-9 {
            return Err(refuse(format!(
                "tab {} is {d:.3} mm across at its narrowest, narrower than the tool \
                 '{tool_name}' ({tool_diameter:.3} mm); make the tab wider or use a smaller tool",
                i + 1
            )));
        }
    }
    Ok(())
}

/// The radius of the part's section by the plane at `x`: the farthest from
/// the axis of the points where the mesh's edges cross the plane. Zero when
/// the part ends in a point there.
fn local_radius(mesh: &TriangleMesh, x: f64) -> f64 {
    let mut radius: f64 = 0.0;
    for t in 0..mesh.triangles.len() {
        let Some(corners) = mesh.triangle(t) else {
            continue;
        };
        for k in 0..3 {
            let (Some(a), Some(b)) = (corners.get(k), corners.get((k + 1) % 3)) else {
                continue;
            };
            if (a.x - x) * (b.x - x) > 0.0 {
                continue;
            }
            if (b.x - a.x).abs() <= f64::EPSILON {
                // An edge in the plane: both ends are on the section.
                radius = radius.max(a.y.hypot(a.z)).max(b.y.hypot(b.z));
            } else {
                let f = (x - a.x) / (b.x - a.x);
                let (y, z) = (a.y + f * (b.y - a.y), a.z + f * (b.z - a.z));
                radius = radius.max(y.hypot(z));
            }
        }
    }
    radius
}

/// The default tab of one zone, or why there is none (interface
/// specification 11.7; OI-334 e).
#[derive(Debug, Clone, PartialEq)]
pub enum ZoneDefault {
    /// The zone's default tab.
    Tab(Tab),
    /// The zone has no length: the held region (or the end of the stock)
    /// reaches the part.
    NoRoom,
    /// The part ends in a point at `x`, so there is no diameter to base a
    /// tab on.
    Point {
        /// Model X of the point.
        x: f64,
    },
    /// The stock leaves no room for a frustum of the tab's size.
    NoFit {
        /// The stock's diameter.
        stock_diameter: f64,
        /// The tab's small end.
        small: f64,
        /// Where the tab meets the part.
        x: f64,
    },
}

/// The default tab of each zone, head then tail, for the held regions of
/// `workholding` (its tabs are ignored), or `None` when the stock's extent is
/// not known (decision 0035, owner's answers; decision 0045 item 3).
///
/// Each runs the whole length of its zone, from the held region's inner edge
/// (or the end of the stock) to the part at the head and from the part to the
/// other held region (or the end of the stock) at the tail. The small end is
/// the smaller of 10 mm and half the part's diameter where the tab meets it;
/// the large end is [`DEFAULT_TAB_END_RATIO`] times that, at most the stock's
/// diameter. The part never lies in the zones, so a default can never enter
/// an avoid selection set (OP-02), which holds triangles of the part.
pub fn zone_defaults(
    workholding: &Workholding,
    stock: &Stock,
    mesh: &TriangleMesh,
    part: &PartExtent,
) -> Option<[ZoneDefault; 2]> {
    let (head, tail) = tab_zones(workholding, stock, part)?;
    let stock_diameter = stock
        .radius(Some(part))
        .map_or(f64::INFINITY, |r| 2.0 * r.0);
    let one = |is_head: bool, (lo, hi): (f64, f64)| {
        if hi - lo <= 1e-6 {
            return ZoneDefault::NoRoom;
        }
        let at = if is_head { hi } else { lo };
        let small = DEFAULT_TAB_MAX_DIAMETER.min(local_radius(mesh, at));
        if small <= 1e-6 {
            return ZoneDefault::Point { x: at };
        }
        let large = (small * DEFAULT_TAB_END_RATIO).min(stock_diameter);
        if large - small < openchuck_types::workholding::FRUSTUM_MIN_DIFFERENCE {
            return ZoneDefault::NoFit {
                stock_diameter,
                small,
                x: at,
            };
        }
        let (start_diameter, end_diameter) = if is_head {
            (large, small)
        } else {
            (small, large)
        };
        ZoneDefault::Tab(Tab {
            auto: None,
            x_start: Millimetres(lo),
            length: Millimetres(hi - lo),
            shape: TabShape::Frustum {
                start_diameter: Millimetres(start_diameter),
                end_diameter: Millimetres(end_diameter),
            },
            layout: TabLayout::OnAxis,
        })
    };
    Some([one(true, head), one(false, tail)])
}

/// The default tabs of decision 0035 (OI-158; owner's answers), as explicit
/// data to add to the setup: one in the head zone and one in the tail zone,
/// frustums with the small end on the part ([`zone_defaults`]). A zone with
/// no length (a held region reaching the part, or the stock ending at it)
/// gets none. Fails, saying so, when the setup already has tabs, the part
/// ends in a point, or the tab would not fit the stock.
pub fn default_tabs(
    workholding: &Workholding,
    stock: &Stock,
    mesh: &TriangleMesh,
    part: &PartExtent,
) -> Result<Vec<Tab>, AppError> {
    if !workholding.tabs.is_empty() {
        return Err(refuse(
            "the setup already has tabs; remove them before adding the defaults",
        ));
    }
    let zones = zone_defaults(workholding, stock, mesh, part).ok_or_else(|| {
        refuse("the stock's extent is not known, so default tabs cannot be placed")
    })?;
    let mut tabs = Vec::new();
    for zone in zones {
        match zone {
            ZoneDefault::Tab(tab) => tabs.push(tab),
            ZoneDefault::NoRoom => {}
            ZoneDefault::Point { x } => {
                return Err(refuse(format!(
                    "the part comes to a point at X {x:.3} mm, so there is no diameter to base a \
                     default tab on; add the tab by hand"
                )));
            }
            ZoneDefault::NoFit {
                stock_diameter,
                small,
                x,
            } => {
                return Err(refuse(format!(
                    "the stock ({stock_diameter:.3} mm across) leaves no room for a tab of \
                     {small:.3} mm at X {x:.3} mm; add the tab by hand"
                )));
            }
        }
    }
    Ok(tabs)
}

/// What the workholding section needs from a setup's placed part (interface
/// specification 11.7; decision record 0114 section C, OI-334 c, d and e): its
/// extent, for the head and tail zones, the end of a frustum that faces the
/// part and the checks, and the default tab of each zone for the stored held
/// regions.
#[derive(Debug, Clone, PartialEq)]
pub struct PartFacts {
    /// The placed part's extent.
    pub extent: PartExtent,
    /// The default tab of the head and the tail zone, or why there is none;
    /// `None` when the setup has no stock.
    pub defaults: Option<[ZoneDefault; 2]>,
}

/// [`PartFacts`] of `setup` of `project`, reading and placing its part.
pub(crate) fn part_facts_of(
    root: &std::path::Path,
    project: &openchuck_project::Project,
    setup: openchuck_project::id::SetupId,
    context: &openchuck_types::job::JobContext<'_>,
) -> Result<PartFacts, AppError> {
    let setup = project
        .setup(setup)
        .ok_or_else(|| AppError::UnknownSetup(setup.to_string()))?;
    let model = setup
        .part
        .and_then(|id| project.models().iter().find(|m| m.id == id))
        .ok_or_else(|| refuse("the setup has no part"))?;
    let mesh = crate::model::setup_mesh(root, model, setup, context)?;
    let extent = openchuck_geom::part_extent(&mesh)
        .ok_or_else(|| refuse("the setup's part has no extent"))?;
    let defaults = setup
        .stock
        .as_ref()
        .and_then(|stock| zone_defaults(&setup.workholding, stock, &mesh, &extent));
    Ok(PartFacts { extent, defaults })
}

/// [`default_tabs`] for `setup` of `project`, reading and placing its part.
pub(crate) fn default_tabs_of(
    root: &std::path::Path,
    project: &openchuck_project::Project,
    setup: openchuck_project::id::SetupId,
    context: &openchuck_types::job::JobContext<'_>,
) -> Result<Vec<Tab>, AppError> {
    let setup = project
        .setup(setup)
        .ok_or_else(|| AppError::UnknownSetup(setup.to_string()))?;
    let stock = setup
        .stock
        .as_ref()
        .ok_or_else(|| refuse("the setup has no stock"))?;
    let model = setup
        .part
        .and_then(|id| project.models().iter().find(|m| m.id == id))
        .ok_or_else(|| refuse("the setup has no part"))?;
    let mesh = crate::model::setup_mesh(root, model, setup, context)?;
    let part = openchuck_geom::part_extent(&mesh)
        .ok_or_else(|| refuse("the setup's part has no extent"))?;
    default_tabs(&setup.workholding, stock, &mesh, &part)
}

/// The comparison target of the deviation report (SIM-05): the model's
/// triangles, then the triangles of each solid, with every triangle that lies
/// inside the union of the parts, where two of them meet or overlap, turned
/// into a degenerate one, so it takes no samples. A held region or tab then
/// counts as part of the finished surface only where it shows, and the
/// model's triangles keep their indices (the deviation colours of the part
/// read the first `model.triangles.len()` entries).
///
/// A triangle is inside when its centroid is inside another part, or on its
/// surface with the other surface facing it. Decided at the centroid, which
/// is exact for a plane or a cylinder wall met at its edge and approximate
/// where a face is partly covered (decision 0045 item 4).
pub fn deviation_target(
    model: &TriangleMesh,
    model_surface: &dyn SurfaceSource,
    solids: &[Solid],
) -> TriangleMesh {
    let mut target = model.clone();
    let model_count = target.triangles.len();
    for solid in solids {
        let offset = u32::try_from(target.vertices.len()).unwrap_or(u32::MAX);
        let part = solid.mesh(TARGET_SEGMENTS);
        target.vertices.extend(part.vertices);
        target.triangles.extend(
            part.triangles
                .iter()
                .map(|t| t.map(|i| i.saturating_add(offset))),
        );
    }
    for t in 0..target.triangles.len() {
        let Some([a, b, c]) = target.triangle(t) else {
            continue;
        };
        let cross = (b - a).cross(c - a);
        let length = cross.length();
        if length.is_nan() || length <= 1e-18 {
            continue;
        }
        let normal = cross * (1.0 / length);
        let centroid = Point3::new(
            (a.x + b.x + c.x) / 3.0,
            (a.y + b.y + c.y) / 3.0,
            (a.z + b.z + c.z) / 3.0,
        );
        // Which solid the triangle belongs to, so it is not tested against
        // itself.
        let own = t
            .checked_sub(model_count)
            .map(|i| i / (4 * TARGET_SEGMENTS as usize));
        let inside_solid = solids.iter().enumerate().any(|(i, s)| {
            own != Some(i) && inside(s.contains(centroid), s.project(centroid), centroid, normal)
        });
        let inside_model = own.is_some()
            && model_surface.project(centroid).is_some_and(|p| {
                if p.point.distance(centroid) <= SURFACE_TOLERANCE {
                    p.normal.get().dot(normal) < -0.5
                } else {
                    // Behind the nearest point's outward normal: inside.
                    (centroid - p.point).dot(p.normal.get()) < 0.0
                }
            });
        if (inside_solid || inside_model)
            && let Some(slot) = target.triangles.get_mut(t)
        {
            let v = slot[0];
            *slot = [v, v, v];
        }
    }
    target
}

/// The target the deviation report compares the stock with for `setup`: the
/// placed part's mesh as it is when the setup holds nothing, else
/// [`deviation_target`] over the part and the setup's solids. The part's own
/// triangles come first and keep their indices either way.
pub(crate) fn setup_deviation_target(
    setup: &openchuck_project::Setup,
    mesh: TriangleMesh,
) -> Result<TriangleMesh, AppError> {
    if setup.workholding.is_empty() {
        return Ok(mesh);
    }
    let part = openchuck_geom::part_extent(&mesh)
        .ok_or_else(|| refuse("the setup's part has no extent"))?;
    let solids = solids(&setup.workholding, setup.stock.as_ref(), &part)?;
    let surface = openchuck_path::MeshSurface::new(&mesh)?;
    Ok(deviation_target(&mesh, &surface, &solids))
}

/// Whether a centroid is inside a part: strictly inside, or on the surface
/// with that surface's outward normal facing the triangle's own.
fn inside(
    contained: bool,
    nearest: Option<openchuck_path::surface::SurfacePoint>,
    centroid: Point3,
    normal: openchuck_types::geometry::Vec3,
) -> bool {
    if !contained {
        return false;
    }
    let Some(nearest) = nearest else {
        return true;
    };
    if nearest.point.distance(centroid) > SURFACE_TOLERANCE {
        return true;
    }
    nearest.normal.get().dot(normal) < -0.5
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;
    use openchuck_types::geometry::Aabb;
    use openchuck_types::workholding::HeldRegion;

    fn part(lo: f64, hi: f64) -> PartExtent {
        PartExtent {
            bounds: Aabb {
                min: Point3::new(lo, -8.0, -8.0),
                max: Point3::new(hi, 8.0, 8.0),
            },
            radius: Millimetres(8.0),
        }
    }

    fn stock() -> Stock {
        Stock::Cylinder {
            diameter: Millimetres(30.0),
            length: Millimetres(100.0),
            x_start: Millimetres(0.0),
        }
    }

    #[test]
    fn req_set_09_req_op_21_solids_follow_the_setup() {
        let w = Workholding {
            start: Some(HeldRegion {
                length: Millimetres(15.0),
                diameter: Some(Millimetres(20.0)),
            }),
            end: None,
            tabs: vec![
                Tab {
                    auto: None,
                    x_start: Millimetres(80.0),
                    length: Millimetres(6.0),
                    shape: TabShape::Cylinder {
                        diameter: Millimetres(6.0),
                    },
                    layout: TabLayout::OnAxis,
                },
                Tab {
                    auto: None,
                    x_start: Millimetres(90.0),
                    length: Millimetres(5.0),
                    shape: TabShape::Frustum {
                        start_diameter: Millimetres(4.0),
                        end_diameter: Millimetres(2.0),
                    },
                    layout: TabLayout::AroundAxis {
                        count: 3,
                        start_angle: openchuck_types::units::Degrees(90.0),
                        radial_offset: Millimetres(8.0),
                    },
                },
            ],
        };
        let solids = solids(&w, Some(&stock()), &part(20.0, 78.0)).unwrap();
        // One held region, one tab on the axis, three tabs around it.
        assert_eq!(solids.len(), 5);
        assert!((solids[0].volume() - std::f64::consts::PI * 100.0 * 15.0).abs() < 1e-9);
        // The first tab around the axis is at 90 degrees: +Z.
        let c = solids[2].base();
        assert!(c.y.abs() < 1e-9 && (c.z - 8.0).abs() < 1e-9, "{c:?}");
        assert!(Workholding::default().is_empty());
        assert!(
            super::solids(&Workholding::default(), None, &part(0.0, 1.0))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn req_set_09_a_held_region_is_its_holding_diameter_else_the_stock_section() {
        // The owner's rule (decision 0160): a holding diameter gives a
        // cylinder on any stock; without one the held region is the stock's
        // own section over the gripped length.
        let boxed = Stock::Box {
            bounds: Aabb {
                min: Point3::new(0.0, -15.0, -12.0),
                max: Point3::new(100.0, 15.0, 12.0),
            },
        };
        let w = Workholding {
            start: Some(HeldRegion {
                length: Millimetres(5.0),
                diameter: Some(Millimetres(20.0)),
            }),
            end: Some(HeldRegion {
                length: Millimetres(8.0),
                diameter: None,
            }),
            ..Workholding::default()
        };
        let held = solids(&w, Some(&boxed), &part(10.0, 90.0)).unwrap();
        assert_eq!(held[0].as_cuboid(), None);
        assert!((held[0].base_radius() - 10.0).abs() < 1e-12);
        assert_eq!((held[0].base().x, held[0].top().x), (0.0, 5.0));
        assert_eq!(
            held[1].as_cuboid(),
            Some(Aabb {
                min: Point3::new(92.0, -15.0, -12.0),
                max: Point3::new(100.0, 15.0, 12.0),
            })
        );
        // A holding diameter wider than the box's smaller side is refused.
        let wide = Workholding {
            start: Some(HeldRegion {
                length: Millimetres(5.0),
                diameter: Some(Millimetres(25.0)),
            }),
            ..Workholding::default()
        };
        let error = solids(&wide, Some(&boxed), &part(10.0, 90.0)).unwrap_err();
        assert!(error.to_string().contains("smaller side"), "{error}");
        // Round stock: the holding diameter, else the stock's diameter.
        let round = solids(&w, Some(&stock()), &part(10.0, 90.0)).unwrap();
        assert_eq!(round[0].as_cuboid(), None);
        assert!((round[0].base_radius() - 10.0).abs() < 1e-12);
        assert_eq!(round[1].as_cuboid(), None);
    }

    #[test]
    fn req_op_21_a_tab_narrower_than_the_tool_is_refused() {
        let w = Workholding {
            tabs: vec![Tab {
                auto: None,
                x_start: Millimetres(80.0),
                length: Millimetres(10.0),
                shape: TabShape::Frustum {
                    start_diameter: Millimetres(3.0),
                    end_diameter: Millimetres(9.0),
                },
                layout: TabLayout::OnAxis,
            }],
            ..Workholding::default()
        };
        let part = part(20.0, 78.0);
        assert!(check_for_tool(&w, Some(&stock()), &part, 3.0, "a").is_ok());
        let error = check_for_tool(&w, Some(&stock()), &part, 4.0, "ball 4").unwrap_err();
        assert!(
            error.to_string().contains("narrower than the tool"),
            "{error}"
        );
    }

    #[test]
    fn req_op_21_a_tab_across_the_part_is_refused() {
        let w = Workholding {
            tabs: vec![Tab {
                auto: None,
                x_start: Millimetres(40.0),
                length: Millimetres(5.0),
                shape: TabShape::Cylinder {
                    diameter: Millimetres(6.0),
                },
                layout: TabLayout::OnAxis,
            }],
            ..Workholding::default()
        };
        let error = check_for_tool(&w, Some(&stock()), &part(20.0, 78.0), 1.0, "a").unwrap_err();
        assert!(
            error.to_string().contains("head or the tail zone"),
            "{error}"
        );
    }
}

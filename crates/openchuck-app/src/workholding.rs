// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workholding and retaining tabs in the application (SET-09, OP-21; work
//! breakdown slice 21c; decision records 0035, 0041, 0042 and 0045).
//!
//! The setup stores explicit dimensions (decision 0041). This module turns
//! them into the analytic solids of `openchuck-path` (the held regions and
//! each tab), which join the model's surface as the composite target of every
//! operation, the simulation and the deviation report; checks what needs the
//! part and the tool (tabs only in the head and tail zones, a tab not
//! shorter along X than the tool is wide, decision 0494); computes the default tabs of decision 0035
//! (OI-158); and builds the deviation target.
//!
//! Positions are model X, which is the rotary axis; angles about the axis run
//! from +Y towards +Z, as in the setup data.

use openchuck_path::{Solid, SurfaceSource};
use openchuck_sim::SampleOn;
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::setup::{PartExtent, SetupWarningKind};
use openchuck_types::stock::Stock;
use openchuck_types::units::Millimetres;
use openchuck_types::workholding::{
    Tab, TabLayout, TabShape, Workholding, check_tab_zones, tab_zones,
};

use crate::AppError;
use crate::operation::HELD_CLEARANCE;

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

/// The note for the first tab shorter along X than a tool of `tool_diameter`
/// is wide, naming the tool and the operation that uses it, or `None`
/// (OI-159; decision 0494; interface specification 11.7). The tabs of `skip`,
/// by their place in the list, are left out: automatic tabs whose stored
/// length is not where they are, because the stock or the part is unknown or
/// the end has no room for them (decision 0202 item 3).
fn narrow_tab_note(
    workholding: &Workholding,
    tool_diameter: f64,
    tool_name: &str,
    operation: &str,
    skip: &[usize],
) -> Option<WorkholdingFinding> {
    workholding.tabs.iter().enumerate().find_map(|(i, tab)| {
        let d = tab.length.0;
        let counts = !skip.contains(&i);
        (counts && d < tool_diameter - 1e-9).then(|| WorkholdingFinding {
            tab: Some(i),
            refuses: false,
            text: format!(
                "Tab {} is {d:.3} mm long along X, shorter than the tool \
                 {tool_name} ({tool_diameter:.3} mm) of {operation}. Make the tab longer or use \
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
    /// True when Apply stores nothing while it stands (a tab outside the end
    /// zones); false for one that is a warning only, stored with the
    /// workholding: a tab shorter than a tool (decision 0523, the owner's
    /// answer of 6 Nov 2026).
    pub refuses: bool,
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
    part: Option<&PartExtent>,
) -> Vec<String> {
    narrow_tab_findings(project, setup, part)
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
    part: Option<&PartExtent>,
) -> Vec<WorkholdingFinding> {
    narrow_findings(
        &setup.workholding,
        setup.stock.as_ref(),
        part,
        &setup_tools(project, setup.id),
    )
}

/// The automatic tabs of `workholding` whose stored length says nothing about
/// where they are, by their place in the list: all of them while the stock or
/// the part is unknown, else those that find no room at their end (listed by
/// [`Workholding::auto_tabs_without_room`]; only a warning of the zone check).
fn unresolved_tabs(
    workholding: &Workholding,
    stock: Option<&Stock>,
    part: Option<&PartExtent>,
) -> Vec<usize> {
    if stock.is_none() || part.is_none() {
        return workholding
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.auto.map(|_| i))
            .collect();
    }
    workholding
        .auto_tabs_without_room(stock, part)
        .into_iter()
        .map(|(i, _)| i)
        .collect()
}

/// The first note for each tool of `tools` that a tab of `workholding` is too
/// short for, with the tabs where they are (automatic ones resolved against
/// the stock and the part; decision 0202 item 3).
fn narrow_findings(
    workholding: &Workholding,
    stock: Option<&Stock>,
    part: Option<&PartExtent>,
    tools: &[SetupTool],
) -> Vec<WorkholdingFinding> {
    let resolved = workholding.resolved(stock, part);
    let skip = unresolved_tabs(workholding, stock, part);
    tools
        .iter()
        .filter_map(|t| narrow_tab_note(&resolved, t.diameter, &t.tool, &t.operation, &skip))
        .collect()
}

/// Whether a zone finding stops Apply and stores nothing: only a tab
/// outside the head and tail zones, which a decision names (decision
/// records 0523 item 5 and 0539 item 3). Every other kind is a warning, so a
/// kind added later warns until a decision makes it refuse.
fn refuses(kind: &SetupWarningKind) -> bool {
    matches!(kind, SetupWarningKind::TabOutsideEndZones { .. })
}

/// The row, from zero, of the tab a zone finding is about: the automatic
/// tab with no room, or the automatic tab that overlaps one placed by hand.
fn finding_tab(kind: &SetupWarningKind) -> Option<usize> {
    match kind {
        SetupWarningKind::AutoTabNoRoom { tab, .. } => tab.checked_sub(1),
        SetupWarningKind::TabsOverlap { auto_tab, .. } => auto_tab.checked_sub(1),
        _ => None,
    }
}

/// What is wrong with the workholding of `setup` as it would be stored: tabs
/// outside the head and tail zones (OI-180), then tabs shorter along X than a tool of
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
                    refuses: refuses(&kind),
                    text: format!(
                        "Tab {tab} (X {:.3} to {:.3} mm) is not wholly in the head or the tail \
                         zone. Tabs lie between the held region (or the end of the stock) and \
                         the part, or between the part and the other end.",
                        x.0, x.1
                    ),
                },
                // Every other finding is a warning: the workholding is stored
                // and the finding stays on its tab's row, as it is a warning
                // when generating (decision record 0539). Only a finding a
                // decision names refuses; a new kind warns until one does.
                other => WorkholdingFinding {
                    tab: finding_tab(&other),
                    refuses: refuses(&other),
                    text: openchuck_types::setup::SetupWarning {
                        kind: other,
                        unconfirmed: Vec::new(),
                    }
                    .to_string(),
                },
            })
            .collect();
    found.extend(narrow_tab_findings(project, setup, extent.as_ref()));
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

/// The words every warning of a tab shorter than the tool starts with, so a
/// host can tell it apart (decision 0523).
pub const TAB_LENGTH_LEAD: &str = "Tab length: ";

/// The check that needs the part, made before generating (decision 0035,
/// owner's answers; OI-180): every tab lies wholly in the head zone or the
/// tail zone. The message names the tab and what to change. A tab shorter
/// than the tool is warned of, not refused ([`short_tab_warnings_where`];
/// decision 0523, amending 0494 item 1).
pub fn check_for_tool(
    workholding: &Workholding,
    stock: Option<&Stock>,
    part: &PartExtent,
) -> Result<(), AppError> {
    check_for_tool_where(workholding, stock, part, &|_| true)
}

/// [`check_for_tool`] over the tabs `counts` keeps, as they are resolved
/// (issue #548; decision 0523, amending 0494 item 2): an operation limited
/// by selections checks only the tabs its path can touch
/// ([`path_reaches_tab`]), so a tab it never comes near does not refuse it.
pub fn check_for_tool_where(
    workholding: &Workholding,
    stock: Option<&Stock>,
    part: &PartExtent,
    counts: &dyn Fn(&Tab) -> bool,
) -> Result<(), AppError> {
    let resolved = workholding.resolved(stock, Some(part));
    let counted = |number: usize| {
        number
            .checked_sub(1)
            .and_then(|i| resolved.tabs.get(i))
            .is_some_and(counts)
    };
    // An automatic tab with no room is only a warning: look past it.
    if let Some((tab, x)) = check_tab_zones(workholding, stock, Some(part))
        .into_iter()
        .find_map(|kind| match kind {
            SetupWarningKind::TabOutsideEndZones { tab, x } if counted(tab) => Some((tab, x)),
            _ => None,
        })
    {
        return Err(refuse(format!(
            "tab {tab} (X {:.3} to {:.3} mm) is not wholly in the head or the tail zone; tabs \
             lie between the held region (or the end of the stock) and the part, or between the \
             part and the other held region (or end of the stock)",
            x.0, x.1
        )));
    }
    Ok(())
}

/// A warning for each tab `counts` keeps that is shorter along X than a
/// tool of `tool_diameter` is wide (decision 0494 item 1, a warning since
/// decision 0523, the owner's choice of 6 Nov 2026): the tool may be unable
/// to cut the gap beside the tab to shape. Each names the tab, its X range
/// and length, and the tool's width; the host names the operation and the
/// tool when it shows it. Nothing in it says the program is safe (NFR-12).
pub fn short_tab_warnings_where(
    workholding: &Workholding,
    stock: Option<&Stock>,
    part: &PartExtent,
    tool_diameter: f64,
    counts: &dyn Fn(&Tab) -> bool,
) -> Vec<String> {
    let resolved = workholding.resolved(stock, Some(part));
    // An automatic tab keeps its last-applied length when the part moves
    // (decision 0202 item 3): compare the length it has now, as the target
    // is built from it.
    let skip = unresolved_tabs(workholding, stock, Some(part));
    resolved
        .tabs
        .iter()
        .enumerate()
        .filter(|(i, tab)| tab.length.0 < tool_diameter - 1e-9 && !skip.contains(i) && counts(tab))
        .map(|(i, tab)| {
            let (lo, d) = (tab.x_start.0, tab.length.0);
            format!(
                "{TAB_LENGTH_LEAD}tab {} (X {lo:.3} to {:.3} mm) is {d:.3} mm long along X, \
                 shorter than the tool is wide ({tool_diameter:.3} mm), so the tool may be \
                 unable to cut the gap beside it to shape. The path was made: check it before \
                 running it. Use a smaller tool, or make the tab longer",
                i + 1,
                lo + d,
            )
        })
        .collect()
}

/// How far past a tab's farthest radius, mm, a tip may stand and still count
/// as reaching it: rounding in the drop onto the tab.
const TAB_REACH_SLACK: f64 = 0.01;

/// True when a fed move of `group` can touch `tab` with a tool whose largest
/// radius is `reach` (issue #548; decision 0523). The tool's body lies at or
/// above its tip along the tool axis (every tool profile rises from its
/// tip) and within `reach` of that axis, which is square to the rotary
/// axis; the tab lies within its farthest radius of the rotary axis. So a
/// tool can touch the tab only where its tip comes within `reach` of the
/// tab along X and no higher above the rotary axis, along the tool axis,
/// than the tab's farthest radius. Each fed move (cuts, plunges, leads and
/// fed links) is read from the pose before it, over the X range and the
/// lowest height of its two ends, which bounds the move; rapids are not
/// cuts. A pose that is not a finite number counts as touching (NFR-07).
pub fn path_reaches_tab(group: &openchuck_types::ir::ToolpathGroup, tab: &Tab, reach: f64) -> bool {
    // A layout this check does not know is taken as far out as the stock
    // could hold it: never a tab missed.
    let top = match tab.layout {
        TabLayout::OnAxis => 0.0,
        TabLayout::AroundAxis { radial_offset, .. } => radial_offset.0.abs(),
        _ => f64::INFINITY,
    } + 0.5 * tab.shape.max_diameter().0;
    let (lo, hi) = (tab.x_start.0 - reach, tab.x_start.0 + tab.length.0 + reach);
    let height = |pose: &openchuck_types::ir::ToolPose| {
        let axis = pose.axis.get();
        pose.tip.y * axis.y + pose.tip.z * axis.z
    };
    let mut from = &group.start;
    for segment in &group.segments {
        if segment.kind.is_fed() {
            let (x0, x1) = (
                from.tip.x.min(segment.end.tip.x),
                from.tip.x.max(segment.end.tip.x),
            );
            let low = height(from).min(height(&segment.end));
            if !(x0.is_finite() && x1.is_finite() && low.is_finite()) {
                return true;
            }
            if x1 >= lo && x0 <= hi && low <= top + TAB_REACH_SLACK {
                return true;
            }
        }
        from = &segment.end;
    }
    false
}

/// The tool of an operation as [`uncleared_tab_notes`] needs it (decision
/// 0503): its cutting radius, its largest radius (profile or shank), which
/// keeps it from a held region (decision 0161), the operation's stock to
/// leave, and the names the note gives.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClearingTool<'a> {
    pub(crate) cutting_radius: f64,
    pub(crate) reach: f64,
    pub(crate) leave: f64,
    pub(crate) tool: &'a str,
    pub(crate) operation: &'a str,
}

/// A note for each zone between a held region and the part that holds a tab
/// and is too short for `tool` to cut down in, or none (OP-21, SET-09;
/// decision 0503; issue #511). The tool's middle keeps its largest radius and
/// [`HELD_CLEARANCE`] from the held region (decision 0161), and its cutting
/// radius and the stock to leave from the part's end, so the room it has is
/// the zone's length less the clearance, the stock to leave and what its
/// largest radius has over its cutting radius. Under the tool's cutting
/// diameter the tool lands on the part's end everywhere in the zone, and the
/// stock round the tab is left above it. A zone with no held region is not
/// reported: the tool can pass off the end of the stock there.
pub(crate) fn uncleared_tab_notes(
    workholding: &Workholding,
    stock: &Stock,
    part: &PartExtent,
    tool: &ClearingTool<'_>,
) -> Vec<String> {
    let Some(bounds) = stock.bounds(Some(part)) else {
        return Vec::new();
    };
    let (held_start, held_end) = workholding.held_intervals((bounds.min.x, bounds.max.x));
    let (part_lo, part_hi) = (part.bounds.min.x, part.bounds.max.x);
    let resolved = workholding.resolved(Some(stock), Some(part));
    let diameter = 2.0 * tool.cutting_radius;
    let zones = [
        held_start.map(|(_, hi)| (hi, part_lo)),
        held_end.map(|(lo, _)| (part_hi, lo)),
    ];
    zones
        .into_iter()
        .flatten()
        .filter(|(lo, hi)| hi > lo)
        .filter_map(|(lo, hi)| {
            let tabs: Vec<String> = resolved
                .tabs
                .iter()
                .enumerate()
                .filter(|(_, t)| t.x_start.0 < hi && t.x_start.0 + t.length.0 > lo)
                .map(|(i, _)| (i + 1).to_string())
                .collect();
            let (last, rest) = tabs.split_last()?;
            let wider = (tool.reach - tool.cutting_radius).max(0.0);
            let room = (hi - lo) - HELD_CLEARANCE - tool.leave - wider;
            if room >= diameter - 1e-9 {
                return None;
            }
            let (word, list) = if rest.is_empty() {
                ("tab", last.clone())
            } else {
                ("tabs", format!("{} and {last}", rest.join(", ")))
            };
            let clearance = format!("the {HELD_CLEARANCE:.3} mm it keeps from the held region");
            let leave = format!("the {:.3} mm stock to leave", tool.leave);
            let taken = if wider > 0.0 {
                format!(
                    "{clearance}, {leave} and the {wider:.3} mm its shank or body is wider \
                     than its cut"
                )
            } else {
                format!("{clearance} and {leave}")
            };
            Some(format!(
                "{} cannot cut down beside {word} {list}: between the held region and the \
                 part (X {lo:.3} to {hi:.3} mm) the tool {} ({diameter:.3} mm) has {room:.3} mm \
                 of room, once {taken} are taken off. It leaves the stock round the tab \
                 above it there. Use a smaller tool or less stock to leave, shorten the held \
                 region or move the part away from it, or clear it with a later operation. \
                 Check the program and the simulation before running it.",
                tool.operation, tool.tool
            ))
        })
        .collect()
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

/// The comparison target of the deviation report (SIM-05): the part and the
/// held regions and tabs as one surface (SET-09, OP-21; decision 0045 item
/// 6), with what tells each sample of it what it lies on (decision 0194).
pub struct DeviationTarget {
    /// The model's triangles, then the triangles of each solid, with every
    /// triangle that lies wholly inside the union, by its centroid, turned
    /// into a degenerate one, so it takes no samples. The model's triangles
    /// keep their indices (the deviation colours of the part read the first
    /// `model.triangles.len()` entries).
    pub mesh: TriangleMesh,
    model_triangles: usize,
    solids: Vec<Solid>,
    /// Each solid's triangles in `mesh`: a held region of a box section has
    /// 12 and a round solid `4 * TARGET_SEGMENTS`.
    ranges: Vec<std::ops::Range<usize>>,
    /// The model's surface, for the samples on the solids.
    model: Option<openchuck_path::MeshSurface>,
    /// The model's triangles turned degenerate above, as they were, so its
    /// closed body can be given back ([`DeviationTarget::bodies`]).
    covered: Vec<(usize, [u32; 3])>,
}

impl std::fmt::Debug for DeviationTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviationTarget")
            .field("triangles", &self.mesh.triangles.len())
            .field("model_triangles", &self.model_triangles)
            .field("solids", &self.solids.len())
            .finish_non_exhaustive()
    }
}

impl DeviationTarget {
    /// The model's own surface, with no held regions or tabs: every sample
    /// is on the part.
    pub fn part_only(mesh: TriangleMesh) -> Self {
        Self {
            model_triangles: mesh.triangles.len(),
            mesh,
            solids: Vec::new(),
            ranges: Vec::new(),
            model: None,
            covered: Vec::new(),
        }
    }

    /// The closed bodies whose union the target's surface bounds: the
    /// model, whole, then each held region and tab (for the reach of the
    /// material left, issue 478).
    pub fn bodies(&self) -> Vec<TriangleMesh> {
        let mut model = TriangleMesh {
            vertices: self.mesh.vertices.clone(),
            triangles: self
                .mesh
                .triangles
                .get(..self.model_triangles)
                .map_or_else(Vec::new, <[_]>::to_vec),
        };
        for (t, original) in &self.covered {
            if let Some(slot) = model.triangles.get_mut(*t) {
                *slot = *original;
            }
        }
        std::iter::once(model)
            .chain(self.solids.iter().map(|s| s.mesh(TARGET_SEGMENTS)))
            .collect()
    }

    /// The target over `model` (whose surface is `model_surface`) and
    /// `solids`.
    pub fn new(
        model: &TriangleMesh,
        model_surface: openchuck_path::MeshSurface,
        solids: Vec<Solid>,
    ) -> Self {
        let mut mesh = model.clone();
        let model_triangles = mesh.triangles.len();
        let mut ranges = Vec::with_capacity(solids.len());
        for solid in &solids {
            let offset = u32::try_from(mesh.vertices.len()).unwrap_or(u32::MAX);
            let part = solid.mesh(TARGET_SEGMENTS);
            let first = mesh.triangles.len();
            mesh.vertices.extend(part.vertices);
            mesh.triangles.extend(
                part.triangles
                    .iter()
                    .map(|t| t.map(|i| i.saturating_add(offset))),
            );
            ranges.push(first..mesh.triangles.len());
        }
        let mut target = Self {
            mesh,
            model_triangles,
            solids,
            ranges,
            model: Some(model_surface),
            covered: Vec::new(),
        };
        for t in 0..target.mesh.triangles.len() {
            let Some([a, b, c]) = target.mesh.triangle(t) else {
                continue;
            };
            let Some(normal) = unit_normal(a, b, c) else {
                continue;
            };
            let centroid = Point3::new(
                (a.x + b.x + c.x) / 3.0,
                (a.y + b.y + c.y) / 3.0,
                (a.z + b.z + c.z) / 3.0,
            );
            if target.covered(t, centroid, normal)
                && let Some(slot) = target.mesh.triangles.get_mut(t)
            {
                let original = *slot;
                let v = slot[0];
                *slot = [v, v, v];
                if t < model_triangles {
                    target.covered.push((t, original));
                }
            }
        }
        target
    }

    /// What a sample at `point` on triangle `triangle`, of outward unit
    /// normal `normal`, lies on: inside the union (under a tab, or where a
    /// tab meets a held region) wherever another part holds it, else the
    /// face of a held region or tab, else the part (decision 0194). Decided
    /// at each sample, so a large triangle partly under a tab, kept by its
    /// centroid, takes no part where it is covered.
    pub fn classify(&self, triangle: usize, point: Point3, normal: Vec3) -> SampleOn {
        if self.covered(triangle, point, normal) {
            SampleOn::Inside
        } else if triangle < self.model_triangles {
            SampleOn::Part
        } else {
            SampleOn::Workholding
        }
    }

    /// Whether `point` on triangle `triangle` lies inside another part of the
    /// union: strictly inside it, or on its surface with that surface facing
    /// the triangle's own.
    fn covered(&self, triangle: usize, point: Point3, normal: Vec3) -> bool {
        // Which solid the triangle belongs to, so it is not tested against
        // itself.
        let own = self.ranges.iter().position(|r| r.contains(&triangle));
        let inside_solid = self.solids.iter().enumerate().any(|(i, s)| {
            own != Some(i) && inside(s.contains(point), s.project(point), point, normal)
        });
        let inside_model = own.is_some()
            && self.model.as_ref().is_some_and(|m| {
                m.project(point).is_some_and(|p| {
                    if p.point.distance(point) <= SURFACE_TOLERANCE {
                        p.normal.get().dot(normal) < -0.5
                    } else {
                        // Behind the nearest point's outward normal: inside.
                        (point - p.point).dot(p.normal.get()) < 0.0
                    }
                })
            });
        inside_solid || inside_model
    }
}

/// The outward unit normal of the triangle `a b c`, or `None` for a
/// degenerate one.
fn unit_normal(a: Point3, b: Point3, c: Point3) -> Option<Vec3> {
    let cross = (b - a).cross(c - a);
    let length = cross.length();
    if length.is_nan() || length <= 1e-18 {
        return None;
    }
    Some(cross * (1.0 / length))
}

/// The target the deviation report compares the stock with for `setup`: the
/// placed part's mesh alone when the setup holds nothing, else the
/// [`DeviationTarget`] over the part and the setup's solids. The part's own
/// triangles come first and keep their indices either way.
pub(crate) fn setup_deviation_target(
    setup: &openchuck_project::Setup,
    mesh: TriangleMesh,
) -> Result<DeviationTarget, AppError> {
    if setup.workholding.is_empty() {
        return Ok(DeviationTarget::part_only(mesh));
    }
    let part = openchuck_geom::part_extent(&mesh)
        .ok_or_else(|| refuse("the setup's part has no extent"))?;
    let solids = solids(&setup.workholding, setup.stock.as_ref(), &part)?;
    let surface = openchuck_path::MeshSurface::new(&mesh)?;
    Ok(DeviationTarget::new(&mesh, surface, solids))
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
    use openchuck_types::workholding::{HeldRegion, TabEnd};

    #[test]
    fn req_ui_25_a_new_zone_finding_warns_unless_named() {
        use openchuck_types::setup::SetupWarningKind as K;
        assert!(super::refuses(&K::TabOutsideEndZones {
            tab: 1,
            x: (0.0, 1.0)
        }));
        for kind in [
            K::AutoTabNoRoom {
                tab: 1,
                end: TabEnd::Head,
            },
            K::TabsOverlap {
                auto_tab: 1,
                hand_tab: 2,
                x: (0.0, 1.0),
            },
            K::NoStock,
            K::PartOutsideStock,
        ] {
            assert!(!super::refuses(&kind), "{kind:?} only warns");
        }
        assert_eq!(
            super::finding_tab(&K::TabsOverlap {
                auto_tab: 2,
                hand_tab: 1,
                x: (0.0, 1.0)
            }),
            Some(1)
        );
    }

    fn part(lo: f64, hi: f64) -> PartExtent {
        PartExtent {
            bounds: Aabb {
                min: Point3::new(lo, -8.0, -8.0),
                max: Point3::new(hi, 8.0, 8.0),
            },
            radius: Millimetres(8.0),
        }
    }

    /// The first warning of a tab shorter than a tool `d` wide, on every
    /// tab (decision 0494 item 1, a warning since decision 0523).
    fn short_warning(
        w: &Workholding,
        stock: Option<&Stock>,
        part: &PartExtent,
        d: f64,
    ) -> Option<String> {
        short_tab_warnings_where(w, stock, part, d, &|_| true)
            .into_iter()
            .next()
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

    /// The triangles of the part in [`owners_layout`]: 24 segments of two
    /// side triangles and two end-face triangles.
    const OWNERS_MODEL: usize = 96;

    /// The owner's layout in blisk_glitchy_generating (issue 248): box
    /// stock held 5 mm at its start by its own section, a tab of diameter
    /// 10 from X 5 to 10, and a part of radius 20 from X 10, whose end faces
    /// are fans of large triangles.
    fn owners_layout() -> DeviationTarget {
        layout_with_tab(5.0, 5.0, None)
    }

    /// [`owners_layout`] with the tab stored at `x_start` and `length`, and
    /// `auto` set or not.
    fn layout_with_tab(x_start: f64, length: f64, auto: Option<TabEnd>) -> DeviationTarget {
        let mesh = openchuck_testkit::parts::revolve(&[(10.0, 20.0), (60.0, 20.0)], 24);
        let boxed = Stock::Box {
            bounds: Aabb {
                min: Point3::new(0.0, -21.0, -20.0),
                max: Point3::new(60.0, 21.0, 20.0),
            },
        };
        let w = Workholding {
            start: Some(HeldRegion {
                length: Millimetres(5.0),
                diameter: None,
            }),
            end: None,
            tabs: vec![Tab {
                x_start: Millimetres(x_start),
                length: Millimetres(length),
                shape: TabShape::Cylinder {
                    diameter: Millimetres(10.0),
                },
                layout: TabLayout::OnAxis,
                auto,
            }],
        };
        let extent = openchuck_geom::part_extent(&mesh).unwrap();
        let solids = solids(&w, Some(&boxed), &extent).unwrap();
        let surface = openchuck_path::MeshSurface::new(&mesh).unwrap();
        DeviationTarget::new(&mesh, surface, solids)
    }

    /// The first triangle of `target` whose centroid satisfies `pick`.
    fn triangle_at(target: &DeviationTarget, pick: impl Fn(Point3) -> bool) -> usize {
        (0..target.mesh.triangles.len())
            .find(|&t| {
                target.mesh.triangle(t).is_some_and(|[a, b, c]| {
                    unit_normal(a, b, c).is_some()
                        && pick(Point3::new(
                            (a.x + b.x + c.x) / 3.0,
                            (a.y + b.y + c.y) / 3.0,
                            (a.z + b.z + c.z) / 3.0,
                        ))
                })
            })
            .unwrap()
    }

    #[test]
    fn req_sim_05_req_op_21_req_set_09_each_sample_says_what_it_lies_on() {
        // Decision 0194: whether a sample is on the part, on the face of a
        // held region or tab, or inside the finished part (under a tab) is
        // decided at the sample, not at its triangle's centre.
        let target = owners_layout();
        let minus_x = Vec3::new(-1.0, 0.0, 0.0);
        let plus_x = Vec3::new(1.0, 0.0, 0.0);
        // The part's end face at X 10: its fan triangles reach the axis, but
        // their centres lie at two thirds of the radius, outside the tab, so
        // they are kept whole.
        let face = triangle_at(&target, |c| (c.x - 10.0).abs() < 1e-9);
        assert!(face < OWNERS_MODEL);
        assert_eq!(
            target.classify(face, Point3::new(10.0, 1.0, 0.5), minus_x),
            SampleOn::Inside,
            "under the tab"
        );
        assert_eq!(
            target.classify(face, Point3::new(10.0, 12.0, 0.0), minus_x),
            SampleOn::Part,
            "beside the tab"
        );
        // The held region's face at X 5 under the tab is inside too; beside
        // it, it is the held region's own face.
        let held = triangle_at(&target, |c| (c.x - 5.0).abs() < 1e-9);
        assert!((OWNERS_MODEL..OWNERS_MODEL + 12).contains(&held), "{held}");
        assert_eq!(
            target.classify(held, Point3::new(5.0, 2.0, 1.0), plus_x),
            SampleOn::Inside
        );
        assert_eq!(
            target.classify(held, Point3::new(5.0, 15.0, 15.0), plus_x),
            SampleOn::Workholding
        );
        // The tab's wall is its own face.
        let wall = triangle_at(&target, |c| {
            c.x > 5.5 && c.x < 9.5 && (c.y.hypot(c.z) - 5.0).abs() < 0.1
        });
        assert_eq!(
            target.classify(wall, Point3::new(7.5, 0.0, 5.0), Vec3::new(0.0, 0.0, 1.0)),
            SampleOn::Workholding
        );
    }

    #[test]
    fn req_sim_05_req_op_21_an_automatic_tab_is_compared_where_it_is_resolved() {
        // Decision 0202: an automatic tab's stored place is not where it is;
        // the target holds the tab where the part, the stock and the held
        // region put it, X 5 to 10 at the head, as in the owner's layout.
        let target = layout_with_tab(40.0, 2.0, Some(TabEnd::Head));
        let face = triangle_at(&target, |c| (c.x - 10.0).abs() < 1e-9);
        assert_eq!(
            target.classify(face, Point3::new(10.0, 1.0, 0.5), Vec3::new(-1.0, 0.0, 0.0)),
            SampleOn::Inside,
            "under the resolved tab"
        );
        let wall = triangle_at(&target, |c| {
            c.x > 5.5 && c.x < 9.5 && (c.y.hypot(c.z) - 5.0).abs() < 0.1
        });
        assert_eq!(
            target.classify(wall, Point3::new(7.5, 0.0, 5.0), Vec3::new(0.0, 0.0, 1.0)),
            SampleOn::Workholding
        );
        // Nothing is left at the stored place, X 40 to 42, inside the part.
        assert!(
            (0..target.mesh.triangles.len())
                .filter_map(|t| target.mesh.triangle(t))
                .all(|[a, b, c]| [a, b, c]
                    .iter()
                    .all(|p| p.x < 39.0 || p.x > 43.0 || p.y.hypot(p.z) > 19.0)),
        );
    }

    #[test]
    fn req_sim_05_req_op_21_a_tab_after_a_box_held_region_is_tested_against_it() {
        // A held region of a box section has 12 triangles, not the 384 of a
        // round solid; the tab after it was taken as part of the box, never
        // tested against it, so its end face, buried in the held region's
        // face at X 5, was sampled as a surface of the finished part.
        let target = owners_layout();
        let ends: Vec<usize> = (0..target.mesh.triangles.len())
            .filter(|&t| {
                t >= OWNERS_MODEL + 12
                    && target
                        .mesh
                        .triangle(t)
                        .is_some_and(|[a, b, c]| [a, b, c].iter().all(|p| (p.x - 5.0).abs() < 1e-9))
            })
            .collect();
        // The tab's whole end disc at X 5 is dropped from the target.
        assert_eq!(ends.len(), 96);
        assert!(ends.iter().all(|&t| {
            target
                .mesh
                .triangles
                .get(t)
                .is_some_and(|[a, b, c]| a == b && b == c)
        }));
    }

    #[test]
    fn req_sim_05_the_reach_bodies_give_back_the_whole_part_under_a_tab() {
        // Issue 478: the part's triangles under a tab take no samples, so the
        // target turns them degenerate; the closed bodies the reach of the
        // material left is cast against must still be the whole part, and
        // each held region and tab.
        let mesh = openchuck_testkit::parts::revolve(&[(10.0, 20.0), (60.0, 20.0)], 24);
        let boxed = Stock::Box {
            bounds: Aabb {
                min: Point3::new(0.0, -21.0, -20.0),
                max: Point3::new(60.0, 21.0, 20.0),
            },
        };
        let w = Workholding {
            start: Some(HeldRegion {
                length: Millimetres(5.0),
                diameter: None,
            }),
            end: None,
            tabs: vec![Tab {
                x_start: Millimetres(5.0),
                length: Millimetres(5.0),
                // Wide enough to cover the centres of the end face's fan.
                shape: TabShape::Cylinder {
                    diameter: Millimetres(30.0),
                },
                layout: TabLayout::OnAxis,
                auto: None,
            }],
        };
        let extent = openchuck_geom::part_extent(&mesh).unwrap();
        let solids = solids(&w, Some(&boxed), &extent).unwrap();
        let count = solids.len();
        let surface = openchuck_path::MeshSurface::new(&mesh).unwrap();
        let target = DeviationTarget::new(&mesh, surface, solids);
        let covered = (0..mesh.triangles.len())
            .filter(|&t| {
                target
                    .mesh
                    .triangles
                    .get(t)
                    .is_some_and(|[a, b, c]| a == b && b == c)
            })
            .count();
        assert!(covered > 0, "the end face under the tab takes no samples");
        let bodies = target.bodies();
        assert_eq!(bodies.len(), 1 + count);
        assert_eq!(bodies[0].triangles, mesh.triangles);
        for body in &bodies {
            body.validate().unwrap();
        }
        // The part alone has one body, itself.
        let alone = DeviationTarget::part_only(mesh.clone()).bodies();
        assert_eq!(alone.len(), 1);
        assert_eq!(alone[0].triangles, mesh.triangles);
    }

    fn tab_of(length: f64, shape: TabShape, layout: TabLayout) -> Workholding {
        Workholding {
            tabs: vec![Tab {
                auto: None,
                x_start: Millimetres(80.0),
                length: Millimetres(length),
                shape,
                layout,
            }],
            ..Workholding::default()
        }
    }

    /// A pose with its tip at model `x`, `height` above the rotary axis
    /// along the tool axis, at A 0 with the tool along +Z.
    fn pose(x: f64, height: f64) -> openchuck_types::ir::ToolPose {
        openchuck_types::ir::ToolPose {
            tip: Point3::new(x, 0.0, height),
            axis: UnitVec3::new(Vec3::new(0.0, 0.0, 1.0)).unwrap(),
            a: openchuck_types::units::Degrees(0.0),
        }
    }

    /// A path from `start` through `moves`, each a pose and whether it is
    /// fed.
    fn group(
        start: openchuck_types::ir::ToolPose,
        moves: &[(openchuck_types::ir::ToolPose, bool)],
    ) -> openchuck_types::ir::ToolpathGroup {
        use openchuck_types::ir::{MoveKind, Segment, ToolpathGroup};
        ToolpathGroup {
            start,
            segments: moves
                .iter()
                .map(|&(end, fed)| Segment {
                    kind: if fed {
                        MoveKind::Cutting
                    } else {
                        MoveKind::Rapid
                    },
                    end,
                    contact_feed: None,
                    duration: openchuck_types::units::Seconds(0.0),
                    contact_length: Millimetres(0.0),
                })
                .collect(),
            rotary_reset_after: Vec::new(),
            tags: openchuck_types::ir::PathTags {
                operation: String::new(),
                setup: String::new(),
                tool: String::new(),
            },
        }
    }

    /// Issue #548: a tab on the axis 6 mm across (radius 3) from X 80 to 84,
    /// and a tool whose largest radius is 3. A path touches the tab only
    /// where a fed move's tip comes within the tool's radius of it along X
    /// and no higher above the axis than the tab's farthest radius: the
    /// tool's body is above its tip, and the tab is within 3 mm of the axis.
    #[test]
    fn req_op_21_a_path_reaches_a_tab_only_within_the_tool_and_the_tab() {
        let w = tab_of(
            4.0,
            TabShape::Cylinder {
                diameter: Millimetres(6.0),
            },
            TabLayout::OnAxis,
        );
        let tab = &w.tabs[0];
        let reaches = |moves: &[(openchuck_types::ir::ToolPose, bool)]| {
            path_reaches_tab(&group(pose(20.0, 10.0), moves), tab, 3.0)
        };
        // Finishing the part's side, 10 mm from the axis, right over the tab:
        // the tool never comes down to it.
        assert!(!reaches(&[
            (pose(70.0, 10.0), true),
            (pose(84.0, 10.0), true)
        ]));
        // A plunge, from a rapid above, down to the tab's top beside it within
        // the tool's radius: touches.
        let plunge = |x: f64, h: f64| reaches(&[(pose(x, 20.0), false), (pose(x, h), true)]);
        assert!(plunge(77.1, 3.0));
        assert!(plunge(86.9, 3.0));
        // Past the tool's radius along X, even at the axis: it does not.
        assert!(!plunge(76.9, 0.0));
        assert!(!plunge(87.1, 0.0));
        // Over the tab but above its top: it does not.
        assert!(!plunge(82.0, 3.1));
        // A fed move across the tab, low, from one side to the other, though
        // neither end is within reach: touches.
        assert!(reaches(&[
            (pose(60.0, 2.0), true),
            (pose(100.0, 2.0), true)
        ]));
        // A rapid is not a cut.
        assert!(!reaches(&[(pose(82.0, 2.0), false)]));
        // A fed move from where a rapid left the tool is read from there.
        assert!(reaches(&[
            (pose(82.0, 20.0), false),
            (pose(82.0, 2.0), true)
        ]));
        // A tab around the axis reaches its offset plus its radius.
        let around = tab_of(
            4.0,
            TabShape::Cylinder {
                diameter: Millimetres(2.0),
            },
            TabLayout::AroundAxis {
                count: 3,
                start_angle: openchuck_types::units::Degrees(0.0),
                radial_offset: Millimetres(6.0),
            },
        );
        let g = group(pose(82.0, 20.0), &[(pose(82.0, 6.9), true)]);
        assert!(path_reaches_tab(&g, &around.tabs[0], 3.0));
        let g = group(pose(82.0, 20.0), &[(pose(82.0, 7.1), true)]);
        assert!(!path_reaches_tab(&g, &around.tabs[0], 3.0));
    }

    /// Issue #548 (amends decision 0494 item 2, decision 0523): the tabs an
    /// operation's check reads are chosen by `counts`. A tab shorter than the
    /// tool that it leaves out is not warned of; one it keeps still is, with
    /// the same words; and with every tab counted (an operation with no
    /// region) the check is as before.
    #[test]
    fn req_op_21_a_tab_the_operation_cannot_touch_is_not_warned_of() {
        let w = tab_of(
            3.0,
            TabShape::Cylinder {
                diameter: Millimetres(12.0),
            },
            TabLayout::OnAxis,
        );
        let part = part(20.0, 78.0);
        let none = |_: &Tab| false;
        let all = |_: &Tab| true;
        assert!(short_tab_warnings_where(&w, Some(&stock()), &part, 4.0, &none).is_empty());
        let text = short_tab_warnings_where(&w, Some(&stock()), &part, 4.0, &all).join("\n");
        assert!(
            text.contains("tab 1 (X 80.000 to 83.000 mm) is 3.000 mm long along X"),
            "{text}"
        );
        assert!(
            text.contains("shorter than the tool is wide (4.000 mm)"),
            "{text}"
        );
        // A tab outside the end zones is left out the same way.
        let mut stray = w.clone();
        stray.tabs[0].x_start = Millimetres(40.0);
        stray.tabs[0].length = Millimetres(5.0);
        assert!(check_for_tool_where(&stray, Some(&stock()), &part, &none).is_ok());
        let text = check_for_tool_where(&stray, Some(&stock()), &part, &all)
            .unwrap_err()
            .to_string();
        assert!(
            text.contains("is not wholly in the head or the tail zone"),
            "{text}"
        );
    }

    /// OP-21, decision 0494 (amends 0035 item 6): the tab's length along X is
    /// compared with the tool's diameter, not the tab's diameter. A wide tab
    /// that is shorter than the tool is warned of (a refusal before decision
    /// 0523).
    #[test]
    fn req_op_21_a_tab_shorter_than_the_tool_is_warned_of_whatever_its_diameter() {
        let w = tab_of(
            3.0,
            TabShape::Cylinder {
                diameter: Millimetres(12.0),
            },
            TabLayout::OnAxis,
        );
        let part = part(20.0, 78.0);
        assert!(short_warning(&w, Some(&stock()), &part, 3.0).is_none());
        let error = short_warning(&w, Some(&stock()), &part, 4.0).unwrap();
        let text = error.to_string();
        assert!(text.contains("3.000 mm long along X"), "{text}");
        assert!(
            text.contains("shorter than the tool is wide (4.000 mm)"),
            "{text}"
        );
        assert!(!text.contains("across"), "{text}");
    }

    fn auto_head_tab(stored_length: f64) -> Workholding {
        Workholding {
            start: Some(HeldRegion {
                length: Millimetres(10.0),
                diameter: None,
            }),
            tabs: vec![Tab {
                auto: Some(TabEnd::Head),
                x_start: Millimetres(10.0),
                length: Millimetres(stored_length),
                shape: TabShape::Cylinder {
                    diameter: Millimetres(8.0),
                },
                layout: TabLayout::OnAxis,
            }],
            ..Workholding::default()
        }
    }

    fn one_ball_tool() -> [SetupTool; 1] {
        [SetupTool {
            tool: "T1 Ball".into(),
            diameter: 4.0,
            operation: "1 Rough".into(),
        }]
    }

    /// OP-21, decision 0202 item 3 and decision 0494: an automatic tab is
    /// compared with the tool where it is now, not at the length it was last
    /// applied with. Stored 8 mm long; the part now starts at X 12, so the
    /// head gap is 2 mm and a 4 mm tool does not fit: warned of when generating
    /// and found by the checks.
    #[test]
    fn req_op_21_an_automatic_tab_that_shrank_is_warned_of_at_its_resolved_length() {
        let shrunk = auto_head_tab(8.0);
        let near = part(12.0, 70.0);
        let error = short_warning(&shrunk, Some(&stock()), &near, 4.0).unwrap();
        assert!(
            error.to_string().contains("2.000 mm long along X"),
            "{error}"
        );
        let notes = narrow_findings(&shrunk, Some(&stock()), Some(&near), &one_ball_tool());
        assert_eq!(notes.len(), 1);
        assert!(notes[0].text.contains("2.000 mm long along X"), "{notes:?}");
    }

    /// The other way: stored 2 mm long; the part now starts at X 18, so the
    /// gap is 8 mm and the 4 mm tool fits.
    #[test]
    fn req_op_21_an_automatic_tab_that_grew_is_accepted_at_its_resolved_length() {
        let grown = auto_head_tab(2.0);
        let far = part(18.0, 70.0);
        assert!(short_warning(&grown, Some(&stock()), &far, 4.0).is_none());
        assert!(narrow_findings(&grown, Some(&stock()), Some(&far), &one_ball_tool()).is_empty());
    }

    /// CodeRabbit finding on pull request 460: with no part or no stock an
    /// automatic tab cannot be resolved, so its stored length says nothing
    /// and the findings skip it; a tab placed by hand is still compared.
    #[test]
    fn req_op_21_findings_wait_for_an_automatic_tab_to_be_resolvable() {
        let tools = one_ball_tool();
        let mut w = auto_head_tab(2.0);
        let manual = Tab {
            auto: None,
            x_start: Millimetres(80.0),
            length: Millimetres(3.0),
            shape: TabShape::Cylinder {
                diameter: Millimetres(6.0),
            },
            layout: TabLayout::OnAxis,
        };
        w.tabs.push(manual);
        let part = part(18.0, 70.0);
        // No part, then no stock: only the manual tab (the second, index 1) is found.
        for notes in [
            narrow_findings(&w, Some(&stock()), None, &tools),
            narrow_findings(&w, None, Some(&part), &tools),
        ] {
            assert_eq!(notes.len(), 1, "{notes:?}");
            assert_eq!(notes[0].tab, Some(1));
            assert!(notes[0].text.starts_with("Tab 2 is 3.000 mm"), "{notes:?}");
        }
        // With both, the automatic tab is resolved (8 mm gap) and fits, the
        // manual one is still short.
        let notes = narrow_findings(&w, Some(&stock()), Some(&part), &tools);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(notes[0].tab, Some(1));
    }

    /// CodeRabbit finding on pull request 460: an automatic tab with no room
    /// (the held region reaches the part) keeps its stored length, which says
    /// nothing; it is only a warning of the zone check and is not compared with
    /// the tool, when generating or in the findings. A tab by hand still is.
    #[test]
    fn req_op_21_an_automatic_tab_with_no_room_is_not_compared_with_the_tool() {
        let mut w = auto_head_tab(2.0);
        // The grip runs to X 20 and the part starts at X 18: no head gap.
        w.start = Some(HeldRegion {
            length: Millimetres(20.0),
            diameter: None,
        });
        let part = part(18.0, 70.0);
        assert_eq!(
            w.auto_tabs_without_room(Some(&stock()), Some(&part)),
            vec![(0, TabEnd::Head)]
        );
        assert!(short_warning(&w, Some(&stock()), &part, 4.0).is_none());
        let tools = one_ball_tool();
        assert!(narrow_findings(&w, Some(&stock()), Some(&part), &tools).is_empty());
        // A short tab placed by hand, in the tail zone, is still warned of.
        w.tabs.push(Tab {
            auto: None,
            x_start: Millimetres(72.0),
            length: Millimetres(3.0),
            shape: TabShape::Cylinder {
                diameter: Millimetres(6.0),
            },
            layout: TabLayout::OnAxis,
        });
        let error = short_warning(&w, Some(&stock()), &part, 4.0).unwrap();
        assert!(
            error.contains("tab 2 (") && error.contains("is 3.000 mm long"),
            "{error}"
        );
        let notes = narrow_findings(&w, Some(&stock()), Some(&part), &tools);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(notes[0].tab, Some(1));
    }

    /// The reverse: a thin tab that is long enough is accepted, for a
    /// cylinder and a frustum, on the axis and around it.
    #[test]
    fn req_op_21_a_thin_tab_at_least_as_long_as_the_tool_is_accepted() {
        let part = part(20.0, 78.0);
        let layouts = [
            TabLayout::OnAxis,
            TabLayout::AroundAxis {
                count: 2,
                start_angle: openchuck_types::units::Degrees(0.0),
                radial_offset: Millimetres(3.0),
            },
        ];
        for layout in layouts {
            for shape in [
                TabShape::Cylinder {
                    diameter: Millimetres(1.0),
                },
                TabShape::Frustum {
                    start_diameter: Millimetres(1.0),
                    end_diameter: Millimetres(2.0),
                },
            ] {
                let w = tab_of(4.0, shape, layout);
                // Length equal to the tool is long enough.
                assert!(short_warning(&w, Some(&stock()), &part, 4.0).is_none());
                assert!(short_warning(&w, Some(&stock()), &part, 4.5).is_some());
                assert!(narrow_tab_note(&w, 4.0, "a", "1 Rough", &[]).is_none());
                let note = narrow_tab_note(&w, 4.5, "T1 Flat", "1 Rough", &[]).unwrap();
                assert_eq!(
                    note.text,
                    "Tab 1 is 4.000 mm long along X, shorter than the tool T1 Flat \
                     (4.500 mm) of 1 Rough. Make the tab longer or use a smaller tool."
                );
            }
        }
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
        let error = check_for_tool(&w, Some(&stock()), &part(20.0, 78.0)).unwrap_err();
        assert!(
            error.to_string().contains("head or the tail zone"),
            "{error}"
        );
    }

    /// An automatic tab with no room is a warning, not a refusal; a tab after
    /// it across the part is still refused (decision record 0202; OP-21).
    #[test]
    fn req_op_21_a_tab_across_the_part_after_an_automatic_tab_is_refused() {
        let cylinder = TabShape::Cylinder {
            diameter: Millimetres(6.0),
        };
        let w = Workholding {
            start: Some(HeldRegion {
                length: Millimetres(25.0),
                diameter: None,
            }),
            tabs: vec![
                Tab {
                    auto: Some(openchuck_types::workholding::TabEnd::Head),
                    x_start: Millimetres(10.0),
                    length: Millimetres(5.0),
                    shape: cylinder,
                    layout: TabLayout::OnAxis,
                },
                Tab {
                    auto: None,
                    x_start: Millimetres(40.0),
                    length: Millimetres(5.0),
                    shape: cylinder,
                    layout: TabLayout::OnAxis,
                },
            ],
            ..Workholding::default()
        };
        let after = part(20.0, 78.0);
        assert!(matches!(
            check_tab_zones(&w, Some(&stock()), Some(&after))[0],
            SetupWarningKind::AutoTabNoRoom { tab: 1, .. }
        ));
        let error = check_for_tool(&w, Some(&stock()), &after).unwrap_err();
        assert!(error.to_string().contains("tab 2"), "{error}");
        // The automatic tab itself is refused when the part has moved over
        // the position it keeps.
        let over = part(12.0, 78.0);
        let error = check_for_tool(&w, Some(&stock()), &over).unwrap_err();
        assert!(error.to_string().contains("tab 1"), "{error}");
    }

    /// The owner's blisk setup in numbers (issue #511): box stock X -8 to
    /// 56, 8 mm held at the start, an automatic head tab, the part from X 4.
    fn blisk_like() -> (Workholding, Stock, PartExtent) {
        let w = Workholding {
            start: Some(HeldRegion {
                length: Millimetres(8.0),
                diameter: None,
            }),
            tabs: vec![Tab {
                auto: Some(TabEnd::Head),
                x_start: Millimetres(0.0),
                length: Millimetres(4.0),
                shape: TabShape::Cylinder {
                    diameter: Millimetres(10.0),
                },
                layout: TabLayout::OnAxis,
            }],
            ..Workholding::default()
        };
        let stock = Stock::Cylinder {
            diameter: Millimetres(42.0),
            length: Millimetres(64.0),
            x_start: Millimetres(-8.0),
        };
        (w, stock, part(4.0, 54.0))
    }

    fn flat(leave: f64, reach: f64) -> ClearingTool<'static> {
        ClearingTool {
            cutting_radius: 1.5875,
            reach,
            leave,
            tool: "T104 Flat 3.175",
            operation: "1 Roughing",
        }
    }

    /// Decision 0503 item 1 (issue #511): between the held region and the
    /// part there is 4 mm; less the 0.5 mm kept clear of the held region and
    /// the 0.5 mm stock to leave, 3 mm, less than the 3.175 mm tool, so the
    /// tool cannot cut down beside the tab and the operation says so.
    #[test]
    fn req_op_21_req_set_09_a_tool_with_no_room_beside_a_tab_is_reported() {
        let (w, stock, part) = blisk_like();
        let notes = uncleared_tab_notes(&w, &stock, &part, &flat(0.5, 1.5875));
        assert_eq!(notes.len(), 1, "{notes:?}");
        let text = &notes[0];
        for want in [
            "1 Roughing",
            "tab 1",
            "X 0.000 to 4.000 mm",
            "3.000 mm",
            "T104 Flat 3.175 (3.175 mm)",
            "0.500 mm stock to leave",
            "smaller tool",
            "Check the program",
        ] {
            assert!(text.contains(want), "{want:?} not in {text}");
        }
    }

    /// With no stock to leave there is 3.5 mm, and the tool fits; with
    /// exactly the tool's width it fits too.
    #[test]
    fn req_op_21_req_set_09_a_tool_that_fits_beside_a_tab_is_not_reported() {
        let (w, stock, part) = blisk_like();
        assert!(uncleared_tab_notes(&w, &stock, &part, &flat(0.0, 1.5875)).is_empty());
        // 4 - 0.5 - leave = 3.175 exactly.
        let leave = 4.0 - HELD_CLEARANCE - 3.175;
        assert!(uncleared_tab_notes(&w, &stock, &part, &flat(leave, 1.5875)).is_empty());
    }

    /// A shank or profile wider than the cutting radius keeps the tool
    /// farther from the held region (decision 0161), so it takes room too.
    #[test]
    fn req_op_21_req_set_09_a_wider_shank_takes_room_beside_a_tab() {
        let (w, stock, part) = blisk_like();
        let notes = uncleared_tab_notes(&w, &stock, &part, &flat(0.0, 2.0875));
        assert_eq!(notes.len(), 1, "{notes:?}");
        // 4 - 0.5 - 0 - (2.0875 - 1.5875) = 3.
        assert!(notes[0].contains("has 3.000 mm of room"), "{notes:?}");
        assert!(
            notes[0].contains("stock to leave and the 0.500 mm its shank"),
            "{notes:?}"
        );
    }

    /// Two tabs in one zone (two short ones side by side) are named together
    /// in one note.
    #[test]
    fn req_op_21_req_set_09_tabs_sharing_a_zone_are_named_together() {
        let (mut w, stock, part) = blisk_like();
        let short = |x: f64| Tab {
            auto: None,
            x_start: Millimetres(x),
            length: Millimetres(2.0),
            shape: TabShape::Cylinder {
                diameter: Millimetres(10.0),
            },
            layout: TabLayout::OnAxis,
        };
        w.tabs = vec![short(0.0), short(2.0)];
        let notes = uncleared_tab_notes(&w, &stock, &part, &flat(0.5, 1.5875));
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("beside tabs 1 and 2:"), "{notes:?}");
    }

    /// The tail zone the same way, and no note where no held region bounds
    /// the zone (the tool can pass off the end of the stock) or where the
    /// zone has no tab.
    #[test]
    fn req_op_21_req_set_09_only_a_held_zone_with_a_tab_is_reported() {
        let stock = Stock::Cylinder {
            diameter: Millimetres(42.0),
            length: Millimetres(64.0),
            x_start: Millimetres(0.0),
        };
        let part = part(10.0, 54.0);
        let tail_tab = Tab {
            auto: None,
            x_start: Millimetres(54.0),
            length: Millimetres(4.0),
            shape: TabShape::Cylinder {
                diameter: Millimetres(10.0),
            },
            layout: TabLayout::OnAxis,
        };
        let held_end = Some(HeldRegion {
            length: Millimetres(6.0),
            diameter: None,
        });
        let tail = Workholding {
            end: held_end,
            tabs: vec![tail_tab],
            ..Workholding::default()
        };
        let notes = uncleared_tab_notes(&tail, &stock, &part, &flat(0.5, 1.5875));
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("X 54.000 to 58.000 mm"), "{notes:?}");
        // No held region at the end: nothing keeps the tool back.
        let free = Workholding {
            tabs: vec![tail_tab],
            ..Workholding::default()
        };
        assert!(uncleared_tab_notes(&free, &stock, &part, &flat(0.5, 1.5875)).is_empty());
        // A held region but no tab in its zone: not this check's business.
        let bare = Workholding {
            end: held_end,
            ..Workholding::default()
        };
        assert!(uncleared_tab_notes(&bare, &stock, &part, &flat(0.5, 1.5875)).is_empty());
    }
}

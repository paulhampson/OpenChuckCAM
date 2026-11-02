// SPDX-License-Identifier: AGPL-3.0-or-later
//! The view model of the workholding panel and of the viewport's held regions
//! and tabs (UI-25, UI-03; interface specification sections 7.5 and 11.7;
//! decision record 0104; work breakdown slice 21d item 3).
//!
//! Everything here is plain data and text, so the front end only draws it
//! (architecture section 5.8): the draft with its undo behaviour, the checks
//! on the draft with the field each marks, the derived lines of a row, the
//! default tabs shown as defaults, the placement of a new tab, and the
//! outlines and legend of the display.

use openchuck_types::geometry::Point3;
use openchuck_types::setup::{PartExtent, SetupWarningKind};
use openchuck_types::stock::Stock;
use openchuck_types::units::{Degrees, Millimetres};
use openchuck_types::workholding::{
    FRUSTUM_MIN_DIFFERENCE, HeldRegion, MAX_TAB_COUNT, Tab, TabEnd, TabLayout, TabShape,
    Workholding, auto_tab_span,
};

use crate::AppError;
use crate::scene::{
    SceneMesh, ScenePolyline, SceneRole, SceneSolid, SceneView, SolidSection, WorkholdingItem,
};

/// Which end of the stock a held region is at: low X or high X.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// The low-X end.
    Start,
    /// The high-X end.
    End,
}

/// A field of the panel, so a message can be shown under the right one.
/// Tab indexes are zero-based here and shown from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// A held region's grip length.
    Grip(End),
    /// A held region's holding diameter.
    HoldingDiameter(End),
    /// A tab's start X.
    TabStartX(usize),
    /// A tab's length.
    TabLength(usize),
    /// A cylinder tab's diameter.
    TabDiameter(usize),
    /// A frustum tab's diameter at start X.
    TabStartDiameter(usize),
    /// A frustum tab's diameter at end X.
    TabEndDiameter(usize),
    /// The count of tabs around the axis.
    TabCount(usize),
    /// The start angle of tabs around the axis.
    TabStartAngle(usize),
    /// The offset from the axis of tabs around it.
    TabOffset(usize),
}

/// A check of the draft that failed: the field it marks and the reason, in
/// plain words (section 15 of the specification: no field paths, tabs
/// numbered from 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldProblem {
    /// The field marked.
    pub field: Field,
    /// The reason, shown in red under it.
    pub message: String,
}

/// A length with the minus sign of the specification (U+2212) and 3 places.
fn x(v: f64) -> String {
    format!("{v:.3}").replacen('-', "\u{2212}", 1)
}

fn problem(field: Field, message: impl Into<String>) -> Option<FieldProblem> {
    Some(FieldProblem {
        field,
        message: message.into(),
    })
}

/// Why `v` is not a length above zero, or `None`.
fn positive(v: f64) -> Option<&'static str> {
    if !v.is_finite() {
        Some("Not a number.")
    } else if v <= 0.0 {
        Some("Must be more than 0 mm.")
    } else {
        None
    }
}

/// The first check `draft` fails, in the order of the fields, against the
/// stock where it is known (11.7, Validation; NFR-07). The checks that need
/// the part and the tools are the job of the setup report.
pub fn check_draft(draft: &Workholding, stock: Option<&Stock>) -> Option<FieldProblem> {
    let bounds = stock.and_then(|s| s.bounds(None));
    let stock_x = bounds.map(|b| (b.min.x, b.max.x));
    let cylinder = matches!(stock, Some(Stock::Cylinder { .. }));
    // The diameter a held region can hold at: the cylinder's, or the
    // smaller side of any other stock.
    let across = if let (Some(b), true) = (bounds, stock.is_some()) {
        if cylinder {
            stock.and_then(|s| s.radius(None)).map(|r| 2.0 * r.0)
        } else {
            Some((b.max.y - b.min.y).min(b.max.z - b.min.z))
        }
    } else {
        None
    };
    for end in [End::Start, End::End] {
        let region = match end {
            End::Start => &draft.start,
            End::End => &draft.end,
        };
        let Some(r) = region else { continue };
        if let Some(m) = positive(r.length.0) {
            return problem(Field::Grip(end), m);
        }
        if let Some(d) = r.diameter
            && let Some(m) = positive(d.0)
        {
            return problem(Field::HoldingDiameter(end), m);
        }
        if let Some((lo, hi)) = stock_x
            && r.length.0 > hi - lo
        {
            return problem(
                Field::Grip(end),
                format!("Longer than the stock ({:.3} mm).", hi - lo),
            );
        }
        // A holding diameter makes the held region a cylinder on any stock;
        // without one it is the stock's own outline, so none is needed
        // (decision 0160).
        if let (Some(across), Some(d)) = (across, r.diameter)
            && d.0 > across + 1e-9
        {
            let side = if cylinder { "" } else { "'s smaller side" };
            return problem(
                Field::HoldingDiameter(end),
                format!("Wider than the stock{side} ({across:.3} mm)."),
            );
        }
    }
    let (start_span, end_span) = stock_x.map_or((None, None), |s| draft.held_intervals(s));
    if let (Some((_, a)), Some((b, _))) = (start_span, end_span)
        && a > b
    {
        return problem(
            Field::Grip(End::End),
            "Overlaps the held region at the start of the stock.",
        );
    }
    let radius = across.filter(|_| cylinder).map(|d| d / 2.0);
    let mut occupied: Vec<(f64, f64, usize)> = Vec::new();
    for (i, tab) in draft.tabs.iter().enumerate() {
        if let Some(p) = check_tab(i, tab) {
            return Some(p);
        }
        let (lo, hi) = (tab.x_start.0, tab.x_start.0 + tab.length.0);
        // An automatic tab's position is worked out from the part (decision
        // record 0202); its stored one is not held against it.
        let placed_by_hand = tab.auto.is_none();
        if let Some((a, b)) = stock_x
            && placed_by_hand
            && (lo < a || hi > b)
        {
            return problem(
                Field::TabStartX(i),
                format!("Not inside the stock (X {} to {} mm).", x(a), x(b)),
            );
        }
        for (name, span) in [("start", start_span), ("end", end_span)] {
            if let Some((a, b)) = span
                && placed_by_hand
                && lo < b
                && hi > a
            {
                return problem(
                    Field::TabStartX(i),
                    format!("Overlaps the held region at the {name} of the stock."),
                );
            }
        }
        if let Some(radius) = radius {
            let (reach, field) = match tab.layout {
                TabLayout::OnAxis => (
                    tab.shape.max_diameter().0 / 2.0,
                    widest_field(i, &tab.shape),
                ),
                TabLayout::AroundAxis { radial_offset, .. } => (
                    radial_offset.0 + tab.shape.max_diameter().0 / 2.0,
                    Field::TabOffset(i),
                ),
                _ => (0.0, Field::TabStartX(i)),
            };
            if reach > radius + 1e-9 {
                return problem(
                    field,
                    format!(
                        "Reaches {reach:.3} mm from the axis, outside the stock (radius \
                         {radius:.3} mm)."
                    ),
                );
            }
        }
        if tab.layout == TabLayout::OnAxis && placed_by_hand {
            if let Some((_, _, other)) = occupied.iter().find(|(a, b, _)| lo < *b && hi > *a) {
                return problem(Field::TabStartX(i), format!("Overlaps tab {}.", other + 1));
            }
            occupied.push((lo, hi, i));
        }
    }
    None
}

fn widest_field(i: usize, shape: &TabShape) -> Field {
    match *shape {
        TabShape::Frustum {
            start_diameter,
            end_diameter,
        } if start_diameter.0 >= end_diameter.0 => Field::TabStartDiameter(i),
        TabShape::Frustum { .. } => Field::TabEndDiameter(i),
        _ => Field::TabDiameter(i),
    }
}

/// The checks on one tab's own values, in the order of its fields.
fn check_tab(i: usize, tab: &Tab) -> Option<FieldProblem> {
    if !tab.x_start.0.is_finite() {
        return problem(Field::TabStartX(i), "Not a number.");
    }
    if let Some(m) = positive(tab.length.0) {
        return problem(Field::TabLength(i), m);
    }
    match tab.shape {
        TabShape::Cylinder { diameter } => {
            if let Some(m) = positive(diameter.0) {
                return problem(Field::TabDiameter(i), m);
            }
        }
        TabShape::Frustum {
            start_diameter,
            end_diameter,
        } => {
            if let Some(m) = positive(start_diameter.0) {
                return problem(Field::TabStartDiameter(i), m);
            }
            if let Some(m) = positive(end_diameter.0) {
                return problem(Field::TabEndDiameter(i), m);
            }
            if (start_diameter.0 - end_diameter.0).abs() < FRUSTUM_MIN_DIFFERENCE {
                return problem(
                    Field::TabEndDiameter(i),
                    "The same as the diameter at start X; use a cylinder.",
                );
            }
        }
        _ => {}
    }
    if let TabLayout::AroundAxis {
        count,
        start_angle,
        radial_offset,
    } = tab.layout
    {
        if count == 0 || count > MAX_TAB_COUNT {
            return problem(Field::TabCount(i), format!("From 1 to {MAX_TAB_COUNT}."));
        }
        if !start_angle.0.is_finite() {
            return problem(Field::TabStartAngle(i), "Not a number.");
        }
        if !radial_offset.0.is_finite() {
            return problem(Field::TabOffset(i), "Not a number.");
        }
        if radial_offset.0 < 0.0 {
            return problem(Field::TabOffset(i), "Must be 0 mm or more.");
        }
    }
    None
}

/// The hint of the holding diameter field when none is set: what the held
/// region is then, the stock's own diameter on round stock, else its outline
/// (SET-09; decision 0160).
pub fn holding_hint(stock: Option<&Stock>) -> String {
    match stock {
        Some(Stock::Cylinder { diameter, .. }) => {
            format!("Stock diameter ({:.3} mm)", diameter.0)
        }
        Some(_) => "Stock outline".to_owned(),
        None => "Stock diameter".to_owned(),
    }
}

/// The X span a held region covers on `stock`, as the read-only "Held span"
/// line ("X −16.000 to −10.000"); `None` when there is no such region or the
/// stock's extent is not known.
pub fn held_span(draft: &Workholding, end: End, stock: &Stock) -> Option<String> {
    let bounds = stock.bounds(None)?;
    let (start, last) = draft.held_intervals((bounds.min.x, bounds.max.x));
    let (lo, hi) = match end {
        End::Start => start,
        End::End => last,
    }?;
    Some(format!("X {} to {}", x(lo), x(hi)))
}

/// The head or the tail zone (decision 0035, owner's answers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// Between the held region at the start (or the start of the stock) and
    /// the part.
    Head,
    /// Between the part and the held region at the end (or the end of the
    /// stock).
    Tail,
}

impl Zone {
    /// The zone's name on a row: "Head", "Tail".
    pub const fn name(self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::Tail => "Tail",
        }
    }

    const fn lower(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Tail => "tail",
        }
    }
}

/// The read-only zone lines of the section (11.7 item 3), head then tail,
/// from the part's extent against the stored held regions: "Head zone X
/// −10.000 to −8.000 mm (2.000 mm)", or "Head zone: no room for a tab".
/// `None` when the stock's extent is not known.
pub fn zone_lines(stored: &Workholding, stock: &Stock, part: &PartExtent) -> Option<[String; 2]> {
    let (head, tail) = openchuck_types::workholding::tab_zones(stored, stock, part)?;
    let line = |zone: Zone, (lo, hi): (f64, f64)| {
        if hi - lo <= 1e-6 {
            format!("{} zone: no room for a tab", zone.name())
        } else {
            format!(
                "{} zone X {} to {} mm ({:.3} mm)",
                zone.name(),
                x(lo),
                x(hi),
                hi - lo
            )
        }
    };
    Some([line(Zone::Head, head), line(Zone::Tail, tail)])
}

/// The zone `tab` lies wholly in, for its collapsed row ("Head"); `None` when
/// it lies in neither or the stock's extent is not known.
pub fn tab_zone(tab: &Tab, stored: &Workholding, stock: &Stock, part: &PartExtent) -> Option<Zone> {
    let (head, tail) = openchuck_types::workholding::tab_zones(stored, stock, part)?;
    let (lo, hi) = (tab.x_start.0, tab.x_start.0 + tab.length.0);
    let inside = |(a, b): (f64, f64)| lo >= a - 1e-6 && hi <= b + 1e-6 && b - a > 1e-6;
    if inside(head) {
        Some(Zone::Head)
    } else if inside(tail) {
        Some(Zone::Tail)
    } else {
        None
    }
}

/// The line that says why a zone has no default tab (11.7, "No default"), or
/// `None` when it has one.
pub fn no_default_line(
    zone: Zone,
    default: &crate::workholding::ZoneDefault,
    stored: &Workholding,
) -> Option<String> {
    use crate::workholding::ZoneDefault;
    match default {
        ZoneDefault::Tab(_) => None,
        ZoneDefault::NoRoom => {
            let held = match zone {
                Zone::Head => stored.start.is_some(),
                Zone::Tail => stored.end.is_some(),
            };
            Some(format!(
                "No room for a default tab at the {}: {}.",
                zone.lower(),
                if held {
                    "the held region reaches the part"
                } else {
                    "the stock ends at the part"
                }
            ))
        }
        ZoneDefault::Point { x: at } => Some(format!(
            "The part comes to a point at X {} mm, so there is no diameter to base a default \
             tab on; add the tab by hand.",
            x(*at)
        )),
        ZoneDefault::NoFit {
            stock_diameter,
            small,
            x: at,
        } => Some(format!(
            "The stock ({stock_diameter:.3} mm across) leaves no room for a default tab of \
             {small:.3} mm at X {} mm; add the tab by hand.",
            x(*at)
        )),
    }
}

/// The read-only Narrowest line of a tab compared with the tools of the
/// setup's operations (11.7 item 4), and whether it is a warning: "3.000 mm,
/// narrower than the tool T2 Flat Ø6 (6.000 mm) of 1 Rough".
pub fn narrowest_line(tab: &Tab, tools: &[crate::workholding::SetupTool]) -> (String, bool) {
    let d = narrowest(tab);
    let widest = tools
        .iter()
        .filter(|t| t.diameter > d + 1e-9)
        .max_by(|a, b| a.diameter.total_cmp(&b.diameter));
    match widest {
        Some(t) => (
            format!(
                "{d:.3} mm, narrower than the tool {} ({:.3} mm) of {}",
                t.tool, t.diameter, t.operation
            ),
            true,
        ),
        None if tools.is_empty() => (format!("{d:.3} mm"), false),
        None => (
            format!("{d:.3} mm, as wide as every tool of the setup's operations or wider"),
            false,
        ),
    }
}

/// The warning for a held region's grip length when it reaches into the part
/// (SET-09, OI-554): how much of the part the grip covers, in plain words.
/// `None` when the grip is clear of the part, or the stock or part is not
/// known. A warning, not a refusal: generation is still allowed (NFR-12).
pub fn grip_warning(
    draft: &Workholding,
    end: End,
    stock: Option<&Stock>,
    part: Option<&PartExtent>,
) -> Option<String> {
    openchuck_types::workholding::check_held_regions(draft, stock, part)
        .into_iter()
        .find_map(|kind| match kind {
            SetupWarningKind::HeldRegionCoversPart { at_end, x: span }
                if at_end == matches!(end, End::End) =>
            {
                Some(format!(
                    "The chuck grips {:.3} mm of the part here (X {} to {} mm), so that length is never \
                     machined. Shorten the grip if it should be cut.",
                    span.1 - span.0,
                    x(span.0),
                    x(span.1)
                ))
            }
            _ => None,
        })
}

/// A new held region as the check box turns it on: 10 mm, at the stock's own
/// diameter.
pub fn new_held_region() -> HeldRegion {
    HeldRegion {
        length: Millimetres(10.0),
        diameter: None,
    }
}

/// `v` rounded to the 3 places the workholding section's fields show (14.3):
/// a value worked out for the user, such as a new tab's diameter, is stored
/// as the field shows it (issue #184).
pub fn to_field_places(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// The tab "Add tab" adds: a cylinder on the axis, 5 mm long and a quarter of
/// the stock's diameter across (at least 1 mm), rounded to the field's 3
/// places (issue #184), at the inner edge of the held region at the start, or
/// at the start of the stock.
pub fn new_tab(draft: &Workholding, stock: Option<&Stock>) -> Tab {
    let bounds = stock.and_then(|s| s.bounds(None));
    let x_start = bounds.map_or(0.0, |b| {
        draft
            .held_intervals((b.min.x, b.max.x))
            .0
            .map_or(b.min.x, |(_, hi)| hi)
    });
    let diameter = stock
        .and_then(|s| s.radius(None))
        .map_or(4.0, |r| to_field_places((2.0 * r.0 / 4.0).max(1.0)));
    Tab {
        auto: None,
        x_start: Millimetres(x_start),
        length: Millimetres(5.0),
        shape: TabShape::Cylinder {
            diameter: Millimetres(diameter),
        },
        layout: TabLayout::OnAxis,
    }
}

/// The tab "Head tab" or "Tail tab" adds (issue #182; decision record
/// 0202): the shape and diameter of [`new_tab`], placed automatically over
/// the whole zone at that end of the part ([`auto_tab_span`]), and kept
/// there as the part, the stock and the held regions change. Each press adds
/// another. Fails with the reason for the button's tooltip when the setup has
/// no part or stock, the part is not read yet, or there is no room at that
/// end.
pub fn end_tab(
    draft: &Workholding,
    stock: Option<&Stock>,
    part: Option<&PartExtent>,
    has_part: bool,
    end: TabEnd,
) -> Result<Tab, String> {
    if !has_part {
        return Err("Import a model first".to_owned());
    }
    let stock = stock
        .ok_or_else(|| "Set the stock first: held regions and tabs are placed on it.".to_owned())?;
    let part = part.ok_or_else(|| "Working out where the part is\u{2026}".to_owned())?;
    let (lo, hi) = auto_tab_span(draft, stock, part, end).ok_or_else(|| no_room(draft, end))?;
    Ok(Tab {
        x_start: Millimetres(lo),
        length: Millimetres(hi - lo),
        auto: Some(end),
        ..new_tab(draft, Some(stock))
    })
}

/// A hand edit of a tab's start or length (issue #186; decision record
/// 0202): the tab is no longer placed automatically, and keeps the other
/// value as it was `shown` (an automatic tab is shown where the part puts
/// it, not where it was last stored).
pub fn set_tab_span(tab: &mut Tab, shown: &Tab, x_start: Option<f64>, length: Option<f64>) {
    tab.auto = None;
    tab.x_start = Millimetres(x_start.unwrap_or(shown.x_start.0));
    tab.length = Millimetres(length.unwrap_or(shown.length.0));
}

/// Ticks "Place automatically" at `end`, or unticks it with `None`, which
/// keeps the position and length the tab is `shown` at (decision record
/// 0202).
pub fn set_tab_auto(tab: &mut Tab, shown: &Tab, end: Option<TabEnd>) {
    tab.x_start = shown.x_start;
    tab.length = shown.length;
    tab.auto = end;
}

/// Why there is no room for a tab at `end` of the part, in words.
pub fn no_room(draft: &Workholding, end: TabEnd) -> String {
    let (held, stock_end) = match end {
        TabEnd::Head => (draft.start.is_some(), "the stock starts"),
        TabEnd::Tail => (draft.end.is_some(), "the stock ends"),
    };
    if held {
        format!(
            "No room at the {}: the held region reaches the part.",
            end.name()
        )
    } else {
        format!("No room at the {}: {stock_end} at the part.", end.name())
    }
}

/// Changes a tab between cylinder and frustum: a frustum keeps the diameter
/// as its diameter at start X and makes the far end 1.5 times it; a cylinder
/// keeps the diameter at the start.
pub fn set_shape(tab: &mut Tab, cylinder: bool) {
    tab.shape = match (cylinder, tab.shape) {
        (true, TabShape::Frustum { start_diameter, .. }) => TabShape::Cylinder {
            diameter: start_diameter,
        },
        (false, TabShape::Cylinder { diameter }) => TabShape::Frustum {
            start_diameter: diameter,
            end_diameter: Millimetres(diameter.0 * 1.5),
        },
        (_, shape) => shape,
    };
}

/// Changes a tab between on the axis and around it, with the defaults of
/// 11.7 (2 tabs, 0 degrees, 5 mm from the axis).
pub fn set_layout(tab: &mut Tab, around: bool) {
    tab.layout = match (around, tab.layout) {
        (false, TabLayout::AroundAxis { .. }) => TabLayout::OnAxis,
        (true, TabLayout::OnAxis) => TabLayout::AroundAxis {
            count: 2,
            start_angle: Degrees(0.0),
            radial_offset: Millimetres(5.0),
        },
        (_, layout) => layout,
    };
}

/// Which end of a frustum faces the part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartFacing {
    /// The small end (a break point at the part).
    Small,
    /// The large end, shown in amber.
    Large,
}

impl PartFacing {
    /// The derived line under a frustum's diameters.
    pub fn text(self) -> &'static str {
        match self {
            Self::Small => "Small end towards the part",
            Self::Large => "Large end towards the part",
        }
    }
}

/// Which end of a frustum tab faces the part, given the part's X extent;
/// `None` for a cylinder or a frustum whose ends are equal.
pub fn part_facing(tab: &Tab, part_x: (f64, f64)) -> Option<PartFacing> {
    let TabShape::Frustum {
        start_diameter,
        end_diameter,
    } = tab.shape
    else {
        return None;
    };
    if (start_diameter.0 - end_diameter.0).abs() < FRUSTUM_MIN_DIFFERENCE {
        return None;
    }
    // A tab below the part faces it with its high-X end.
    let toward_high = tab.x_start.0 + tab.length.0 / 2.0 < (part_x.0 + part_x.1) / 2.0;
    let facing = if toward_high {
        end_diameter.0
    } else {
        start_diameter.0
    };
    let other = if toward_high {
        start_diameter.0
    } else {
        end_diameter.0
    };
    Some(if facing < other {
        PartFacing::Small
    } else {
        PartFacing::Large
    })
}

/// The smallest diameter of a tab, for the read-only "Narrowest" line.
pub fn narrowest(tab: &Tab) -> f64 {
    match tab.shape {
        TabShape::Cylinder { diameter } => diameter.0,
        TabShape::Frustum {
            start_diameter,
            end_diameter,
        } => start_diameter.0.min(end_diameter.0),
        _ => f64::NAN,
    }
}

/// The tolerance within which a tab equals a default, in millimetres.
const DEFAULT_TOLERANCE: f64 = 1e-3;

fn same_as_default(tab: &Tab, default: &Tab) -> bool {
    let close = |a: f64, b: f64| (a - b).abs() <= DEFAULT_TOLERANCE;
    let shape = match (tab.shape, default.shape) {
        (TabShape::Cylinder { diameter: a }, TabShape::Cylinder { diameter: b }) => close(a.0, b.0),
        (
            TabShape::Frustum {
                start_diameter: a0,
                end_diameter: a1,
            },
            TabShape::Frustum {
                start_diameter: b0,
                end_diameter: b1,
            },
        ) => close(a0.0, b0.0) && close(a1.0, b1.0),
        _ => false,
    };
    shape
        && tab.layout == default.layout
        && close(tab.x_start.0, default.x_start.0)
        && close(tab.length.0, default.length.0)
}

/// For each tab, whether it still equals, to 0.001 mm, one of the default
/// tabs: the "Default" chip. Editing a value removes it; nothing records
/// where a tab came from (11.7).
pub fn default_chips(tabs: &[Tab], defaults: &[Tab]) -> Vec<bool> {
    tabs.iter()
        .map(|t| defaults.iter().any(|d| same_as_default(t, d)))
        .collect()
}

/// The warning shown while a setup stores no tabs (11.7). With a held region
/// the part stays joined to the held bar, so the words say what is true
/// then: only a cut that frees the part from the held end drops it.
pub fn no_tabs_warning(draft: &Workholding) -> &'static str {
    if draft.start.is_some() || draft.end.is_some() {
        "No tabs are stored. The part stays joined to the stock at the held end. Add tabs if \
         an operation will cut it free of the held end, so it cannot drop or shift."
    } else {
        "No tabs are stored. Without tabs the part is not joined to the stock when its last \
         support is cut."
    }
}

/// Whether "Add default tabs" can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultTabsState {
    /// Enabled.
    Available,
    /// Disabled, with the reason for its tooltip.
    Refused(&'static str),
}

/// The state of "Add default tabs": enabled only while the draft has no tabs
/// and its held regions are the stored ones, since the defaults fit the
/// stored held regions.
pub fn default_tabs_state(stored: &Workholding, draft: &Workholding) -> DefaultTabsState {
    if !draft.tabs.is_empty() {
        DefaultTabsState::Refused("Remove the tabs first")
    } else if draft.start != stored.start || draft.end != stored.end {
        DefaultTabsState::Refused(
            "Apply the held regions first: the default tabs fit the stored ones.",
        )
    } else {
        DefaultTabsState::Available
    }
}

/// The note shown when a change to the stored workholding replaced a draft
/// with unapplied changes (11.7).
pub const DISCARDED_NOTE: &str = "The stored workholding changed (an undo, a redo or another \
                                  edit), so your unapplied changes were discarded.";

/// The draft a setup's workholding is edited in (11.7): kept until applied or
/// reverted, and replaced by the stored values, with a note, when the stored
/// workholding changes under unapplied changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    stored: Workholding,
    draft: Workholding,
    note: Option<String>,
}

impl Draft {
    /// A draft equal to `stored`.
    pub fn new(stored: &Workholding) -> Self {
        Self {
            stored: stored.clone(),
            draft: stored.clone(),
            note: None,
        }
    }

    /// The draft's values.
    pub fn workholding(&self) -> &Workholding {
        &self.draft
    }

    /// The stored values the draft started from.
    pub fn stored(&self) -> &Workholding {
        &self.stored
    }

    /// Edits the draft; the note about a discard goes.
    pub fn edit(&mut self) -> &mut Workholding {
        self.note = None;
        &mut self.draft
    }

    /// True when the draft differs from the stored values.
    pub fn is_changed(&self) -> bool {
        self.draft != self.stored
    }

    /// Puts the stored values back.
    pub fn revert(&mut self) {
        self.draft = self.stored.clone();
    }

    /// The note under the header, if any.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Follows the project: when `stored` is not what the draft started from,
    /// the draft becomes it. Returns the discard note when that threw away
    /// unapplied changes (the draft just applied is not one).
    pub fn sync(&mut self, stored: &Workholding) -> Option<String> {
        if *stored == self.stored {
            return None;
        }
        let discarded = self.is_changed() && self.draft != *stored;
        self.stored = stored.clone();
        self.draft = stored.clone();
        if discarded {
            self.note = Some(DISCARDED_NOTE.to_owned());
        }
        self.note.clone()
    }
}

/// How many segments the solids and their end circles have.
const SEGMENTS: u32 = 96;

fn ring(base: Point3, radius: f64, segments: u32) -> Vec<Point3> {
    (0..=segments)
        .map(|i| {
            let a = std::f64::consts::TAU * f64::from(i) / f64::from(segments);
            Point3::new(base.x, base.y + radius * a.cos(), base.z + radius * a.sin())
        })
        .collect()
}

/// The outline of the box section of `b` at `x`, closed.
fn square(x: f64, b: openchuck_types::geometry::Aabb) -> Vec<Point3> {
    vec![
        Point3::new(x, b.min.y, b.min.z),
        Point3::new(x, b.max.y, b.min.z),
        Point3::new(x, b.max.y, b.max.z),
        Point3::new(x, b.min.y, b.max.z),
        Point3::new(x, b.min.y, b.min.z),
    ]
}

/// The meshes, end circles and solids [`display_items`] gives.
pub type DisplayItems = (Vec<SceneMesh>, Vec<ScenePolyline>, Vec<SceneSolid>);

/// The shaded solids of `workholding` for the viewport (7.5), each with
/// both end circles as polylines of its role, and with which held region or
/// tab it is and its tooltip. The view draws each solid's outline and a held
/// region's cross-hatch in screen space, so the two roles differ by more
/// than colour (NFR-15).
pub fn display_items(
    workholding: &Workholding,
    stock: Option<&Stock>,
    part: &PartExtent,
) -> Result<DisplayItems, AppError> {
    let mut meshes = Vec::new();
    let mut lines = Vec::new();
    let mut solids = Vec::new();
    // The solids of one tab around the axis follow one another.
    let mut previous: Option<(WorkholdingItem, u32)> = None;
    for (name, solid) in crate::workholding::labelled_solids(workholding, stock, part)? {
        let item = item_of(&name);
        let role = if matches!(item, WorkholdingItem::Tab(_)) {
            SceneRole::Tab
        } else {
            SceneRole::HeldRegion
        };
        let (base, top) = (solid.base(), solid.top());
        let (base_line, top_line, section) = match solid.as_cuboid() {
            // A held region on stock that is not round: the stock's box
            // section at each end (decision 0160). The Setup panes' front
            // view shows its height along Z.
            Some(b) => (
                square(base.x, b),
                square(top.x, b),
                SolidSection {
                    x: (b.min.x, b.max.x),
                    radii: ((b.max.z - b.min.z) / 2.0, (b.max.z - b.min.z) / 2.0),
                    centre: ((b.min.y + b.max.y) / 2.0, (b.min.z + b.max.z) / 2.0),
                },
            ),
            None => (
                ring(base, solid.base_radius(), SEGMENTS),
                ring(top, solid.top_radius(), SEGMENTS),
                SolidSection {
                    x: (base.x, top.x),
                    radii: (solid.base_radius(), solid.top_radius()),
                    centre: (base.y, base.z),
                },
            ),
        };
        lines.push(ScenePolyline {
            role,
            points: base_line,
        });
        lines.push(ScenePolyline {
            role,
            points: top_line,
        });
        let nth = match previous {
            Some((last, k)) if last == item => k + 1,
            _ => 0,
        };
        previous = Some((item, nth));
        solids.push(SceneSolid {
            mesh: meshes.len(),
            item,
            description: describe(workholding, item, &solid, nth),
            section,
        });
        meshes.push(SceneMesh {
            role,
            model: None,
            mesh: solid.mesh(SEGMENTS),
            feature_edges: Vec::new(),
            brep: None,
        });
    }
    Ok((meshes, lines, solids))
}

/// The item a solid's name from `labelled_solids` stands for.
fn item_of(name: &str) -> WorkholdingItem {
    if let Some(n) = name
        .strip_prefix("tab ")
        .and_then(|n| n.parse::<usize>().ok())
    {
        WorkholdingItem::Tab(n.saturating_sub(1))
    } else if name.contains("end") {
        WorkholdingItem::End
    } else {
        WorkholdingItem::Start
    }
}

/// The tooltip of a solid (7.5): "Tab 2 · frustum · X 14.000 to 16.000 mm ·
/// Ø 4.000 to Ø 8.000 mm", with "1 of 3 at 90.00°" for a tab around the
/// axis, or "Held region at the start · 6.000 mm · Ø 26.000 mm (stock)".
/// `nth` is the solid's place among a tab's solids around the axis.
fn describe(
    workholding: &Workholding,
    item: WorkholdingItem,
    solid: &openchuck_path::Solid,
    nth: u32,
) -> String {
    let (lo, hi) = (solid.base().x, solid.top().x);
    match item {
        WorkholdingItem::Start | WorkholdingItem::End => {
            let (end, region) = match item {
                WorkholdingItem::Start => ("start", workholding.start.as_ref()),
                _ => ("end", workholding.end.as_ref()),
            };
            if let Some(b) = solid.as_cuboid() {
                return format!(
                    "Held region at the {end} · {:.3} mm · stock outline {:.3} × {:.3} mm",
                    hi - lo,
                    b.max.y - b.min.y,
                    b.max.z - b.min.z
                );
            }
            let source = match region.and_then(|r| r.diameter) {
                Some(_) => "",
                None => " (stock)",
            };
            format!(
                "Held region at the {end} · {:.3} mm · Ø {:.3} mm{source}",
                hi - lo,
                2.0 * solid.base_radius()
            )
        }
        WorkholdingItem::Tab(i) => {
            let tab = workholding.tabs.get(i);
            let (d0, d1) = (2.0 * solid.base_radius(), 2.0 * solid.top_radius());
            let shape = match tab.map(|t| &t.shape) {
                Some(TabShape::Cylinder { .. }) => {
                    format!("cylinder · X {lo:.3} to {hi:.3} mm · Ø {d0:.3} mm")
                }
                _ => format!("frustum · X {lo:.3} to {hi:.3} mm · Ø {d0:.3} to Ø {d1:.3} mm"),
            };
            let around = match tab.map(|t| &t.layout) {
                Some(TabLayout::AroundAxis {
                    count, start_angle, ..
                }) => format!(
                    " · {} of {count} at {:.2}°",
                    nth + 1,
                    start_angle.0 + 360.0 * f64::from(nth) / f64::from((*count).max(1))
                ),
                _ => String::new(),
            };
            format!("Tab {} · {shape}{around}", i + 1)
        }
    }
}

/// A scene holding only the held regions and tabs of `workholding`: what the
/// view draws for a draft (7.5).
pub fn display_scene(
    workholding: &Workholding,
    stock: Option<&Stock>,
    part: &PartExtent,
) -> Result<SceneView, AppError> {
    let (meshes, lines, solids) = display_items(workholding, stock, part)?;
    Ok(SceneView {
        meshes,
        lines,
        solids,
        ..SceneView::default()
    })
}

/// The legend entries of the roles present in `view`, in the order held
/// region, tab (7.5).
pub fn legend(view: &SceneView) -> Vec<&'static str> {
    let has = |role| view.meshes.iter().any(|m| m.role == role);
    let mut entries = Vec::new();
    if has(SceneRole::HeldRegion) {
        entries.push("Held region");
    }
    if has(SceneRole::Tab) {
        entries.push("Tab");
    }
    entries
}

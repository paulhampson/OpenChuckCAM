// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workholding and retaining tabs of a setup (SET-09, OP-21; decision records
//! 0035 and 0041).
//!
//! Positions are along the rotary axis (model X), in the model frame, like
//! the stock's. A **held region** is the length of the stock the chuck grips
//! at one end; it is never machined. A **tab** is a small solid that keeps
//! the finished part joined to the stock. Both are part of the target for the
//! whole job; this module holds the data and its validation only. The target
//! surface is built in `openchuck-path` (slice 21b).
//!
//! No default values are chosen for tabs (OI-158): every dimension is given.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result, require_finite, require_positive};
use crate::setup::{PartExtent, SetupWarningKind};
use crate::stock::Stock;
use crate::units::{Degrees, Millimetres};

/// The most tabs around the axis one entry may ask for.
pub const MAX_TAB_COUNT: u32 = 64;

/// Two frustum end diameters closer than this are the same diameter, so the
/// shape is a cylinder (millimetres).
pub const FRUSTUM_MIN_DIFFERENCE: f64 = 1e-3;

/// A closed interval of model X, as (low, high), in millimetres.
pub type Interval = (f64, f64);

/// The length of the stock a chuck or other workholding grips at one end
/// (SET-09). The region is a cylinder about the rotary axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeldRegion {
    /// How far the grip reaches from the end of the stock along the axis.
    pub length: Millimetres,
    /// The diameter of the held region, where the grip is narrower than the
    /// stock; `None` holds the stock at its own diameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diameter: Option<Millimetres>,
}

/// The shape of one retaining tab (OP-21).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum TabShape {
    /// A cylinder.
    Cylinder {
        /// Diameter.
        diameter: Millimetres,
    },
    /// A frustum: a cone with a flat end. The diameters are named by their
    /// position along X, so the smaller end can face the part (a break point
    /// at the part) whichever end of the stock the tab is at.
    Frustum {
        /// Diameter at the tab's start (the lower X end).
        start_diameter: Millimetres,
        /// Diameter at the tab's far end (the higher X end).
        end_diameter: Millimetres,
    },
}

impl TabShape {
    /// The largest diameter of the shape.
    pub fn max_diameter(&self) -> Millimetres {
        match *self {
            Self::Cylinder { diameter } => diameter,
            Self::Frustum {
                start_diameter,
                end_diameter,
            } => Millimetres(start_diameter.0.max(end_diameter.0)),
        }
    }
}

/// Where a tab's centre line lies in the cross-section.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum TabLayout {
    /// One tab on the rotary axis.
    OnAxis,
    /// `count` tabs, centre lines parallel to the axis at `radial_offset`
    /// from it, evenly spaced about it from `start_angle`.
    AroundAxis {
        /// Number of tabs, 1 to [`MAX_TAB_COUNT`].
        count: u32,
        /// Angle of the first tab about the axis, measured from model +Y
        /// towards +Z.
        start_angle: Degrees,
        /// Distance of each tab's centre line from the axis.
        radial_offset: Millimetres,
    },
}

/// One retaining tab (OP-21): a cylinder or frustum, `length` long from
/// `x_start` along +X.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tab {
    /// Model X of the tab's start (its lower X end).
    pub x_start: Millimetres,
    /// Length along X.
    pub length: Millimetres,
    /// Cylinder or frustum, with its diameters.
    pub shape: TabShape,
    /// On the axis or around it.
    pub layout: TabLayout,
}

/// The workholding and tabs of one setup. The default holds nothing and has
/// no tabs, which leaves generated G-code unchanged.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workholding {
    /// The grip at the lower X end of the stock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<HeldRegion>,
    /// The grip at the higher X end of the stock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<HeldRegion>,
    /// Retaining tabs, in the order given.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<Tab>,
}

impl Workholding {
    /// True when there is no held region and no tab.
    pub fn is_empty(&self) -> bool {
        self.start.is_none() && self.end.is_none() && self.tabs.is_empty()
    }

    /// The X interval each held region covers, given the stock's X extent.
    pub fn held_intervals(&self, stock_x: Interval) -> (Option<Interval>, Option<Interval>) {
        (
            self.start.map(|r| (stock_x.0, stock_x.0 + r.length.0)),
            self.end.map(|r| (stock_x.1 - r.length.0, stock_x.1)),
        )
    }
}

/// Fails if the workholding is impossible for `stock` (SET-09, OP-21, NFR-07).
///
/// Values are always checked for being finite and positive. Where the stock's
/// extent is known (a cylinder or box), each held region and tab must lie in
/// it, the two held regions and the tabs must not overlap, and diameters must
/// fit the stock. A stock that needs the part (the bounding-box offset) or is
/// not available yet is not checked for extent. Whether a tab lies between
/// the held region and the part's far end needs the part, so it is not checked
/// here (OI-180).
pub fn validate_workholding(
    workholding: &Workholding,
    stock: Option<&Stock>,
    field: &str,
) -> Result<()> {
    for (name, region) in [("start", &workholding.start), ("end", &workholding.end)] {
        if let Some(r) = region {
            require_positive(&format!("{field}.workholding.{name}.length"), r.length.0)?;
            if let Some(d) = r.diameter {
                require_positive(&format!("{field}.workholding.{name}.diameter"), d.0)?;
            }
        }
    }
    for (i, tab) in workholding.tabs.iter().enumerate() {
        validate_tab(tab, &format!("{field}.workholding.tabs[{i}]"))?;
    }

    let extent = stock.and_then(|s| s.bounds(None));
    let Some(bounds) = extent else {
        return Ok(());
    };
    let stock_x = (bounds.min.x, bounds.max.x);
    let radius = stock.and_then(|s| s.radius(None)).map(|r| r.0);

    let (start, end) = workholding.held_intervals(stock_x);
    for (name, region, interval) in [
        ("start", &workholding.start, start),
        ("end", &workholding.end, end),
    ] {
        if let (Some(r), Some((lo, hi))) = (region, interval) {
            let f = format!("{field}.workholding.{name}.length");
            if r.length.0 > stock_x.1 - stock_x.0 {
                return Err(Error::invalid(
                    f,
                    format!(
                        "{:.3} mm is longer than the stock ({:.3} mm)",
                        r.length.0,
                        stock_x.1 - stock_x.0
                    ),
                ));
            }
            debug_assert!(lo <= hi);
            if let (Some(d), Some(radius)) = (r.diameter, radius)
                && d.0 > 2.0 * radius + 1e-9
            {
                return Err(Error::invalid(
                    format!("{field}.workholding.{name}.diameter"),
                    format!(
                        "{:.3} mm is wider than the stock ({:.3} mm)",
                        d.0,
                        2.0 * radius
                    ),
                ));
            }
        }
    }
    if let (Some((_, a)), Some((b, _))) = (start, end)
        && a > b
    {
        return Err(Error::invalid(
            format!("{field}.workholding.end.length"),
            "the held regions at the two ends of the stock overlap",
        ));
    }

    let mut occupied: Vec<(f64, f64, String)> = Vec::new();
    for (i, tab) in workholding.tabs.iter().enumerate() {
        let f = format!("{field}.workholding.tabs[{i}]");
        let (lo, hi) = (tab.x_start.0, tab.x_start.0 + tab.length.0);
        if lo < stock_x.0 || hi > stock_x.1 {
            return Err(Error::invalid(
                format!("{f}.x_start"),
                format!(
                    "the tab (X {lo:.3} to {hi:.3}) is not inside the stock (X {:.3} to {:.3})",
                    stock_x.0, stock_x.1
                ),
            ));
        }
        for (name, interval) in [("start", start), ("end", end)] {
            if let Some((a, b)) = interval
                && lo < b
                && hi > a
            {
                return Err(Error::invalid(
                    format!("{f}.x_start"),
                    format!("the tab overlaps the held region at the {name} of the stock"),
                ));
            }
        }
        if let Some(radius) = radius {
            let reach = match tab.layout {
                TabLayout::OnAxis => tab.shape.max_diameter().0 / 2.0,
                TabLayout::AroundAxis { radial_offset, .. } => {
                    radial_offset.0 + tab.shape.max_diameter().0 / 2.0
                }
            };
            if reach > radius + 1e-9 {
                return Err(Error::invalid(
                    format!("{f}.diameter"),
                    format!(
                        "the tab reaches {reach:.3} mm from the axis, outside the stock \
                         (radius {radius:.3} mm)"
                    ),
                ));
            }
        }
        // Tabs on the axis may not share axial space; tabs around the axis
        // may, because they sit at different angles or radii.
        if tab.layout == TabLayout::OnAxis {
            if let Some((_, _, other)) = occupied.iter().find(|(a, b, _)| lo < *b && hi > *a) {
                return Err(Error::invalid(
                    format!("{f}.x_start"),
                    format!("the tab overlaps {other}"),
                ));
            }
            occupied.push((lo, hi, f));
        }
    }
    Ok(())
}

fn validate_tab(tab: &Tab, field: &str) -> Result<()> {
    require_finite(&format!("{field}.x_start"), tab.x_start.0)?;
    require_positive(&format!("{field}.length"), tab.length.0)?;
    match tab.shape {
        TabShape::Cylinder { diameter } => {
            require_positive(&format!("{field}.diameter"), diameter.0)?;
        }
        TabShape::Frustum {
            start_diameter,
            end_diameter,
        } => {
            require_positive(&format!("{field}.start_diameter"), start_diameter.0)?;
            require_positive(&format!("{field}.end_diameter"), end_diameter.0)?;
            if (start_diameter.0 - end_diameter.0).abs() < FRUSTUM_MIN_DIFFERENCE {
                return Err(Error::invalid(
                    format!("{field}.end_diameter"),
                    "the two diameters of a frustum are the same; use a cylinder",
                ));
            }
        }
    }
    if let TabLayout::AroundAxis {
        count,
        start_angle,
        radial_offset,
    } = tab.layout
    {
        if count == 0 || count > MAX_TAB_COUNT {
            return Err(Error::invalid(
                format!("{field}.count"),
                format!("the count must be from 1 to {MAX_TAB_COUNT}"),
            ));
        }
        require_finite(&format!("{field}.start_angle"), start_angle.0)?;
        require_finite(&format!("{field}.radial_offset"), radial_offset.0)?;
        if radial_offset.0 < 0.0 {
            return Err(Error::invalid(
                format!("{field}.radial_offset"),
                "the offset from the axis must be zero or more",
            ));
        }
    }
    Ok(())
}

/// The head zone and the tail zone, where tabs may lie: from the inner edge
/// of the held region (or the end of the stock) to the part, and from the
/// part to the inner edge of the other held region (or end of the stock).
/// A zone is never inverted: where a held region reaches into the part, the
/// zone on that side is empty (low equals high), so no tab fits in it.
/// `None` when the stock's extent is not known.
pub fn tab_zones(
    workholding: &Workholding,
    stock: &Stock,
    part: &PartExtent,
) -> Option<(Interval, Interval)> {
    let bounds = stock.bounds(Some(part))?;
    let (held_start, held_end) = workholding.held_intervals((bounds.min.x, bounds.max.x));
    let head_from = held_start.map_or(bounds.min.x, |(_, hi)| hi);
    let tail_to = held_end.map_or(bounds.max.x, |(lo, _)| lo);
    let (part_lo, part_hi) = (part.bounds.min.x, part.bounds.max.x);
    Some((
        (head_from.min(part_lo), part_lo),
        (part_hi, tail_to.max(part_hi)),
    ))
}

/// Tabs on a rotary part are restricted to the head and tail zones (OP-21,
/// decision record 0035): the warnings for each tab that is not wholly inside
/// one, in the order of the tabs. Needs the part's extent, so it is a check of
/// the setup (reported with [`crate::setup::check_setup`]), not of the data.
pub fn check_tab_zones(
    workholding: &Workholding,
    stock: Option<&Stock>,
    part: Option<&PartExtent>,
) -> Vec<SetupWarningKind> {
    let (Some(stock), Some(part)) = (stock, part) else {
        return Vec::new();
    };
    let Some((head, tail)) = tab_zones(workholding, stock, part) else {
        return Vec::new();
    };
    // A small allowance for rounding in the part's extent.
    const EPS: f64 = 1e-9;
    let inside = |lo: f64, hi: f64, z: Interval| lo >= z.0 - EPS && hi <= z.1 + EPS;
    workholding
        .tabs
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let (lo, hi) = (t.x_start.0, t.x_start.0 + t.length.0);
            (!inside(lo, hi, head) && !inside(lo, hi, tail)).then_some(
                SetupWarningKind::TabOutsideEndZones {
                    tab: i + 1,
                    x: (lo, hi),
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;
    use crate::geometry::{Aabb, Point3};

    fn tab(x_start: f64, length: f64) -> Tab {
        Tab {
            x_start: Millimetres(x_start),
            length: Millimetres(length),
            shape: TabShape::Cylinder {
                diameter: Millimetres(4.0),
            },
            layout: TabLayout::OnAxis,
        }
    }

    #[test]
    fn req_op_21_tabs_are_restricted_to_the_head_and_tail() {
        // Stock X 0 to 100, part X 30 to 80, 15 mm held at the start.
        let stock = Stock::Cylinder {
            diameter: Millimetres(40.0),
            length: Millimetres(100.0),
            x_start: Millimetres(0.0),
        };
        let part = PartExtent {
            bounds: Aabb {
                min: Point3::new(30.0, -10.0, -10.0),
                max: Point3::new(80.0, 10.0, 10.0),
            },
            radius: Millimetres(10.0),
        };
        let held = Some(HeldRegion {
            length: Millimetres(15.0),
            diameter: None,
        });
        let w = |tabs: Vec<Tab>| Workholding {
            start: held,
            end: None,
            tabs,
        };
        let check = |tabs| check_tab_zones(&w(tabs), Some(&stock), Some(&part));
        // Head 15 to 30, tail 80 to 100: these two fit.
        assert!(check(vec![tab(16.0, 10.0), tab(80.0, 20.0)]).is_empty());
        // Mid-part, across the part's start, and across the held edge do not.
        let found = check(vec![tab(40.0, 5.0), tab(25.0, 10.0), tab(10.0, 10.0)]);
        assert_eq!(found.len(), 3);
        assert!(matches!(
            found[0],
            SetupWarningKind::TabOutsideEndZones { tab: 1, .. }
        ));
        // A held region reaching into the part leaves an empty zone, not an
        // inverted one, and no tab fits in it.
        let deep = Workholding {
            start: Some(HeldRegion {
                length: Millimetres(40.0),
                diameter: None,
            }),
            ..Workholding::default()
        };
        let (head, _) = tab_zones(&deep, &stock, &part).unwrap();
        assert!(head.0 <= head.1, "{head:?}");
        assert_eq!(
            check_tab_zones(
                &Workholding {
                    tabs: vec![tab(31.0, 5.0)],
                    ..deep
                },
                Some(&stock),
                Some(&part)
            )
            .len(),
            1
        );
        // Nothing is checked without the part.
        assert!(check_tab_zones(&w(vec![tab(40.0, 5.0)]), Some(&stock), None).is_empty());
    }
}

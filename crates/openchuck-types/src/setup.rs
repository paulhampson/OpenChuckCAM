// SPDX-License-Identifier: AGPL-3.0-or-later
//! Where the part sits on the machine, and the checks of a setup against its
//! stock, part and machine (SET-05, SET-06; decision record 0011).
//!
//! A setup's frames follow from two values ([`SetupFrame`]):
//!
//! - the **part offset**: how far this setup moves the part along the rotary
//!   axis (SET-05; decision record 0201). The model frame of a setup has its
//!   origin on the rotary axis at the machine profile's rotary reference
//!   point (the chuck face, say), and the stock, the workholding, the work
//!   zero and the toolpaths are written in it. The part is put in it by its
//!   placement steps (IMP-05), which are shared by every setup and put the
//!   rotary axis on the model X axis, and then moved along X by the part
//!   offset ([`SetupFrame::place_part`]). So the offset moves the part
//!   against the stock and the work zero, and the toolpaths and G-code follow;
//!   a zero offset leaves them as the placement steps alone give them;
//! - the **work offset**: the controller register the program uses (G54 to
//!   G59) and where its zero lies. The work zero is always on the rotary
//!   axis, at model X = `x`, so a program's Y0 Z0 is the axis (the Makera Z1
//!   needs this, OI-40). The operator sets the register on the machine;
//!   [`expected_work_offset`] says what it should hold.
//!
//! [`crate::kinematics::RotaryFrame`] turns these into the machine and work frames.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result, require_finite};
use crate::geometry::{Aabb, Point3};
use crate::kinematics::RotaryFrame;
use crate::machine::{Clearance, MachineProfile, ProfileField, RotaryAxis};
use crate::stock::Stock;
use crate::units::Millimetres;

/// A controller work offset register (SET-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorkOffsetRegister {
    /// G54, the first work offset.
    #[default]
    G54,
    /// G55.
    G55,
    /// G56.
    G56,
    /// G57.
    G57,
    /// G58.
    G58,
    /// G59.
    G59,
}

impl WorkOffsetRegister {
    /// Every register, in order.
    pub const ALL: [Self; 6] = [
        Self::G54,
        Self::G55,
        Self::G56,
        Self::G57,
        Self::G58,
        Self::G59,
    ];

    /// The register's G-code name, such as `G54`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::G54 => "G54",
            Self::G55 => "G55",
            Self::G56 => "G56",
            Self::G57 => "G57",
            Self::G58 => "G58",
            Self::G59 => "G59",
        }
    }

    /// Reads a name such as `g54` or `G55`.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|r| r.name().eq_ignore_ascii_case(text.trim()))
    }
}

impl std::fmt::Display for WorkOffsetRegister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The work coordinate system a setup's program is written in (SET-05).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkOffset {
    /// The controller register.
    pub register: WorkOffsetRegister,
    /// Model X of the work zero, which lies on the rotary axis.
    pub x: Millimetres,
}

/// Where the part and the work zero sit on the machine (SET-05).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupFrame {
    /// How far this setup moves the part along the rotary axis, after its
    /// placement steps: with a zero offset the model origin of the part is at
    /// the rotary reference point. The stock, workholding and work zero stay
    /// where they are, so the toolpaths and G-code follow (decision record
    /// 0201).
    pub part_offset: Millimetres,
    /// The work coordinate system.
    pub work_offset: WorkOffset,
}

impl SetupFrame {
    /// Fails unless both values are finite.
    pub fn validate(&self, field: &str) -> Result<()> {
        require_finite(&format!("{field}.part_offset"), self.part_offset.0)?;
        require_finite(&format!("{field}.work_offset.x"), self.work_offset.x.0)
    }

    /// Where the part point `p`, placed by its placement steps, lies in this
    /// setup: moved along X by the part offset.
    pub fn place_part(&self, p: Point3) -> Point3 {
        if self.moves_part() {
            Point3::new(p.x + self.part_offset.0, p.y, p.z)
        } else {
            p
        }
    }

    /// True when the part offset moves the part. A zero offset (of either
    /// sign) moves nothing, so every coordinate stays bit for bit as the
    /// placement steps gave it (GC-07).
    pub fn moves_part(&self) -> bool {
        self.part_offset.0 != 0.0
    }
}

/// The machine coordinates the work offset register should hold for `setup`
/// at A = 0: the machine position of the work zero. The work zero is fixed
/// in the model frame of the setup, so the part offset does not move it
/// (decision record 0201).
pub fn expected_work_offset(profile: &MachineProfile, setup: &SetupFrame) -> Point3 {
    RotaryFrame::of_profile(profile).on_axis(setup.work_offset.x)
}

/// The extent of a placed part in the model frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartExtent {
    /// The bounding box.
    pub bounds: Aabb,
    /// The largest distance of any point of the part from the rotary axis.
    pub radius: Millimetres,
}

impl PartExtent {
    /// The extent of the same part in the setup of `frame`: moved along X by
    /// its part offset ([`SetupFrame::place_part`]); the radius is unchanged.
    pub fn in_setup(self, frame: &SetupFrame) -> Self {
        Self {
            bounds: Aabb {
                min: frame.place_part(self.bounds.min),
                max: frame.place_part(self.bounds.max),
            },
            radius: self.radius,
        }
    }
}

impl Stock {
    /// The stock's bounding box in the model frame. Offset bounding-box stock
    /// needs the part; imported stock (SET-04) is not measured here.
    pub fn bounds(&self, part: Option<&PartExtent>) -> Option<Aabb> {
        match self {
            Self::Cylinder {
                diameter,
                length,
                x_start,
            } => {
                let r = diameter.0 / 2.0;
                Some(Aabb {
                    min: Point3::new(x_start.0, -r, -r),
                    max: Point3::new(x_start.0 + length.0, r, r),
                })
            }
            Self::Box { bounds } => Some(*bounds),
            Self::OffsetBoundingBox { offset } => part.map(|p| Aabb {
                min: Point3::new(
                    p.bounds.min.x - offset.0,
                    p.bounds.min.y - offset.0,
                    p.bounds.min.z - offset.0,
                ),
                max: Point3::new(
                    p.bounds.max.x + offset.0,
                    p.bounds.max.y + offset.0,
                    p.bounds.max.z + offset.0,
                ),
            }),
            Self::Imported { .. } => None,
        }
    }

    /// The largest distance of the stock from the rotary axis.
    pub fn radius(&self, part: Option<&PartExtent>) -> Option<Millimetres> {
        match self {
            Self::Cylinder { diameter, .. } => Some(Millimetres(diameter.0 / 2.0)),
            Self::Imported { .. } => None,
            _ => self.bounds(part).map(|b| corner_radius(&b)),
        }
    }
}

/// The largest distance from the X axis of a box's corners.
fn corner_radius(b: &Aabb) -> Millimetres {
    let y = b.min.y.abs().max(b.max.y.abs());
    let z = b.min.z.abs().max(b.max.z.abs());
    Millimetres(y.hypot(z))
}

/// A problem found by [`check_setup`]. It is a warning: the user may have
/// reasons, and nothing is refused (work breakdown slice 3 item 5).
#[derive(Debug, Clone, PartialEq)]
pub struct SetupWarning {
    /// What is wrong.
    pub kind: SetupWarningKind,
    /// Machine profile values the check used that are not confirmed on the
    /// machine.
    pub unconfirmed: Vec<ProfileField>,
}

/// What [`check_setup`] found.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SetupWarningKind {
    /// The setup has no stock.
    NoStock,
    /// Bounding-box stock needs a part to measure.
    StockNeedsPart,
    /// Imported stock is SET-04 (P2) and cannot be checked.
    ImportedStock,
    /// The safe cylinder does not enclose the stock.
    ClearanceInsideStock {
        /// Safe cylinder radius.
        radius: Millimetres,
        /// Stock radius.
        stock: Millimetres,
    },
    /// The safe cylinder does not enclose the part.
    ClearanceInsidePart {
        /// Safe cylinder radius.
        radius: Millimetres,
        /// Part radius.
        part: Millimetres,
    },
    /// Some of the part lies outside the stock.
    PartOutsideStock,
    /// A position is outside the machine's travel.
    OutsideTravel {
        /// What it is, in words.
        what: &'static str,
        /// The machine axis.
        axis: &'static str,
        /// Its machine coordinate.
        value: f64,
        /// The travel on that axis.
        min: f64,
        /// The travel on that axis.
        max: f64,
    },
    /// A retaining tab is not in the head zone (between the held region and
    /// the start of the part) or the tail zone (between the end of the part
    /// and the end of the stock); on a rotary part tabs are restricted to
    /// those zones (OP-21, decision record 0035).
    TabOutsideEndZones {
        /// Position of the tab in the setup's list, from 1.
        tab: usize,
        /// The tab's X extent.
        x: (f64, f64),
    },
    /// A tab placed automatically at one end of the part finds no room
    /// there: the held region (or the end of the stock) reaches the part. It
    /// keeps the position it had (decision record 0202).
    AutoTabNoRoom {
        /// Position of the tab in the setup's list, from 1.
        tab: usize,
        /// The end of the part it is placed at.
        end: crate::workholding::TabEnd,
    },
    /// A held region reaches into the part, so the length of the part inside
    /// it is never machined (SET-09, OI-554). Generation is still allowed;
    /// the user verifies the G-code (NFR-12).
    HeldRegionCoversPart {
        /// True for the held region at the high-X end of the stock.
        at_end: bool,
        /// The X extent of the part inside the held region.
        x: (f64, f64),
    },
}

impl std::fmt::Display for SetupWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            SetupWarningKind::NoStock => f.write_str("the setup has no stock")?,
            SetupWarningKind::StockNeedsPart => {
                f.write_str("bounding-box stock needs a part model; set the setup's part")?
            }
            SetupWarningKind::ImportedStock => {
                f.write_str("imported stock (SET-04) is not available yet, so it is not checked")?
            }
            SetupWarningKind::ClearanceInsideStock { radius, stock } => write!(
                f,
                "the safe cylinder (radius {:.3} mm) does not enclose the stock (radius {:.3} mm)",
                radius.0, stock.0
            )?,
            SetupWarningKind::ClearanceInsidePart { radius, part } => write!(
                f,
                "the safe cylinder (radius {:.3} mm) does not enclose the part (radius {:.3} mm)",
                radius.0, part.0
            )?,
            SetupWarningKind::TabOutsideEndZones { tab, x } => write!(
                f,
                "tab {tab} (X {:.3} to {:.3}) is not at the head or the tail: a tab must lie between \
                 the held region and the start of the part, or between the end of the part and the \
                 end of the stock",
                x.0, x.1
            )?,
            SetupWarningKind::AutoTabNoRoom { tab, end } => write!(
                f,
                "tab {tab} is placed automatically at the {} of the part, but there is no room \
                 there: the {} reaches the part. It keeps its last position; move the part or \
                 shorten the held region, or untick \"Place automatically\" and place it by hand",
                end.name(),
                match end {
                    crate::workholding::TabEnd::Head => {
                        "held region at the start (or the start of the stock)"
                    }
                    crate::workholding::TabEnd::Tail => {
                        "held region at the end (or the end of the stock)"
                    }
                }
            )?,
            SetupWarningKind::PartOutsideStock => {
                f.write_str("some of the part lies outside the stock")?
            }
            SetupWarningKind::HeldRegionCoversPart { at_end, x } => write!(
                f,
                "the grip at the {} of the stock covers {:.3} mm of the part (X {:.3} to {:.3}), \
                 which is never machined; shorten the grip if that length should be cut",
                if *at_end { "end" } else { "start" },
                x.1 - x.0,
                x.0,
                x.1
            )?,
            SetupWarningKind::OutsideTravel {
                what,
                axis,
                value,
                min,
                max,
            } => write!(
                f,
                "{what} is at machine {axis} {value:.3}, outside the travel {min:.3} to {max:.3}"
            )?,
        }
        if !self.unconfirmed.is_empty() {
            let names: Vec<&str> = self.unconfirmed.iter().map(|p| p.name()).collect();
            write!(
                f,
                " (uses machine values not confirmed on the machine: {})",
                names.join(", ")
            )?;
        }
        Ok(())
    }
}

/// What [`check_setup`] checks.
#[derive(Debug, Clone, Copy)]
pub struct SetupCheck<'a> {
    /// The machine.
    pub profile: &'a MachineProfile,
    /// The part and work zero positions.
    pub frame: &'a SetupFrame,
    /// The stock, if set.
    pub stock: Option<&'a Stock>,
    /// The setup's own clearance; `None` uses the profile's.
    pub clearance: Option<&'a Clearance>,
    /// The part as this setup places it (moved by the part offset,
    /// [`PartExtent::in_setup`]), if the setup has one and it could be read.
    pub part: Option<&'a PartExtent>,
}

/// Fails if a setup's values are invalid: the frame is not finite, the stock
/// or clearance is out of range, or the stock is imported (SET-04, P2).
pub fn validate_setup(
    frame: &SetupFrame,
    stock: Option<&Stock>,
    clearance: Option<&Clearance>,
    field: &str,
) -> Result<()> {
    frame.validate(field)?;
    if let Some(stock) = stock {
        if matches!(stock, Stock::Imported { .. }) {
            return Err(Error::invalid(
                format!("{field}.stock"),
                "imported stock (SET-04) is not available yet; use a cylinder, a box or the bounding box",
            ));
        }
        stock.validate()?;
    }
    if let Some(clearance) = clearance {
        clearance.validate(&format!("{field}.clearance"))?;
    }
    Ok(())
}

/// Checks a setup against its stock, part and machine (SET-06): the safe
/// cylinder encloses stock and part, the stock encloses the part, and the
/// stock, work zero, rotary axis and safe Z are within the machine's travel.
/// Warnings are in a fixed order.
pub fn check_setup(check: &SetupCheck<'_>) -> Vec<SetupWarning> {
    let profile = check.profile;
    let warn = |kind, fields: &[ProfileField]| {
        let mut unconfirmed: Vec<ProfileField> = fields
            .iter()
            .copied()
            .filter(|f| !profile.is_confirmed(*f))
            .collect();
        unconfirmed.sort();
        unconfirmed.dedup();
        SetupWarning { kind, unconfirmed }
    };
    let clearance = check.clearance.copied().unwrap_or(profile.clearance);
    let clearance_fields: &[ProfileField] = if check.clearance.is_some() {
        &[]
    } else {
        &[ProfileField::Clearance]
    };
    let mut warnings = Vec::new();

    let stock_bounds = match check.stock {
        None => {
            warnings.push(warn(SetupWarningKind::NoStock, &[]));
            None
        }
        Some(Stock::Imported { .. }) => {
            warnings.push(warn(SetupWarningKind::ImportedStock, &[]));
            None
        }
        Some(stock) => match (stock.bounds(check.part), stock.radius(check.part)) {
            (Some(bounds), Some(radius)) => {
                if radius.0 > clearance.radius.0 {
                    warnings.push(warn(
                        SetupWarningKind::ClearanceInsideStock {
                            radius: clearance.radius,
                            stock: radius,
                        },
                        clearance_fields,
                    ));
                }
                Some((stock, bounds))
            }
            _ => {
                warnings.push(warn(SetupWarningKind::StockNeedsPart, &[]));
                None
            }
        },
    };

    if let Some(part) = check.part {
        if part.radius.0 > clearance.radius.0 {
            warnings.push(warn(
                SetupWarningKind::ClearanceInsidePart {
                    radius: clearance.radius,
                    part: part.radius,
                },
                clearance_fields,
            ));
        }
        if let Some((stock, bounds)) = stock_bounds
            && !stock_encloses(stock, &bounds, part)
        {
            warnings.push(warn(SetupWarningKind::PartOutsideStock, &[]));
        }
    }

    // Travel: along the rotary axis, the stock (or the part) and the work
    // zero; across it, the axis itself and the safe Z.
    let machine = RotaryFrame::of_profile(profile);
    let travel = &profile.travel;
    let (along, along_min, along_max) = match profile.rotary_axis {
        RotaryAxis::Y => ("Y", travel.min.y, travel.max.y),
        _ => ("X", travel.min.x, travel.max.x),
    };
    let along_value = |p: Point3| match profile.rotary_axis {
        RotaryAxis::Y => p.y,
        _ => p.x,
    };
    let travel_fields = [ProfileField::Travel, ProfileField::RotaryOrigin];
    let mut along_check = |what: &'static str, x: f64| {
        let value = along_value(machine.on_axis(Millimetres(x)));
        if value < along_min || value > along_max {
            warnings.push(warn(
                SetupWarningKind::OutsideTravel {
                    what,
                    axis: along,
                    value,
                    min: along_min,
                    max: along_max,
                },
                &travel_fields,
            ));
        }
    };
    let extent = stock_bounds
        .map(|(_, b)| ("the stock", b))
        .or(check.part.map(|p| ("the part", p.bounds)));
    if let Some((what, bounds)) = extent {
        let (start, end) = if what == "the stock" {
            ("the start of the stock", "the end of the stock")
        } else {
            ("the start of the part", "the end of the part")
        };
        along_check(start, bounds.min.x);
        along_check(end, bounds.max.x);
    }
    along_check("the work zero", check.frame.work_offset.x.0);

    let axis_point = machine.origin;
    let (across, across_value, across_min, across_max) = match profile.rotary_axis {
        RotaryAxis::Y => ("X", axis_point.x, travel.min.x, travel.max.x),
        _ => ("Y", axis_point.y, travel.min.y, travel.max.y),
    };
    if across_value < across_min || across_value > across_max {
        warnings.push(warn(
            SetupWarningKind::OutsideTravel {
                what: "the rotary axis",
                axis: across,
                value: across_value,
                min: across_min,
                max: across_max,
            },
            &travel_fields,
        ));
    }
    let safe = axis_point.z + clearance.safe_z.0;
    if safe < travel.min.z || safe > travel.max.z {
        let mut fields = travel_fields.to_vec();
        fields.extend_from_slice(clearance_fields);
        warnings.push(warn(
            SetupWarningKind::OutsideTravel {
                what: "the safe Z",
                axis: "Z",
                value: safe,
                min: travel.min.z,
                max: travel.max.z,
            },
            &fields,
        ));
    }
    warnings
}

/// Whether `stock` (with model-frame `bounds`) holds every point of `part`.
/// Relative tolerance of the check that the stock encloses the part: the
/// 32-bit float epsilon (2^-23, about 1.19e-7) times the size compared,
/// never less than 1 mm of size. A binary STL stores coordinates as 32-bit
/// floats, which rounds each to about 6e-8 of its size: a part exactly as
/// wide as its stock then measures up to 4.6e-7 mm (radius 10 mm) to
/// 6.5e-6 mm (radius 150 mm) outside it, while double-precision coordinates
/// (ASCII STL, STEP) measure at most 3e-14 mm (decision 0096, OI-314).
const ENCLOSE_TOLERANCE: f64 = f32::EPSILON as f64;

/// Whether `a <= b` within [`ENCLOSE_TOLERANCE`] of the larger size.
fn at_most(a: f64, b: f64) -> bool {
    a <= b + ENCLOSE_TOLERANCE * a.abs().max(b.abs()).max(1.0)
}

fn stock_encloses(stock: &Stock, bounds: &Aabb, part: &PartExtent) -> bool {
    let along =
        at_most(bounds.min.x, part.bounds.min.x) && at_most(part.bounds.max.x, bounds.max.x);
    match stock {
        Stock::Cylinder { diameter, .. } => along && at_most(part.radius.0, diameter.0 / 2.0),
        _ => {
            along
                && at_most(bounds.min.y, part.bounds.min.y)
                && at_most(part.bounds.max.y, bounds.max.y)
                && at_most(bounds.min.z, part.bounds.min.z)
                && at_most(part.bounds.max.z, bounds.max.z)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;
    use crate::machine::{Confirmation, Provenance};

    fn part() -> PartExtent {
        PartExtent {
            bounds: Aabb {
                min: Point3::new(0.0, -10.0, -10.0),
                max: Point3::new(50.0, 10.0, 10.0),
            },
            radius: Millimetres(10.0),
        }
    }

    fn cylinder(diameter: f64) -> Stock {
        Stock::Cylinder {
            diameter: Millimetres(diameter),
            length: Millimetres(60.0),
            x_start: Millimetres(-5.0),
        }
    }

    #[test]
    fn req_set_06_clean_setup_has_no_warnings() {
        let profile = MachineProfile::example_x_rotary();
        let frame = SetupFrame::default();
        let stock = cylinder(30.0);
        let part = part();
        let warnings = check_setup(&SetupCheck {
            profile: &profile,
            frame: &frame,
            stock: Some(&stock),
            clearance: None,
            part: Some(&part),
        });
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn req_set_06_clearance_must_enclose_stock_and_part() {
        let profile = MachineProfile::example_x_rotary();
        let frame = SetupFrame::default();
        let stock = cylinder(100.0);
        let part = part();
        let small = Clearance {
            radius: Millimetres(8.0),
            safe_z: Millimetres(20.0),
        };
        let warnings = check_setup(&SetupCheck {
            profile: &profile,
            frame: &frame,
            stock: Some(&stock),
            clearance: Some(&small),
            part: Some(&part),
        });
        let kinds: Vec<_> = warnings.iter().map(|w| &w.kind).collect();
        assert!(matches!(
            kinds[0],
            SetupWarningKind::ClearanceInsideStock { .. }
        ));
        assert!(matches!(
            kinds[1],
            SetupWarningKind::ClearanceInsidePart { .. }
        ));
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        // The setup's own clearance relies on no machine value.
        assert!(warnings.iter().all(|w| w.unconfirmed.is_empty()));
    }

    #[test]
    fn req_set_06_warnings_name_unconfirmed_machine_values() {
        let mut profile = MachineProfile::example_x_rotary();
        profile.clearance.radius = Millimetres(5.0);
        let frame = SetupFrame::default();
        let part = part();
        let w = check_setup(&SetupCheck {
            profile: &profile,
            frame: &frame,
            stock: Some(&cylinder(30.0)),
            clearance: None,
            part: Some(&part),
        });
        assert!(
            w[0].to_string()
                .contains("not confirmed on the machine: clearance"),
            "{}",
            w[0]
        );
        profile.provenance.insert(
            ProfileField::Clearance,
            Provenance {
                state: Confirmation::Confirmed,
                source: "measured".into(),
            },
        );
        let w = check_setup(&SetupCheck {
            profile: &profile,
            frame: &frame,
            stock: Some(&cylinder(30.0)),
            clearance: None,
            part: Some(&part),
        });
        assert!(w[0].unconfirmed.is_empty());
    }

    /// The largest radius of a ring of `n` points of radius `r` about the X
    /// axis, with the coordinates kept in double precision or rounded to
    /// 32-bit floats as a binary STL stores them.
    fn ring_radius(r: f64, n: u32, f32_coordinates: bool) -> f64 {
        (0..n)
            .map(|i| {
                let angle = f64::from(i) * std::f64::consts::TAU / f64::from(n);
                let (mut y, mut z) = (r * angle.cos(), r * angle.sin());
                if f32_coordinates {
                    (y, z) = (f64::from(y as f32), f64::from(z as f32));
                }
                y.hypot(z)
            })
            .fold(0.0, f64::max)
    }

    fn outside(stock: &Stock, part: &PartExtent) -> bool {
        check_setup(&SetupCheck {
            profile: &MachineProfile::example_x_rotary(),
            frame: &SetupFrame::default(),
            stock: Some(stock),
            clearance: None,
            part: Some(part),
        })
        .iter()
        .any(|w| w.kind == SetupWarningKind::PartOutsideStock)
    }

    fn cylinder_part(radius: f64, length: f64) -> PartExtent {
        PartExtent {
            bounds: Aabb {
                min: Point3::new(0.0, -radius, -radius),
                max: Point3::new(length, radius, radius),
            },
            radius: Millimetres(radius),
        }
    }

    fn cylinder_stock(radius: f64, length: f64) -> Stock {
        Stock::Cylinder {
            diameter: Millimetres(2.0 * radius),
            length: Millimetres(length),
            x_start: Millimetres(0.0),
        }
    }

    /// Stock exactly as wide as the part encloses it, whether the part's
    /// coordinates are double precision or rounded to 32-bit floats (a
    /// binary STL), at 10 mm and 150 mm; stock 0.01 mm smaller does not
    /// (OI-314, decision 0096).
    #[test]
    fn req_set_03_req_nfr_05_stock_as_wide_as_the_part_encloses_it() {
        for (radius, f32_coordinates) in
            [(10.0, false), (10.0, true), (150.0, false), (150.0, true)]
        {
            let measured = ring_radius(radius, 144, f32_coordinates);
            let part = cylinder_part(measured, 30.0);
            assert!(
                !outside(&cylinder_stock(radius, 30.0), &part),
                "radius {radius}, f32 {f32_coordinates}: measured {measured}"
            );
            let boxed = Stock::Box {
                bounds: Aabb {
                    min: Point3::new(0.0, -radius, -radius),
                    max: Point3::new(30.0, radius, radius),
                },
            };
            assert!(
                !outside(&boxed, &part),
                "box, radius {radius}, f32 {f32_coordinates}"
            );
            assert!(
                outside(&cylinder_stock(radius - 0.01, 30.0), &part),
                "radius {radius}, f32 {f32_coordinates}: stock 0.01 mm smaller"
            );
            let short = Stock::Box {
                bounds: Aabb {
                    min: Point3::new(0.01, -radius, -radius),
                    max: Point3::new(30.0, radius, radius),
                },
            };
            assert!(outside(&short, &part), "box 0.01 mm short, radius {radius}");
        }
        // A real overhang of 1e-4 mm on a 10 mm part is far beyond 32-bit
        // rounding, and still warns.
        assert!(outside(
            &cylinder_stock(10.0, 30.0),
            &cylinder_part(10.0001, 30.0)
        ));
    }

    #[test]
    fn req_set_03_part_outside_stock_and_bounding_box_stock() {
        let profile = MachineProfile::example_x_rotary();
        let frame = SetupFrame::default();
        let part = part();
        let short = Stock::Cylinder {
            diameter: Millimetres(30.0),
            length: Millimetres(20.0),
            x_start: Millimetres(0.0),
        };
        let w = check_setup(&SetupCheck {
            profile: &profile,
            frame: &frame,
            stock: Some(&short),
            clearance: None,
            part: Some(&part),
        });
        assert!(
            w.iter()
                .any(|w| w.kind == SetupWarningKind::PartOutsideStock)
        );

        let bbox = Stock::OffsetBoundingBox {
            offset: Millimetres(2.0),
        };
        assert_eq!(
            bbox.radius(Some(&part)),
            Some(Millimetres(12.0f64.hypot(12.0)))
        );
        let w = check_setup(&SetupCheck {
            profile: &profile,
            frame: &frame,
            stock: Some(&bbox),
            clearance: None,
            part: None,
        });
        assert_eq!(w[0].kind, SetupWarningKind::StockNeedsPart);
    }

    #[test]
    fn req_set_05_travel_is_checked_along_and_across_the_axis() {
        let profile = MachineProfile::example_x_rotary();
        // The stock is fixed in the setup's model frame, whose origin is the
        // rotary reference point (decision record 0201).
        let far = Stock::Cylinder {
            diameter: Millimetres(30.0),
            length: Millimetres(60.0),
            x_start: Millimetres(175.0),
        };
        let w = check_setup(&SetupCheck {
            profile: &profile,
            frame: &SetupFrame::default(),
            stock: Some(&far),
            clearance: None,
            part: None,
        });
        let text: Vec<String> = w.iter().map(ToString::to_string).collect();
        assert!(
            text.iter()
                .any(|t| t.starts_with("the end of the stock is at machine X 235.000")),
            "{text:?}"
        );
        let mut tall = profile.clone();
        tall.clearance.safe_z = Millimetres(100.0);
        let w = check_setup(&SetupCheck {
            profile: &tall,
            frame: &SetupFrame::default(),
            stock: Some(&cylinder(30.0)),
            clearance: None,
            part: None,
        });
        assert!(w.iter().any(|w| {
            w.to_string()
                .starts_with("the safe Z is at machine Z 60.000")
        }));
    }

    /// The part offset moves the part along the axis and nothing else
    /// (SET-05; decision record 0201): the part leaves stock that stays put,
    /// and a zero offset leaves every coordinate bit for bit.
    #[test]
    fn req_set_05_the_part_offset_moves_the_part_not_the_stock() {
        let profile = MachineProfile::example_x_rotary();
        let part = cylinder_part(5.0, 40.0);
        let check = |frame: &SetupFrame| {
            let placed = part.in_setup(frame);
            check_setup(&SetupCheck {
                profile: &profile,
                frame,
                stock: Some(&cylinder(30.0)),
                clearance: None,
                part: Some(&placed),
            })
        };
        assert!(check(&SetupFrame::default()).is_empty());
        let moved = SetupFrame {
            part_offset: Millimetres(30.0),
            ..SetupFrame::default()
        };
        let placed = part.in_setup(&moved);
        assert_eq!(placed.bounds.min, Point3::new(30.0, -5.0, -5.0));
        assert_eq!(placed.bounds.max, Point3::new(70.0, 5.0, 5.0));
        assert_eq!(placed.radius, part.radius);
        let kinds: Vec<_> = check(&moved).into_iter().map(|w| w.kind).collect();
        assert_eq!(kinds, vec![SetupWarningKind::PartOutsideStock]);
        // A zero offset of either sign moves nothing, even a -0.0 coordinate.
        let p = Point3::new(-0.0, 1.0, 2.0);
        for zero in [0.0, -0.0] {
            let frame = SetupFrame {
                part_offset: Millimetres(zero),
                ..SetupFrame::default()
            };
            assert!(!frame.moves_part());
            assert_eq!(frame.place_part(p).x.to_bits(), p.x.to_bits());
            assert_eq!(part.in_setup(&frame), part);
        }
    }

    #[test]
    fn req_set_05_expected_work_offset_is_the_work_zero_on_the_axis() {
        let profile = MachineProfile::example_x_rotary();
        let frame = SetupFrame {
            part_offset: Millimetres(20.0),
            work_offset: WorkOffset {
                register: WorkOffsetRegister::G55,
                x: Millimetres(-3.0),
            },
        };
        // The work zero stays where it is in the setup when the part moves
        // (decision record 0201): the part offset does not change it.
        let g55 = expected_work_offset(&profile, &frame);
        assert_eq!(g55, Point3::new(-3.0, 12.5, -40.0));
        let still = SetupFrame {
            part_offset: Millimetres(0.0),
            ..frame
        };
        assert_eq!(expected_work_offset(&profile, &still), g55);
        assert_eq!(
            WorkOffsetRegister::parse("g55"),
            Some(WorkOffsetRegister::G55)
        );
        assert_eq!(WorkOffsetRegister::parse("G60"), None);
    }

    #[test]
    fn req_set_03_imported_stock_is_refused() {
        let stock = Stock::Imported {
            mesh: crate::mesh::TriangleMesh {
                vertices: vec![],
                triangles: vec![],
            },
        };
        let err = validate_setup(&SetupFrame::default(), Some(&stock), None, "setup").unwrap_err();
        assert!(err.to_string().contains("SET-04"), "{err}");
    }
}

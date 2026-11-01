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

use crate::error::{Error, Result, require_finite, require_positive};
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

/// The margin "Clearance from stock" starts with when it is switched on
/// (decision record 0530).
pub const DEFAULT_STOCK_MARGIN: Millimetres = Millimetres(5.0);

/// A setup's clearance rules beyond its safe cylinder and safe Z (SET-06,
/// decision record 0530): its own minimum clearance radius, and whether the
/// clearance grows from the stock. The default has neither, which leaves
/// the clearance and the generated G-code unchanged (GC-07).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClearanceRules {
    /// The smallest safe cylinder radius that clears this setup's
    /// workholding; `None` takes the machine's
    /// ([`MachineProfile::min_clearance_radius`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_radius: Option<Millimetres>,
    /// "Clearance from stock": when set, the margin added to the stock's
    /// radius; the safe cylinder grows to at least that, and to the minimum.
    /// `None` is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_stock: Option<Millimetres>,
}

impl ClearanceRules {
    /// Whether the rules are the default (nothing set).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Fails unless the minimum is positive and the margin is zero or more,
    /// both finite.
    pub fn validate(&self, field: &str) -> Result<()> {
        if let Some(min) = self.min_radius {
            require_positive(&format!("{field}.min_radius"), min.0)?;
        }
        if let Some(margin) = self.from_stock {
            let name = format!("{field}.from_stock");
            require_finite(&name, margin.0)?;
            if margin.0 < 0.0 {
                return Err(Error::invalid(name, "the margin must be 0 mm or more"));
            }
        }
        Ok(())
    }
}

/// Where a setup's minimum clearance radius comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimumSource {
    /// The machine profile's.
    Machine,
    /// The setup's own.
    Setup,
}

impl MinimumSource {
    /// The words for it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Machine => "from the machine",
            Self::Setup => "set for this setup",
        }
    }
}

/// The clearance a setup uses, with the minimum it is checked against
/// ([`effective_clearance`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectiveClearance {
    /// The safe cylinder and safe Z that retracts and links use.
    pub clearance: Clearance,
    /// The minimum clearance radius and where it comes from, when one is
    /// given.
    pub minimum: Option<(Millimetres, MinimumSource)>,
    /// Whether the clearance was grown from the stock.
    pub from_stock: bool,
}

impl EffectiveClearance {
    /// How far the safe cylinder is inside the minimum, when it is (to the
    /// G-code's three places: a cylinder within 0.0005 mm of the minimum
    /// clears it).
    pub fn below_minimum(&self) -> Option<Millimetres> {
        let (min, _) = self.minimum?;
        (min.0 - self.clearance.radius.0 > 5e-4).then_some(min)
    }
}

/// The clearance a setup uses (SET-06, decision record 0530). The base is
/// the setup's own clearance, else the machine's. The minimum is the
/// setup's, else the machine's. With "Clearance from stock" on, the safe
/// cylinder radius is the largest of the base radius, the minimum and the
/// stock's radius plus the margin (the stock's radius when it can be
/// measured), and the safe Z is raised to at least that radius; it never
/// lowers the clearance. With it off the base is returned unchanged, so the
/// G-code is unchanged (GC-07). Pure and deterministic.
pub fn effective_clearance(
    profile: &MachineProfile,
    own: Option<&Clearance>,
    rules: &ClearanceRules,
    stock: Option<&Stock>,
    part: Option<&PartExtent>,
) -> EffectiveClearance {
    let base = own.copied().unwrap_or(profile.clearance);
    let minimum = match (rules.min_radius, profile.min_clearance_radius) {
        (Some(own), _) => Some((own, MinimumSource::Setup)),
        (None, Some(machine)) => Some((machine, MinimumSource::Machine)),
        (None, None) => None,
    };
    let Some(margin) = rules.from_stock else {
        return EffectiveClearance {
            clearance: base,
            minimum,
            from_stock: false,
        };
    };
    let mut radius = base.radius.0;
    if let Some((min, _)) = minimum {
        radius = radius.max(min.0);
    }
    if let Some(stock) = stock.and_then(|s| s.radius(part)) {
        radius = radius.max(stock.0 + margin.0);
    }
    EffectiveClearance {
        clearance: Clearance {
            radius: Millimetres(radius),
            safe_z: Millimetres(base.safe_z.0.max(radius)),
        },
        minimum,
        from_stock: true,
    }
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
    /// The safe cylinder is inside the minimum clearance radius, so links
    /// may hit the chuck, jaws, headstock or tailstock (decision record
    /// 0530). A warning: generation and export still run (NFR-12).
    ClearanceBelowMinimum {
        /// Safe cylinder radius.
        radius: Millimetres,
        /// The minimum clearance radius.
        minimum: Millimetres,
        /// Where the minimum comes from.
        source: MinimumSource,
    },
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
    /// A tab placed automatically lies over a tab placed by hand. The
    /// G-code is not wrong (the target is the union of the tabs); the user
    /// may want to remove one (OP-21, decision record 0202, OI-841).
    TabsOverlap {
        /// Position of the automatic tab in the setup's list, from 1.
        auto_tab: usize,
        /// Position of the hand-placed tab in the setup's list, from 1.
        hand_tab: usize,
        /// The X extent they share.
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
            SetupWarningKind::TabsOverlap {
                auto_tab,
                hand_tab,
                x,
            } => write!(
                f,
                "tab {auto_tab} (placed automatically) overlaps tab {hand_tab} (placed by hand) \
                 from X {:.3} to {:.3}; the part is held by both, so the G-code is not wrong, but \
                 you may want to remove one",
                x.0, x.1
            )?,
            SetupWarningKind::ClearanceBelowMinimum {
                radius,
                minimum,
                source,
            } => write!(
                f,
                "the safe cylinder (radius {:.3} mm) is inside the minimum clearance (radius {:.3} mm, \
                 {}): links may hit the chuck, jaws, headstock or tailstock. Raise the setup's \
                 clearance or switch on Clearance from stock, and check the clearance on your \
                 machine before running",
                radius.0,
                minimum.0,
                source.name()
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
    /// The setup's minimum clearance and Clearance from stock (decision
    /// record 0530).
    pub rules: &'a ClearanceRules,
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
    let effective = effective_clearance(
        profile,
        check.clearance,
        check.rules,
        check.stock,
        check.part,
    );
    let clearance = effective.clearance;
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

    if let (Some(minimum), Some((_, source))) = (effective.below_minimum(), effective.minimum) {
        let mut fields = clearance_fields.to_vec();
        if source == MinimumSource::Machine {
            fields.push(ProfileField::MinClearanceRadius);
        }
        warnings.push(warn(
            SetupWarningKind::ClearanceBelowMinimum {
                radius: clearance.radius,
                minimum,
                source,
            },
            &fields,
        ));
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
            rules: &ClearanceRules::default(),
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
            rules: &ClearanceRules::default(),
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
            rules: &ClearanceRules::default(),
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
            rules: &ClearanceRules::default(),
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
            rules: &ClearanceRules::default(),
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
            rules: &ClearanceRules::default(),
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
            rules: &ClearanceRules::default(),
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
            rules: &ClearanceRules::default(),
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
            rules: &ClearanceRules::default(),
            part: None,
        });
        assert!(w.iter().any(|w| {
            w.to_string()
                .starts_with("the safe Z is at machine Z 60.000")
        }));
    }

    /// The safe Z is checked against the top of the Z travel at its boundary,
    /// with the Makera Z1's figures: the axis at machine Z -48.6 and the
    /// travel top at 0 (SET-06, decision record 0317). A safe Z exactly at
    /// the top or below it passes; 1.4 mm above it (safe Z 50, the old
    /// default) warns with the value and the travel.
    #[test]
    fn req_set_06_the_safe_z_is_checked_against_the_top_of_the_z_travel() {
        let mut profile = MachineProfile::example_x_rotary();
        profile.rotary_origin.z = -48.6;
        profile.travel.min.z = -100.0;
        profile.travel.max.z = 0.0;
        let outside = |safe_z: f64| {
            let mut p = profile.clone();
            p.clearance = Clearance {
                radius: Millimetres(20.0),
                safe_z: Millimetres(safe_z),
            };
            check_setup(&SetupCheck {
                profile: &p,
                frame: &SetupFrame::default(),
                stock: Some(&cylinder(30.0)),
                clearance: None,
                rules: &ClearanceRules::default(),
                part: None,
            })
            .iter()
            .map(ToString::to_string)
            .find(|t| t.starts_with("the safe Z"))
        };
        assert_eq!(outside(40.0), None);
        assert_eq!(outside(48.6), None, "exactly at the top of the travel");
        let above = outside(50.0).unwrap();
        assert!(
            above.starts_with(
                "the safe Z is at machine Z 1.400, outside the travel -100.000 to 0.000"
            ),
            "{above}"
        );
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
                rules: &ClearanceRules::default(),
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

    fn rules(min: Option<f64>, margin: Option<f64>) -> ClearanceRules {
        ClearanceRules {
            min_radius: min.map(Millimetres),
            from_stock: margin.map(Millimetres),
        }
    }

    /// With no minimum and Clearance from stock off, the clearance is
    /// exactly the setup's own, else the machine's, as before decision 0530
    /// (GC-07).
    #[test]
    fn req_set_06_req_gc_07_default_rules_leave_the_clearance_unchanged() {
        let profile = MachineProfile::example_x_rotary();
        let stock = cylinder(200.0);
        let own = Clearance {
            radius: Millimetres(12.0),
            safe_z: Millimetres(20.0),
        };
        let none = ClearanceRules::default();
        assert!(none.is_default());
        let e = effective_clearance(&profile, None, &none, Some(&stock), None);
        assert_eq!(e.clearance, profile.clearance);
        assert_eq!(e.minimum, None);
        assert!(!e.from_stock);
        let e = effective_clearance(&profile, Some(&own), &none, Some(&stock), None);
        assert_eq!(e.clearance, own);
        // A minimum alone changes nothing either: it is only checked (D2).
        let e = effective_clearance(&profile, Some(&own), &rules(Some(90.0), None), None, None);
        assert_eq!(e.clearance, own);
        assert_eq!(e.below_minimum(), Some(Millimetres(90.0)));
    }

    /// The setup's own minimum replaces the machine's (decision 0530, D1).
    #[test]
    fn req_set_06_the_setup_minimum_replaces_the_machine_minimum() {
        let mut profile = MachineProfile::example_x_rotary();
        let none = ClearanceRules::default();
        assert_eq!(
            effective_clearance(&profile, None, &none, None, None).minimum,
            None
        );
        profile.min_clearance_radius = Some(Millimetres(45.0));
        assert_eq!(
            effective_clearance(&profile, None, &none, None, None).minimum,
            Some((Millimetres(45.0), MinimumSource::Machine))
        );
        assert_eq!(
            effective_clearance(&profile, None, &rules(Some(30.0), None), None, None).minimum,
            Some((Millimetres(30.0), MinimumSource::Setup))
        );
    }

    /// Clearance from stock: the radius is the largest of the base radius,
    /// the minimum and the stock radius plus the margin, and the safe Z is
    /// lifted to at least that radius; it never lowers the clearance
    /// (decision 0530, D3).
    #[test]
    fn req_set_06_clearance_from_stock_grows_the_safe_cylinder_and_safe_z() {
        let mut profile = MachineProfile::example_x_rotary(); // radius 40, safe Z 50
        // Stock radius 50 + margin 5 = 55: above the base and the safe Z.
        let e = effective_clearance(
            &profile,
            None,
            &rules(None, Some(5.0)),
            Some(&cylinder(100.0)),
            None,
        );
        assert!(e.from_stock);
        assert_eq!(e.clearance.radius, Millimetres(55.0));
        assert_eq!(e.clearance.safe_z, Millimetres(55.0));
        // A small stock never lowers the base clearance.
        let e = effective_clearance(
            &profile,
            None,
            &rules(None, Some(5.0)),
            Some(&cylinder(20.0)),
            None,
        );
        assert_eq!(e.clearance, profile.clearance);
        // The minimum wins when it is the largest; the safe Z follows.
        profile.min_clearance_radius = Some(Millimetres(70.0));
        let e = effective_clearance(
            &profile,
            None,
            &rules(None, Some(5.0)),
            Some(&cylinder(100.0)),
            None,
        );
        assert_eq!(e.clearance.radius, Millimetres(70.0));
        assert_eq!(e.clearance.safe_z, Millimetres(70.0));
        assert_eq!(e.below_minimum(), None);
        // A margin of 0 clears the stock exactly.
        profile.min_clearance_radius = None;
        let e = effective_clearance(
            &profile,
            None,
            &rules(None, Some(0.0)),
            Some(&cylinder(90.0)),
            None,
        );
        assert_eq!(e.clearance.radius, Millimetres(45.0));
        assert_eq!(e.clearance.safe_z, Millimetres(50.0));
        // Bounding-box stock measures the part; without it only the
        // minimum and the base count.
        let bbox = Stock::OffsetBoundingBox {
            offset: Millimetres(2.0),
        };
        let p = part();
        let with = effective_clearance(
            &profile,
            None,
            &rules(None, Some(5.0)),
            Some(&bbox),
            Some(&p),
        );
        let measured = bbox.radius(Some(&p)).unwrap().0 + 5.0;
        assert_eq!(
            with.clearance.radius.0,
            profile.clearance.radius.0.max(measured)
        );
        let without =
            effective_clearance(&profile, None, &rules(None, Some(5.0)), Some(&bbox), None);
        assert_eq!(without.clearance, profile.clearance);
    }

    /// A safe cylinder inside the minimum warns, naming both values and
    /// where the minimum comes from; it never refuses (decision 0530, D2;
    /// NFR-12). A machine minimum that is not confirmed is named.
    #[test]
    fn req_set_06_req_nfr_12_a_clearance_inside_the_minimum_warns() {
        let mut profile = MachineProfile::example_x_rotary();
        profile.min_clearance_radius = Some(Millimetres(45.0));
        let frame = SetupFrame::default();
        let stock = cylinder(30.0);
        let p = part();
        let check = |rules: &ClearanceRules| {
            check_setup(&SetupCheck {
                profile: &profile,
                frame: &frame,
                stock: Some(&stock),
                clearance: None,
                rules,
                part: Some(&p),
            })
        };
        let w = check(&ClearanceRules::default());
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(
            w[0].kind,
            SetupWarningKind::ClearanceBelowMinimum {
                radius: Millimetres(40.0),
                minimum: Millimetres(45.0),
                source: MinimumSource::Machine,
            }
        );
        assert_eq!(
            w[0].unconfirmed,
            vec![ProfileField::Clearance, ProfileField::MinClearanceRadius]
        );
        let text = w[0].to_string();
        for words in [
            "radius 40.000 mm",
            "radius 45.000 mm, from the machine",
            "chuck",
            "tailstock",
            "Clearance from stock",
            "check the clearance on your machine before running",
        ] {
            assert!(text.contains(words), "{words}: {text}");
        }
        for never in ["safe to run", "verified"] {
            assert!(!text.contains(never), "{never}: {text}");
        }
        // The setup's own minimum is named as such and needs no machine value.
        let w = check(&rules(Some(41.0), None));
        assert!(w[0].to_string().contains("set for this setup"), "{}", w[0]);
        assert_eq!(w[0].unconfirmed, vec![ProfileField::Clearance]);
        // Clearance from stock clears it; so does a minimum within rounding.
        assert!(check(&rules(None, Some(5.0))).is_empty());
        assert!(check(&rules(Some(40.0004), None)).is_empty());
    }

    #[test]
    fn req_nfr_07_bad_clearance_rules_are_refused() {
        for v in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let e = rules(Some(v), None).validate("setup").unwrap_err();
            assert!(e.to_string().contains("setup.min_radius"), "{v}: {e}");
        }
        for v in [-0.1, f64::NAN, f64::INFINITY] {
            let e = rules(None, Some(v)).validate("setup").unwrap_err();
            assert!(e.to_string().contains("setup.from_stock"), "{v}: {e}");
        }
        rules(Some(1.0), Some(0.0)).validate("setup").unwrap();
        assert_eq!(DEFAULT_STOCK_MARGIN, Millimetres(5.0));
    }

    proptest::proptest! {
        /// Any finite rules give a clearance that never lowers the base,
        /// encloses the minimum when growing from stock, keeps the safe Z on
        /// or above the cylinder, and never panics (NFR-07).
        #[test]
        fn req_set_06_req_nfr_07_effective_clearance_never_lowers_the_base(
            min in proptest::option::of(0.001f64..1000.0),
            margin in proptest::option::of(0.0f64..1000.0),
            diameter in 0.001f64..2000.0,
        ) {
            let mut profile = MachineProfile::example_x_rotary();
            profile.min_clearance_radius = Some(Millimetres(33.0));
            let r = rules(min, margin);
            let e = effective_clearance(&profile, None, &r, Some(&cylinder(diameter)), None);
            proptest::prop_assert!(e.clearance.radius.0 >= profile.clearance.radius.0);
            proptest::prop_assert!(e.clearance.safe_z.0 >= profile.clearance.safe_z.0);
            proptest::prop_assert!(e.clearance.safe_z.0 >= e.clearance.radius.0);
            proptest::prop_assert!(e.clearance.validate("c").is_ok());
            if margin.is_some() {
                proptest::prop_assert!(e.below_minimum().is_none());
                proptest::prop_assert!(e.clearance.radius.0 >= diameter / 2.0);
            } else {
                proptest::prop_assert_eq!(e.clearance, profile.clearance);
            }
        }
    }
}

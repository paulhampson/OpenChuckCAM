// SPDX-License-Identifier: AGPL-3.0-or-later
//! The Stock, Part position, and Work offset and clearance sections of the
//! setup inspector as a view model (SET-03, SET-05, SET-06; UI-07; interface
//! specification section 11.3; decision record 0114 C15).
//!
//! Each section is a list of rows with their values as text, and a typed
//! value is checked here, in plain words (section 15), before it becomes an
//! edited setup. The front end stores the edited setup with one undoable
//! command (`Command::Replace`), so the project is unchanged when a value is
//! refused.

use openchuck_project::Setup;
use openchuck_types::geometry::{Aabb, Point3};

use crate::setup::{Clearance, MachineProfile, Millimetres, PartExtent, Stock, WorkOffsetRegister};

/// The stock's shape, as the segmented control offers it (11.3). Imported
/// stock (SET-04) is P2 and is shown, not offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockShape {
    /// A cylinder on the rotary axis.
    Cylinder,
    /// A box in the model frame.
    Box,
    /// The model's bounding box plus an offset.
    Bounds,
}

impl StockShape {
    /// The shapes, in the order of the control.
    pub const ALL: [Self; 3] = [Self::Cylinder, Self::Box, Self::Bounds];

    /// The shape's name on the control.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cylinder => "Cylinder",
            Self::Box => "Box",
            Self::Bounds => "Bounds",
        }
    }

    /// The shape of `stock`; `None` for no stock or imported stock.
    pub fn of(stock: Option<&Stock>) -> Option<Self> {
        match stock? {
            Stock::Cylinder { .. } => Some(Self::Cylinder),
            Stock::Box { .. } => Some(Self::Box),
            Stock::OffsetBoundingBox { .. } => Some(Self::Bounds),
            _ => None,
        }
    }
}

/// A value of the setup a row edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SetupField {
    /// A cylinder's diameter.
    Diameter,
    /// A cylinder's length along X.
    Length,
    /// Where a cylinder starts along X.
    StartX,
    /// A box's lowest X, Y or Z (0, 1 or 2).
    BoxMin(usize),
    /// A box's highest X, Y or Z.
    BoxMax(usize),
    /// The offset added to the model's bounding box.
    BoundsOffset,
    /// The stock material.
    Material,
    /// The part's offset along X from the rotary reference point.
    PartOffset,
    /// Model X of the work zero.
    WorkZeroX,
    /// The safe cylinder's radius.
    SafeRadius,
    /// The safe Z above the axis.
    SafeZ,
}

/// One row of a setup section.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupRow {
    /// The value it edits.
    pub field: SetupField,
    /// The label.
    pub label: &'static str,
    /// What it is, for the label's tooltip.
    pub description: &'static str,
    /// The unit inside the field; empty for text.
    pub unit: &'static str,
    /// The value's text; empty when the setup does not set it.
    pub value: String,
    /// What applies while it is empty, with its source.
    pub hint: Option<String>,
    /// The allowed range in words.
    pub range: Option<&'static str>,
}

/// What a value taken from the machine is called wherever the interface shows
/// where it comes from (issue 189): one wording, so the same words appear in
/// the hint, the tooltips and the messages.
pub const MACHINE_SETTING: &str = "Machine setting";

fn mm(v: f64) -> String {
    format!("{v:.3}")
}

fn row(
    field: SetupField,
    label: &'static str,
    description: &'static str,
    value: Option<f64>,
    range: Option<&'static str>,
) -> SetupRow {
    SetupRow {
        field,
        label,
        description,
        unit: "mm",
        value: value.map(mm).unwrap_or_default(),
        hint: None,
        range,
    }
}

const AXES: [&str; 3] = ["X", "Y", "Z"];
const BOX_MIN: [&str; 3] = ["X from", "Y from", "Z from"];
const BOX_MAX: [&str; 3] = ["X to", "Y to", "Z to"];

fn component(p: Point3, axis: usize) -> f64 {
    match axis {
        0 => p.x,
        1 => p.y,
        _ => p.z,
    }
}

fn set_component(p: &mut Point3, axis: usize, v: f64) {
    match axis {
        0 => p.x = v,
        1 => p.y = v,
        _ => p.z = v,
    }
}

/// The rows of the Stock section for the setup's stock, then the material.
pub fn stock_rows(setup: &Setup) -> Vec<SetupRow> {
    let mut rows = Vec::new();
    match &setup.stock {
        Some(Stock::Cylinder {
            diameter,
            length,
            x_start,
        }) => {
            rows.push(row(
                SetupField::Diameter,
                "Diameter",
                "The stock's diameter.",
                Some(diameter.0),
                Some("above 0 mm"),
            ));
            rows.push(row(
                SetupField::Length,
                "Length",
                "The stock's length along the rotary axis.",
                Some(length.0),
                Some("above 0 mm"),
            ));
            rows.push(row(
                SetupField::StartX,
                "Start X",
                "Model X of the stock's low-X end.",
                Some(x_start.0),
                None,
            ));
        }
        Some(Stock::Box { bounds }) => {
            for (axis, (from, to)) in BOX_MIN.into_iter().zip(BOX_MAX).enumerate() {
                rows.push(row(
                    SetupField::BoxMin(axis),
                    from,
                    "The box's lowest coordinate along this model axis.",
                    Some(component(bounds.min, axis)),
                    None,
                ));
                rows.push(row(
                    SetupField::BoxMax(axis),
                    to,
                    "The box's highest coordinate along this model axis.",
                    Some(component(bounds.max, axis)),
                    None,
                ));
            }
        }
        Some(Stock::OffsetBoundingBox { offset }) => rows.push(row(
            SetupField::BoundsOffset,
            "Offset",
            "Added to the model's bounding box on every side.",
            Some(offset.0),
            Some("at least 0 mm"),
        )),
        _ => {}
    }
    rows.push(SetupRow {
        field: SetupField::Material,
        label: "Material",
        description: "The stock material, such as Softwood. It chooses the tools' cutting data, matched by name ignoring capital letters and extra spaces. Pick one the tools already know, or type a new one.",
        unit: "",
        value: setup.material.clone().unwrap_or_default(),
        hint: Some("Not set".to_owned()),
        range: None,
    });
    rows
}

/// The row of the Part position section (SET-05): the part's offset along X.
pub fn part_position_rows(setup: &Setup) -> Vec<SetupRow> {
    vec![row(
        SetupField::PartOffset,
        "Offset along X",
        "How far along the rotary axis the model origin is from the machine's rotary \
         reference point (the chuck face).",
        Some(setup.frame.part_offset.0),
        None,
    )]
}

/// The rows of the Work offset and clearance section (SET-05, SET-06): the
/// work zero along X, the safe radius and the safe Z. A clearance the setup
/// does not set is the Machine setting, given as the hint (issue 189).
pub fn clearance_rows(setup: &Setup, machine: Option<&MachineProfile>) -> Vec<SetupRow> {
    let own = setup.clearance;
    let from_machine = machine.map(|m| m.clearance);
    let hint = |v: Option<f64>| v.map(|v| format!("{} \u{b7} {MACHINE_SETTING}", mm(v)));
    vec![
        row(
            SetupField::WorkZeroX,
            "Zero at X",
            "Model X of the work zero, which lies on the rotary axis.",
            Some(setup.frame.work_offset.x.0),
            None,
        ),
        SetupRow {
            hint: hint(from_machine.map(|c| c.radius.0)),
            ..row(
                SetupField::SafeRadius,
                "Safe radius",
                "The radius of the safe cylinder about the rotary axis that links keep \
                 outside. Empty uses the Machine setting.",
                own.map(|c| c.radius.0),
                Some("above 0 mm"),
            )
        },
        SetupRow {
            hint: hint(from_machine.map(|c| c.safe_z.0)),
            ..row(
                SetupField::SafeZ,
                "Safe Z",
                "The safe height above the rotary axis. Empty uses the Machine setting.",
                own.map(|c| c.safe_z.0),
                Some("at least the safe radius"),
            )
        },
    ]
}

/// The work offset registers' names, in order, and the setup's.
pub fn registers(setup: &Setup) -> (Vec<&'static str>, usize) {
    let names = WorkOffsetRegister::ALL.iter().map(|r| r.name()).collect();
    let current = WorkOffsetRegister::ALL
        .iter()
        .position(|r| *r == setup.frame.work_offset.register)
        .unwrap_or(0);
    (names, current)
}

/// `setup` with the work offset register at `index` of
/// [`WorkOffsetRegister::ALL`].
pub fn with_register(setup: &Setup, index: usize) -> Setup {
    let mut edited = setup.clone();
    if let Some(r) = WorkOffsetRegister::ALL.get(index) {
        edited.frame.work_offset.register = *r;
    }
    edited
}

/// The stock of `shape`, made from `current` where it can be: a cylinder
/// keeps the length and start of a box and takes its larger side as the
/// diameter, a box keeps the cylinder's length and start and is as wide as
/// its diameter, and without either the stock is sized from `part` with 1 mm
/// to spare (or 20 by 30 mm with no part).
pub fn stock_of_shape(
    current: Option<&Stock>,
    shape: StockShape,
    part: Option<&PartExtent>,
) -> Stock {
    let (diameter, length, x_start) = match current {
        Some(Stock::Cylinder {
            diameter,
            length,
            x_start,
        }) => (diameter.0, length.0, x_start.0),
        Some(Stock::Box { bounds }) => (
            (bounds.max.y - bounds.min.y).max(bounds.max.z - bounds.min.z),
            bounds.max.x - bounds.min.x,
            bounds.min.x,
        ),
        _ => match part {
            Some(p) => (
                2.0 * p.radius.0 + 2.0,
                p.bounds.max.x - p.bounds.min.x + 2.0,
                p.bounds.min.x - 1.0,
            ),
            None => (20.0, 30.0, -15.0),
        },
    };
    match shape {
        StockShape::Cylinder => Stock::Cylinder {
            diameter: Millimetres(diameter),
            length: Millimetres(length),
            x_start: Millimetres(x_start),
        },
        StockShape::Box => {
            let r = diameter / 2.0;
            Stock::Box {
                bounds: Aabb {
                    min: Point3::new(x_start, -r, -r),
                    max: Point3::new(x_start + length, r, r),
                },
            }
        }
        StockShape::Bounds => Stock::OffsetBoundingBox {
            offset: Millimetres(1.0),
        },
    }
}

/// `setup` with its stock of `shape` (one undoable command for the caller).
pub fn with_stock_shape(setup: &Setup, shape: StockShape, part: Option<&PartExtent>) -> Setup {
    let mut edited = setup.clone();
    edited.stock = Some(stock_of_shape(setup.stock.as_ref(), shape, part));
    edited
}

fn number(text: &str) -> Result<f64, String> {
    text.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| "Not a number.".to_owned())
}

fn positive(v: f64) -> Result<f64, String> {
    if v > 0.0 {
        Ok(v)
    } else {
        Err("Must be more than 0 mm.".to_owned())
    }
}

/// `setup` with `field` set from the typed `text`, or the reason it is
/// refused, in plain words (section 15). Empty text clears a value that may
/// be empty (the material, the setup's own clearance).
pub fn apply(
    setup: &Setup,
    machine: Option<&MachineProfile>,
    field: SetupField,
    text: &str,
) -> Result<Setup, String> {
    let text = text.trim();
    let mut edited = setup.clone();
    match field {
        SetupField::Material => {
            edited.material = (!text.is_empty()).then(|| text.to_owned());
            return Ok(edited);
        }
        SetupField::SafeRadius | SetupField::SafeZ if text.is_empty() => {
            edited.clearance = None;
            return Ok(edited);
        }
        _ if text.is_empty() => return Err("Needs a value.".to_owned()),
        _ => {}
    }
    let v = number(text)?;
    match (field, &mut edited.stock) {
        (
            SetupField::Diameter,
            Some(Stock::Cylinder {
                diameter: target, ..
            }),
        )
        | (SetupField::Length, Some(Stock::Cylinder { length: target, .. })) => {
            *target = Millimetres(positive(v)?);
        }
        (SetupField::StartX, Some(Stock::Cylinder { x_start, .. })) => *x_start = Millimetres(v),
        (SetupField::BoxMin(axis), Some(Stock::Box { bounds })) => {
            let max = component(bounds.max, axis);
            if v >= max {
                return Err(format!(
                    "Must be less than {} to ({} mm).",
                    AXES.get(axis).copied().unwrap_or("X"),
                    mm(max)
                ));
            }
            set_component(&mut bounds.min, axis, v);
        }
        (SetupField::BoxMax(axis), Some(Stock::Box { bounds })) => {
            let min = component(bounds.min, axis);
            if v <= min {
                return Err(format!(
                    "Must be more than {} from ({} mm).",
                    AXES.get(axis).copied().unwrap_or("X"),
                    mm(min)
                ));
            }
            set_component(&mut bounds.max, axis, v);
        }
        (SetupField::BoundsOffset, Some(Stock::OffsetBoundingBox { offset })) => {
            if v < 0.0 {
                return Err("Must be 0 mm or more.".to_owned());
            }
            *offset = Millimetres(v);
        }
        (SetupField::PartOffset, _) => edited.frame.part_offset = Millimetres(v),
        (SetupField::WorkZeroX, _) => edited.frame.work_offset.x = Millimetres(v),
        (SetupField::SafeRadius | SetupField::SafeZ, _) => {
            let base = setup
                .clearance
                .or_else(|| machine.map(|m| m.clearance))
                .unwrap_or(Clearance {
                    radius: Millimetres(v),
                    safe_z: Millimetres(v),
                });
            let mut c = base;
            if field == SetupField::SafeRadius {
                c.radius = Millimetres(positive(v)?);
                if c.radius.0 > c.safe_z.0 {
                    return Err(format!(
                        "Must not be above the safe Z ({} mm).",
                        mm(c.safe_z.0)
                    ));
                }
            } else {
                c.safe_z = Millimetres(positive(v)?);
                if c.safe_z.0 < c.radius.0 {
                    return Err(format!(
                        "Must not be below the safe radius ({} mm).",
                        mm(c.radius.0)
                    ));
                }
            }
            edited.clearance = Some(c);
        }
        _ => return Err("The stock changed; choose the value again.".to_owned()),
    }
    Ok(edited)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use openchuck_project::id::SetupId;

    use super::*;

    fn setup() -> Setup {
        let mut s = Setup::new(SetupId::new(), "Setup 1");
        s.stock = Some(Stock::Cylinder {
            diameter: Millimetres(26.0),
            length: Millimetres(32.0),
            x_start: Millimetres(-16.0),
        });
        s
    }

    #[test]
    fn req_set_03_the_stock_rows_follow_its_shape_and_a_shape_change_keeps_its_size() {
        let s = setup();
        let rows = stock_rows(&s);
        let labels: Vec<&str> = rows.iter().map(|r| r.label).collect();
        assert_eq!(labels, ["Diameter", "Length", "Start X", "Material"]);
        assert_eq!(rows[0].value, "26.000");
        let boxed = with_stock_shape(&s, StockShape::Box, None);
        assert_eq!(StockShape::of(boxed.stock.as_ref()), Some(StockShape::Box));
        assert_eq!(stock_rows(&boxed).len(), 7);
        let back = with_stock_shape(&boxed, StockShape::Cylinder, None);
        assert_eq!(back.stock, s.stock);
        let bounds = with_stock_shape(&s, StockShape::Bounds, None);
        assert_eq!(stock_rows(&bounds)[0].label, "Offset");
    }

    #[test]
    fn req_set_03_req_ui_07_a_typed_value_is_checked_in_plain_words() {
        let s = setup();
        let edited = apply(&s, None, SetupField::Diameter, "30").unwrap();
        assert_eq!(stock_rows(&edited)[0].value, "30.000");
        assert_eq!(
            apply(&s, None, SetupField::Diameter, "0").unwrap_err(),
            "Must be more than 0 mm."
        );
        assert_eq!(
            apply(&s, None, SetupField::Length, "abc").unwrap_err(),
            "Not a number."
        );
        assert_eq!(
            apply(&s, None, SetupField::Length, "").unwrap_err(),
            "Needs a value."
        );
        let boxed = with_stock_shape(&s, StockShape::Box, None);
        assert_eq!(
            apply(&boxed, None, SetupField::BoxMin(0), "50").unwrap_err(),
            "Must be less than X to (16.000 mm)."
        );
        let m = apply(&s, None, SetupField::Material, " Oak ").unwrap();
        assert_eq!(m.material.as_deref(), Some("Oak"));
        assert_eq!(
            apply(&m, None, SetupField::Material, "").unwrap().material,
            None
        );
    }

    #[test]
    fn req_set_05_req_set_06_position_work_zero_and_clearance() {
        let s = setup();
        let moved = apply(&s, None, SetupField::PartOffset, "12.5").unwrap();
        assert_eq!(moved.frame.part_offset, Millimetres(12.5));
        let zero = apply(&s, None, SetupField::WorkZeroX, "-3").unwrap();
        assert_eq!(zero.frame.work_offset.x, Millimetres(-3.0));
        let (names, current) = registers(&s);
        assert_eq!(names[current], "G54");
        assert_eq!(registers(&with_register(&s, 2)).1, 2);
        // With no clearance of its own, the machine's is the hint and the
        // base of an edit.
        let z1 = crate::setup::builtin(crate::setup::MAKERA_Z1)
            .unwrap()
            .unwrap();
        let rows = clearance_rows(&s, Some(&z1));
        assert!(rows[1].value.is_empty());
        assert!(rows[1].hint.as_deref().unwrap().ends_with(MACHINE_SETTING));
        // Issue 189: every row that falls back to the machine says "Machine
        // setting", in the hint and in the tooltip, never "the machine's".
        for row in &rows[1..] {
            assert!(row.hint.as_deref().unwrap().ends_with(MACHINE_SETTING));
            assert!(
                row.description.contains(MACHINE_SETTING),
                "{}",
                row.description
            );
            assert!(!row.description.contains("the machine's"));
        }
        let own = apply(&s, Some(&z1), SetupField::SafeRadius, "18").unwrap();
        let c = own.clearance.unwrap();
        assert_eq!(c.radius, Millimetres(18.0));
        assert_eq!(c.safe_z, z1.clearance.safe_z);
        let refused = apply(&own, Some(&z1), SetupField::SafeZ, "1").unwrap_err();
        assert_eq!(refused, "Must not be below the safe radius (18.000 mm).");
        // Empty goes back to the machine's.
        assert_eq!(
            apply(&own, Some(&z1), SetupField::SafeZ, "")
                .unwrap()
                .clearance,
            None
        );
    }

    #[test]
    fn req_set_03_box_and_bounds_values_are_checked_in_plain_words() {
        let s = setup();
        let boxed = with_stock_shape(&s, StockShape::Box, None);
        assert_eq!(
            apply(&boxed, None, SetupField::BoxMax(2), "-50").unwrap_err(),
            "Must be more than Z from (\u{2212}13.000 mm).".replace('\u{2212}', "-")
        );
        let wider = apply(&boxed, None, SetupField::BoxMax(1), "20").unwrap();
        assert_eq!(stock_rows(&wider)[3].value, "20.000");
        let lower = apply(&boxed, None, SetupField::BoxMin(0), "-20").unwrap();
        assert_eq!(stock_rows(&lower)[0].value, "-20.000");
        let bounds = with_stock_shape(&s, StockShape::Bounds, None);
        assert_eq!(
            apply(&bounds, None, SetupField::BoundsOffset, "-1").unwrap_err(),
            "Must be 0 mm or more."
        );
        let grown = apply(&bounds, None, SetupField::BoundsOffset, "2").unwrap();
        assert_eq!(stock_rows(&grown)[0].value, "2.000");
        // A row of another shape than the stock's is refused, not misapplied.
        assert!(apply(&bounds, None, SetupField::Diameter, "3").is_err());
        // Sized from the part when there is no stock to keep.
        let part = PartExtent {
            bounds: Aabb {
                min: Point3::new(0.0, -4.0, -4.0),
                max: Point3::new(30.0, 4.0, 4.0),
            },
            radius: Millimetres(5.0),
        };
        let Stock::Cylinder {
            diameter,
            length,
            x_start,
        } = stock_of_shape(None, StockShape::Cylinder, Some(&part))
        else {
            panic!("not a cylinder");
        };
        assert_eq!((diameter.0, length.0, x_start.0), (12.0, 32.0, -1.0));
        assert_eq!(StockShape::of(None), None);
        assert_eq!(StockShape::Box.name(), "Box");
    }
}

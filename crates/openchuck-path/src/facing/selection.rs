// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ordered indexed box faces and model-derived offsets (OP-24; decision0252).

use openchuck_types::geometry::Point3;
use openchuck_types::units::Degrees;
use serde::{Deserialize, Serialize};

use super::geometry::FacingPlane;
use crate::error::{PathError, Result};
use crate::rotary::RotaryMapping;

/// A derived default remains distinguishable from an explicit user edit.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FaceOffset {
    /// Follow placed-part support plus the operation's stock allowance.
    #[default]
    FromPart,
    /// Target plane position from the rotary axis, with allowance included.
    Explicit(f64),
}

/// One arbitrary-A row; angle and offset travel together during reordering.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SideRequest {
    /// Machine A as displayed/entered, in degrees. Order is never normalised.
    pub angle: Degrees,
    /// Target derivation or explicit override.
    #[serde(default)]
    pub offset: FaceOffset,
}

/// Decode ordered angle/offset rows from the generic parameter map's text
/// storage. Null/omitted offset means model-derived; a number is an override.
pub fn decode_rows(text: &str) -> Result<Vec<SideRequest>> {
    if text.len() > 1_048_576 {
        return Err(PathError::parameter(
            "arbitrary_faces",
            "row data exceeds 1 MiB",
        ));
    }
    let rows: Vec<SideRequest> = serde_json::from_str(text).map_err(|e| {
        PathError::parameter("arbitrary_faces", format!("invalid indexed-face rows: {e}"))
    })?;
    validate_rows(&rows)?;
    Ok(rows)
}

/// Encode rows without changing order or silently serialising nonfinite
/// coordinates as null. Hosts store this text as one undoable parameter edit.
pub fn encode_rows(rows: &[SideRequest]) -> Result<String> {
    validate_rows(rows)?;
    serde_json::to_string(rows).map_err(|e| PathError::parameter("arbitrary_faces", e.to_string()))
}

fn validate_rows(rows: &[SideRequest]) -> Result<()> {
    if rows.len() > 4096 {
        return Err(PathError::parameter(
            "arbitrary_faces",
            "more than 4096 arbitrary faces",
        ));
    }
    for row in rows {
        validate_row(*row)?;
    }
    Ok(())
}

/// Selections for the new box-side mode. Empty selections intentionally refuse.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SideSelection {
    /// +Z, +Y, -Z, -Y in fixed model order; `None` leaves a face unselected.
    pub standard: [Option<FaceOffset>; 4],
    /// Additional faces in user order, after selected standard faces.
    pub arbitrary: Vec<SideRequest>,
}

/// A resolved target plane, still distinct from the cutter's centre position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedSide {
    /// Entered/indexed A, before shared assembly fits equivalent whole turns.
    pub angle: Degrees,
    /// Target plane offset, including allowance once.
    pub offset: f64,
    /// Projection frame shared with coverage and host diagrams.
    pub plane: FacingPlane,
}

impl SideSelection {
    /// Read active selections, leaving inactive saved offsets out of targets.
    pub fn read(parameters: &crate::params::Parameters) -> Result<Self> {
        let mut selection = Self::default();
        for (face, side) in selection.standard.iter_mut().zip([
            "positive_z",
            "positive_y",
            "negative_z",
            "negative_y",
        ]) {
            if parameters.flag(side)? {
                *face = Some(if parameters.flag(&format!("{side}_from_part"))? {
                    FaceOffset::FromPart
                } else {
                    FaceOffset::Explicit(parameters.number(&format!("{side}_offset"))?)
                });
            }
        }
        selection.arbitrary = decode_rows(parameters.text(super::parameter::ARBITRARY)?)?;
        Ok(selection)
    }
    /// Select all four model faces, preserving existing overrides and arbitrary
    /// rows. Newly selected faces use their model-derived defaults.
    pub fn all_four(&mut self) {
        for face in &mut self.standard {
            face.get_or_insert(FaceOffset::FromPart);
        }
    }

    /// Move a row upwards, keeping its offset. The first row is a no-op.
    pub fn move_up(&mut self, index: usize) -> Result<bool> {
        self.check_index(index)?;
        if index == 0 {
            return Ok(false);
        }
        self.arbitrary.swap(index, index - 1);
        Ok(true)
    }

    /// Move a row downwards, keeping its offset. The last row is a no-op.
    pub fn move_down(&mut self, index: usize) -> Result<bool> {
        self.check_index(index)?;
        if index + 1 >= self.arbitrary.len() {
            return Ok(false);
        }
        self.arbitrary.swap(index, index + 1);
        Ok(true)
    }

    /// Explicit stable ascending sort by displayed A; never called implicitly.
    pub fn sort_by_angle(&mut self) -> Result<()> {
        for row in &self.arbitrary {
            validate_row(*row)?;
        }
        self.arbitrary
            .sort_by(|a, b| a.angle.0.total_cmp(&b.angle.0));
        Ok(())
    }

    fn check_index(&self, index: usize) -> Result<()> {
        if index >= self.arbitrary.len() {
            return Err(PathError::parameter(
                "facing sides",
                "the row to move no longer exists",
            ));
        }
        Ok(())
    }

    /// Resolve targets against the whole placed part, not tabs or selected
    /// surface fragments. Defaults update; explicit offsets remain overrides.
    /// This does not certify coverage, stock access or any motion envelope.
    pub fn resolve(
        &self,
        rotary: RotaryMapping,
        part: &[Point3],
        allowance: f64,
    ) -> Result<Vec<ResolvedSide>> {
        if !allowance.is_finite() || allowance < 0.0 {
            return Err(PathError::parameter(
                "stock to leave",
                "must be finite and nonnegative",
            ));
        }
        if self.arbitrary.len() > 4096 {
            return Err(PathError::parameter(
                "facing sides",
                "more than 4096 arbitrary faces",
            ));
        }
        let mut requests = Vec::new();
        for (offset, a) in self.standard.iter().zip([0.0, 90.0, 180.0, 270.0]) {
            if let Some(offset) = offset {
                requests.push(SideRequest {
                    angle: Degrees(a * rotary.sense.sign()),
                    offset: *offset,
                });
            }
        }
        requests.extend_from_slice(&self.arbitrary);
        if requests.is_empty() {
            return Err(PathError::parameter(
                "facing sides",
                "select at least one face",
            ));
        }
        let mut result: Vec<ResolvedSide> = Vec::with_capacity(requests.len());
        for row in requests {
            validate_row(row)?;
            let plane = FacingPlane::side(rotary, row.angle)?;
            let minimum = plane.support(part.iter().copied(), &[])? + allowance;
            let offset = match row.offset {
                FaceOffset::FromPart => minimum,
                FaceOffset::Explicit(v) => v,
            };
            if !minimum.is_finite() || !offset.is_finite() {
                return Err(PathError::parameter(
                    "face offset",
                    "derived target overflowed",
                ));
            }
            if offset < minimum {
                return Err(PathError::parameter(
                    "face offset",
                    "would cut into the part or its stock allowance",
                ));
            }
            if let Some(previous) = result.iter().find(|s| same_angle(s.angle, row.angle)) {
                // Whole-turn rotations can introduce a few ulps in support.
                // Deduplicate only the same plane to arithmetic rounding.
                let roundoff =
                    32.0 * f64::EPSILON * offset.abs().max(previous.offset.abs()).max(1.0);
                if (previous.offset - offset).abs() > roundoff {
                    return Err(PathError::parameter(
                        "facing sides",
                        "duplicate orientation has conflicting plane offsets",
                    ));
                }
            } else {
                result.push(ResolvedSide {
                    angle: row.angle,
                    offset,
                    plane,
                });
            }
        }
        Ok(result)
    }
}

fn same_angle(a: Degrees, b: Degrees) -> bool {
    let d = (a.0.rem_euclid(360.0) - b.0.rem_euclid(360.0)).abs();
    d.min(360.0 - d) <= 1e-10
}

fn validate_row(row: SideRequest) -> Result<()> {
    if !row.angle.0.is_finite() {
        return Err(PathError::parameter("facing angle", "must be finite"));
    }
    if let FaceOffset::Explicit(v) = row.offset
        && !v.is_finite()
    {
        return Err(PathError::parameter("face offset", "must be finite"));
    }
    Ok(())
}

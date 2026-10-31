// SPDX-License-Identifier: AGPL-3.0-or-later
//! Continuous swept-envelope certificates for fixed-A entry moves (0242/0248).
use std::collections::{BTreeMap, VecDeque};

use crate::cutter::{ToolLine, grow_profile};
use crate::error::{PathError, Result};
use crate::{MeshSurface, Solid, SurfaceSource};
use openchuck_types::geometry::{Aabb, Point3, Vec3};
use openchuck_types::ir::ToolPose;
use openchuck_types::job::JobContext;
use openchuck_types::kinematics::{
    MachinePosition, RotaryFrame, machine_xyz, tool_axis_in_model, tool_tip_in_model,
};
use openchuck_types::machine::MachineProfile;
use openchuck_types::tool::RevolvedProfile;

/// Additional controller coordinate precision beyond written decimals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotaryPrecision {
    /// The controller preserves the written A to double precision.
    Decimal,
    /// The controller stores A as a single-precision float. Reserve a full
    /// ULP at the canonical counter magnitude plus 721 degrees. Hosts must
    /// bound the connected/controller offset to at most 720 degrees or recertify.
    Float32,
}

/// Explicit whole-target inputs, separate from selected or warn-only surfaces.
/// The body must enclose the entire configured cutter and shank, measured
/// from the cutting tip. Mesh certificates apply to that mesh target only.
#[derive(Debug)]
pub struct EntrySafety<'a> {
    /// Whole target represented by exact triangle geometry.
    pub target: &'a MeshSurface,
    /// Protected solids, checked conservatively by their complete bounds.
    pub protected: &'a [Solid],
    /// Complete tool body, not only its cutting profile.
    pub body: &'a RevolvedProfile,
    /// Machine frame and configured travel constraints.
    pub machine: &'a MachineProfile,
    /// Precision bound valid for every post that will consume this path.
    pub rotary_precision: RotaryPrecision,
}

impl EntrySafety<'_> {
    /// Proves a straight machine-XYZ, fixed-A sweep. `budget` is shared by
    /// every leg and connection of one entry. Hosts must post XYZ and A to
    /// at least three decimal places; rounding is included in the bound.
    /// For `Float32`, the posted counter must differ from these canonical
    /// coordinates by at most 720 degrees, including any reset offset.
    /// Uses bounded adaptive interval enclosures (0248), never endpoints-only
    /// acceptance. Each attempted enclosure spends one unit, including failures.
    /// Shared assembly also memoises exact queries across passes; a standalone
    /// call retains no cache after returning.
    #[allow(clippy::too_many_arguments)]
    pub fn verify(
        &self,
        from: ToolPose,
        to: ToolPose,
        leave: f64,
        tolerance: f64,
        budget: &mut usize,
        context: &JobContext<'_>,
    ) -> Result<()> {
        SweepSession::new(self)
            .verify(from, to, leave, tolerance, budget, context)
            .map_err(|refused| match refused {
                Refused::Blocked(blocked) => PathError::Strategy(format!(
                    "The back-and-forth ramp cannot be used here: {}. {}",
                    blocked.cause(),
                    blocked.fix()
                )),
                Refused::Error(error) => error,
            })
    }
}

/// What to change in the operation when a back-and-forth ramp is refused:
/// the field and the choices, in the inspector's words (decision 0114).
pub(crate) const ENTRY_FIX: &str = "In the operation's Entry, links and retract settings, \
     turn off Back-and-forth ramp or set Entry to Plunge, Lead or Helix.";

/// Why a straight entry move could not be proved clear, in plain words with
/// the place the user changes it (#291, lane RAMPERRORS). The checks behind
/// each are those of decisions 0242 and 0248; only the wording is here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Blocked {
    /// The swept tool reaches the whole target beyond the tolerance.
    Target,
    /// The swept tool's bounds reach a held region or a retaining tab.
    Protected,
    /// A move ends beyond the machine's linear travel.
    Travel,
    /// The fixed A is outside the machine's A range.
    ALimit,
    /// The controller cannot hold A precisely enough for the bound.
    Precision,
    /// The shared limit of checks for one entry is spent.
    Budget,
    /// The complete tool body has no supported sweep bound.
    Shape,
    /// The move itself cannot be bounded (non-finite, not at a fixed A, or
    /// no progress at machine precision): a defect, not a user setting.
    Unchecked,
}

impl Blocked {
    /// What would happen, as a clause.
    pub(crate) fn cause(self) -> &'static str {
        match self {
            Self::Target => "the tool would cut into the finished part by more than the tolerance",
            Self::Protected => "the tool would reach a held region or a retaining tab",
            Self::Travel => "the tool would go beyond the machine's X, Y or Z travel",
            Self::ALimit => "the A angle of this pass is outside the machine's A range",
            Self::Precision => {
                "the A angle is too large for the controller to hold precisely enough for the \
                 clearance check"
            }
            Self::Budget => {
                "the clearance check reached its limit of 100000 checks for one entry before it \
                 could prove every move clear"
            }
            Self::Shape => {
                "the clearance check cannot bound this tool's shape (its outline, with the \
                 shank, curves inwards)"
            }
            Self::Unchecked => "one of its moves could not be checked for clearance",
        }
    }

    /// Where to change it, as one or two sentences.
    pub(crate) fn fix(self) -> String {
        match self {
            Self::Protected => format!(
                "In the Setup step, under Workholding and tabs, move the held region or tab \
                 clear of the start of the pass, or change the entry: {}",
                lower_first(ENTRY_FIX)
            ),
            Self::Travel => format!(
                "In the Setup step, under Machine, check Travel (in Travel and maximum feeds), \
                 or move the part with Offset along X under Part position, or change the \
                 entry: {}",
                lower_first(ENTRY_FIX)
            ),
            Self::ALimit => format!(
                "In the Setup step, under Machine, check A range, or change the entry: {}",
                lower_first(ENTRY_FIX)
            ),
            Self::Shape => format!(
                "Choose a tool with a plain shank no wider than its cutter, or change the \
                 entry: {}",
                lower_first(ENTRY_FIX)
            ),
            Self::Unchecked => {
                format!("{ENTRY_FIX} Please report this as a defect, with the project if you can.")
            }
            Self::Target | Self::Precision | Self::Budget => ENTRY_FIX.to_owned(),
        }
    }
}

/// `text` with its first letter in lower case, to follow a colon.
pub(crate) fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Why [`SweepSession::verify`] gave no certificate: a refusal the user can
/// act on, or another error (cancellation, an invalid machine) passed on.
#[derive(Debug)]
pub(crate) enum Refused {
    /// The sweep could not be proved clear.
    Blocked(Blocked),
    /// Any other error, unchanged.
    Error(PathError),
}

impl From<PathError> for Refused {
    fn from(error: PathError) -> Self {
        Self::Error(error)
    }
}

impl From<openchuck_types::Error> for Refused {
    fn from(error: openchuck_types::Error) -> Self {
        Self::Error(error.into())
    }
}

// A cache cannot outlive or change its target/body: both are immutably borrowed
// for this complete assembly. Offset identifies the deterministic grown profile
// of that fixed body. No global cache, coordinate quantisation or axial reuse.
type QueryKey = [u64; 7];
const MAX_ATTEMPTS: usize = 100_000;

pub(crate) struct SweepSession<'a> {
    safety: &'a EntrySafety<'a>,
    answers: BTreeMap<QueryKey, Option<f64>>,
    insertion_order: VecDeque<QueryKey>,
    capacity: usize,
}

impl<'a> SweepSession<'a> {
    pub(crate) fn new(safety: &'a EntrySafety<'a>) -> Self {
        Self::with_capacity(safety, MAX_ATTEMPTS)
    }

    fn with_capacity(safety: &'a EntrySafety<'a>, capacity: usize) -> Self {
        Self {
            safety,
            answers: BTreeMap::new(),
            insertion_order: VecDeque::new(),
            capacity,
        }
    }

    fn drop_offset(
        &mut self,
        grown: &RevolvedProfile,
        line: &ToolLine,
        offset: f64,
        context: &JobContext<'_>,
    ) -> Result<Option<f64>> {
        context.check_cancelled()?;
        let a = line.axis.get();
        let o = line.origin;
        let key = [
            o.x.to_bits(),
            o.y.to_bits(),
            o.z.to_bits(),
            a.x.to_bits(),
            a.y.to_bits(),
            a.z.to_bits(),
            offset.to_bits(),
        ];
        let answer = if let Some(answer) = self.answers.get(&key) {
            let _timing = context.time_stage("entry clearance cache hit");
            *answer
        } else {
            let answer = {
                let _timing = context.time_stage("entry clearance query");
                self.safety.target.drop_tool(grown, line).map(|c| c.offset)
            };
            context.check_cancelled()?;
            if self.answers.len() == self.capacity
                && let Some(oldest) = self.insertion_order.pop_front()
            {
                self.answers.remove(&oldest);
            }
            self.answers.insert(key, answer);
            self.insertion_order.push_back(key);
            answer
        };
        context.check_cancelled()?;
        Ok(answer)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn verify(
        &mut self,
        from: ToolPose,
        to: ToolPose,
        leave: f64,
        tolerance: f64,
        budget: &mut usize,
        context: &JobContext<'_>,
    ) -> std::result::Result<(), Refused> {
        let _timing = context.time_stage("entry clearance sweep");
        let proof = self.safety;
        context.check_cancelled()?;
        let refuse = Refused::Blocked;
        if !from.tip.is_finite()
            || !to.tip.is_finite()
            || !from.a.0.is_finite()
            || !to.a.0.is_finite()
            || (from.a.0 - to.a.0).abs() > 1e-12
            || !leave.is_finite()
            || leave < 0.0
            || !tolerance.is_finite()
            || tolerance <= 0.0
        {
            return Err(refuse(Blocked::Unchecked));
        }
        proof.machine.validate()?;
        let frame = RotaryFrame::of_profile(proof.machine);
        let axis = tool_axis_in_model(&frame, from.a);
        if (from.axis.get() - axis.get()).length() > 1e-9
            || (to.axis.get() - axis.get()).length() > 1e-9
        {
            return Err(refuse(Blocked::Unchecked));
        }
        if !proof.body.is_convex() {
            return Err(refuse(Blocked::Shape));
        }
        if let openchuck_types::machine::ARange::Limited { min, max } = proof.machine.a_range
            && (from.a.0 < min.0 || from.a.0 > max.0)
        {
            return Err(refuse(Blocked::ALimit));
        }
        // This certificate requires at least three written decimals on XYZ
        // and A. Bound linear rounding plus the displacement of the entire
        // tool under the half-unit rotary rounding error.
        let radial = |p: Point3| p.y.hypot(p.z);
        let controller_error = match proof.rotary_precision {
            RotaryPrecision::Decimal => 0.0,
            RotaryPrecision::Float32 => {
                // Nearest-equivalent connection plus the firmware
                // non-reset at exactly +/-360 can shift the counter by
                // 720 degrees. One extra degree covers written rounding
                // across a float exponent boundary.
                #[allow(clippy::cast_possible_truncation)]
                let word = (from.a.0.abs().max(to.a.0.abs()) + 721.0) as f32;
                if !word.is_finite() {
                    return Err(refuse(Blocked::Precision));
                }
                let next = f32::from_bits(word.to_bits() + 1);
                let ulp = f64::from(next) - f64::from(word);
                if !ulp.is_finite() {
                    return Err(refuse(Blocked::Precision));
                }
                ulp
            }
        };
        let written_margin = 0.002
            + (radial(from.tip).max(radial(to.tip))
                + proof.body.top()
                + proof.body.cutting_radius())
                * (0.0005 + controller_error).to_radians();
        let start = machine_xyz(&frame, from.tip, from.a);
        let end = machine_xyz(&frame, to.tip, to.a);
        if !proof.machine.travel.contains(start) || !proof.machine.travel.contains(end) {
            return Err(refuse(Blocked::Travel));
        }
        // Bounds of the whole configured tool at every point of the segment.
        // The radial cube is conservative at any supported axis orientation.
        let r = proof.body.cutting_radius() + written_margin;
        let extension = axis.get() * proof.body.top();
        let low = Vec3::new(
            extension.x.min(0.0) - r,
            extension.y.min(0.0) - r,
            extension.z.min(0.0) - r,
        );
        let high = Vec3::new(
            extension.x.max(0.0) + r,
            extension.y.max(0.0) + r,
            extension.z.max(0.0) + r,
        );
        let bounds = Aabb {
            min: Point3::new(
                from.tip.x.min(to.tip.x),
                from.tip.y.min(to.tip.y),
                from.tip.z.min(to.tip.z),
            ) + low,
            max: Point3::new(
                from.tip.x.max(to.tip.x),
                from.tip.y.max(to.tip.y),
                from.tip.z.max(to.tip.z),
            ) + high,
        };
        for protected in proof.protected {
            context.check_cancelled()?;
            let b = protected.bounds();
            if bounds.min.x <= b.max.x
                && bounds.max.x >= b.min.x
                && bounds.min.y <= b.max.y
                && bounds.max.y >= b.min.y
                && bounds.min.z <= b.max.z
                && bounds.max.z >= b.min.z
            {
                return Err(refuse(Blocked::Protected));
            }
        }
        let distance = start.distance(end);
        if !distance.is_finite() || !written_margin.is_finite() {
            return Err(refuse(Blocked::Unchecked));
        }
        // Each attempted interval is a continuous certificate, not a point
        // sample. At its midpoint, P + B_(leave + written + length/2) encloses
        // all translations of the complete body, plus leave and written error.
        // grow_profile restores the tip origin by shifting that Minkowski sum
        // up by offset, hence the unchanged q + offset <= tolerance test.
        // It does not require leave < tolerance (0248 clause 3).
        let mut pending = vec![(start, end)];
        let mut attempts = 0;
        while let Some((low, high)) = pending.pop() {
            context.check_cancelled()?;
            if *budget == 0 || attempts == MAX_ATTEMPTS {
                return Err(refuse(Blocked::Budget));
            }
            *budget -= 1;
            attempts += 1;
            let length = low.distance(high);
            let middle = low + (high - low) * 0.5;
            let offset = leave + written_margin + length * 0.5;
            let grown = grow_profile(proof.body, offset)?;
            let origin = tool_tip_in_model(
                &frame,
                &MachinePosition {
                    xyz: middle,
                    a: from.a,
                },
            );
            let answer = self.drop_offset(&grown, &ToolLine { origin, axis }, offset, context)?;
            if answer.is_none_or(|q| q.is_finite() && q + offset <= tolerance) {
                continue;
            }
            // A coarse failure is inconclusive; every child still needs a
            // certificate. Keep the existing fine-resolution conservative
            // refusal, and bound work even if the arithmetic cannot bisect.
            if length <= tolerance * 0.25 {
                return Err(refuse(Blocked::Target));
            }
            if middle == low || middle == high {
                return Err(refuse(Blocked::Unchecked));
            }
            // Stack order is deterministic, independent of worker count.
            pending.push((middle, high));
            pending.push((low, middle));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

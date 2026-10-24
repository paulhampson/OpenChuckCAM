// SPDX-License-Identifier: AGPL-3.0-or-later
//! Continuous swept-envelope certificates for fixed-A entry moves (0239).
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
}

impl EntrySafety<'_> {
    /// Proves a straight machine-XYZ, fixed-A sweep. `budget` is shared by
    /// every leg and connection of one entry. No endpoints-only acceptance.
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
        context.check_cancelled()?;
        let refuse =
            |reason: &str| PathError::Strategy(format!("back-and-forth ramp clearance: {reason}"));
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
            return Err(refuse("invalid or non-fixed-A sweep"));
        }
        self.machine.validate()?;
        let frame = RotaryFrame::of_profile(self.machine);
        let axis = tool_axis_in_model(&frame, from.a);
        if (from.axis.get() - axis.get()).length() > 1e-9
            || (to.axis.get() - axis.get()).length() > 1e-9
            || !self.body.is_convex()
        {
            return Err(refuse(
                "the complete tool body has no supported sweep bound",
            ));
        }
        if let openchuck_types::machine::ARange::Limited { min, max } = self.machine.a_range
            && (from.a.0 < min.0 || from.a.0 > max.0)
        {
            return Err(refuse("A exceeds the machine limit"));
        }
        let start = machine_xyz(&frame, from.tip, from.a);
        let end = machine_xyz(&frame, to.tip, to.a);
        if !self.machine.travel.contains(start) || !self.machine.travel.contains(end) {
            return Err(refuse("the sweep exceeds linear travel"));
        }
        // Bounds of the whole configured tool at every point of the segment.
        // The radial cube is conservative at any supported axis orientation.
        let r = self.body.cutting_radius();
        let extension = axis.get() * self.body.top();
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
        for protected in self.protected {
            context.check_cancelled()?;
            let b = protected.bounds();
            if bounds.min.x <= b.max.x
                && bounds.max.x >= b.min.x
                && bounds.min.y <= b.max.y
                && bounds.max.y >= b.min.y
                && bounds.min.z <= b.max.z
                && bounds.max.z >= b.min.z
            {
                return Err(refuse("the swept tool bound reaches protected geometry"));
            }
        }
        let distance = start.distance(end);
        let steps = (distance / (tolerance * 0.25)).ceil().max(1.0);
        if !steps.is_finite() || steps > 100_000.0 || steps > (*budget as f64) {
            return Err(refuse("the entry's clearance budget is exhausted"));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = steps as usize;
        *budget -= steps;
        // A ball of half the subsegment length encloses every translation
        // from its midpoint. Minkowski growth therefore certifies continuous
        // motion, including arbitrarily thin obstacles between query points.
        let padding = distance / (2.0 * steps as f64);
        let offset = leave + padding;
        let grown = grow_profile(self.body, offset)?;
        for i in 0..steps {
            context.check_cancelled()?;
            let t = (i as f64 + 0.5) / steps as f64;
            let xyz = start + (end - start) * t;
            let origin = tool_tip_in_model(&frame, &MachinePosition { xyz, a: from.a });
            if self
                .target
                .drop_tool(&grown, &ToolLine { origin, axis })
                .is_some_and(|c| c.offset + offset > tolerance)
            {
                return Err(refuse(
                    "the swept tool reaches the whole target beyond tolerance",
                ));
            }
        }
        Ok(())
    }
}

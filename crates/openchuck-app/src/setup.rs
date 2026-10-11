// SPDX-License-Identifier: AGPL-3.0-or-later
//! Machine and setups: choosing the project's machine profile, and checking
//! each setup against its stock, part and machine (SET-01, SET-03, SET-05,
//! SET-06; work breakdown slice 3, decision record 0011).
//!
//! The types a front end needs are re-exported here, so it needs no engine
//! crate of its own (architecture section 5.8).

use openchuck_types::geometry::Vec3;
use openchuck_types::kinematics::{RotaryFrame, facing_at, model_up};

pub use openchuck_project::machines::{
    MACHINE_SCHEMA, MAKERA_Z1, builtin, builtin_keys, list_library, load_profile, save_profile,
};
pub use openchuck_types::geometry::{Aabb, Point3};
pub use openchuck_types::machine::{
    ARange, ASense, Clearance, Confirmation, MachineProfile, MaxFeeds, ProfileField, Provenance,
    RotaryAxis,
};
pub use openchuck_types::setup::{
    PartExtent, SetupFrame, SetupWarning, SetupWarningKind, WorkOffset, WorkOffsetRegister,
    expected_work_offset,
};
pub use openchuck_types::stock::Stock;
pub use openchuck_types::units::Millimetres;

use openchuck_project::id::SetupId;

/// What checking one setup found (SET-05, SET-06).
#[derive(Debug, Clone, PartialEq)]
pub struct SetupReport {
    /// The setup.
    pub setup: SetupId,
    /// Its name.
    pub name: String,
    /// Problems with stock, clearance and travel, in a fixed order. Empty
    /// when the project has no machine (see [`SetupReport::notes`]).
    pub warnings: Vec<SetupWarning>,
    /// What the work offset register should hold (machine coordinates of
    /// the work zero at A = 0), when the project has a machine.
    pub expected_work_offset: Option<Point3>,
    /// The part's extent in the model frame, when it could be read.
    pub part: Option<PartExtent>,
    /// Other things the user should know: no machine, an unreadable part.
    pub notes: Vec<String>,
}

/// The direction of positive A in words, from the shared kinematics: where
/// the side of the part that faces up at A = 0 faces at A = +90. Shown by the
/// CLI now and in the viewport in slice 8 (work breakdown slice 3 review
/// checklist).
pub fn a_direction(profile: &MachineProfile) -> String {
    let frame = RotaryFrame::of_profile(profile);
    let facing = facing_at(
        &frame,
        model_up(&frame),
        openchuck_types::units::Degrees(90.0),
    );
    format!(
        "positive A turns the part about +{} so that the side facing up (+Z) at A 0 faces {} at A 90",
        profile.rotary_axis.name(),
        direction_name(facing)
    )
}

/// The machine direction nearest `v`, such as `-Y`.
pub fn direction_name(v: Vec3) -> &'static str {
    let candidates = [
        (Vec3::new(1.0, 0.0, 0.0), "+X"),
        (Vec3::new(-1.0, 0.0, 0.0), "-X"),
        (Vec3::new(0.0, 1.0, 0.0), "+Y"),
        (Vec3::new(0.0, -1.0, 0.0), "-Y"),
        (Vec3::new(0.0, 0.0, 1.0), "+Z"),
        (Vec3::new(0.0, 0.0, -1.0), "-Z"),
    ];
    candidates
        .iter()
        .max_by(|a, b| a.0.dot(v).total_cmp(&b.0.dot(v)))
        .map_or("?", |c| c.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_set_01_a_direction_follows_the_sense() {
        let mut profile = MachineProfile::example_x_rotary();
        assert!(a_direction(&profile).ends_with("faces -Y at A 90"));
        profile.a_sense = ASense::Negative;
        assert!(a_direction(&profile).ends_with("faces +Y at A 90"));
    }
}

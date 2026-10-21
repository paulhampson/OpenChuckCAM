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
    MACHINE_SCHEMA, MAKERA_Z1, ProfileEntry, builtin, builtin_entry, builtin_keys, list_library,
    list_library_entries, load_profile, load_profile_entry, new_machine_id, save_profile,
    save_profile_entry,
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
pub use openchuck_types::workholding::{
    HeldRegion, Tab, TabEnd, TabLayout, TabShape, Workholding, auto_tab_span, validate_workholding,
};

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

/// A machine profile the user can choose from, built in or in a library
/// folder (SET-01).
#[derive(Debug, Clone, PartialEq)]
pub struct AvailableMachine {
    /// Where it comes from: `built-in makera-z1` or the file's path.
    pub source: String,
    /// Its ID, which project copies keep (OI-128); `None` if it has none.
    pub id: Option<String>,
    /// The profile, or why its file cannot be read.
    pub profile: Result<MachineProfile, String>,
}

/// The profiles in the library folder `library` (if it exists), in file
/// name order, then the built-in ones.
pub fn available_machines(library: Option<&std::path::Path>) -> Vec<AvailableMachine> {
    let mut out = Vec::new();
    let mut push = |source: String, entry: Result<ProfileEntry, String>| {
        let (id, profile) = match entry {
            Ok(e) => (e.id, Ok(e.profile)),
            Err(e) => (None, Err(e)),
        };
        out.push(AvailableMachine {
            source,
            id,
            profile,
        });
    };
    if let Some(dir) = library.filter(|d| d.is_dir())
        && let Ok(listing) = list_library_entries(dir)
    {
        for (path, entry) in listing {
            push(path.display().to_string(), entry.map_err(|e| e.to_string()));
        }
    }
    for key in builtin_keys() {
        if let Some(entry) = builtin_entry(key) {
            push(format!("built-in {key}"), entry.map_err(|e| e.to_string()));
        }
    }
    out
}

/// Where a project's machine copy stands against the profile it came from
/// (OI-54; decision record 0029 item 16).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum MachineSource {
    /// The project has no machine.
    NoMachine,
    /// The copy records no source: it was set before profiles had IDs, or
    /// from a profile that never had one.
    Unrecorded,
    /// No available profile has the copy's ID.
    NotFound {
        /// The copy's ID.
        id: String,
    },
    /// The copy matches its source.
    Same {
        /// Where the source is.
        source: String,
    },
    /// The source has changed since the copy was taken, or the copy was
    /// edited. The project's copy always wins until the user updates it.
    Differs {
        /// Where the source is.
        source: String,
        /// The source's ID, to keep with the updated copy.
        id: String,
        /// The source profile, to update the copy from.
        profile: Box<MachineProfile>,
        /// The fields that differ, in the user's words.
        fields: Vec<&'static str>,
    },
}

/// Compares the project's machine copy (`copy`, recorded as coming from the
/// profile with `copy_id`) with the available profiles. The first profile
/// with that ID is its source; library profiles come before built-in ones,
/// so a library profile the user derived from a built-in one, keeping its
/// ID, is the one compared.
pub fn compare_machine(
    copy: Option<&MachineProfile>,
    copy_id: Option<&str>,
    available: &[AvailableMachine],
) -> MachineSource {
    let Some(copy) = copy else {
        return MachineSource::NoMachine;
    };
    let Some(id) = copy_id else {
        return MachineSource::Unrecorded;
    };
    let found = available.iter().find_map(|a| {
        a.profile
            .as_ref()
            .ok()
            .filter(|_| a.id.as_deref() == Some(id))
            .map(|p| (a.source.clone(), p))
    });
    match found {
        None => MachineSource::NotFound { id: id.to_owned() },
        Some((source, profile)) => {
            let fields = differing_fields(copy, profile);
            if fields.is_empty() {
                MachineSource::Same { source }
            } else {
                MachineSource::Differs {
                    source,
                    id: id.to_owned(),
                    profile: Box::new(profile.clone()),
                    fields,
                }
            }
        }
    }
}

/// The user's words for a machine profile value (specification section 15),
/// never its name in the profile file.
pub fn profile_field_label(field: ProfileField) -> &'static str {
    match field {
        ProfileField::RotaryAxis => "rotary axis",
        ProfileField::ASense => "A+ direction",
        ProfileField::RotaryOrigin => "axis height",
        ProfileField::ARange => "A range",
        ProfileField::Travel => "travel",
        ProfileField::MaxFeeds => "maximum feeds",
        ProfileField::Clearance => "clearance",
    }
}

/// The fields of two profiles that differ, in the user's words. The
/// destructuring is exhaustive, so a field added to the profile fails to
/// build here until it is compared.
fn differing_fields(a: &MachineProfile, b: &MachineProfile) -> Vec<&'static str> {
    let MachineProfile {
        name,
        rotary_axis,
        a_sense,
        rotary_origin,
        a_range,
        travel,
        max_feeds,
        clearance,
        max_tool_number,
        provenance,
    } = a;
    let checks = [
        ("name", *name == b.name),
        ("rotary axis", *rotary_axis == b.rotary_axis),
        ("A+ direction", *a_sense == b.a_sense),
        ("axis height", *rotary_origin == b.rotary_origin),
        ("A range", *a_range == b.a_range),
        ("travel", *travel == b.travel),
        ("maximum feeds", *max_feeds == b.max_feeds),
        ("clearance", *clearance == b.clearance),
        ("highest tool number", *max_tool_number == b.max_tool_number),
        ("which values are confirmed", *provenance == b.provenance),
    ];
    checks
        .into_iter()
        .filter(|(_, same)| !same)
        .map(|(name, _)| name)
        .collect()
}

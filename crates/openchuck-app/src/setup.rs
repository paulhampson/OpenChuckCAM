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
    MACHINE_SCHEMA, MAKERA_Z1, ProfileEntry, builtin, builtin_entry, builtin_keys,
    has_old_z1_axis_height, list_library, list_library_entries, load_profile, load_profile_entry,
    new_machine_id, save_profile, save_profile_entry,
};
pub use openchuck_types::geometry::{Aabb, Point3};
pub use openchuck_types::machine::{
    ARange, ASense, Clearance, Confirmation, MachineProfile, MaxFeeds, ProfileField, Provenance,
    RotaryAxis, ShortBlockMode,
};
pub use openchuck_types::setup::{
    ClearanceRules, DEFAULT_STOCK_MARGIN, EffectiveClearance, MinimumSource, PartExtent,
    SetupFrame, SetupWarning, SetupWarningKind, WorkOffset, WorkOffsetRegister,
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

/// The clearance `setup` uses on `machine` (SET-06, decision record 0530):
/// its own or the machine's, grown from the stock when Clearance from stock
/// is on, with the minimum it is checked against. `part_bounds` are the
/// setup's part bounds (in the setup's frame), which bounding-box stock
/// measures. Every consumer (toolpaths, links, the post and the checks)
/// takes the clearance from here, so they all use the same one.
pub fn effective_clearance(
    machine: &MachineProfile,
    setup: &openchuck_project::Setup,
    part_bounds: Option<&Aabb>,
) -> EffectiveClearance {
    let part = part_bounds.map(|b| PartExtent {
        bounds: *b,
        radius: Millimetres(
            b.min
                .y
                .abs()
                .max(b.max.y.abs())
                .hypot(b.min.z.abs().max(b.max.z.abs())),
        ),
    });
    openchuck_types::setup::effective_clearance(
        machine,
        setup.clearance.as_ref(),
        &setup.clearance_rules,
        setup.stock.as_ref(),
        part.as_ref(),
    )
}

/// The length a tool is assumed to stick out beyond its flutes, since the
/// tool library holds no stick-out (decision record 0318).
pub const STICK_OUT_BEYOND_FLUTES: f64 = 5.0;

/// A tool that would take the spindle past the top of the Z travel at the
/// safe Z (decision record 0318), in millimetres.
#[derive(Debug, Clone, PartialEq)]
pub struct SpindleHeight {
    /// The tool's assumed stick-out: flute length + 5 mm.
    pub stick_out: f64,
    /// The highest the tool's tip reaches above the axis: the spindle-to-axis
    /// distance less the stick-out. A safe Z up to this fits.
    pub highest_tip: f64,
    /// How far past the top of the travel the spindle would go.
    pub past_top: f64,
    /// The profile's recommended minimum safe Z, when the fit is under it.
    pub recommended_min: Option<f64>,
    /// The minimum clearance radius, when the tool's highest tip is inside
    /// it: no safe Z then clears both the workholding and the top of the Z
    /// travel (decision record 0530).
    pub below_minimum: Option<f64>,
}

/// Whether `tool` would take the spindle past the top of the Z travel at
/// `safe_z` above the axis on `profile` (decision record 0318), or cannot
/// rise clear of the `minimum` clearance radius within the Z travel at all
/// (decision record 0530): only on a machine that measures tool lengths and
/// gives its spindle-to-axis distance. `safe_z` is the setup's effective
/// safe Z ([`effective_clearance`]).
pub fn spindle_height(
    profile: &MachineProfile,
    safe_z: f64,
    minimum: Option<f64>,
    tool: &openchuck_types::tool::Tool,
) -> Option<SpindleHeight> {
    if !profile.automatic_tool_length {
        return None;
    }
    let distance = profile.spindle_to_axis?.0;
    let stick_out = tool.flute_length.0 + STICK_OUT_BEYOND_FLUTES;
    let highest_tip = distance - stick_out;
    // Millimetres to three places, as the G-code: a fit equal to the safe Z
    // within rounding passes.
    let past_top = safe_z - highest_tip;
    let below_minimum = minimum.filter(|m| m - highest_tip > 5e-4);
    if past_top <= 5e-4 && below_minimum.is_none() {
        return None;
    }
    Some(SpindleHeight {
        below_minimum,
        stick_out,
        highest_tip,
        past_top,
        recommended_min: profile
            .recommended_min_safe_z
            .map(|m| m.0)
            .filter(|m| highest_tip < *m),
    })
}

/// The setup check's words for a tool of [`spindle_height`] (NFR-12).
pub fn spindle_height_text(number: u32, name: &str, found: &SpindleHeight) -> String {
    let mut text = if let Some(minimum) = found.below_minimum {
        // Lowering the safe Z would take links into the workholding, so the
        // 0318 advice is not given (decision record 0530).
        format!(
            "Tool {number} ({name}) may be too long to clear the workholding within the Z travel: \
             assuming it sticks out {:.3} mm (flute length + 5 mm), its tip can rise at most {:.3} mm \
             above the axis, inside the minimum clearance radius of {minimum:.3} mm, so no safe Z \
             clears both the chuck, jaws, headstock or tailstock and the top of the Z travel. \
             Use a shorter tool, and check the clearance over the chuck and the Z travel on your \
             machine before running.",
            found.stick_out, found.highest_tip
        )
    } else {
        format!(
            "Tool {number} ({name}) may take the spindle above the top of the Z travel at the safe Z: \
         assuming it sticks out {:.3} mm (flute length + 5 mm), the spindle would be {:.3} mm past the top. \
         Lower the setup's safe Z and clearance to {:.3} mm or less, or use a shorter tool, \
         and check the clearance over the chuck and the Z travel on your machine before running.",
            found.stick_out, found.past_top, found.highest_tip
        )
    };
    if let Some(minimum) = found.recommended_min {
        text.push_str(&format!(
            " The recommended minimum is {minimum} mm. This must be checked before machining."
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat end mill with `flute` mm of flute, as the Z1 defaults are.
    fn tool(flute: f64) -> openchuck_types::tool::Tool {
        openchuck_types::tool::Tool {
            shape: openchuck_types::tool::ToolShape::FlatEnd,
            diameter: Millimetres(3.175),
            flute_length: Millimetres(flute),
            overall_length: Millimetres(flute + 20.0),
            shank_diameter: Millimetres(3.175),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        }
    }

    /// The Makera Z1 as the owner measured it: the axis at -48.6 for the
    /// probe, the probe sticking out 24.5 mm, so D = 73.1 (decision 0318).
    fn z1() -> MachineProfile {
        builtin(MAKERA_Z1)
            .and_then(Result::ok)
            .unwrap_or_else(MachineProfile::example_x_rotary)
    }

    /// The owner's worked cases at the safe Z 40: tool 104 (flute 25) and 204
    /// (flute 22) fit; the 42 mm flute default does not, fits at 26.1 mm,
    /// under the recommended 30 (SET-06, NFR-12, decision 0318).
    #[test]
    fn req_set_06_req_nfr_12_the_spindle_height_follows_the_owners_worked_cases() {
        let z1 = z1();
        assert_eq!(z1.spindle_to_axis.map(|d| d.0), Some(73.1));
        assert_eq!(spindle_height(&z1, 40.0, None, &tool(25.0)), None);
        assert_eq!(spindle_height(&z1, 40.0, None, &tool(22.0)), None);
        let long = spindle_height(&z1, 40.0, None, &tool(42.0)).unwrap();
        assert!((long.stick_out - 47.0).abs() < 1e-9, "{long:?}");
        assert!((long.highest_tip - 26.1).abs() < 1e-9, "{long:?}");
        assert!((long.past_top - 13.9).abs() < 1e-9, "{long:?}");
        assert_eq!(long.recommended_min, Some(30.0));
        // At the old safe Z 50, tool 104 would have been 6.9 mm past the top.
        let gate5 = spindle_height(&z1, 50.0, None, &tool(25.0)).unwrap();
        assert!((gate5.past_top - 6.9).abs() < 1e-9, "{gate5:?}");
        assert_eq!(gate5.recommended_min, None, "43.1 is above 30");
    }

    /// The boundary: a tool that just reaches the top of the travel at the
    /// safe Z passes; 0.001 mm more warns (decision 0318).
    #[test]
    fn req_set_06_the_spindle_height_is_checked_at_its_boundary() {
        let z1 = z1();
        // D - S = safe Z: 73.1 - (28.1 + 5) = 40.
        assert_eq!(spindle_height(&z1, 40.0, None, &tool(28.1)), None);
        let over = spindle_height(&z1, 40.0, None, &tool(28.101)).unwrap();
        assert!((over.past_top - 0.001).abs() < 1e-9, "{over:?}");
    }

    /// No warning without the distance or without tool length measurement:
    /// the check cannot be made, and is not guessed (decision 0318).
    #[test]
    fn req_set_06_req_nfr_12_no_distance_or_no_measurement_gives_no_spindle_height_warning() {
        let mut unknown = z1();
        unknown.spindle_to_axis = None;
        assert_eq!(spindle_height(&unknown, 40.0, None, &tool(42.0)), None);
        let mut manual = z1();
        manual.automatic_tool_length = false;
        assert_eq!(spindle_height(&manual, 40.0, None, &tool(42.0)), None);
        assert_eq!(
            spindle_height(&MachineProfile::example_x_rotary(), 40.0, None, &tool(42.0)),
            None
        );
    }

    /// The text names the tool, its assumed stick-out, how far past the top
    /// the spindle goes and the safe Z that fits, says to check it on the
    /// machine, and adds the owner's recommended minimum when the fit is
    /// under it (NFR-12, decision 0318).
    #[test]
    fn req_nfr_12_the_spindle_height_warning_says_what_to_check() {
        let z1 = z1();
        let long = spindle_height(&z1, 40.0, None, &tool(42.0)).unwrap();
        let text = spindle_height_text(7, "Z1 O-flute 3.175x42", &long);
        for words in [
            "Tool 7 (Z1 O-flute 3.175x42)",
            "47.000 mm",
            "flute length + 5 mm",
            "13.900 mm",
            "26.100 mm",
            "on your machine",
            "The recommended minimum is 30 mm. This must be checked before machining.",
        ] {
            assert!(text.contains(words), "{words}: {text}");
        }
        let gate5 = spindle_height(&z1, 50.0, None, &tool(25.0)).unwrap();
        let text = spindle_height_text(1, "104", &gate5);
        assert!(!text.contains("recommended minimum"), "{text}");
    }

    #[test]
    fn req_set_01_a_direction_follows_the_sense() {
        let mut profile = MachineProfile::example_x_rotary();
        assert!(a_direction(&profile).ends_with("faces -Y at A 90"));
        profile.a_sense = ASense::Negative;
        assert!(a_direction(&profile).ends_with("faces +Y at A 90"));
    }

    /// Case a of decision 0530: the check takes the setup's effective safe
    /// Z, so a tool that fits at the base safe Z warns once Clearance from
    /// stock lifts it (SET-06, NFR-12).
    #[test]
    fn req_set_06_req_nfr_12_the_spindle_height_uses_the_safe_z_grown_from_stock() {
        let z1 = z1();
        let mut setup = openchuck_project::Setup::new(SetupId::from_u128(1), "S");
        setup.stock = Some(Stock::Cylinder {
            diameter: Millimetres(100.0),
            length: Millimetres(50.0),
            x_start: Millimetres(0.0),
        });
        let base = effective_clearance(&z1, &setup, None);
        assert_eq!(base.clearance, z1.clearance);
        assert_eq!(
            spindle_height(&z1, base.clearance.safe_z.0, None, &tool(25.0)),
            None
        );
        setup.clearance_rules.from_stock = Some(DEFAULT_STOCK_MARGIN);
        let grown = effective_clearance(&z1, &setup, None);
        assert_eq!(grown.clearance.safe_z, Millimetres(55.0));
        let found = spindle_height(&z1, grown.clearance.safe_z.0, None, &tool(25.0)).unwrap();
        // 55 - 43.1
        assert!((found.past_top - 11.9).abs() < 1e-9, "{found:?}");
        assert_eq!(found.below_minimum, None);
    }

    /// Case b of decision 0530: a tool whose highest tip is inside the
    /// minimum clearance radius cannot clear both the workholding and the
    /// top of the travel; the warning says so and does not advise lowering
    /// the safe Z below the minimum (NFR-12).
    #[test]
    fn req_set_06_req_nfr_12_a_tool_too_long_for_the_minimum_clearance_warns() {
        let z1 = z1();
        // Tool 104 reaches 43.1: clear of a 40 mm minimum, inside a 45 mm one.
        assert_eq!(spindle_height(&z1, 40.0, Some(40.0), &tool(25.0)), None);
        let found = spindle_height(&z1, 40.0, Some(45.0), &tool(25.0)).unwrap();
        assert_eq!(found.below_minimum, Some(45.0));
        assert!(found.past_top < 0.0, "{found:?}");
        let text = spindle_height_text(1, "104", &found);
        for words in [
            "Tool 1 (104)",
            "too long to clear the workholding within the Z travel",
            "30.000 mm (flute length + 5 mm)",
            "43.100 mm",
            "45.000 mm",
            "Use a shorter tool",
            "on your machine before running",
        ] {
            assert!(text.contains(words), "{words}: {text}");
        }
        assert!(!text.contains("Lower the setup's safe Z"), "{text}");
        // Within rounding of the minimum, it fits.
        assert_eq!(spindle_height(&z1, 40.0, Some(43.1004), &tool(25.0)), None);
        // The recommended-minimum line still follows decision 0318.
        let long = spindle_height(&z1, 40.0, Some(45.0), &tool(42.0)).unwrap();
        assert!(
            spindle_height_text(7, "long", &long).contains("The recommended minimum is 30 mm."),
        );
        // Without measurement there is still no guess (decision 0318).
        let mut manual = z1.clone();
        manual.automatic_tool_length = false;
        assert_eq!(spindle_height(&manual, 40.0, Some(80.0), &tool(42.0)), None);
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
        ProfileField::SpindleRange => "spindle speed range",
        ProfileField::MinBlockMs => "minimum block time",
        ProfileField::BlockWindow => "block window",
        ProfileField::ShortBlockMode => "short-block mode",
        ProfileField::ToolLengthMeasurement => "tool length measurement",
        ProfileField::SpindleToAxis => "spindle-to-axis distance",
        ProfileField::RecommendedMinSafeZ => "recommended minimum safe Z",
        ProfileField::MinClearanceRadius => "minimum clearance radius",
    }
}

/// The fields of two profiles that differ, in the user's words. The
/// destructuring is exhaustive, so a field added to the profile fails to
/// build here until it is compared.
pub(crate) fn differing_fields(a: &MachineProfile, b: &MachineProfile) -> Vec<&'static str> {
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
        min_spindle_rpm,
        max_spindle_rpm,
        min_block_ms,
        block_window,
        short_block_mode,
        automatic_tool_length,
        spindle_to_axis,
        recommended_min_safe_z,
        min_clearance_radius,
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
        (
            "spindle speed range",
            *min_spindle_rpm == b.min_spindle_rpm && *max_spindle_rpm == b.max_spindle_rpm,
        ),
        (
            "minimum block time",
            min_block_ms.to_bits() == b.min_block_ms.to_bits(),
        ),
        ("block window", *block_window == b.block_window),
        ("short-block mode", *short_block_mode == b.short_block_mode),
        (
            "tool length measurement",
            *automatic_tool_length == b.automatic_tool_length,
        ),
        (
            "spindle-to-axis distance",
            *spindle_to_axis == b.spindle_to_axis,
        ),
        (
            "recommended minimum safe Z",
            *recommended_min_safe_z == b.recommended_min_safe_z,
        ),
        (
            "minimum clearance radius",
            *min_clearance_radius == b.min_clearance_radius,
        ),
        ("which values are confirmed", *provenance == b.provenance),
    ];
    checks
        .into_iter()
        .filter(|(_, same)| !same)
        .map(|(name, _)| name)
        .collect()
}

#[cfg(test)]
mod short_block_tests {
    use super::*;

    #[test]
    fn req_gc_17_req_set_01_profile_comparison_names_short_move_settings() {
        let source = MachineProfile::example_x_rotary();
        let mut copy = source.clone();
        copy.min_block_ms = 7.0;
        assert_eq!(differing_fields(&source, &copy), ["minimum block time"]);
        copy.block_window = 32;
        copy.short_block_mode = ShortBlockMode::Report;
        assert_eq!(
            differing_fields(&source, &copy),
            ["minimum block time", "block window", "short-block mode"]
        );
        assert_eq!(source.min_block_ms.to_bits(), 0.0f64.to_bits());
    }
}

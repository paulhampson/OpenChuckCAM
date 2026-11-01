// SPDX-License-Identifier: AGPL-3.0-or-later
//! Machine profile, stock, tool and unit tests (NFR-05, NFR-07).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::machine::{ARange, MachineProfile};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres, Tolerance};
use proptest::prelude::*;

#[test]
fn req_nfr_05_default_tolerance_is_0_01_mm_and_can_change() {
    assert_eq!(Tolerance::default().get(), Millimetres(0.01));
    assert_eq!(
        Tolerance::new(Millimetres(0.002)).unwrap().get(),
        Millimetres(0.002)
    );
    assert!(Tolerance::new(Millimetres(0.0)).is_err());
    assert!(Tolerance::new(Millimetres(f64::INFINITY)).is_err());
    let t: Tolerance = serde_json::from_str("0.005").unwrap();
    assert_eq!(t.get(), Millimetres(0.005));
    assert!(serde_json::from_str::<Tolerance>("-1.0").is_err());
}

#[test]
fn req_nfr_05_units_are_double_precision() {
    // A value that f32 cannot hold survives in every unit type.
    let fine = 1_234.567_890_123_4;
    assert_eq!(Millimetres(fine).value(), fine);
    assert_eq!(Degrees(fine).value(), fine);
    assert_ne!(f64::from(fine as f32), fine);
}

#[test]
fn machine_profile_round_trips_and_validates() {
    let profile = MachineProfile::example_x_rotary();
    profile.validate().unwrap();
    let json = serde_json::to_string_pretty(&profile).unwrap();
    let back: MachineProfile = serde_json::from_str(&json).unwrap();
    assert_eq!(back, profile);
    // The A sense and the rotary axis offset are profile data (decision record 0001, item 2).
    assert!(json.contains("\"a_sense\""));
    assert!(json.contains("\"rotary_origin\""));
}

#[test]
fn req_nfr_07_bad_machine_profiles_give_clear_errors() {
    let mut p = MachineProfile::example_x_rotary();
    p.a_range = ARange::Limited {
        min: Degrees(10.0),
        max: Degrees(-10.0),
    };
    assert!(
        p.validate()
            .unwrap_err()
            .to_string()
            .contains("machine.a_range")
    );

    let mut p = MachineProfile::example_x_rotary();
    p.travel = Aabb {
        min: Point3::new(1.0, 0.0, 0.0),
        max: Point3::new(0.0, 0.0, 0.0),
    };
    assert!(p.validate().is_err());

    let mut p = MachineProfile::example_x_rotary();
    p.rotary_origin.y = f64::NAN;
    assert!(
        p.validate()
            .unwrap_err()
            .to_string()
            .contains("rotary_origin.y")
    );

    let json = serde_json::to_string(&MachineProfile::example_x_rotary())
        .unwrap()
        .replace("\"positive\"", "\"sideways\"");
    assert!(serde_json::from_str::<MachineProfile>(&json).is_err());
}

#[test]
fn req_nfr_07_bad_tools_and_stock_give_errors() {
    let tool = Tool {
        shape: ToolShape::BullNose {
            corner_radius: Millimetres(4.0),
        },
        diameter: Millimetres(6.0),
        flute_length: Millimetres(12.0),
        overall_length: Millimetres(20.0),
        shank_diameter: Millimetres(6.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    };
    assert!(
        tool.validate()
            .unwrap_err()
            .to_string()
            .contains("corner radius")
    );
    let vbit = Tool {
        shape: ToolShape::VBit {
            included_angle: Degrees(180.0),
            tip_diameter: Millimetres(0.0),
        },
        ..tool.clone()
    };
    assert!(vbit.validate().is_err());

    let stock = Stock::Cylinder {
        diameter: Millimetres(-30.0),
        length: Millimetres(60.0),
        x_start: Millimetres(0.0),
    };
    assert!(stock.validate().is_err());
    let mesh = TriangleMesh {
        vertices: vec![Point3::ORIGIN],
        triangles: vec![[0, 0, 7]],
    };
    assert!(
        mesh.validate()
            .unwrap_err()
            .to_string()
            .contains("out of range")
    );
    assert_eq!(mesh.triangle(0), None);
}

#[test]
fn req_nfr_07_zero_direction_is_an_error() {
    assert!(UnitVec3::new(Vec3::ZERO).is_err());
    assert!(UnitVec3::new(Vec3::new(f64::NAN, 0.0, 1.0)).is_err());
}

proptest! {
    /// Arbitrary text fed to every user-suppliable type gives an error or a
    /// value, never a panic (NFR-07). Fuzzing extends this (fuzz/).
    #[test]
    fn req_nfr_07_arbitrary_text_never_panics(text in ".{0,200}") {
        let _ = serde_json::from_str::<MachineProfile>(&text);
        let _ = serde_json::from_str::<Tool>(&text);
        let _ = serde_json::from_str::<Stock>(&text);
        let _ = serde_json::from_str::<openchuck_types::ir::Toolpath>(&text);
    }
}

#[test]
fn req_gc_17_req_nfr_07_profiles_refuse_invalid_block_time_settings() {
    for limit in [f64::NAN, f64::INFINITY, -1.0] {
        let mut machine = openchuck_types::machine::MachineProfile::example_x_rotary();
        machine.min_block_ms = limit;
        assert!(
            machine
                .validate()
                .unwrap_err()
                .to_string()
                .contains("min_block_ms")
        );
    }
    let mut machine = openchuck_types::machine::MachineProfile::example_x_rotary();
    machine.block_window = 0;
    assert!(
        machine
            .validate()
            .unwrap_err()
            .to_string()
            .contains("block_window")
    );
}

/// A machine profile may give the spindle speed range the machine runs at,
/// as a limit only (decision 0231; OI-754): a speed outside it is reported
/// with the limit, and a profile without one checks nothing.
#[test]
fn req_gc_14_req_nfr_12_a_profile_spindle_range_is_a_limit() {
    use openchuck_types::machine::{ProfileField, SpindleLimit};
    use openchuck_types::units::SpindleSpeed;
    let mut m = MachineProfile::example_x_rotary();
    assert_eq!(m.spindle_limit(SpindleSpeed(1.0e6)), None);
    // No range: nothing to confirm either.
    assert!(!m.unconfirmed_fields().contains(&ProfileField::SpindleRange));
    // The block-time pair, the software policy and the three spindle-height
    // fields (decision 0317) and the minimum clearance radius (decision
    // 0530) are off too.
    assert_eq!(m.fields().len(), ProfileField::ALL.len() - 8);
    m.max_spindle_rpm = Some(SpindleSpeed(13000.0));
    m.validate().unwrap();
    assert!(m.unconfirmed_fields().contains(&ProfileField::SpindleRange));
    assert_eq!(m.spindle_limit(SpindleSpeed(13000.0)), None);
    assert_eq!(
        m.spindle_limit(SpindleSpeed(13000.5)),
        Some(SpindleLimit::Above(SpindleSpeed(13000.0)))
    );
    m.min_spindle_rpm = Some(SpindleSpeed(2000.0));
    assert_eq!(
        m.spindle_limit(SpindleSpeed(1500.0)),
        Some(SpindleLimit::Below(SpindleSpeed(2000.0)))
    );
    let text = serde_json::to_string(&m).unwrap();
    assert!(text.contains("\"max_spindle_rpm\":13000"), "{text}");
    let back: MachineProfile = serde_json::from_str(&text).unwrap();
    assert_eq!(back, m);
    // Written only when set.
    let plain = serde_json::to_string(&MachineProfile::example_x_rotary()).unwrap();
    assert!(!plain.contains("spindle"), "{plain}");
}

/// A machine that measures each tool's length (a tool setter) places every
/// tip correctly, but the spindle rises with a longer tool; the profile can
/// say so, give the distance from the spindle at home down to the axis, and a
/// recommended minimum safe Z, each with its provenance (SET-01, UI-13,
/// decision 0317). Unset, they are not written and not listed.
#[test]
fn req_set_01_req_ui_13_a_profile_can_state_tool_length_measurement_and_the_spindle_to_axis_distance()
 {
    use openchuck_types::machine::ProfileField;
    let plain = MachineProfile::example_x_rotary();
    assert!(!plain.automatic_tool_length);
    assert_eq!(plain.spindle_to_axis, None);
    assert_eq!(plain.recommended_min_safe_z, None);
    for field in [
        ProfileField::ToolLengthMeasurement,
        ProfileField::SpindleToAxis,
        ProfileField::RecommendedMinSafeZ,
    ] {
        assert!(ProfileField::ALL.contains(&field), "{field}");
        assert!(!plain.fields().contains(&field), "{field}");
    }
    assert_eq!(
        [
            ProfileField::ToolLengthMeasurement.name(),
            ProfileField::SpindleToAxis.name(),
            ProfileField::RecommendedMinSafeZ.name()
        ],
        [
            "tool_length_measurement",
            "spindle_to_axis",
            "recommended_min_safe_z"
        ]
    );
    let text = serde_json::to_string(&plain).unwrap();
    for key in [
        "automatic_tool_length",
        "spindle_to_axis",
        "recommended_min_safe_z",
    ] {
        assert!(!text.contains(key), "{key}: {text}");
    }

    // With measurement, the distance is listed even while it is not known:
    // it is what the spindle-height check needs.
    let mut m = plain.clone();
    m.automatic_tool_length = true;
    m.validate().unwrap();
    assert!(m.fields().contains(&ProfileField::ToolLengthMeasurement));
    assert!(m.fields().contains(&ProfileField::SpindleToAxis));
    assert!(!m.fields().contains(&ProfileField::RecommendedMinSafeZ));
    m.spindle_to_axis = Some(Millimetres(78.6));
    m.recommended_min_safe_z = Some(Millimetres(30.0));
    m.validate().unwrap();
    assert!(m.fields().contains(&ProfileField::RecommendedMinSafeZ));
    let back: MachineProfile = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
    assert_eq!(back, m);
}

#[test]
fn req_nfr_07_bad_spindle_height_values_are_refused() {
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let mut m = MachineProfile::example_x_rotary();
        m.spindle_to_axis = Some(Millimetres(value));
        let e = m.validate().unwrap_err().to_string();
        assert!(e.contains("spindle_to_axis"), "{value}: {e}");
        let mut m = MachineProfile::example_x_rotary();
        m.recommended_min_safe_z = Some(Millimetres(value));
        let e = m.validate().unwrap_err().to_string();
        assert!(e.contains("recommended_min_safe_z"), "{value}: {e}");
    }
}

#[test]
fn req_nfr_07_a_bad_spindle_range_is_refused() {
    use openchuck_types::units::SpindleSpeed;
    for (min, max) in [
        (None, Some(0.0)),
        (Some(-1.0), None),
        (None, Some(f64::NAN)),
        (Some(9000.0), Some(8000.0)),
        (Some(8000.0), Some(8000.0)),
    ] {
        let mut m = MachineProfile::example_x_rotary();
        m.min_spindle_rpm = min.map(SpindleSpeed);
        m.max_spindle_rpm = max.map(SpindleSpeed);
        let e = m.validate().unwrap_err().to_string();
        assert!(e.contains("spindle"), "{min:?} {max:?}: {e}");
    }
}

// ---- the exact curve of a B-rep edge (decision 0227) -------------------------

#[test]
fn req_imp_06_nfr_07_an_edge_reads_with_or_without_its_exact_curve() {
    use openchuck_types::brep::{BrepEdge, BrepModel, EdgeCurve};
    // A model written before the field existed has no curve.
    let old = r#"{"id":0,"faces":[],"vertices":null,"polyline":[{"x":0,"y":0,"z":0},{"x":1,"y":0,"z":0}]}"#;
    let edge: BrepEdge = serde_json::from_str(old).unwrap();
    assert_eq!(edge.curve, None);

    // A line round-trips, and a damaged one is refused by the model check.
    let with_curve = BrepEdge {
        curve: Some(EdgeCurve::Line {
            origin: Point3::ORIGIN,
            direction: Vec3::new(1.0, 0.0, 0.0),
            first: 0.0,
            last: 1.0,
        }),
        ..edge
    };
    let text = serde_json::to_string(&with_curve).unwrap();
    assert!(text.contains(r#""kind":"line""#));
    assert_eq!(serde_json::from_str::<BrepEdge>(&text).unwrap(), with_curve);
    let mut model = BrepModel {
        edges: vec![with_curve],
        ..BrepModel::default()
    };
    assert!(model.validate().is_ok());
    model.edges[0].curve = Some(EdgeCurve::Line {
        origin: Point3::ORIGIN,
        direction: Vec3::new(0.0, 0.0, 0.0),
        first: 0.0,
        last: 1.0,
    });
    assert!(model.validate().is_err());
}

/// GC-17 defaults leave old profiles off; configured values are validated.
#[test]
fn req_gc_17_req_set_01_profile_defaults_roundtrip_and_validation() {
    use openchuck_types::machine::{ProfileField, ShortBlockMode};
    let mut machine = MachineProfile::example_x_rotary();
    assert_eq!(machine.min_block_ms, 0.0);
    assert_eq!(machine.block_window, 1);
    assert_eq!(machine.short_block_mode, ShortBlockMode::Off);
    let old = serde_json::to_string(&machine).unwrap();
    assert!(!old.contains("min_block_ms"));
    assert_eq!(
        serde_json::from_str::<MachineProfile>(&old).unwrap(),
        machine
    );
    machine.min_block_ms = 7.0;
    machine.block_window = 32;
    machine.short_block_mode = ShortBlockMode::SlowDown;
    machine.validate().unwrap();
    let text = serde_json::to_string(&machine).unwrap();
    assert!(text.contains("slow_down"));
    assert_eq!(
        serde_json::from_str::<MachineProfile>(&text).unwrap(),
        machine
    );
    assert!(machine.fields().contains(&ProfileField::MinBlockMs));
    for limit in [-1.0, f64::NAN, f64::INFINITY] {
        machine.min_block_ms = limit;
        assert!(
            machine
                .validate()
                .unwrap_err()
                .to_string()
                .contains("min_block_ms")
        );
    }
    machine.min_block_ms = 7.0;
    for window in [0, 4097, usize::MAX] {
        machine.block_window = window;
        assert!(
            machine
                .validate()
                .unwrap_err()
                .to_string()
                .contains("block_window")
        );
    }
    assert!(serde_json::from_str::<ShortBlockMode>("\"merge\"").is_err());
}

/// Only active controller facts need confirmation; a reporting policy is software configuration.
#[test]
fn req_gc_17_req_set_01_only_active_block_time_values_need_machine_confirmation() {
    use openchuck_types::machine::{Confirmation, ProfileField, Provenance, ShortBlockMode};
    let mut machine = MachineProfile::example_x_rotary();
    let baseline = machine.fields();
    assert_eq!(baseline.len(), 7);
    for (milliseconds, mode, active) in [
        (0.0, ShortBlockMode::Off, false),
        (7.0, ShortBlockMode::Off, false),
        (0.0, ShortBlockMode::Report, false),
        (0.0, ShortBlockMode::SlowDown, false),
        (7.0, ShortBlockMode::Report, true),
        (7.0, ShortBlockMode::SlowDown, true),
    ] {
        machine.min_block_ms = milliseconds;
        machine.block_window = 32;
        machine.short_block_mode = mode;
        machine.validate().unwrap();
        let fields = machine.fields();
        let unconfirmed = machine.unconfirmed_fields();
        for field in [ProfileField::MinBlockMs, ProfileField::BlockWindow] {
            assert_eq!(
                fields.contains(&field),
                active,
                "{mode:?} at {milliseconds} ms"
            );
            assert_eq!(
                unconfirmed.contains(&field),
                active,
                "{mode:?} at {milliseconds} ms"
            );
        }
        assert!(!fields.contains(&ProfileField::ShortBlockMode));
        assert!(!unconfirmed.contains(&ProfileField::ShortBlockMode));
        if !active {
            assert_eq!(fields, baseline);
        }
    }
    // Imported policy provenance cannot turn a software choice into a confirmed machine fact.
    for field in [
        ProfileField::MinBlockMs,
        ProfileField::BlockWindow,
        ProfileField::ShortBlockMode,
    ] {
        machine.provenance.insert(
            field,
            Provenance {
                state: Confirmation::Confirmed,
                source: "Recorded in an imported profile".into(),
            },
        );
    }
    assert!(machine.is_confirmed(ProfileField::MinBlockMs));
    assert!(machine.is_confirmed(ProfileField::BlockWindow));
    assert!(!machine.is_confirmed(ProfileField::ShortBlockMode));
    assert!(
        machine
            .unconfirmed_fields()
            .iter()
            .all(|field| baseline.contains(field))
    );
}

/// A profile can give the smallest safe cylinder radius that clears its
/// chuck, jaws, headstock and tailstock, with its provenance (SET-06, SET-10,
/// UI-13; decision 0530). Unset, it is not written and not listed, so an
/// existing profile reads and writes as before (GC-07).
#[test]
fn req_set_06_req_ui_13_a_profile_can_give_a_minimum_clearance_radius() {
    use openchuck_types::machine::ProfileField;
    let plain = MachineProfile::example_x_rotary();
    assert_eq!(plain.min_clearance_radius, None);
    assert!(ProfileField::ALL.contains(&ProfileField::MinClearanceRadius));
    assert!(!plain.fields().contains(&ProfileField::MinClearanceRadius));
    assert_eq!(
        ProfileField::MinClearanceRadius.name(),
        "min_clearance_radius"
    );
    let text = serde_json::to_string(&plain).unwrap();
    assert!(!text.contains("min_clearance_radius"), "{text}");

    let mut m = plain.clone();
    m.min_clearance_radius = Some(Millimetres(45.0));
    m.validate().unwrap();
    assert!(m.fields().contains(&ProfileField::MinClearanceRadius));
    assert!(
        m.unconfirmed_fields()
            .contains(&ProfileField::MinClearanceRadius)
    );
    let back: MachineProfile = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
    assert_eq!(back, m);
    // A minimum above the profile's own safe cylinder is allowed: the setup
    // checks warn about it, the profile does not refuse it (decision 0530).
    m.min_clearance_radius = Some(Millimetres(m.clearance.radius.0 + 10.0));
    m.validate().unwrap();
}

#[test]
fn req_nfr_07_a_bad_minimum_clearance_radius_is_refused() {
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut m = MachineProfile::example_x_rotary();
        m.min_clearance_radius = Some(Millimetres(value));
        let e = m.validate().unwrap_err().to_string();
        assert!(e.contains("min_clearance_radius"), "{value}: {e}");
    }
}

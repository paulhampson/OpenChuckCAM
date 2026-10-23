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
    assert_eq!(m.fields().len(), ProfileField::ALL.len() - 1);
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

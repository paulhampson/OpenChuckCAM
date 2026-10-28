// SPDX-License-Identifier: AGPL-3.0-or-later
//! Mode-specific settings preserve legacy requirements and inactive edits.
// Exact stored/default values are compared without geometric arithmetic.
#![allow(clippy::unwrap_used, clippy::float_cmp)]
use openchuck_path::facing::selection::{FaceOffset, SideRequest, decode_rows, encode_rows};
use openchuck_path::{Facing, ParameterValue, Strategy};
use openchuck_types::units::Degrees;
use std::collections::BTreeMap;

fn values(shape: &str) -> BTreeMap<String, ParameterValue> {
    [
        ("shape", ParameterValue::Text(shape.into())),
        ("feed", ParameterValue::Number(600.)),
        ("plunge_feed", ParameterValue::Number(200.)),
        ("stepover", ParameterValue::Number(1.)),
        ("stepdown", ParameterValue::Number(2.)),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect()
}

#[test]
fn req_op_24_legacy_size_stays_required_while_new_modes_have_own_settings() {
    for mode in ["flat", "round"] {
        assert!(Facing.resolve_parameters(&values(mode)).is_err());
    }
    for mode in ["end", "box"] {
        let p = Facing.resolve_parameters(&values(mode)).unwrap();
        assert_eq!(p.text("shape").unwrap(), mode);
        assert_eq!(p.text("extent").unwrap(), "whole_stock");
        assert_eq!(p.number("extent_margin").unwrap(), 0.);
        assert!(p.get("size").is_none());
    }
    let p = Facing.resolve_parameters(&values("box")).unwrap();
    assert_eq!(p.text("layer_distribution").unwrap(), "full");
}

#[test]
fn req_op_24_inactive_edits_are_retained_without_becoming_active_targets() {
    let mut v = values("box");
    v.insert("size".into(), ParameterValue::Number(9.));
    v.insert("angles".into(), ParameterValue::Text("130, 40".into()));
    v.insert("positive_z_offset".into(), ParameterValue::Number(12.));
    let active = Facing.resolve_parameters(&v).unwrap();
    assert!(active.get("size").is_none());
    assert!(active.get("positive_z_offset").is_none());
    assert_eq!(v.get("size"), Some(&ParameterValue::Number(9.)));
    v.insert("positive_z_from_part".into(), ParameterValue::Flag(false));
    v.insert("positive_z".into(), ParameterValue::Flag(true));
    assert_eq!(
        Facing
            .resolve_parameters(&v)
            .unwrap()
            .number("positive_z_offset")
            .unwrap(),
        12.
    );
    v.insert("not_a_setting".into(), ParameterValue::Flag(true));
    assert!(Facing.resolve_parameters(&v).is_err());
}

#[test]
fn req_op_24_manual_plane_offset_requires_an_entered_value() {
    let mut v = values("box");
    v.insert("positive_z".into(), ParameterValue::Flag(true));
    v.insert("positive_z_from_part".into(), ParameterValue::Flag(false));
    assert!(Facing.resolve_parameters(&v).is_err());
    v.insert("positive_z_offset".into(), ParameterValue::Number(0.));
    assert_eq!(
        Facing
            .resolve_parameters(&v)
            .unwrap()
            .number("positive_z_offset")
            .unwrap(),
        0.
    );
}

#[test]
fn req_op_24_unselected_face_keeps_its_edits_without_requiring_a_target() {
    let mut v = values("box");
    v.insert("positive_z_from_part".into(), ParameterValue::Flag(false));
    let active = Facing.resolve_parameters(&v).unwrap();
    assert!(active.get("positive_z_offset").is_none());
    assert!(active.get("positive_z_from_part").is_none());
}

#[test]
fn req_op_24_req_nfr_07_arbitrary_rows_roundtrip_order_offsets_and_validate() {
    let rows = vec![
        SideRequest {
            angle: Degrees(130.),
            offset: FaceOffset::Explicit(9.),
        },
        SideRequest {
            angle: Degrees(40.),
            offset: FaceOffset::FromPart,
        },
    ];
    let text = encode_rows(&rows).unwrap();
    assert_eq!(decode_rows(&text).unwrap(), rows);
    for text in [
        r#"[{"angle":40,"offset":null,"unknown":true}]"#,
        r#"[{"angle":"NaN"}]"#,
        r#"[{"angle":1,"offset":"bad"}]"#,
        r#"[{}]"#,
    ] {
        assert!(decode_rows(text).is_err(), "{text}");
    }
    let mut bad = rows.clone();
    bad[0].angle = Degrees(f64::NAN);
    assert!(encode_rows(&bad).is_err());
}

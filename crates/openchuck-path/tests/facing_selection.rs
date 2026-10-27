// SPDX-License-Identifier: AGPL-3.0-or-later
//! New side choices and user order (OP-24; accepted 0244).
#![allow(clippy::float_cmp, clippy::indexing_slicing)]
use openchuck_path::facing::selection::{FaceOffset, SideRequest, SideSelection};
use openchuck_path::rotary::RotaryMapping;
use openchuck_types::geometry::Point3;
use openchuck_types::machine::{ARange, ASense, RotaryAxis};
use openchuck_types::units::Degrees;

fn mapping(sense: ASense) -> RotaryMapping {
    RotaryMapping {
        axis: RotaryAxis::X,
        sense,
        range: ARange::Continuous,
    }
}
fn part() -> [Point3; 2] {
    [Point3::new(10., -3., -4.), Point3::new(50., 5., 7.)]
}
fn row(a: f64, offset: FaceOffset) -> SideRequest {
    SideRequest {
        angle: Degrees(a),
        offset,
    }
}

#[test]
fn req_op_24_every_nonempty_standard_face_combination_and_both_a_senses() {
    for mask in 1..16 {
        for (sense, sign) in [(ASense::Positive, 1.), (ASense::Negative, -1.)] {
            let mut choice = SideSelection::default();
            for (i, slot) in choice.standard.iter_mut().enumerate() {
                *slot = if mask & (1 << i) != 0 {
                    Some(FaceOffset::FromPart)
                } else {
                    None
                };
            }
            let out = choice.resolve(mapping(sense), &part(), 0.2).unwrap();
            assert_eq!(out.len(), (mask as u32).count_ones() as usize);
            let expected = [
                (0., 7.2),
                (90. * sign, 5.2),
                (180. * sign, 4.2),
                (270. * sign, 3.2),
            ];
            for (actual, (a, h)) in out.iter().zip(
                expected
                    .into_iter()
                    .enumerate()
                    .filter_map(|(i, p)| if mask & (1 << i) != 0 { Some(p) } else { None }),
            ) {
                assert!((actual.angle.0 - a).abs() < 1e-9, "{actual:?}");
                assert!((actual.offset - h).abs() < 1e-9, "{actual:?}");
            }
        }
    }
}

#[test]
fn req_op_24_arbitrary_rows_follow_standard_faces_and_preserve_user_order() {
    let choice = SideSelection {
        standard: [Some(FaceOffset::FromPart), None, None, None],
        arbitrary: vec![
            row(130., FaceOffset::Explicit(9.)),
            row(40., FaceOffset::Explicit(12.)),
        ],
    };
    let out = choice
        .resolve(mapping(ASense::Positive), &part(), 0.2)
        .unwrap();
    assert_eq!(
        out.iter().map(|s| s.angle.0).collect::<Vec<_>>(),
        vec![0., 130., 40.]
    );
    assert_eq!(
        out.iter().map(|s| s.offset).collect::<Vec<_>>(),
        vec![7.2, 9., 12.]
    );
}

#[test]
fn req_op_24_moving_and_sorting_rows_keeps_their_offsets_attached() {
    let mut choice = SideSelection {
        standard: [None; 4],
        arbitrary: vec![
            row(130., FaceOffset::Explicit(9.)),
            row(-10., FaceOffset::FromPart),
            row(40., FaceOffset::Explicit(12.)),
        ],
    };
    assert!(choice.move_up(2).unwrap());
    assert_eq!(choice.arbitrary[1], row(40., FaceOffset::Explicit(12.)));
    assert!(choice.move_down(0).unwrap());
    assert_eq!(choice.arbitrary[1], row(130., FaceOffset::Explicit(9.)));
    assert!(!choice.move_up(0).unwrap());
    assert!(!choice.move_down(2).unwrap());
    choice.sort_by_angle().unwrap();
    assert_eq!(
        choice.arbitrary,
        vec![
            row(-10., FaceOffset::FromPart),
            row(40., FaceOffset::Explicit(12.)),
            row(130., FaceOffset::Explicit(9.))
        ]
    );
    assert!(choice.move_up(3).is_err());
}

#[test]
fn req_op_24_req_nfr_07_empty_conflicting_duplicate_and_invalid_faces_refuse() {
    assert!(
        SideSelection::default()
            .resolve(mapping(ASense::Positive), &part(), 0.)
            .is_err()
    );
    for bad in [f64::NAN, f64::INFINITY] {
        let mut choice = SideSelection {
            standard: [None; 4],
            arbitrary: vec![row(bad, FaceOffset::FromPart)],
        };
        assert!(
            choice
                .resolve(mapping(ASense::Positive), &part(), 0.)
                .is_err()
        );
        assert!(choice.sort_by_angle().is_err());
    }
    let choice = SideSelection {
        standard: [Some(FaceOffset::Explicit(7.)), None, None, None],
        arbitrary: vec![row(360., FaceOffset::Explicit(8.))],
    };
    assert!(
        choice
            .resolve(mapping(ASense::Positive), &part(), 0.)
            .unwrap_err()
            .to_string()
            .contains("conflicting")
    );
    let choice = SideSelection {
        standard: [Some(FaceOffset::Explicit(6.)), None, None, None],
        arbitrary: vec![],
    };
    assert!(
        choice
            .resolve(mapping(ASense::Positive), &part(), 0.)
            .unwrap_err()
            .to_string()
            .contains("part")
    );
}

#[test]
fn req_op_24_explicit_offset_is_not_replaced_by_changed_default_allowance() {
    let choice = SideSelection {
        standard: [Some(FaceOffset::Explicit(9.)), None, None, None],
        arbitrary: vec![],
    };
    for allowance in [0., 0.2, 1.] {
        let out = choice
            .resolve(mapping(ASense::Positive), &part(), allowance)
            .unwrap();
        assert_eq!(out[0].offset, 9.);
    }
    let derived = SideSelection {
        standard: [Some(FaceOffset::FromPart), None, None, None],
        arbitrary: vec![],
    };
    assert_eq!(
        derived
            .resolve(mapping(ASense::Positive), &part(), 1.)
            .unwrap()[0]
            .offset,
        8.
    );
}

#[test]
fn req_op_24_all_four_selects_missing_faces_without_erasing_explicit_offsets() {
    let mut choice = SideSelection {
        standard: [Some(FaceOffset::Explicit(9.0)), None, None, None],
        arbitrary: vec![row(40.0, FaceOffset::Explicit(12.0))],
    };
    choice.all_four();
    assert_eq!(
        choice.standard,
        [
            Some(FaceOffset::Explicit(9.0)),
            Some(FaceOffset::FromPart),
            Some(FaceOffset::FromPart),
            Some(FaceOffset::FromPart)
        ]
    );
    assert_eq!(
        choice.arbitrary,
        vec![row(40.0, FaceOffset::Explicit(12.0))]
    );
}

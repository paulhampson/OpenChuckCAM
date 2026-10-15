// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tool types, the revolved profile and its validation (slice 4; TL-02 to
//! TL-06, NFR-07; decision records 0012 and 0014).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use openchuck_types::tool::{
    ControllerMapping, CuttingData, CuttingTable, ProfilePoint, ProfileSegment, Tool, ToolProfile,
    ToolShape, validate_entry,
};
use openchuck_types::units::{Degrees, FeedRate, Millimetres, SpindleSpeed};

const D: f64 = 6.0;
const FLUTE: f64 = 12.0;

fn tool(shape: ToolShape) -> Tool {
    Tool {
        shape,
        diameter: Millimetres(D),
        flute_length: Millimetres(FLUTE),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(6.0),
        flute_count: 2,
        material: Some("carbide".into()),
        centre_cutting: None,
    }
}

fn line(r: f64, z: f64) -> ProfileSegment {
    ProfileSegment::Line {
        to: ProfilePoint::new(r, z),
    }
}

fn arc(r: f64, z: f64, cr: f64, cz: f64) -> ProfileSegment {
    ProfileSegment::Arc {
        to: ProfilePoint::new(r, z),
        centre: ProfilePoint::new(cr, cz),
    }
}

fn custom(segments: Vec<ProfileSegment>) -> ToolShape {
    ToolShape::Custom {
        profile: ToolProfile { segments },
    }
}

/// Every standard type with its closed-form lower envelope `h(r)` for
/// `0 <= r <= D/2`.
#[allow(clippy::type_complexity)]
fn standard_types() -> Vec<(ToolShape, Box<dyn Fn(f64) -> f64>)> {
    let r = D / 2.0;
    let t30 = 30f64.to_radians();
    let (rt, a) = (1.0, 15f64.to_radians());
    let (tr, tz) = (rt * a.cos(), rt * (1.0 - a.sin()));
    vec![
        (ToolShape::FlatEnd, Box::new(|_| 0.0)),
        (
            ToolShape::BallEnd,
            Box::new(move |x: f64| r - (r * r - x * x).sqrt()),
        ),
        (
            ToolShape::BullNose {
                corner_radius: Millimetres(1.0),
            },
            Box::new(move |x: f64| {
                if x <= r - 1.0 {
                    0.0
                } else {
                    1.0 - (1.0 - (x - (r - 1.0)).powi(2)).sqrt()
                }
            }),
        ),
        (
            ToolShape::TaperedBall {
                tip_radius: Millimetres(rt),
                taper_half_angle: Degrees(15.0),
            },
            Box::new(move |x: f64| {
                if x <= tr {
                    rt - (rt * rt - x * x).sqrt()
                } else {
                    tz + (x - tr) / a.tan()
                }
            }),
        ),
        (
            ToolShape::VBit {
                included_angle: Degrees(60.0),
                tip_diameter: Millimetres(0.0),
            },
            Box::new(move |x: f64| x / t30.tan()),
        ),
        (
            ToolShape::Chamfer {
                included_angle: Degrees(90.0),
                tip_diameter: Millimetres(1.0),
            },
            Box::new(|x: f64| (x - 0.5).max(0.0)),
        ),
        (
            ToolShape::Drill {
                point_angle: Degrees(118.0),
            },
            Box::new(|x: f64| x / 59f64.to_radians().tan()),
        ),
    ]
}

#[test]
fn req_tl_02_req_tl_03_standard_profiles_match_their_closed_form_silhouettes() {
    for (shape, h) in standard_types() {
        let name = shape.type_name();
        let profile = tool(shape).profile().unwrap();
        // The outline ends at the tool radius and the flute length.
        assert_eq!(profile.cutting_radius(), D / 2.0, "{name}");
        assert_eq!(profile.top(), FLUTE, "{name}");
        for i in 0..=300 {
            let r = f64::from(i) * 0.01;
            let got = profile.height_at(r).unwrap();
            assert!(
                (got - h(r)).abs() < 1e-12,
                "{name} at r {r}: {got} against {}",
                h(r)
            );
        }
        assert_eq!(profile.height_at(D / 2.0 + 0.1), None, "{name}");
        // Every standard type is convex, so the normal-offset fast path applies.
        assert!(profile.is_convex(), "{name}");
    }
}

#[test]
fn req_tl_04_every_type_validates_and_carries_its_parameters() {
    for (shape, _) in standard_types() {
        let t = tool(shape);
        t.validate().unwrap();
        // Parameters survive a JSON round trip (INT-02).
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<Tool>(&json).unwrap(), t);
    }
    let t = tool(custom(vec![arc(3.0, 3.0, 0.0, 3.0)]));
    t.validate().unwrap();
}

#[test]
fn req_tl_02_req_tl_03_impossible_values_are_rejected() {
    let cases: Vec<(Tool, &str)> = vec![
        (
            tool(ToolShape::BullNose {
                corner_radius: Millimetres(3.5),
            }),
            "corner_radius",
        ),
        (
            tool(ToolShape::BullNose {
                corner_radius: Millimetres(0.0),
            }),
            "corner_radius",
        ),
        (
            tool(ToolShape::VBit {
                included_angle: Degrees(0.0),
                tip_diameter: Millimetres(0.0),
            }),
            "included_angle",
        ),
        (
            tool(ToolShape::Chamfer {
                included_angle: Degrees(180.0),
                tip_diameter: Millimetres(0.0),
            }),
            "included_angle",
        ),
        (
            tool(ToolShape::VBit {
                included_angle: Degrees(60.0),
                tip_diameter: Millimetres(6.0),
            }),
            "tip_diameter",
        ),
        (
            tool(ToolShape::Drill {
                point_angle: Degrees(0.0),
            }),
            "point_angle",
        ),
        (
            tool(ToolShape::TaperedBall {
                tip_radius: Millimetres(3.5),
                taper_half_angle: Degrees(10.0),
            }),
            "tip_radius",
        ),
        (
            tool(ToolShape::TaperedBall {
                tip_radius: Millimetres(1.0),
                taper_half_angle: Degrees(90.0),
            }),
            "taper_half_angle",
        ),
        // A 20 degree V reaches 3 mm radius at about 17 mm, above the flutes.
        (
            tool(ToolShape::VBit {
                included_angle: Degrees(20.0),
                tip_diameter: Millimetres(0.0),
            }),
            "flute length",
        ),
        (
            Tool {
                flute_length: Millimetres(2.0),
                ..tool(ToolShape::BallEnd)
            },
            "flute length",
        ),
        (
            Tool {
                diameter: Millimetres(f64::NAN),
                ..tool(ToolShape::FlatEnd)
            },
            "tool.diameter",
        ),
        (
            Tool {
                overall_length: Millimetres(10.0),
                ..tool(ToolShape::FlatEnd)
            },
            "overall length",
        ),
        (
            Tool {
                flute_count: 0,
                ..tool(ToolShape::FlatEnd)
            },
            "flute_count",
        ),
        (
            Tool {
                material: Some(" ".into()),
                centre_cutting: None,
                ..tool(ToolShape::FlatEnd)
            },
            "tool.material",
        ),
        (tool(custom(vec![arc(2.0, 2.0, 0.0, 2.0)])), "tool radius"),
    ];
    for (t, expected) in cases {
        let err = t.validate().expect_err(expected).to_string();
        assert!(err.contains(expected), "{expected}: {err}");
    }
}

#[test]
fn req_tl_03_bad_custom_profiles_are_rejected_naming_the_segment() {
    let cases: Vec<(Vec<ProfileSegment>, &str, &str)> = vec![
        // Self-intersecting: the outline folds back down across itself.
        (
            vec![
                line(3.0, 0.0),
                line(3.0, 6.0),
                line(1.0, 2.0),
                line(3.0, 12.0),
            ],
            "segment 3",
            "turns back",
        ),
        // Undercut (dovetail): the radius shrinks going up.
        (
            vec![line(3.0, 0.0), line(2.0, 4.0), line(3.0, 12.0)],
            "segment 2",
            "undercut",
        ),
        // Disconnected: the input form joins each segment to the last, so a
        // gap shows as an arc whose start is not on its own circle.
        (
            vec![line(1.0, 0.0), arc(3.0, 2.0, 1.0, 1.5)],
            "segment 2",
            "from its centre",
        ),
        (
            vec![line(3.0, 0.0), line(3.0, 0.0)],
            "segment 2",
            "zero length",
        ),
        (vec![line(0.0, 3.0)], "segment 1", "up the tool axis"),
        (vec![line(f64::INFINITY, 0.0)], "segment 1", "finite"),
        // A half circle is more than a quarter turn.
        (
            vec![line(1.0, 0.0), arc(1.0, 2.0, 1.0, 1.0)],
            "segment 2",
            "quarter turn",
        ),
        (Vec::new(), "tool.profile", "at least one segment"),
    ];
    for (segments, segment, reason) in cases {
        let err = tool(custom(segments)).validate().unwrap_err().to_string();
        assert!(
            err.contains(segment) && err.contains(reason),
            "{segment}, {reason}: {err}"
        );
    }
}

#[test]
fn req_tl_03_custom_profile_agrees_with_the_equivalent_standard_tool() {
    let r = D / 2.0;
    let pairs = vec![
        // A ball typed as two arcs.
        (
            ToolShape::BallEnd,
            vec![
                arc(
                    r * 45f64.to_radians().sin(),
                    r - r * 45f64.to_radians().cos(),
                    0.0,
                    r,
                ),
                arc(r, r, 0.0, r),
            ],
        ),
        // A flat with a split bottom.
        (ToolShape::FlatEnd, vec![line(1.0, 0.0), line(r, 0.0)]),
        // A bull-nose typed in by hand.
        (
            ToolShape::BullNose {
                corner_radius: Millimetres(1.0),
            },
            vec![line(2.0, 0.0), arc(3.0, 1.0, 2.0, 1.0)],
        ),
        // A V with a split flank, running all the way to the flute length.
        (
            ToolShape::VBit {
                included_angle: Degrees(90.0),
                tip_diameter: Millimetres(0.0),
            },
            vec![line(1.0, 1.0), line(3.0, 3.0), line(3.0, FLUTE)],
        ),
    ];
    for (shape, segments) in pairs {
        let name = shape.type_name();
        let standard = tool(shape).profile().unwrap();
        let typed = tool(custom(segments)).profile().unwrap();
        assert_eq!(typed.top(), standard.top(), "{name}");
        for i in 0..=300 {
            let x = f64::from(i) * 0.01;
            let (a, b) = (standard.height_at(x).unwrap(), typed.height_at(x).unwrap());
            assert!((a - b).abs() < 1e-12, "{name} at {x}: {a} against {b}");
        }
    }
}

#[test]
fn req_tl_03_a_concave_custom_profile_is_valid_but_not_convex() {
    // A corner-rounding cutter: flat, concave arc, side.
    let t = tool(custom(vec![line(1.0, 0.0), arc(3.0, 2.0, 3.0, 0.0)]));
    let profile = t.profile().unwrap();
    assert!(!profile.is_convex());
    assert!(profile.point_for_normal(-0.5).is_none());
    assert!(
        profile
            .envelope()
            .iter()
            .any(|e| !e.piece.is_convex_in_radius())
    );
}

fn data() -> CuttingData {
    CuttingData {
        spindle_speed: SpindleSpeed(18000.0),
        feed_per_tooth: Millimetres(0.03),
        plunge_feed: FeedRate(300.0),
        stepover: Millimetres(2.4),
        stepdown: Millimetres(1.5),
    }
}

#[test]
fn req_tl_05_cutting_data_validates_and_gives_the_feed() {
    let t = tool(ToolShape::FlatEnd);
    let d = data();
    d.validate(&t, "cutting_data.oak").unwrap();
    assert!((d.cutting_feed(&t).0 - 1080.0).abs() < 1e-9);
    let bad = [
        (
            CuttingData {
                spindle_speed: SpindleSpeed(0.0),
                ..d
            },
            "spindle_speed",
        ),
        (
            CuttingData {
                stepover: Millimetres(7.0),
                ..d
            },
            "stepover",
        ),
        (
            CuttingData {
                stepdown: Millimetres(13.0),
                ..d
            },
            "stepdown",
        ),
        (
            CuttingData {
                plunge_feed: FeedRate(f64::NAN),
                ..d
            },
            "plunge_feed",
        ),
    ];
    for (data, field) in bad {
        let err = data
            .validate(&t, "cutting_data.oak")
            .unwrap_err()
            .to_string();
        assert!(err.contains(&format!("cutting_data.oak.{field}")), "{err}");
    }
    let mut table = CuttingTable::new();
    table.insert("oak".into(), d);
    validate_entry(&t, &ControllerMapping::numbered(3), &table).unwrap();
    table.insert(" oak".into(), d);
    assert!(validate_entry(&t, &ControllerMapping::numbered(3), &table).is_err());
}

#[test]
fn req_tl_06_controller_mapping_defaults_offsets_to_the_tool_number() {
    let m = ControllerMapping::numbered(4);
    assert_eq!(
        (m.number, m.length_offset(), m.diameter_offset()),
        (4, 4, 4)
    );
    let m = ControllerMapping {
        number: 4,
        length_offset: Some(14),
        diameter_offset: None,
    };
    assert_eq!((m.length_offset(), m.diameter_offset()), (14, 4));
    m.validate().unwrap();
    assert!(ControllerMapping::numbered(0).validate().is_err());
    let m = ControllerMapping {
        diameter_offset: Some(0),
        ..m
    };
    assert!(
        m.validate()
            .unwrap_err()
            .to_string()
            .contains("diameter_offset")
    );
}

proptest::proptest! {
    /// Random profiles never panic, and an accepted one has a non-decreasing
    /// lower envelope that starts at the tip and ends at its end point
    /// (NFR-07, decision record 0012 item 2).
    #[test]
    fn req_tl_03_req_nfr_07_random_profiles_validate_or_fail_cleanly(
        raw in proptest::collection::vec(
            (0u8..3, -1.0f64..4.0, -1.0f64..6.0, -1.0f64..4.0, -1.0f64..6.0),
            0..6,
        )
    ) {
        let segments: Vec<ProfileSegment> = raw
            .iter()
            .map(|&(kind, r, z, cr, cz)| match kind {
                0 => line(r, z),
                1 => arc(r, z, cr, cz),
                _ => line(r.abs(), z.abs()),
            })
            .collect();
        if let Ok(profile) = (ToolProfile { segments }).validate() {
            let mut last = 0.0;
            let end = profile.cutting_radius();
            proptest::prop_assert!(profile.height_at(0.0).unwrap().abs() < 1e-9);
            for i in 0..=50 {
                let h = profile.height_at(end * f64::from(i) / 50.0).unwrap();
                proptest::prop_assert!(h.is_finite() && h >= last - 1e-9);
                last = h;
            }
        }
    }
}

#[test]
fn req_tl_04_a_stated_centre_cutting_flag_overrides_the_shape() {
    let mut t = tool(ToolShape::FlatEnd);
    assert!(t.can_plunge());
    t.centre_cutting = Some(false);
    assert!(!t.can_plunge());
    let mut c = tool(custom(vec![line(3.0, 0.0), line(3.0, 6.0)]));
    assert!(
        !c.can_plunge(),
        "a custom profile states nothing about its centre"
    );
    c.centre_cutting = Some(true);
    assert!(c.can_plunge());
}

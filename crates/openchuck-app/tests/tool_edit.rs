// SPDX-License-Identifier: AGPL-3.0-or-later
//! Editing a project's tools: the tool form and the custom-profile editor
//! (UI-07, TL-03; work breakdown slice 8 item 5; decision record 0029
//! item 12).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_app::form::FieldKind;
use openchuck_app::project::id::ToolId;
use openchuck_app::project::{Item, OrderKey, ParameterValue, Placement, ToolSnapshot};
use openchuck_app::tool_edit::{
    check_profile, fields, profile_outline, starting_profile, tool_form,
};
use openchuck_app::tools::{
    ControllerMapping, ProfilePoint, ProfileSegment, Tool, ToolProfile, ToolShape,
};
use openchuck_app::{Application, Command, Threads};
use openchuck_types::units::{Degrees, Millimetres};

fn tool(shape: ToolShape) -> Tool {
    Tool {
        shape,
        diameter: Millimetres(6.0),
        flute_length: Millimetres(12.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(6.0),
        flute_count: 2,
        material: None,
    }
}

fn project(name: &str, shape: ToolShape) -> (PathBuf, Application, ToolId) {
    let dir =
        std::env::temp_dir().join(format!("openchuck-tooledit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(1)).unwrap();
    let id = ToolId::new();
    app.execute(Command::Add {
        item: Item::Tool(ToolSnapshot {
            id,
            order: OrderKey::default(),
            name: "test tool".into(),
            tool: tool(shape),
            controller: ControllerMapping {
                number: 1,
                length_offset: None,
                diameter_offset: None,
            },
            cutting_data: BTreeMap::new(),
        }),
        placement: Placement::End,
    })
    .unwrap();
    (dir, app, id)
}

fn the_tool(app: &Application, id: ToolId) -> Tool {
    app.project()
        .tools()
        .iter()
        .find(|t| t.id == id)
        .unwrap()
        .tool
        .clone()
}

#[test]
fn req_ui_07_tool_form_lists_the_fields_each_shape_needs() {
    let names = |t: &Tool| tool_form(t).into_iter().map(|f| f.name).collect::<Vec<_>>();
    let common = [
        fields::DIAMETER,
        fields::FLUTE_LENGTH,
        fields::OVERALL_LENGTH,
        fields::SHANK_DIAMETER,
        fields::FLUTE_COUNT,
    ];
    assert_eq!(names(&tool(ToolShape::BallEnd)), common);
    let bull = names(&tool(ToolShape::BullNose {
        corner_radius: Millimetres(1.0),
    }));
    assert_eq!(bull.last().unwrap(), fields::CORNER_RADIUS);
    let v = names(&tool(ToolShape::VBit {
        included_angle: Degrees(60.0),
        tip_diameter: Millimetres(0.0),
    }));
    assert!(v.ends_with(&[fields::INCLUDED_ANGLE.into(), fields::TIP_DIAMETER.into()]));
    // A custom profile sets the diameter, so the form does not.
    let custom = tool(ToolShape::Custom {
        profile: starting_profile(&tool(ToolShape::BallEnd)).unwrap(),
    });
    assert!(!names(&custom).contains(&fields::DIAMETER.to_owned()));
    // Units and ranges are stated.
    let form = tool_form(&tool(ToolShape::Drill {
        point_angle: Degrees(118.0),
    }));
    let point = form.iter().find(|f| f.name == fields::POINT_ANGLE).unwrap();
    assert!(
        matches!(point.kind, FieldKind::Number { unit: "deg", max: Some(m), .. } if m == 180.0)
    );
}

#[test]
fn req_ui_07_invalid_tool_values_are_refused_and_leave_the_project_unchanged() {
    let (dir, mut app, id) = project(
        "values",
        ToolShape::BullNose {
            corner_radius: Millimetres(1.0),
        },
    );
    let before = app.project().clone();
    for (name, value) in [
        (fields::DIAMETER, ParameterValue::Number(-6.0)),
        (fields::FLUTE_LENGTH, ParameterValue::Number(60.0)), // longer than the tool
        (fields::CORNER_RADIUS, ParameterValue::Number(4.0)), // above the radius
        (fields::FLUTE_COUNT, ParameterValue::Integer(0)),
        (fields::FLUTE_COUNT, ParameterValue::Number(2.5)),
        (fields::POINT_ANGLE, ParameterValue::Number(118.0)), // not a drill
    ] {
        let e = app
            .set_tool_field(id, name, &value)
            .unwrap_err()
            .to_string();
        assert!(e.contains(name), "{name}: {e}");
        assert_eq!(app.project(), &before, "{name}");
    }
    app.set_tool_field(id, fields::CORNER_RADIUS, &ParameterValue::Number(0.5))
        .unwrap();
    assert!(matches!(
        the_tool(&app, id).shape,
        ToolShape::BullNose { corner_radius } if corner_radius.0 == 0.5
    ));
    app.undo().unwrap();
    assert_eq!(app.project(), &before);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_tl_03_profile_editor_checks_like_the_file_import() {
    let (dir, mut app, id) = project("profile", ToolShape::BallEnd);
    let ball = the_tool(&app, id);
    // The ball's own outline: the tip arc, then the flute line.
    let mut profile = starting_profile(&ball).unwrap();
    assert!(matches!(
        profile.segments[0],
        ProfileSegment::Arc { to, centre }
            if to == ProfilePoint::new(3.0, 3.0) && centre == ProfilePoint::new(0.0, 3.0)
    ));
    assert_eq!(check_profile(&ball, &profile), Ok(()));
    // The preview draws the arc on its circle.
    for p in profile_outline(&profile).iter().take(10) {
        assert!((p[0].hypot(p[1] - 3.0) - 3.0).abs() < 1e-9, "{p:?}");
    }

    // An undercut (the radius going back in) is refused by segment, and the
    // file import gives the same verdict.
    profile.segments.push(ProfileSegment::Line {
        to: ProfilePoint::new(2.0, 14.0),
    });
    let reason = check_profile(&ball, &profile).unwrap_err();
    assert!(reason.contains("segment 3"), "{reason}");
    let text = toml_profile(&profile);
    let imported = openchuck_app::tools::parse_profile(Path::new("p.toml"), &text);
    let refused = match imported {
        Err(e) => e.to_string(),
        Ok(p) => p.validate().unwrap_err().to_string(),
    };
    assert!(refused.contains("segment 3"), "{refused}");
    let before = app.project().clone();
    assert!(app.set_tool_profile(id, profile.clone()).is_err());
    assert_eq!(app.project(), &before);

    // A valid custom profile, a 90 degree V to 4 mm radius, then straight up:
    // applied as one undoable step, and the diameter follows the profile.
    let v = ToolProfile {
        segments: vec![
            ProfileSegment::Line {
                to: ProfilePoint::new(4.0, 4.0),
            },
            ProfileSegment::Line {
                to: ProfilePoint::new(4.0, 12.0),
            },
        ],
    };
    // The text form reads back through the real file import, so the refusal
    // above is the import's verdict, not a parse failure of the test text.
    let parsed =
        openchuck_app::tools::parse_profile(Path::new("v.toml"), &toml_profile(&v)).unwrap();
    assert_eq!(parsed, v);
    app.set_tool_profile(id, v.clone()).unwrap();
    let custom = the_tool(&app, id);
    assert_eq!(custom.diameter, Millimetres(8.0));
    assert!(matches!(&custom.shape, ToolShape::Custom { profile } if *profile == v));
    assert_eq!(starting_profile(&custom).unwrap(), v);
    app.undo().unwrap();
    assert_eq!(app.project(), &before);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The profile as the TOML text the profile file import reads.
fn toml_profile(profile: &ToolProfile) -> String {
    let mut text = String::from("schema = 1\n");
    for s in &profile.segments {
        text.push_str("\n[[segments]]\n");
        match s {
            ProfileSegment::Line { to } => {
                text.push_str(&format!(
                    "type = \"line\"\nto = {{ radius = {:?}, height = {:?} }}\n",
                    to.radius.0, to.height.0
                ));
            }
            ProfileSegment::Arc { to, centre } => {
                text.push_str(&format!(
                    "type = \"arc\"\nto = {{ radius = {:?}, height = {:?} }}\ncentre = {{ radius = {:?}, height = {:?} }}\n",
                    to.radius.0, to.height.0, centre.radius.0, centre.height.0
                ));
            }
        }
    }
    text
}

#[test]
fn req_tl_03_profile_editor_adds_moves_and_deletes_segments() {
    use openchuck_app::tool_edit::{
        insert_segment, remove_segment, segment_start, set_segment_arc,
    };
    let flat = tool(ToolShape::FlatEnd);
    let mut p = ToolProfile::default();
    // Add: a line from the tip, then move its end out to the radius.
    insert_segment(&mut p, None);
    assert_eq!(p.segments[0].end(), ProfilePoint::new(0.0, 1.0));
    p.segments[0] = ProfileSegment::Line {
        to: ProfilePoint::new(3.0, 0.0),
    };
    insert_segment(&mut p, Some(0));
    assert_eq!(segment_start(&p, 1), ProfilePoint::new(3.0, 0.0));
    p.segments[1] = ProfileSegment::Line {
        to: ProfilePoint::new(3.0, 12.0),
    };
    assert_eq!(check_profile(&flat, &p), Ok(()));
    // The corner as an arc: a quarter turn, still valid.
    insert_segment(&mut p, Some(0));
    p.segments[0] = ProfileSegment::Line {
        to: ProfilePoint::new(2.0, 0.0),
    };
    p.segments[1] = ProfileSegment::Line {
        to: ProfilePoint::new(3.0, 1.0),
    };
    set_segment_arc(&mut p, 1, true);
    let ProfileSegment::Arc { centre, .. } = p.segments[1] else {
        panic!("{:?}", p.segments[1]);
    };
    assert!(
        centre.distance(ProfilePoint::new(2.0, 1.0)) < 1e-12,
        "{centre:?}"
    );
    assert_eq!(check_profile(&flat, &p), Ok(()));
    set_segment_arc(&mut p, 1, false);
    assert!(matches!(p.segments[1], ProfileSegment::Line { .. }));
    // Delete: the profile no longer reaches its height, and stays valid.
    remove_segment(&mut p, 2);
    remove_segment(&mut p, 9);
    assert_eq!(p.segments.len(), 2);
    assert_eq!(check_profile(&flat, &p), Ok(()));
}

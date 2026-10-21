// SPDX-License-Identifier: AGPL-3.0-or-later
//! The workholding panel's view model and the viewport display of held
//! regions and tabs (UI-25, UI-03; interface specification sections 7.5 and
//! 11.7; decision record 0104; work breakdown slice 21d item 3).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_app::project::id::SetupId;
use openchuck_app::project::{Item, Placement, Setup};
use openchuck_app::scene::{SceneRole, SceneView};
use openchuck_app::setup::{
    Aabb, HeldRegion, Millimetres, Point3, Stock, Tab, TabLayout, TabShape, Workholding,
};
use openchuck_app::workholding_view::{
    DefaultTabsState, Draft, End, Field, PartFacing, check_draft, default_chips,
    default_tabs_state, held_span, holding_hint, new_tab, part_facing, set_layout, set_shape,
};
use openchuck_app::{Application, Command, Threads};
use openchuck_types::units::Degrees;

fn cyl() -> Stock {
    Stock::Cylinder {
        diameter: Millimetres(26.0),
        length: Millimetres(32.0),
        x_start: Millimetres(-16.0),
    }
}

fn cylinder_tab(x: f64, length: f64, d: f64) -> Tab {
    Tab {
        x_start: Millimetres(x),
        length: Millimetres(length),
        shape: TabShape::Cylinder {
            diameter: Millimetres(d),
        },
        layout: TabLayout::OnAxis,
    }
}

fn held(length: f64) -> HeldRegion {
    HeldRegion {
        length: Millimetres(length),
        diameter: None,
    }
}

fn problem(w: &Workholding, stock: Option<&Stock>) -> (Field, String) {
    let p = check_draft(w, stock).expect("a problem");
    (p.field, p.message)
}

#[test]
fn req_ui_25_a_valid_draft_has_no_problem() {
    let w = Workholding {
        start: Some(held(6.0)),
        end: Some(held(6.0)),
        tabs: vec![cylinder_tab(-8.0, 2.0, 5.0)],
    };
    assert!(check_draft(&w, Some(&cyl())).is_none());
    assert!(check_draft(&Workholding::default(), None).is_none());
}

#[test]
fn req_ui_25_validation_marks_the_field_with_a_plain_message() {
    let stock = cyl();
    let s = Some(&stock);
    let with_tab = |t: Tab| Workholding {
        tabs: vec![t],
        ..Workholding::default()
    };
    let with_start = |r: HeldRegion| Workholding {
        start: Some(r),
        ..Workholding::default()
    };
    // No field paths, no zero-based indices.
    assert_eq!(
        problem(&with_start(held(0.0)), s),
        (
            Field::Grip(End::Start),
            "Must be more than 0 mm.".to_owned()
        )
    );
    assert_eq!(
        problem(&with_start(held(f64::NAN)), s),
        (Field::Grip(End::Start), "Not a number.".to_owned())
    );
    assert_eq!(
        problem(&with_start(held(40.0)), s),
        (
            Field::Grip(End::Start),
            "Longer than the stock (32.000 mm).".to_owned()
        )
    );
    assert_eq!(
        problem(
            &with_start(HeldRegion {
                length: Millimetres(5.0),
                diameter: Some(Millimetres(30.0))
            }),
            s
        ),
        (
            Field::HoldingDiameter(End::Start),
            "Wider than the stock (26.000 mm).".to_owned()
        )
    );
    assert_eq!(
        problem(
            &Workholding {
                start: Some(held(20.0)),
                end: Some(held(20.0)),
                tabs: Vec::new()
            },
            s
        ),
        (
            Field::Grip(End::End),
            "Overlaps the held region at the start of the stock.".to_owned()
        )
    );
    assert_eq!(
        problem(&with_tab(cylinder_tab(10.0, 10.0, 5.0)), s),
        (
            Field::TabStartX(0),
            "Not inside the stock (X \u{2212}16.000 to 16.000 mm).".to_owned()
        )
    );
    assert_eq!(
        problem(
            &Workholding {
                start: Some(held(6.0)),
                tabs: vec![cylinder_tab(-14.0, 2.0, 5.0)],
                ..Workholding::default()
            },
            s
        ),
        (
            Field::TabStartX(0),
            "Overlaps the held region at the start of the stock.".to_owned()
        )
    );
    assert_eq!(
        problem(&with_tab(cylinder_tab(0.0, 2.0, 30.0)), s),
        (
            Field::TabDiameter(0),
            "Reaches 15.000 mm from the axis, outside the stock (radius 13.000 mm).".to_owned()
        )
    );
    // Tabs are numbered from 1 in the messages.
    assert_eq!(
        problem(
            &Workholding {
                tabs: vec![cylinder_tab(0.0, 4.0, 5.0), cylinder_tab(2.0, 4.0, 5.0)],
                ..Workholding::default()
            },
            s
        ),
        (Field::TabStartX(1), "Overlaps tab 1.".to_owned())
    );
    assert_eq!(
        problem(
            &with_tab(Tab {
                shape: TabShape::Frustum {
                    start_diameter: Millimetres(5.0),
                    end_diameter: Millimetres(5.0)
                },
                ..cylinder_tab(0.0, 2.0, 5.0)
            }),
            s
        ),
        (
            Field::TabEndDiameter(0),
            "The same as the diameter at start X; use a cylinder.".to_owned()
        )
    );
    let around = |count, offset| Tab {
        layout: TabLayout::AroundAxis {
            count,
            start_angle: Degrees(0.0),
            radial_offset: Millimetres(offset),
        },
        ..cylinder_tab(0.0, 2.0, 2.0)
    };
    assert_eq!(
        problem(&with_tab(around(65, 5.0)), s),
        (Field::TabCount(0), "From 1 to 64.".to_owned())
    );
    assert_eq!(
        problem(&with_tab(around(2, -1.0)), s),
        (Field::TabOffset(0), "Must be 0 mm or more.".to_owned())
    );
    assert_eq!(
        problem(&with_tab(around(2, 13.0)), s),
        (
            Field::TabOffset(0),
            "Reaches 14.000 mm from the axis, outside the stock (radius 13.000 mm).".to_owned()
        )
    );
}

#[test]
fn req_ui_25_req_set_09_a_held_region_on_box_stock_needs_no_holding_diameter() {
    // Without a holding diameter box stock is held at its own outline over
    // the gripped length (decision 0160; issue #188), so none is asked for;
    // one that is given makes a cylinder and must fit the smaller side.
    let boxed = Stock::Box {
        bounds: Aabb {
            min: Point3::new(-16.0, -10.0, -12.0),
            max: Point3::new(16.0, 10.0, 12.0),
        },
    };
    let w = Workholding {
        start: Some(held(6.0)),
        ..Workholding::default()
    };
    assert_eq!(check_draft(&w, Some(&boxed)), None);
    let fits = Workholding {
        start: Some(HeldRegion {
            length: Millimetres(6.0),
            diameter: Some(Millimetres(20.0)),
        }),
        ..Workholding::default()
    };
    assert_eq!(check_draft(&fits, Some(&boxed)), None);
    let too_wide = Workholding {
        start: Some(HeldRegion {
            length: Millimetres(6.0),
            diameter: Some(Millimetres(24.0)),
        }),
        ..Workholding::default()
    };
    assert_eq!(
        problem(&too_wide, Some(&boxed)),
        (
            Field::HoldingDiameter(End::Start),
            "Wider than the stock's smaller side (20.000 mm).".to_owned()
        )
    );
    // The field's hint says what the held region is without a diameter.
    assert_eq!(holding_hint(Some(&boxed)), "Stock outline");
    let round = Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(32.0),
        x_start: Millimetres(-16.0),
    };
    assert_eq!(holding_hint(Some(&round)), "Stock diameter (20.000 mm)");
    assert_eq!(holding_hint(None), "Stock diameter");
}

#[test]
fn req_ui_25_the_held_span_follows_the_stock_and_the_grip() {
    let w = Workholding {
        start: Some(held(6.0)),
        end: Some(held(4.0)),
        ..Workholding::default()
    };
    assert_eq!(
        held_span(&w, End::Start, &cyl()),
        Some("X \u{2212}16.000 to \u{2212}10.000".to_owned())
    );
    assert_eq!(
        held_span(&w, End::End, &cyl()),
        Some("X 12.000 to 16.000".to_owned())
    );
    assert_eq!(held_span(&Workholding::default(), End::Start, &cyl()), None);
}

#[test]
fn req_ui_25_add_tab_starts_at_the_inner_edge_of_the_held_region() {
    let none = new_tab(&Workholding::default(), Some(&cyl()));
    assert_eq!(none.x_start.0, -16.0);
    assert_eq!(none.length.0, 5.0);
    assert_eq!(
        none.shape,
        TabShape::Cylinder {
            diameter: Millimetres(6.5)
        }
    );
    assert_eq!(none.layout, TabLayout::OnAxis);
    let w = Workholding {
        start: Some(held(6.0)),
        ..Workholding::default()
    };
    assert_eq!(new_tab(&w, Some(&cyl())).x_start.0, -10.0);
    // A thin stock keeps the diameter at least 1 mm.
    let thin = Stock::Cylinder {
        diameter: Millimetres(2.0),
        length: Millimetres(10.0),
        x_start: Millimetres(0.0),
    };
    assert_eq!(
        new_tab(&Workholding::default(), Some(&thin))
            .shape
            .max_diameter()
            .0,
        1.0
    );
}

#[test]
fn req_ui_25_shape_and_placement_changes_keep_the_values_the_specification_names() {
    let mut tab = cylinder_tab(0.0, 2.0, 4.0);
    set_shape(&mut tab, false);
    assert_eq!(
        tab.shape,
        TabShape::Frustum {
            start_diameter: Millimetres(4.0),
            end_diameter: Millimetres(6.0)
        }
    );
    set_shape(&mut tab, true);
    assert_eq!(
        tab.shape,
        TabShape::Cylinder {
            diameter: Millimetres(4.0)
        }
    );
    set_layout(&mut tab, true);
    assert_eq!(
        tab.layout,
        TabLayout::AroundAxis {
            count: 2,
            start_angle: Degrees(0.0),
            radial_offset: Millimetres(5.0)
        }
    );
    set_layout(&mut tab, false);
    assert_eq!(tab.layout, TabLayout::OnAxis);
}

#[test]
fn req_ui_25_a_frustum_says_which_end_faces_the_part() {
    let frustum = |a, b| Tab {
        shape: TabShape::Frustum {
            start_diameter: Millimetres(a),
            end_diameter: Millimetres(b),
        },
        ..cylinder_tab(-10.0, 2.0, 4.0)
    };
    // The part starts at X -8: the tab's high-X end faces it.
    assert_eq!(
        part_facing(&frustum(8.0, 4.0), (-8.0, 8.0)),
        Some(PartFacing::Small)
    );
    assert_eq!(
        part_facing(&frustum(4.0, 8.0), (-8.0, 8.0)),
        Some(PartFacing::Large)
    );
    assert_eq!(
        part_facing(&cylinder_tab(-10.0, 2.0, 4.0), (-8.0, 8.0)),
        None
    );
    let tail = Tab {
        x_start: Millimetres(9.0),
        ..frustum(4.0, 8.0)
    };
    assert_eq!(part_facing(&tail, (-8.0, 8.0)), Some(PartFacing::Small));
}

#[test]
fn req_ui_25_a_tab_equal_to_a_default_carries_the_default_chip_until_edited() {
    let defaults = vec![cylinder_tab(-10.0, 2.0, 4.0), cylinder_tab(8.0, 8.0, 4.0)];
    let mut tabs = defaults.clone();
    assert_eq!(default_chips(&tabs, &defaults), vec![true, true]);
    tabs[0].length = Millimetres(2.0005);
    assert_eq!(default_chips(&tabs, &defaults), vec![true, true]);
    tabs[0].length = Millimetres(2.01);
    assert_eq!(default_chips(&tabs, &defaults), vec![false, true]);
    assert_eq!(default_chips(&[], &defaults), Vec::<bool>::new());
}

#[test]
fn req_ui_25_add_default_tabs_says_why_it_is_disabled() {
    let stored = Workholding::default();
    assert_eq!(
        default_tabs_state(&stored, &stored),
        DefaultTabsState::Available
    );
    let with_tab = Workholding {
        tabs: vec![cylinder_tab(0.0, 1.0, 1.0)],
        ..Workholding::default()
    };
    assert_eq!(
        default_tabs_state(&stored, &with_tab),
        DefaultTabsState::Refused("Remove the tabs first")
    );
    let held_changed = Workholding {
        start: Some(held(5.0)),
        ..Workholding::default()
    };
    assert_eq!(
        default_tabs_state(&stored, &held_changed),
        DefaultTabsState::Refused(
            "Apply the held regions first: the default tabs fit the stored ones."
        )
    );
}

fn project(name: &str) -> (std::path::PathBuf, Application, SetupId) {
    let dir = std::env::temp_dir().join(format!("openchuck-wv-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    let id = SetupId::new();
    let mut setup = Setup::new(id, "Setup 1");
    setup.stock = Some(cyl());
    app.execute(Command::Add {
        item: Item::Setup(setup),
        placement: Placement::End,
    })
    .unwrap();
    (dir, app, id)
}

fn store(app: &mut Application, id: SetupId, w: Workholding) {
    let mut setup = app.project().setup(id).unwrap().clone();
    setup.workholding = w;
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
}

#[test]
fn req_ui_25_a_draft_is_stored_as_one_undoable_edit_and_an_undo_discards_unapplied_changes() {
    let (dir, mut app, id) = project("draft");
    let mut draft = Draft::new(&app.project().setup(id).unwrap().workholding);
    assert!(!draft.is_changed());
    draft.edit().start = Some(held(6.0));
    draft.edit().tabs.push(cylinder_tab(-10.0, 2.0, 4.0));
    assert!(draft.is_changed());
    // Apply: one command, and the draft is the stored value.
    store(&mut app, id, draft.workholding().clone());
    assert_eq!(
        draft.sync(&app.project().setup(id).unwrap().workholding),
        None
    );
    assert!(!draft.is_changed());
    assert_eq!(app.project().setup(id).unwrap().workholding.tabs.len(), 1);

    // An unapplied edit survives while the stored value is the same...
    draft.edit().tabs.clear();
    assert!(
        draft
            .sync(&app.project().setup(id).unwrap().workholding)
            .is_none()
    );
    assert!(draft.is_changed());
    // ...and one undo takes the stored value back and discards it, saying so.
    assert!(app.undo().unwrap());
    let note = draft
        .sync(&app.project().setup(id).unwrap().workholding)
        .expect("the discard is noted");
    assert_eq!(
        note,
        "The stored workholding changed (an undo, a redo or another edit), so your unapplied \
         changes were discarded."
    );
    assert!(!draft.is_changed());
    assert!(draft.workholding().is_empty());
    assert_eq!(draft.note(), Some(note.as_str()));
    // The note goes when the draft is next edited.
    draft.edit().end = Some(held(3.0));
    assert_eq!(draft.note(), None);
    // Revert shows the stored values again.
    draft.revert();
    assert!(!draft.is_changed());
    // A change with nothing unapplied replaces the draft without a note.
    store(
        &mut app,
        id,
        Workholding {
            end: Some(held(2.0)),
            ..Workholding::default()
        },
    );
    assert_eq!(
        draft.sync(&app.project().setup(id).unwrap().workholding),
        None
    );
    assert_eq!(draft.note(), None);
    assert!(draft.workholding().end.is_some());
    std::fs::remove_dir_all(dir).unwrap();
}

fn count(view: &SceneView, role: SceneRole) -> usize {
    view.meshes.iter().filter(|m| m.role == role).count()
}

#[test]
fn req_ui_25_req_ui_03_the_view_draws_held_regions_and_tabs_with_outlines() {
    let (dir, mut app, id) = project("display");
    store(
        &mut app,
        id,
        Workholding {
            start: Some(held(6.0)),
            end: None,
            tabs: vec![
                cylinder_tab(-10.0, 2.0, 4.0),
                Tab {
                    layout: TabLayout::AroundAxis {
                        count: 3,
                        start_angle: Degrees(90.0),
                        radial_offset: Millimetres(5.0),
                    },
                    ..cylinder_tab(10.0, 4.0, 2.0)
                },
            ],
        },
    );
    let _ = (&app, id);
    let view = openchuck_app::workholding_view::display_scene(
        &app.project().setup(id).unwrap().workholding,
        Some(&cyl()),
        &openchuck_app::setup::PartExtent {
            bounds: Aabb {
                min: Point3::new(-8.0, -5.0, -5.0),
                max: Point3::new(8.0, 5.0, 5.0),
            },
            radius: Millimetres(5.0),
        },
    )
    .unwrap();
    // One held region, then a tab on the axis and three around it.
    assert_eq!(count(&view, SceneRole::HeldRegion), 1);
    assert_eq!(count(&view, SceneRole::Tab), 4);
    // The roles differ by more than colour: each solid has an outline of its
    // own role, and a held region also a hatch.
    let tab_lines = view
        .lines
        .iter()
        .filter(|l| l.role == SceneRole::Tab)
        .count();
    let held_lines = view
        .lines
        .iter()
        .filter(|l| l.role == SceneRole::HeldRegion)
        .count();
    assert_eq!(tab_lines, 4 * 2, "two end circles each");
    // The cross-hatch is drawn on the screen by the view, not as lines here.
    assert_eq!(held_lines, 2, "two end circles");
    // Each solid is named, for hover and selection: a tab around the axis is
    // three solids of one item.
    use openchuck_app::scene::WorkholdingItem;
    let items: Vec<WorkholdingItem> = view.solids.iter().map(|s| s.item).collect();
    assert_eq!(
        items,
        [
            WorkholdingItem::Start,
            WorkholdingItem::Tab(0),
            WorkholdingItem::Tab(1),
            WorkholdingItem::Tab(1),
            WorkholdingItem::Tab(1),
        ]
    );
    assert!(view.solids.iter().all(|s| matches!(
        view.meshes[s.mesh].role,
        SceneRole::HeldRegion | SceneRole::Tab
    )));
    let text: Vec<&str> = view.solids.iter().map(|s| s.description.as_str()).collect();
    assert_eq!(
        text[0],
        "Held region at the start · 6.000 mm · Ø 26.000 mm (stock)"
    );
    assert_eq!(
        text[1],
        "Tab 1 · cylinder · X -10.000 to -8.000 mm · Ø 4.000 mm"
    );
    assert_eq!(
        text[3],
        "Tab 2 · cylinder · X 10.000 to 14.000 mm · Ø 2.000 mm · 2 of 3 at 210.00°"
    );
    // The legend lists the roles that are present.
    assert_eq!(
        openchuck_app::workholding_view::legend(&view),
        vec!["Held region", "Tab"]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// A project with the corpus cube (10 mm, X 0 to 10) on the rotary axis in
/// stock of X -2 to 12, and one operation that cuts in the cube's top face.
fn selection_project(
    name: &str,
    workholding: Workholding,
) -> (
    std::path::PathBuf,
    Application,
    openchuck_app::project::id::OperationId,
) {
    use openchuck_app::RegionRequest;
    use openchuck_app::model::TransformStep;
    use openchuck_app::project::id::OperationId;
    use openchuck_app::project::{Operation, OrderKey};
    use openchuck_types::geometry::Vec3;
    let dir = std::env::temp_dir().join(format!("openchuck-wv-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let cube =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/cube_ascii.stl");
    std::fs::copy(cube, dir.join("models/cube.stl")).unwrap();
    let (model, _) = app.add_model(&dir.join("models/cube.stl"), None).unwrap();
    app.execute(Command::SetModelTransform {
        model,
        transform: vec![TransformStep::CentreOnRotaryAxis],
    })
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(14.0),
        x_start: Millimetres(-2.0),
    });
    setup.workholding = workholding;
    app.execute(Command::Add {
        item: Item::Setup(setup),
        placement: Placement::End,
    })
    .unwrap();
    let top = app
        .select_region(
            model,
            &RegionRequest::ByDirection {
                direction: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                max_angle: 10.0,
            },
        )
        .unwrap();
    let roof = app.save_selection("Roof", model, top).unwrap();
    let op = OperationId::new();
    app.execute(Command::Add {
        item: Item::Operation(Operation {
            id: op,
            order: OrderKey::default(),
            name: "Ring".into(),
            setup: setup_id,
            strategy: "helical_finishing".into(),
            tool: None,
            enabled: true,
            selections: vec![roof],
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            parameters: std::collections::BTreeMap::new(),
        }),
        placement: Placement::End,
    })
    .unwrap();
    (dir, app, op)
}

#[test]
fn req_ui_25_the_selection_warning_names_what_is_reached_in_plain_words() {
    // A held region of 4 mm reaches into the cube's top face (X 0 to 10).
    let (dir, app, op) = selection_project(
        "reach-held",
        Workholding {
            start: Some(held(4.0)),
            ..Workholding::default()
        },
    );
    let notes = app.start_workholding_warnings(op).wait().unwrap();
    assert_eq!(
        notes,
        vec![
            "The selection 'Roof' reaches the held region at the start of the stock, which is \
             never machined. This operation leaves it uncut there."
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();

    // A tab numbered from 1; three around the axis are one tab, one note.
    let (dir, app, op) = selection_project(
        "reach-tab",
        Workholding {
            tabs: vec![Tab {
                layout: TabLayout::AroundAxis {
                    count: 3,
                    start_angle: Degrees(90.0),
                    radial_offset: Millimetres(4.0),
                },
                ..cylinder_tab(-1.0, 2.0, 12.0)
            }],
            ..Workholding::default()
        },
    );
    let notes = app.start_workholding_warnings(op).wait().unwrap();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].contains("reaches tab 1, which is never machined"),
        "{notes:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();

    // A tab clear of the selection gives no warning.
    let (dir, app, op) = selection_project(
        "reach-none",
        Workholding {
            tabs: vec![cylinder_tab(10.5, 1.5, 4.0)],
            ..Workholding::default()
        },
    );
    assert!(
        app.start_workholding_warnings(op)
            .wait()
            .unwrap()
            .is_empty()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_ui_25_check_findings_are_sentences_naming_the_tab_by_its_number() {
    let (dir, app, _) = selection_project("findings", Workholding::default());
    let setup = app.project().setups()[0].id;
    let draft = Workholding {
        tabs: vec![cylinder_tab(-2.0, 1.0, 4.0), cylinder_tab(3.0, 2.0, 4.0)],
        ..Workholding::default()
    };
    let found = app.start_check_workholding(setup, &draft).wait().unwrap();
    // The finding is on the second tab's row (OI-334 i).
    assert_eq!(found.iter().map(|f| f.tab).collect::<Vec<_>>(), [Some(1)]);
    let found: Vec<String> = found.into_iter().map(|f| f.text).collect();
    assert_eq!(
        found,
        vec![
            "Tab 2 (X 3.000 to 5.000 mm) is not wholly in the head or the tail zone. Tabs lie \
             between the held region (or the end of the stock) and the part, or between the \
             part and the other end."
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_ui_25_a_frustum_tab_and_a_held_region_with_its_own_diameter_are_described() {
    use openchuck_app::scene::WorkholdingItem;
    // The tooltip of 7.5: "Tab 2 · frustum · X 14.000 to 16.000 mm · Ø 4.000
    // to Ø 8.000 mm"; a held region whose diameter is given does not say
    // "(stock)".
    let w = Workholding {
        start: None,
        end: Some(HeldRegion {
            length: Millimetres(4.0),
            diameter: Some(Millimetres(20.0)),
        }),
        tabs: vec![
            cylinder_tab(-10.0, 2.0, 4.0),
            Tab {
                shape: TabShape::Frustum {
                    start_diameter: Millimetres(4.0),
                    end_diameter: Millimetres(8.0),
                },
                ..cylinder_tab(10.0, 2.0, 4.0)
            },
        ],
    };
    let part = openchuck_app::setup::PartExtent {
        bounds: Aabb {
            min: Point3::new(-8.0, -5.0, -5.0),
            max: Point3::new(8.0, 5.0, 5.0),
        },
        radius: Millimetres(5.0),
    };
    let view = openchuck_app::workholding_view::display_scene(&w, Some(&cyl()), &part).unwrap();
    let described: Vec<(WorkholdingItem, &str)> = view
        .solids
        .iter()
        .map(|s| (s.item, s.description.as_str()))
        .collect();
    assert_eq!(
        described,
        [
            (
                WorkholdingItem::End,
                "Held region at the end · 4.000 mm · Ø 20.000 mm"
            ),
            (
                WorkholdingItem::Tab(0),
                "Tab 1 · cylinder · X -10.000 to -8.000 mm · Ø 4.000 mm"
            ),
            (
                WorkholdingItem::Tab(1),
                "Tab 2 · frustum · X 10.000 to 12.000 mm · Ø 4.000 to Ø 8.000 mm"
            ),
        ]
    );
}

/// A collapsed tab row names its zone, and the Narrowest line compares the
/// tab with the tools of the setup's operations, naming the operation by its
/// number (interface specification 11.7; OI-334 c and i).
#[test]
fn req_ui_25_a_tab_names_its_zone_and_its_narrowest_against_the_tools() {
    use openchuck_app::workholding::SetupTool;
    use openchuck_app::workholding_view::{Zone, narrowest_line, tab_zone};
    let part = openchuck_app::setup::PartExtent {
        bounds: Aabb {
            min: Point3::new(-8.0, -5.0, -5.0),
            max: Point3::new(8.0, 5.0, 5.0),
        },
        radius: Millimetres(7.0),
    };
    let stored = Workholding {
        start: Some(held(4.0)),
        ..Workholding::default()
    };
    let stock = cyl();
    let head = cylinder_tab(-12.0, 3.0, 4.0);
    assert_eq!(tab_zone(&head, &stored, &stock, &part), Some(Zone::Head));
    let tail = cylinder_tab(9.0, 2.0, 4.0);
    assert_eq!(tab_zone(&tail, &stored, &stock, &part), Some(Zone::Tail));
    let across = cylinder_tab(-1.0, 2.0, 4.0);
    assert_eq!(tab_zone(&across, &stored, &stock, &part), None);

    let tools = vec![
        SetupTool {
            tool: "T1 Ball \u{d8}3".into(),
            diameter: 3.0,
            operation: "1 Rough".into(),
        },
        SetupTool {
            tool: "T2 Flat \u{d8}6".into(),
            diameter: 6.0,
            operation: "2 Finish".into(),
        },
    ];
    let (line, warning) = narrowest_line(&head, &tools);
    assert!(warning);
    assert_eq!(
        line,
        "4.000 mm, narrower than the tool T2 Flat \u{d8}6 (6.000 mm) of 2 Finish"
    );
    let (line, warning) = narrowest_line(&cylinder_tab(-12.0, 3.0, 8.0), &tools);
    assert!(!warning, "{line}");
    assert_eq!(narrowest_line(&head, &[]).0, "4.000 mm");
}

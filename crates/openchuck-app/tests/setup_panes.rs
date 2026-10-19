// SPDX-License-Identifier: AGPL-3.0-or-later
//! The Setup step's two panes (UI-03, SET-05, SET-06, SET-09; interface
//! specification 11.3 and 7.5; slice 22 lane X5, decision record 0129).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_app::project::id::SetupId;
use openchuck_app::project::machines::{MAKERA_Z1, builtin};
use openchuck_app::project::{Item, Placement, Setup};
use openchuck_app::scene::{SceneView, WorkholdingItem};
use openchuck_app::setup::{
    Aabb, HeldRegion, Millimetres, Point3, Stock, Tab, TabLayout, TabShape, Workholding,
};
use openchuck_app::setup_panes::{EndStock, LegendEntry, setup_panes};
use openchuck_app::workholding_view::display_items;
use openchuck_app::{Application, Command, Threads};
use openchuck_types::setup::PartExtent;
use openchuck_types::units::Degrees;

fn cyl() -> Stock {
    Stock::Cylinder {
        diameter: Millimetres(26.0),
        length: Millimetres(32.0),
        x_start: Millimetres(-16.0),
    }
}

fn extent() -> PartExtent {
    PartExtent {
        bounds: Aabb {
            min: Point3::new(-8.0, -10.0, -10.0),
            max: Point3::new(8.0, 10.0, 10.0),
        },
        radius: Millimetres(10.0),
    }
}

fn workholding() -> Workholding {
    Workholding {
        start: Some(HeldRegion {
            length: Millimetres(6.0),
            diameter: None,
        }),
        end: None,
        tabs: vec![
            Tab {
                x_start: Millimetres(-10.0),
                length: Millimetres(2.0),
                shape: TabShape::Frustum {
                    start_diameter: Millimetres(4.0),
                    end_diameter: Millimetres(6.0),
                },
                layout: TabLayout::OnAxis,
            },
            Tab {
                x_start: Millimetres(10.0),
                length: Millimetres(4.0),
                shape: TabShape::Cylinder {
                    diameter: Millimetres(2.0),
                },
                layout: TabLayout::AroundAxis {
                    count: 2,
                    start_angle: Degrees(90.0),
                    radial_offset: Millimetres(5.0),
                },
            },
        ],
    }
}

fn project(name: &str, machine: bool) -> (std::path::PathBuf, Application, SetupId) {
    let dir = std::env::temp_dir().join(format!("openchuck-sp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    if machine {
        let profile = builtin(MAKERA_Z1).unwrap().unwrap();
        app.execute(Command::SetMachine(Some(Box::new(profile))))
            .unwrap();
    }
    let id = SetupId::new();
    let mut setup = Setup::new(id, "Setup 1");
    setup.stock = Some(cyl());
    setup.workholding = workholding();
    app.execute(Command::Add {
        item: Item::Setup(setup),
        placement: Placement::End,
    })
    .unwrap();
    (dir, app, id)
}

fn view(id: SetupId) -> SceneView {
    SceneView {
        setup: Some(id),
        part_extent: Some(extent()),
        part_silhouette: vec![[-8.0, -10.0, 10.0], [8.0, -10.0, 10.0]],
        ..SceneView::default()
    }
}

#[test]
fn req_ui_03_the_front_view_draws_stock_clearance_safe_z_and_dimensions() {
    let (dir, app, id) = project("front", true);
    let view = view(id);
    let panes = setup_panes(app.project(), &view, &[], &[], &[]).unwrap();
    let f = &panes.front;
    let stock = f.stock.unwrap();
    assert_eq!(
        (stock.h0, stock.h1, stock.v0, stock.v1),
        (-16.0, 16.0, -13.0, 13.0)
    );
    // No chuck is drawn: the project holds no chuck geometry (OI-380); the
    // held region is the only indication.
    // The clearance and safe Z are the machine's defaults (SET-06).
    let machine = app.project().machine().unwrap().clearance;
    assert_eq!(f.safe_z, Some(machine.safe_z.0));
    let (clearance, radius) = f.clearance.unwrap();
    assert_eq!((radius, clearance.v1), (machine.radius.0, machine.radius.0));
    // The travel limit lies above the safe Z cylinder's axis, in the axis frame.
    assert!(f.travel_z.is_some());
    assert_eq!(f.work_zero, 0.0);
    assert!(f.axis.0 < stock.h0 && f.axis.1 > stock.h1);
    let texts: Vec<&str> = f.dimensions.iter().map(|d| d.text.as_str()).collect();
    assert!(texts.contains(&"Stock length 32.000"), "{texts:?}");
    assert!(texts.contains(&"Ø26.000"));
    assert!(texts.iter().any(|t| t.starts_with("Axis to safe Z")));
    assert_eq!(f.part, view.part_silhouette);
    assert_eq!(
        panes.legend,
        [
            LegendEntry::RotaryAxis,
            LegendEntry::Stock,
            LegendEntry::Clearance,
            LegendEntry::TravelLimit,
            LegendEntry::YIntoScreen,
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_ui_25_req_ui_03_both_panes_draw_held_regions_and_tabs_in_section() {
    let (dir, app, id) = project("section", true);
    let view = view(id);
    let wh = workholding();
    let (_, _, solids) = display_items(&wh, Some(&cyl()), &extent()).unwrap();
    let panes = setup_panes(
        app.project(),
        &view,
        &solids,
        &[WorkholdingItem::Tab(1)],
        &[],
    )
    .unwrap();
    let f = &panes.front;
    // One held region, dimensioned "Held 6.000", with no label of its own.
    assert_eq!(f.held.len(), 1);
    assert_eq!(f.held[0].item, WorkholdingItem::Start);
    assert!(f.held[0].label.is_none());
    assert!(f.dimensions.iter().any(|d| d.text == "Held 6.000"));
    // The frustum is a trapezoid; the three-way tab's solids are labelled once.
    let first = &f.tabs[0];
    assert_eq!(first.label.as_deref(), Some("Tab 1"));
    assert_eq!(first.outline[0][1], -2.0);
    assert_eq!(first.outline[1][1], -3.0);
    let tab2: Vec<_> = f
        .tabs
        .iter()
        .filter(|t| t.item == WorkholdingItem::Tab(1))
        .collect();
    assert_eq!(tab2.len(), 2);
    assert_eq!(tab2.iter().filter(|t| t.label.is_some()).count(), 1);
    assert!(tab2.iter().all(|t| t.failing));
    assert!(!first.failing);
    // The end view: the on-axis tab at its largest diameter and the two
    // around the axis at their angles.
    let e = &panes.end;
    assert_eq!(e.tabs.len(), 3);
    assert_eq!(e.tabs[0].centre, [0.0, 0.0]);
    assert_eq!(e.tabs[0].radius, 3.0);
    assert!(
        e.tabs[1..]
            .iter()
            .all(|t| (t.centre[0].hypot(t.centre[1]) - 5.0).abs() < 1e-9)
    );
    // "Start 90.00°": the first tab around the axis, from +Y towards +Z.
    let (angle, text) = e.start_angle.clone().unwrap();
    assert!((angle - 90.0).abs() < 1e-6, "{angle}");
    assert_eq!(text, "Start 90.00°");
    assert_eq!(e.stock, Some(EndStock::Circle(13.0)));
    assert_eq!(e.part_radius, Some(10.0));
    assert!(
        panes
            .legend
            .ends_with(&[LegendEntry::HeldRegion, LegendEntry::Tab])
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// Default tabs offered and not added are drawn in section too, apart from
/// the stored tabs, labelled "Default tab, not added" (7.5; OI-383).
#[test]
fn req_ui_25_the_default_tabs_offered_are_in_both_panes() {
    let (dir, app, id) = project("offered", true);
    let offered = Workholding {
        tabs: vec![Tab {
            x_start: Millimetres(-12.0),
            length: Millimetres(3.0),
            shape: TabShape::Frustum {
                start_diameter: Millimetres(8.0),
                end_diameter: Millimetres(4.0),
            },
            layout: TabLayout::OnAxis,
        }],
        ..Workholding::default()
    };
    let (_, _, solids) = display_items(&offered, Some(&cyl()), &extent()).unwrap();
    let panes = setup_panes(app.project(), &view(id), &[], &[], &solids).unwrap();
    let f = &panes.front;
    assert!(f.tabs.is_empty());
    assert_eq!(f.offered.len(), 1);
    assert_eq!(
        f.offered[0].label.as_deref(),
        Some("Default tab, not added")
    );
    assert_eq!(f.offered[0].outline[0], [-12.0, -4.0]);
    assert_eq!(f.offered[0].outline[1], [-9.0, -2.0]);
    let e = &panes.end;
    assert!(e.tabs.is_empty());
    assert_eq!(e.offered.len(), 1);
    assert_eq!(
        (e.offered[0].centre, e.offered[0].radius),
        ([0.0, 0.0], 4.0)
    );
    // Not a stored tab: no start angle, and the legend has no tab entry.
    assert!(e.start_angle.is_none());
    assert!(!panes.legend.contains(&LegendEntry::Tab));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_ui_03_the_a_arrow_follows_the_machine_and_is_absent_without_one() {
    let (dir, app, id) = project("arrow", true);
    let panes = setup_panes(app.project(), &view(id), &[], &[], &[]).unwrap();
    let arc = &panes.end.a_arc;
    assert_eq!(arc.len(), 55);
    // It starts at the top of the part (Z up) and turns through 270 degrees.
    assert!(arc[0][1] > 0.0 && arc[0][0].abs() < 1e-9);
    assert!(
        arc.iter()
            .all(|p| (p[0].hypot(p[1]) - arc[0][1]).abs() < 1e-9)
    );
    std::fs::remove_dir_all(dir).unwrap();

    let (dir, app, id) = project("noarrow", false);
    let panes = setup_panes(app.project(), &view(id), &[], &[], &[]).unwrap();
    assert!(panes.end.a_arc.is_empty());
    assert!(panes.front.travel_z.is_none());
    // Without a machine, no clearance either: the legend leaves them out.
    assert!(panes.front.clearance.is_none() && panes.front.safe_z.is_none());
    assert_eq!(
        panes.legend,
        [
            LegendEntry::RotaryAxis,
            LegendEntry::Stock,
            LegendEntry::YIntoScreen
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_nfr_07_a_setup_without_stock_or_part_and_an_unknown_setup_draw_what_exists() {
    let (dir, mut app, id) = project("bare", true);
    let mut setup = app.project().setup(id).unwrap().clone();
    setup.stock = None;
    setup.workholding = Workholding::default();
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let bare = SceneView {
        setup: Some(id),
        ..SceneView::default()
    };
    let panes = setup_panes(app.project(), &bare, &[], &[], &[]).unwrap();
    assert!(panes.front.stock.is_none());
    assert!(panes.end.stock.is_none() && panes.end.part_radius.is_none());
    // The axis and the clearance still show, round the work zero.
    assert!(panes.front.axis.0 < 0.0 && panes.front.axis.1 > 0.0);
    assert!(!panes.end.a_arc.is_empty());
    // A scene of no setup, or of one the project does not hold, has no panes.
    assert!(setup_panes(app.project(), &SceneView::default(), &[], &[], &[]).is_none());
    let other = SceneView {
        setup: Some(SetupId::new()),
        ..SceneView::default()
    };
    assert!(setup_panes(app.project(), &other, &[], &[], &[]).is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_set_03_box_stock_is_a_rectangle_in_the_end_view() {
    let (dir, mut app, id) = project("box", true);
    let mut setup = app.project().setup(id).unwrap().clone();
    setup.workholding = Workholding::default();
    setup.stock = Some(Stock::Box {
        bounds: Aabb {
            min: Point3::new(-10.0, -8.0, -6.0),
            max: Point3::new(10.0, 8.0, 6.0),
        },
    });
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let panes = setup_panes(app.project(), &view(id), &[], &[], &[]).unwrap();
    let Some(EndStock::Box(b)) = panes.end.stock else {
        panic!("a box")
    };
    assert_eq!((b.h0, b.h1, b.v0, b.v1), (-8.0, 8.0, -6.0, 6.0));
    assert!(panes.front.dimensions.iter().any(|d| d.text == "12.000"));
    std::fs::remove_dir_all(dir).unwrap();
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Camera tests: standard views, orbit, pan, zoom, fit, perspective and
//! orthographic projection (UI-02), all without a GPU.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_viewport::camera::{Camera, Projection, StandardView};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn camera(view: StandardView, projection: Projection) -> Camera {
    let mut c = Camera {
        projection,
        ..Camera::default()
    };
    c.fit([-10.0; 3], [10.0; 3], 1.0);
    c.set_view(view);
    c
}

#[test]
fn req_ui_02_standard_views_put_model_axes_where_expected() {
    // (view, model direction, expected screen direction (x right, y up))
    let cases = [
        (StandardView::Front, [1.0, 0.0, 0.0], [1.0, 0.0]),
        (StandardView::Front, [0.0, 0.0, 1.0], [0.0, 1.0]),
        (StandardView::Back, [1.0, 0.0, 0.0], [-1.0, 0.0]),
        (StandardView::Right, [0.0, 1.0, 0.0], [1.0, 0.0]),
        (StandardView::Left, [0.0, 1.0, 0.0], [-1.0, 0.0]),
        (StandardView::Top, [1.0, 0.0, 0.0], [1.0, 0.0]),
        (StandardView::Top, [0.0, 1.0, 0.0], [0.0, 1.0]),
        (StandardView::Bottom, [1.0, 0.0, 0.0], [1.0, 0.0]),
        (StandardView::Bottom, [0.0, 1.0, 0.0], [0.0, -1.0]),
    ];
    for projection in [Projection::Perspective, Projection::Orthographic] {
        for (view, direction, expected) in cases {
            let c = camera(view, projection);
            let centre = c.project([0.0; 3], 1.0).unwrap();
            let p = c.project(direction.map(|v| v * 5.0), 1.0).unwrap();
            let (dx, dy) = (p[0] - centre[0], p[1] - centre[1]);
            assert!(
                dx * expected[0] > 0.0 || (expected[0] == 0.0 && dx.abs() < 1e-9),
                "{view:?} {projection:?} {direction:?}: dx {dx}"
            );
            assert!(
                dy * expected[1] > 0.0 || (expected[1] == 0.0 && dy.abs() < 1e-9),
                "{view:?} {projection:?} {direction:?}: dy {dy}"
            );
        }
    }
}

#[test]
fn req_ui_02_isometric_view_sees_x_minus_y_and_z_equally() {
    let c = camera(StandardView::Isometric, Projection::Orthographic);
    let back = c.basis().2;
    let s = 1.0 / 3f64.sqrt();
    assert!(
        close(back[0], s) && close(back[1], -s) && close(back[2], s),
        "{back:?}"
    );
}

#[test]
fn req_ui_02_fit_shows_the_whole_box() {
    for projection in [Projection::Perspective, Projection::Orthographic] {
        for aspect in [0.5, 1.0, 2.0] {
            for view in StandardView::ALL {
                let mut c = Camera {
                    projection,
                    ..Camera::default()
                };
                c.set_view(view);
                c.fit([-40.0, -10.0, -10.0], [60.0, 10.0, 10.0], aspect);
                for i in 0..8 {
                    let corner = [
                        if i & 1 == 0 { -40.0 } else { 60.0 },
                        if i & 2 == 0 { -10.0 } else { 10.0 },
                        if i & 4 == 0 { -10.0 } else { 10.0 },
                    ];
                    let p = c.project(corner, aspect).unwrap();
                    assert!(
                        p[0].abs() <= 1.0 && p[1].abs() <= 1.0 && (0.0..=1.0).contains(&p[2]),
                        "{view:?} {projection:?} aspect {aspect}: {corner:?} -> {p:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn req_ui_02_orbit_keeps_target_and_distance() {
    let mut c = camera(StandardView::Front, Projection::Perspective);
    let (target, distance) = (c.target, c.distance);
    c.orbit(37.0, 12.0);
    assert_eq!(c.target, target);
    let eye = c.eye();
    let d = ((eye[0] - target[0]).powi(2)
        + (eye[1] - target[1]).powi(2)
        + (eye[2] - target[2]).powi(2))
    .sqrt();
    assert!(close(d, distance));
    // Pitch stops at the poles; non-finite input is ignored.
    c.orbit(0.0, 500.0);
    assert_eq!(c.pitch, 90.0);
    c.orbit(f64::NAN, 1.0);
    assert_eq!(c.pitch, 90.0);
}

#[test]
fn req_ui_02_pan_follows_the_pointer() {
    // Dragging right by a quarter of the viewport height moves the target
    // point a quarter of the visible height to the left of the centre.
    for projection in [Projection::Perspective, Projection::Orthographic] {
        let mut c = camera(StandardView::Front, projection);
        let height = 400.0;
        let before = c.project([0.0; 3], 1.0).unwrap();
        c.pan(100.0, 0.0, height);
        let after = c.project([0.0; 3], 1.0).unwrap();
        // The model origin moves right on screen by 100 px = 0.5 in NDC.
        assert!(
            close(after[0] - before[0], 0.5),
            "{projection:?} {before:?} {after:?}"
        );
        assert!(close(after[1], before[1]));
    }
}

#[test]
fn req_ui_02_zoom_changes_size_on_screen() {
    for projection in [Projection::Perspective, Projection::Orthographic] {
        let mut c = camera(StandardView::Front, projection);
        let size = |c: &Camera| c.project([5.0, 0.0, 0.0], 1.0).unwrap()[0];
        let before = size(&c);
        c.zoom(0.5);
        assert!(size(&c) > before * 1.5, "{projection:?}");
        c.zoom(-1.0); // ignored
        c.zoom(f64::INFINITY); // ignored
        assert!(c.distance.is_finite() && c.distance > 0.0);
    }
}

#[test]
fn req_ui_02_switching_projection_keeps_the_target_size() {
    let p = camera(StandardView::Front, Projection::Perspective);
    let o = Camera {
        projection: Projection::Orthographic,
        ..p
    };
    let x = |c: &Camera| c.project([5.0, 0.0, 0.0], 1.0).unwrap()[0];
    assert!(close(x(&p), x(&o)), "{} {}", x(&p), x(&o));
}

#[test]
fn req_ui_02_ray_passes_through_the_projected_point() {
    for projection in [Projection::Perspective, Projection::Orthographic] {
        let mut c = camera(StandardView::Isometric, projection);
        c.orbit(20.0, -10.0);
        let point = [3.0, -4.0, 2.5];
        let ndc = c.project(point, 1.5).unwrap();
        let (origin, direction) = c.ray([ndc[0], ndc[1]], 1.5);
        let to = [
            point[0] - origin[0],
            point[1] - origin[1],
            point[2] - origin[2],
        ];
        let t = to[0] * direction[0] + to[1] * direction[1] + to[2] * direction[2];
        let miss = (0..3)
            .map(|i| (origin[i] + t * direction[i] - point[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(miss < 1e-6, "{projection:?}: {miss}");
    }
}

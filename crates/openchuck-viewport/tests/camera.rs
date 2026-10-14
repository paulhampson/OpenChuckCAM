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

mod bench {
    use std::time::Duration;

    use openchuck_viewport::bench::{CameraPath, FrameTimes, P95_LIMIT, STANDARD_DURATION};
    use openchuck_viewport::camera::Camera;

    fn start() -> Camera {
        let mut c = Camera::default();
        c.fit([0.0, -25.0, -25.0], [100.0, 25.0, 25.0], 16.0 / 9.0);
        c
    }

    #[test]
    fn req_nfr_03_camera_path_orbits_pans_and_zooms_and_returns() {
        let path = CameraPath::new(start(), STANDARD_DURATION);
        let at = |s: f64| path.at(Duration::from_secs_f64(s));
        let begin = at(0.0);
        let same_eye = |a: &Camera, b: &Camera| {
            let (p, q) = (a.eye(), b.eye());
            (0..3).all(|i| (p[i] - q[i]).abs() < 1e-6)
        };
        assert!(same_eye(&begin, &start()));
        // Orbit phase: the yaw moves, the target and distance do not.
        let o = at(7.0);
        assert!(!same_eye(&o, &begin));
        assert_eq!((o.target, o.distance), (begin.target, begin.distance));
        // Pan phase: the target moves, the distance does not.
        let p = at(30.0);
        assert_ne!(p.target, begin.target);
        assert_eq!(p.distance, begin.distance);
        // Zoom phase: the distance moves and returns.
        let z = at(45.0);
        assert!((z.distance - begin.distance).abs() > 0.1 * begin.distance);
        let end = at(60.0);
        assert!((end.distance - begin.distance).abs() < 1e-6 * begin.distance);
        for i in 0..3 {
            assert!((end.target[i] - begin.target[i]).abs() < 1e-6);
        }
        // Past the end it stays at the end; the path depends on time only.
        assert_eq!(at(90.0), end);
        assert_eq!(at(17.3), at(17.3));
    }

    #[test]
    fn req_nfr_03_camera_path_keeps_the_scene_in_view() {
        // Every pose keeps the scene's centre on screen.
        let path = CameraPath::new(start(), STANDARD_DURATION);
        let centre = [50.0, 0.0, 0.0];
        for step in 0..=600 {
            let c = path.at(Duration::from_millis(step * 100));
            let p = c.project(centre, 16.0 / 9.0).unwrap();
            assert!(p[0].abs() < 1.0 && p[1].abs() < 1.0, "step {step}: {p:?}");
        }
    }

    #[test]
    fn req_nfr_03_frame_summary_percentiles_and_pass_rule() {
        let mut times = FrameTimes::default();
        assert!(times.summary().is_none());
        // 95 frames of 10 ms and 5 of 50 ms: the 95th percentile is 10 ms.
        for _ in 0..95 {
            times.push(Duration::from_millis(10));
        }
        for _ in 0..5 {
            times.push(Duration::from_millis(50));
        }
        let s = times.summary().unwrap();
        assert_eq!(s.frames, 100);
        assert_eq!(s.p50, Duration::from_millis(10));
        assert_eq!(s.p95, Duration::from_millis(10));
        assert_eq!(s.p99, Duration::from_millis(50));
        assert_eq!(s.max, Duration::from_millis(50));
        assert_eq!(s.mean, Duration::from_millis(12));
        assert!(s.passes());
        // One more slow frame moves the 95th percentile past the limit.
        times.push(Duration::from_millis(50));
        let s = times.summary().unwrap();
        assert!(s.p95 > P95_LIMIT && !s.passes());
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Independent final-footprint completion cases (OP-24/OP-12).
use openchuck_path::facing::coverage::{FlatSweep, verify_flat_raster as certify_flat_raster};
use openchuck_path::facing::geometry::FacingPlane;
use openchuck_path::rotary::RotaryMapping;
use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::ir::ToolPose;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{ARange, ASense, RotaryAxis};
use openchuck_types::units::Degrees;

fn verify_flat_raster(
    rect: openchuck_path::facing::geometry::CoverageRectangle,
    p: FacingPlane,
    h: f64,
    sweeps: &[FlatSweep],
    r: f64,
    tol: f64,
) -> openchuck_path::Result<()> {
    let token = CancelToken::new();
    certify_flat_raster(
        rect,
        p,
        h,
        sweeps,
        r,
        tol,
        &JobContext::new(&token, &NoProgress),
    )
}

#[test]
fn req_op_24_req_nfr_04_completion_proof_honours_cancellation() {
    let token = CancelToken::new();
    token.cancel();
    let p = plane();
    let rect = p
        .bounds(
            [Point3::new(0., 0., 7.), Point3::new(10., 10., 7.)],
            &[],
            0.,
        )
        .unwrap();
    assert!(matches!(
        certify_flat_raster(
            rect,
            p,
            7.,
            &rows(),
            1.,
            0.01,
            &JobContext::new(&token, &NoProgress)
        ),
        Err(openchuck_path::PathError::Cancelled)
    ));
}

#[allow(clippy::unwrap_used)] // Constant finite test frame.
fn plane() -> FacingPlane {
    FacingPlane::side(
        RotaryMapping {
            axis: RotaryAxis::X,
            sense: ASense::Positive,
            range: ARange::Continuous,
        },
        Degrees(0.),
    )
    .unwrap()
}
fn sweep(y: f64, z: f64) -> FlatSweep {
    let pose = |x| ToolPose {
        tip: Point3::new(x, y, z),
        axis: UnitVec3::Z,
        a: Degrees(0.),
    };
    FlatSweep {
        from: pose(0.),
        to: pose(10.),
    }
}
fn rows() -> Vec<FlatSweep> {
    (0..=5).map(|i| sweep(2. * i as f64, 7.)).collect()
}

#[test]
fn req_op_24_complete_flat_footprints_cover_edges_and_corners() {
    let p = plane();
    let rect = p
        .bounds(
            [Point3::new(0., 0., 7.), Point3::new(10., 10., 7.)],
            &[],
            0.,
        )
        .unwrap();
    verify_flat_raster(rect, p, 7., &rows(), 1., 0.01).unwrap();
}

#[test]
fn req_op_24_missing_final_row_or_edge_is_not_successful_completion() {
    let p = plane();
    let rect = p
        .bounds(
            [Point3::new(0., 0., 7.), Point3::new(10., 10., 7.)],
            &[],
            0.,
        )
        .unwrap();
    for missing in [0, 2, 5] {
        let rows: Vec<_> = rows()
            .into_iter()
            .enumerate()
            .filter_map(|(i, row)| if i != missing { Some(row) } else { None })
            .collect();
        assert!(verify_flat_raster(rect, p, 7., &rows, 1., 0.01).is_err());
    }
}

#[test]
fn req_op_24_req_op_12_clipped_or_raised_passes_and_overcut_refuse() {
    let p = plane();
    let rect = p
        .bounds(
            [Point3::new(0., 0., 7.), Point3::new(10., 10., 7.)],
            &[],
            0.,
        )
        .unwrap();
    let raised: Vec<_> = rows()
        .into_iter()
        .map(|s| sweep(s.from.tip.y, 7.02))
        .collect();
    assert!(verify_flat_raster(rect, p, 7., &raised, 1., 0.01).is_err());
    let low: Vec<_> = rows()
        .into_iter()
        .map(|s| sweep(s.from.tip.y, 6.98))
        .collect();
    assert!(verify_flat_raster(rect, p, 7., &low, 1., 0.01).is_err());
    let clipped: Vec<_> = rows()
        .into_iter()
        .map(|mut s| {
            s.from.tip.x = 0.1;
            s
        })
        .collect();
    assert!(verify_flat_raster(rect, p, 7., &clipped, 1., 0.01).is_err());
}

#[test]
fn req_op_24_req_nfr_07_footprint_proof_rejects_rotary_motion_and_bad_geometry() {
    let p = plane();
    let rect = p
        .bounds(
            [Point3::new(0., 0., 7.), Point3::new(10., 10., 7.)],
            &[],
            0.,
        )
        .unwrap();
    let mut turning = rows();
    turning[0].to.a = Degrees(360.);
    assert!(verify_flat_raster(rect, p, 7., &turning, 1., 0.01).is_err());
    for radius in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(verify_flat_raster(rect, p, 7., &rows(), radius, 0.01).is_err());
    }
    let mut bad = rows();
    bad[0].from.tip.x = f64::NAN;
    assert!(verify_flat_raster(rect, p, 7., &bad, 1., 0.01).is_err());
}

#[test]
fn req_op_24_actual_end_caps_cover_shortened_written_centreline_edges() {
    let p = plane();
    let rect = p
        .bounds(
            [Point3::new(0., 0., 7.), Point3::new(10., 10., 7.)],
            &[],
            0.,
        )
        .unwrap();
    // Eleven rows spaced 1 mm with radius 1: even at the x edges, the
    // vertical half-width sqrt(1 - .001^2) exceeds 0.999999 mm.
    let actual: Vec<_> = (0..=10)
        .map(|i| {
            let mut s = sweep(i as f64, 7.);
            s.from.tip.x = 0.001;
            s.to.tip.x = 9.999;
            s
        })
        .collect();
    verify_flat_raster(rect, p, 7., &actual, 1., 0.01).unwrap();
}
#[test]
fn req_op_24_actual_small_rotary_word_error_reserves_disc_tilt_within_tolerance() {
    use openchuck_types::geometry::Vec3;
    let p = plane();
    let rect = p
        .bounds(
            [Point3::new(0., 0., 7.), Point3::new(10., 10., 7.)],
            &[],
            0.,
        )
        .unwrap();
    let rows = |angle: f64| {
        (0..=10)
            .map(|i| {
                let mut s = sweep(i as f64, 7.);
                let q = angle.to_radians();
                for pose in [&mut s.from, &mut s.to] {
                    pose.a = Degrees(angle);
                    pose.axis = UnitVec3::new(Vec3::new(0., q.sin(), q.cos())).unwrap();
                }
                s
            })
            .collect::<Vec<_>>()
    };
    // Disc-height error < 0.000002 mm, far inside the 0.01 mm budget.
    verify_flat_raster(rect, p, 7., &rows(0.0001), 1., 0.01).unwrap();
    assert!(verify_flat_raster(rect, p, 7., &rows(1.), 1., 0.01).is_err());
}

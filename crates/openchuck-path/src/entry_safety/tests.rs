// SPDX-License-Identifier: AGPL-3.0-or-later
//! 0248: immutable cache scope, charged hits, eviction and cancellation.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use openchuck_types::geometry::UnitVec3;
use openchuck_types::job::{CancelToken, NoProgress, Progress, ProgressSink};
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct Counts {
    queries: AtomicUsize,
    hits: AtomicUsize,
    cancel_query: Option<CancelToken>,
    cancel_hit: Option<CancelToken>,
}
impl ProgressSink for Counts {
    fn report(&self, _: Progress) {}
    fn wants_timings(&self) -> bool {
        true
    }
    fn timing(&self, stage: &str, _: std::time::Duration) {
        if stage == "entry clearance query" {
            self.queries.fetch_add(1, Ordering::Relaxed);
            if let Some(c) = &self.cancel_query {
                c.cancel();
            }
        }
        if stage == "entry clearance cache hit" {
            self.hits.fetch_add(1, Ordering::Relaxed);
            if let Some(c) = &self.cancel_hit {
                c.cancel();
            }
        }
    }
}
fn with_proof(f: impl FnOnce(&EntrySafety<'_>)) {
    let target = MeshSurface::new(
        &Solid::cuboid(Aabb {
            min: Point3::new(-10.0, -1.0, -1.0),
            max: Point3::new(10.0, 1.0, 1.0),
        })
        .unwrap()
        .mesh(8),
    )
    .unwrap();
    let body = Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(1.0),
        flute_length: Millimetres(12.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(1.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
    .profile()
    .unwrap();
    let mut machine = MachineProfile::example_x_rotary();
    machine.rotary_origin = Point3::ORIGIN;
    f(&EntrySafety {
        target: &target,
        body: &body,
        protected: &[],
        machine: &machine,
        rotary_precision: RotaryPrecision::Float32,
    });
}
fn pose(x: f64) -> ToolPose {
    ToolPose {
        tip: Point3::new(x, 0.0, 10.0),
        axis: UnitVec3::Z,
        a: Degrees(0.0),
    }
}

#[test]
fn req_op_12_req_nfr_04_memo_reuses_contacts_and_no_contact_only_for_exact_inputs() {
    with_proof(|proof| {
        let counts = Counts::default();
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &counts);
        let mut session = SweepSession::new(proof);
        for x in [0.0, 50.0] {
            let before = counts.queries.load(Ordering::Relaxed);
            session
                .verify(pose(x), pose(x + 1.0), 0.5, 0.01, &mut 1, &ctx)
                .unwrap();
            let mut budget = 1;
            session
                .verify(pose(x), pose(x + 1.0), 0.5, 0.01, &mut budget, &ctx)
                .unwrap();
            assert_eq!(budget, 0, "cache hits must consume the same attempt");
            assert_eq!(counts.queries.load(Ordering::Relaxed), before + 1);
        }
        assert_eq!(counts.hits.load(Ordering::Relaxed), 2);
        for (x, leave) in [(0.001, 0.5), (0.0, 0.5001)] {
            session
                .verify(pose(x), pose(x + 1.0), leave, 0.01, &mut 1, &ctx)
                .unwrap();
        }
        let rotated = |x| ToolPose {
            a: Degrees(360.0),
            axis: tool_axis_in_model(&RotaryFrame::of_profile(proof.machine), Degrees(360.0)),
            ..pose(x)
        };
        session
            .verify(rotated(0.0), rotated(1.0), 0.5, 0.01, &mut 1, &ctx)
            .unwrap();
        assert_eq!(
            counts.queries.load(Ordering::Relaxed),
            5,
            "changed origin, inflation or axis must miss"
        );
        // A fresh generation holds new immutable target/body references. It
        // cannot inherit even exact matching coordinates from an old session.
        let mut fresh = SweepSession::new(proof);
        fresh
            .verify(pose(0.0), pose(1.0), 0.5, 0.01, &mut 1, &ctx)
            .unwrap();
        assert_eq!(counts.queries.load(Ordering::Relaxed), 6);
        let protected = [Solid::cuboid(Aabb {
            min: Point3::new(0.0, -1.0, 9.0),
            max: Point3::new(1.0, 1.0, 11.0),
        })
        .unwrap()];
        let unsafe_proof = EntrySafety {
            protected: &protected,
            ..*proof
        };
        assert!(
            SweepSession::new(&unsafe_proof)
                .verify(pose(0.0), pose(1.0), 0.5, 0.01, &mut 1, &ctx)
                .is_err()
        );
    });
}

#[test]
fn req_nfr_04_req_nfr_06_memo_evicts_in_insertion_order_with_bounded_storage() {
    with_proof(|proof| {
        let mut session = SweepSession::with_capacity(proof, 2);
        let counts = Counts::default();
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &counts);
        for x in [0.0, 2.0, 0.0, 4.0, 0.0] {
            session
                .verify(pose(x), pose(x + 1.0), 0.0, 0.01, &mut 1, &ctx)
                .unwrap();
        }
        assert_eq!(
            counts.queries.load(Ordering::Relaxed),
            4,
            "hit at 0 must not refresh FIFO order"
        );
        assert_eq!(session.answers.len(), 2);
        assert_eq!(session.insertion_order.len(), 2);
    });
}

#[test]
fn req_nfr_04_req_nfr_07_cancellation_during_query_and_cached_check_returns_no_certificate() {
    with_proof(|proof| {
        let cancel = CancelToken::new();
        let counts = Counts {
            cancel_query: Some(cancel.clone()),
            ..Counts::default()
        };
        let ctx = JobContext::new(&cancel, &counts);
        let mut session = SweepSession::new(proof);
        assert!(matches!(
            session.verify(pose(0.0), pose(1.0), 0.0, 0.01, &mut 1, &ctx),
            Err(PathError::Cancelled)
        ));
        assert_eq!(
            counts.queries.load(Ordering::Relaxed),
            1,
            "cancel occurred during certification, not before it"
        );
        let cancel = CancelToken::new();
        session
            .verify(
                pose(0.0),
                pose(1.0),
                0.0,
                0.01,
                &mut 1,
                &JobContext::new(&cancel, &NoProgress),
            )
            .unwrap();
        let counts = Counts {
            cancel_hit: Some(cancel.clone()),
            ..Counts::default()
        };
        let ctx = JobContext::new(&cancel, &counts);
        assert!(matches!(
            session.verify(pose(0.0), pose(1.0), 0.0, 0.01, &mut 1, &ctx),
            Err(PathError::Cancelled)
        ));
        assert_eq!(counts.hits.load(Ordering::Relaxed), 1);
    });
}

#[test]
fn req_op_12_req_set_09_new_target_or_complete_body_never_inherits_a_previous_clear_answer() {
    with_proof(|proof| {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let mut session = SweepSession::new(proof);
        session
            .verify(pose(-1.0), pose(1.0), 0.0, 0.01, &mut 1, &ctx)
            .unwrap();
        let obstructed = MeshSurface::new(
            &Solid::cuboid(Aabb {
                min: Point3::new(-2.0, -1.0, 0.0),
                max: Point3::new(2.0, 1.0, 11.0),
            })
            .unwrap()
            .mesh(8),
        )
        .unwrap();
        let changed = EntrySafety {
            target: &obstructed,
            ..*proof
        };
        assert!(
            SweepSession::new(&changed)
                .verify(pose(-1.0), pose(1.0), 0.0, 0.01, &mut 100_000, &ctx)
                .is_err()
        );
        let shank_obstacle = MeshSurface::new(
            &Solid::cuboid(Aabb {
                min: Point3::new(-2.0, -1.0, 40.0),
                max: Point3::new(2.0, 1.0, 41.0),
            })
            .unwrap()
            .mesh(8),
        )
        .unwrap();
        // Move laterally beside a high obstacle: only the wider shank reaches
        // it. Both profiles are supported convex enclosures of the whole body.
        let narrow = openchuck_types::tool::profile::ToolProfile {
            segments: vec![
                openchuck_types::tool::profile::ProfileSegment::Line {
                    to: openchuck_types::tool::profile::ProfilePoint::new(0.5, 0.0),
                },
                openchuck_types::tool::profile::ProfileSegment::Line {
                    to: openchuck_types::tool::profile::ProfilePoint::new(0.5, 50.0),
                },
            ],
        }
        .validate()
        .unwrap();
        let wide = openchuck_types::tool::profile::ToolProfile {
            segments: vec![
                openchuck_types::tool::profile::ProfileSegment::Line {
                    to: openchuck_types::tool::profile::ProfilePoint::new(3.0, 0.0),
                },
                openchuck_types::tool::profile::ProfileSegment::Line {
                    to: openchuck_types::tool::profile::ProfilePoint::new(3.0, 50.0),
                },
            ],
        }
        .validate()
        .unwrap();
        let beside = |x| ToolPose {
            tip: Point3::new(x, 2.0, 10.0),
            ..pose(x)
        };
        let narrow_proof = EntrySafety {
            target: &shank_obstacle,
            body: &narrow,
            ..*proof
        };
        SweepSession::new(&narrow_proof)
            .verify(beside(-0.1), beside(0.1), 0.0, 0.01, &mut 100_000, &ctx)
            .unwrap();
        let wide_proof = EntrySafety {
            body: &wide,
            ..narrow_proof
        };
        assert!(
            SweepSession::new(&wide_proof)
                .verify(beside(-0.1), beside(0.1), 0.0, 0.01, &mut 100_000, &ctx)
                .is_err()
        );
    });
}

#[test]
fn req_op_12_req_nfr_04_memo_preserves_certificates_and_budget_at_lower_query_cost() {
    with_proof(|proof| {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let mut cached_budget = 10_000;
        let mut session = SweepSession::new(proof);
        let cached_start = std::time::Instant::now();
        for _ in 0..10_000 {
            session
                .verify(pose(0.0), pose(1.0), 0.0, 0.01, &mut cached_budget, &ctx)
                .unwrap();
        }
        let cached = cached_start.elapsed();
        let mut fresh_budget = 10_000;
        let fresh_start = std::time::Instant::now();
        for _ in 0..10_000 {
            proof
                .verify(pose(0.0), pose(1.0), 0.0, 0.01, &mut fresh_budget, &ctx)
                .unwrap();
        }
        let fresh = fresh_start.elapsed();
        assert_eq!(fresh_budget, cached_budget);
        assert_eq!(cached_budget, 0);
        eprintln!(
            "RAMPBUDGET 10000 clear identical sweeps cached={cached:?} fresh_query_per_sweep={fresh:?}"
        );
    });
}

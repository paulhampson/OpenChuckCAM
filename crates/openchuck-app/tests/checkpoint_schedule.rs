// SPDX-License-Identifier: AGPL-3.0-or-later
//! PRJ-08 and UI-15: independent periodic and interim checkpoint clocks.
#![allow(clippy::unwrap_used)]

use openchuck_app::checkpoint_schedule::{Kind, Schedule};
use std::time::Duration;

fn at(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

#[test]
fn req_prj_08_periodic_deadline_survives_interim_publication() {
    let mut schedule = Schedule::new(3, at(0)).unwrap();
    assert!(schedule.next(at(0), 1, true).is_none());
    assert!(schedule.next(at(14), 1, true).is_none());
    let interim = schedule.next(at(15), 1, true).unwrap();
    assert_eq!(interim.kind, Kind::Interim);
    assert!(schedule.complete(interim, true, at(16)));
    assert!(schedule.next(at(179), 1, true).is_none());
    let periodic = schedule.next(at(180), 1, true).unwrap();
    assert_eq!(periodic.kind, Kind::Periodic);
    assert!(schedule.complete(periodic, true, at(181)));
    assert!(schedule.next(at(360), 1, true).is_none());
}

#[test]
fn req_prj_08_continuous_editing_does_not_postpone_periodic_checkpoint() {
    let mut schedule = Schedule::new(1, at(0)).unwrap();
    for seconds in 0..60 {
        assert!(schedule.next(at(seconds), seconds + 1, true).is_none());
    }
    let request = schedule.next(at(60), 61, true).unwrap();
    assert_eq!(request.kind, Kind::Periodic);
    assert_eq!(request.revision, 61);
}

#[test]
fn req_prj_08_debounce_uses_confirmed_revision_and_clean_projects_do_not_run() {
    let mut schedule = Schedule::new(3, at(0)).unwrap();
    assert!(schedule.next(at(0), 1, true).is_none());
    assert!(schedule.next(at(10), 2, true).is_none());
    assert!(schedule.next(at(24), 2, true).is_none());
    let request = schedule.next(at(25), 2, true).unwrap();
    assert_eq!(request.kind, Kind::Interim);
    assert!(schedule.complete(request, true, at(26)));
    // Polls with unchanged committed state (including unconfirmed text)
    // cannot create more interim checkpoints.
    assert!(schedule.next(at(50), 2, true).is_none());
    assert!(schedule.next(at(180), 3, false).is_none());
    assert!(schedule.next(at(200), 4, true).is_some());
}

#[test]
fn req_prj_08_single_flight_and_failed_jobs_retry_without_losing_new_edits() {
    let mut schedule = Schedule::new(3, at(0)).unwrap();
    schedule.next(at(0), 1, true);
    let old = schedule.next(at(15), 1, true).unwrap();
    assert!(schedule.next(at(20), 2, true).is_none());
    assert!(schedule.complete(old, false, at(21)));
    assert!(schedule.next(at(25), 2, true).is_none());
    let newer = schedule.next(at(35), 2, true).unwrap();
    assert_eq!(newer.revision, 2);
    assert!(!schedule.complete(old, true, at(36)));
    assert!(schedule.next(at(37), 3, true).is_none());
    assert!(schedule.complete(newer, true, at(38)));
    assert!(schedule.next(at(51), 3, true).is_none());
    assert_eq!(schedule.next(at(52), 3, true).unwrap().revision, 3);
}

#[test]
fn req_prj_08_clean_then_dirty_cannot_reuse_an_interim_from_before_save() {
    let mut schedule = Schedule::new(3, at(0)).unwrap();
    schedule.next(at(0), 1, true);
    let first = schedule.next(at(15), 1, true).unwrap();
    schedule.complete(first, true, at(16));
    schedule.next(at(17), 2, false);
    // Revision tokens may be reused by a caller only after an intervening
    // clean state; the new edit still needs a fresh checkpoint.
    assert!(schedule.next(at(20), 1, true).is_none());
    assert_eq!(schedule.next(at(35), 1, true).unwrap().kind, Kind::Interim);
}

#[test]
fn req_ui_15_req_prj_08_default_and_interval_bounds() {
    assert_eq!(Schedule::default().interval(), at(180));
    for minutes in 1..=15 {
        assert!(Schedule::new(minutes, at(0)).is_ok());
    }
    for minutes in [0, 16, u16::MAX] {
        assert!(Schedule::new(minutes, at(0)).is_err());
    }
}

#[test]
fn req_ui_15_req_prj_08_interval_changes_apply_without_resetting_the_inflight_ticket() {
    let mut schedule = Schedule::new(1, at(0)).unwrap();
    for seconds in 0..60 {
        assert!(schedule.next(at(seconds), seconds + 1, true).is_none());
    }
    let pending = schedule.next(at(60), 61, true).unwrap();
    schedule.configure(15, at(61)).unwrap();
    assert!(schedule.complete(pending, true, at(62)));
    for seconds in 62..961 {
        assert!(schedule.next(at(seconds), seconds + 1, true).is_none());
    }
    assert_eq!(
        schedule.next(at(961), 962, true).unwrap().kind,
        Kind::Periodic
    );
    assert!(schedule.configure(0, at(962)).is_err());
    assert_eq!(schedule.interval(), at(900));
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Decision 0209 M4 must post each form, not just compare its serialized IR.
//! These contract tests use a small known path and need no STEP kernel.
#![allow(clippy::unwrap_used)]

mod support;

#[path = "../examples/step_accuracy/posting.rs"]
mod posting;

#[test]
fn req_gc_07_req_nfr_06_m4_posts_repeatable_bytes_for_a_measured_path() {
    let job = support::job();
    let groups: Vec<_> = support::small_part().groups.into_iter().map(Ok).collect();
    let first = posting::post_run(&groups, &job).unwrap();
    let repeated = posting::post_run(&groups.clone(), &job).unwrap();
    assert_eq!(first, repeated);
    assert!(
        first.contains("G1"),
        "M4 must compare posted motion, not empty output"
    );
    assert!(
        first.contains("10000"),
        "the measured program needs its job's spindle speed"
    );
    assert!(
        first.contains("Small part"),
        "the fixed job title must reach the post"
    );
}

#[test]
fn req_gc_07_m4_posts_only_the_operations_that_generated_successfully() {
    let job = support::job();
    let mut groups: Vec<_> = support::small_part().groups.into_iter().map(Ok).collect();
    let successful = posting::post_run(&groups, &job).unwrap();
    assert!(!successful.is_empty());
    groups.push(Err("simultaneous finishing refused this part".into()));
    assert_eq!(posting::post_run(&groups, &job).unwrap(), successful);
}

#[test]
fn req_nfr_07_m4_refuses_an_empty_program_and_propagates_post_errors() {
    let mut job = support::job();
    assert!(posting::post_run(&[], &job).is_err());
    assert!(posting::post_run(&[Err("strategy refused".into())], &job).is_err());
    let groups: Vec<_> = support::small_part().groups.into_iter().map(Ok).collect();
    job.tools.clear();
    assert!(posting::post_run(&groups, &job).is_err());
}

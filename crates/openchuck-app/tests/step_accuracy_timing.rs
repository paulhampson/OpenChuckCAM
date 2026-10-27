// SPDX-License-Identifier: AGPL-3.0-or-later
//! Accepted decisions 0209/0211: five successful equivalent runs, with
//! refused attempts excluded from claims of performance compliance.
#[path = "../examples/step_accuracy/timing.rs"]
mod timing;

#[test]
fn req_nfr_04_req_op_23_m3_requires_five_successful_runs() {
    assert_eq!(timing::successful_median(&[]), None);
    assert_eq!(timing::successful_median(&[Some(1.0); 4]), None);
    assert_eq!(
        timing::successful_median(&[Some(1.0), None, Some(2.0), Some(3.0), Some(4.0), Some(5.0)]),
        None
    );
    for invalid in [f64::NAN, f64::INFINITY, -1.0] {
        assert_eq!(
            timing::successful_median(&[Some(1.0), Some(2.0), Some(invalid), Some(4.0), Some(5.0)]),
            None
        );
    }
}

#[test]
fn req_nfr_04_m3_uses_the_median_not_the_first_or_fastest_run() {
    let samples = [Some(20.0), Some(1.0), Some(4.0), Some(3.0), Some(2.0)];
    assert_eq!(timing::successful_median(&samples), Some(3.0));
    assert_eq!(
        timing::successful_median(&[
            Some(6.0),
            Some(2.0),
            Some(4.0),
            Some(1.0),
            Some(5.0),
            Some(3.0)
        ]),
        Some(3.5)
    );
}

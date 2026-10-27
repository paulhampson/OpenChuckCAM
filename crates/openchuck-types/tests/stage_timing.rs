// SPDX-License-Identifier: AGPL-3.0-or-later
//! Optional host timing receives complete scopes, including early exits.
#![allow(clippy::unwrap_used, clippy::panic)]
use openchuck_types::job::{CancelToken, JobContext, Progress, ProgressSink};
use std::sync::Mutex;
use std::time::Duration;
#[derive(Default)]
struct Sink(Mutex<Vec<String>>);
impl ProgressSink for Sink {
    fn report(&self, _: Progress) {}
    fn wants_timings(&self) -> bool {
        true
    }
    fn timing(&self, stage: &str, _: Duration) {
        self.0.lock().unwrap().push(stage.to_owned());
    }
}
#[test]
fn req_nfr_04_req_int_04_host_receives_completed_and_failed_stage_timings() {
    let sink = Sink::default();
    let cancel = CancelToken::new();
    let context = JobContext::new(&cancel, &sink);
    {
        let _timer = context.time_stage("assembly");
    }
    let failed = || -> Result<(), &'static str> {
        let _timer = context.time_stage("fitting");
        Err("refused")
    };
    assert!(failed().is_err());
    assert_eq!(*sink.0.lock().unwrap(), ["assembly", "fitting"]);
}

#[test]
fn req_int_04_existing_hosts_do_not_receive_unsolicited_timings() {
    struct OptOut;
    impl ProgressSink for OptOut {
        fn report(&self, _: Progress) {}
        fn timing(&self, _: &str, _: Duration) {
            panic!("a host must opt in before receiving timings");
        }
    }
    let cancel = CancelToken::new();
    let context = JobContext::new(&cancel, &OptOut);
    let _timer = context.time_stage("assembly");
}

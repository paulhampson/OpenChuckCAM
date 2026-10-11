// SPDX-License-Identifier: AGPL-3.0-or-later
//! Golden-file helper (NFR-08).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_testkit::golden;

#[test]
fn req_nfr_08_golden_comparison_reports_the_first_difference() {
    assert!(golden::compare("G0 X0\nG1 X1\n", "G0 X0\nG1 X1\n").is_ok());
    let message = golden::compare("G0 X0\nG1 X1\n", "G0 X0\nG1 X2\n").unwrap_err();
    assert!(message.contains("line 2"), "{message}");
    assert!(
        golden::compare("a\n", "a\nb\n")
            .unwrap_err()
            .contains("extra output")
    );
    assert!(golden::compare("a\n", "a").is_err());
}

#[test]
fn req_nfr_08_missing_golden_file_is_an_error() {
    let missing = std::env::temp_dir().join("openchuck-no-such-golden.ngc");
    if std::env::var(golden::BLESS_VAR).is_err() {
        assert!(
            golden::check(&missing, "x")
                .unwrap_err()
                .contains("OPENCHUCK_BLESS")
        );
    }
}

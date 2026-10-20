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

/// NFR-16 and GC-07: a golden file stores the generator comment with the version masked, so a
/// version bump never changes a golden file; only the version is masked.
#[test]
fn req_nfr_16_req_gc_07_the_generator_version_is_masked_in_golden_comparisons() {
    use openchuck_testkit::golden::mask_generator;
    assert_eq!(
        mask_generator("(Small part)\n(OpenChuckCAM 0.10.0)\nG90\n"),
        "(Small part)\n(OpenChuckCAM <version>)\nG90\n"
    );
    // With a line number in front, as a post with numbering writes it.
    assert_eq!(
        mask_generator("N20 (OpenChuckCAM 12.3.45)\n"),
        "N20 (OpenChuckCAM <version>)\n"
    );
    // Other text, including a lookalike inside a longer comment, is left as it is.
    for text in [
        "(Not OpenChuckCAM 1.0.0)\n",
        "(OpenChuckCAM 1.0.0 extra)\n",
        "G1 X1\n",
    ] {
        assert_eq!(mask_generator(text), text);
    }
    let dir = std::env::temp_dir().join(format!("openchuck-mask-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("g.nc");
    std::fs::write(&path, "(OpenChuckCAM <version>)\nG0\n").unwrap();
    golden::check(&path, "(OpenChuckCAM 0.11.0)\nG0\n").unwrap();
    // A bless run rewrites the golden file instead of failing on a difference.
    if std::env::var(golden::BLESS_VAR).is_err() {
        assert!(golden::check(&path, "(OpenChuckCAM 0.11.0)\nG1\n").is_err());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

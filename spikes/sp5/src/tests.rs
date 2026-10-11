// SPDX-License-Identifier: AGPL-3.0-or-later
//! Each finding in RESULT.md, pinned as a test.

use crate::model::Reader;
use crate::readers::{GCodeCrate, GcodeCrate, InHouse};
use crate::{FIXTURES, interp, oracle};

const SUBSET: &str = include_str!("../fixtures/linuxcnc_post_subset.ngc");

fn post_fixtures() -> impl Iterator<Item = (&'static str, &'static str)> {
    FIXTURES.iter().copied().filter(|(n, _)| *n != "linuxcnc_dialect_extra.ngc")
}

#[test]
fn every_post_code_parsed_with_exact_value_and_position() {
    // The SP-5 pass criterion, for the in-house reader and gcode with the f64 layer.
    for r in [&InHouse as &dyn Reader, &GcodeCrate { f64_layer: true }] {
        for (name, src) in post_fixtures() {
            let truth = oracle::split(src).unwrap();
            let p = r.read(src).unwrap_or_else(|e| panic!("{} on {name}: {}", r.name(), e[0]));
            let words: Vec<_> = p.words().cloned().collect();
            oracle::compare(&truth, &words).unwrap_or_else(|e| panic!("{} on {name}: {e}", r.name()));
            interp::replay(&p).unwrap_or_else(|e| panic!("{} on {name}: {e}", r.name()));
        }
    }
}

#[test]
fn subset_fixture_uses_every_known_code() {
    let p = InHouse.read(SUBSET).unwrap();
    for (letter, list) in [('G', interp::KNOWN_G), ('M', interp::KNOWN_M)] {
        for code in list {
            let v: f64 = code.parse().unwrap();
            assert!(p.words().any(|w| w.letter == letter && w.value == v), "{letter}{code} missing from the fixture");
        }
    }
}

#[test]
fn a_words_replay_unwrapped_and_incremental() {
    let p = InHouse.read(SUBSET).unwrap();
    let m = interp::replay(&p).unwrap();
    let a: Vec<f64> = m.iter().map(|m| m.to[3]).collect();
    assert!(a.contains(&-12345.6789));
    assert!(a.contains(&1234.5678));
    // N240 is G91: X and A move by 1 and 10 from N230's X16 and N220's A60.
    let n240 = m.iter().find(|m| m.line == 30).unwrap();
    assert_eq!(n240.to, [17.0, 5.0, -1.25, 70.0]);
    assert!(m.iter().any(|m| m.inverse_time && m.feed == Some(45.5)));
}

#[test]
fn gcode_f32_values_exceed_the_kinematic_limit() {
    // Verification plan 4.1: 1e-6 mm and 1e-6 degrees.
    let p = GcodeCrate { f64_layer: false }.read(&oracle::stress_program()).unwrap();
    let worst = p.words().map(|w| (w.value - w.text.parse::<f64>().unwrap()).abs()).fold(0.0, f64::max);
    assert!(worst > 1e-3, "{worst}");
}

#[test]
fn gcode_silently_drops_codes_it_cannot_store() {
    for src in ["G1.25 X1", "G99999999999 X1", "N99999999999 G1 X1"] {
        let p = GcodeCrate { f64_layer: true }.read(src).unwrap();
        assert!(!p.words().any(|w| w.letter == 'G' && w.value != 1.0) || src.starts_with('N'));
        let in_house = InHouse.read(src).unwrap();
        assert!(in_house.words().count() > p.words().count(), "{src}");
    }
}

#[test]
fn gcode_accepts_an_unclosed_comment() {
    assert!(GcodeCrate { f64_layer: true }.read("G1 X1 (no close").is_ok());
    assert!(InHouse.read("G1 X1 (no close").is_err());
}

#[test]
fn g_code_panics_on_sign_then_sign() {
    let r = std::panic::catch_unwind(|| GCodeCrate.read("G1 X+-1.0"));
    assert!(r.is_err());
}

#[test]
fn g_code_rejects_percent_newline_at_end() {
    assert!(GCodeCrate.read("%\nG1 X1\n%\n").is_err());
    assert!(GCodeCrate.read("%\nG1 X1\n%").is_ok());
    assert!(InHouse.read("%\nG1 X1\n%\n").is_ok());
}

#[test]
fn in_house_errors_name_line_and_column() {
    let e = InHouse.read("G1 X1\nG1 Y#1\n").unwrap_err();
    assert_eq!((e[0].line, e[0].col), (2, 4));
    let e = InHouse.read("G0 X0\n\nG1 X1.2.3").unwrap_err();
    assert_eq!((e[0].line, e[0].col), (3, 4));
}

#[test]
fn in_house_never_panics_under_mutation() {
    let f = crate::fuzz::run(&InHouse, 20_000, 7);
    assert_eq!(f.panics, 0, "{:?}", f.example);
}

#[test]
fn readers_are_deterministic() {
    for r in crate::readers() {
        for (_, src) in post_fixtures() {
            assert_eq!(r.read(src).ok(), r.read(src).ok(), "{}", r.name());
        }
    }
}

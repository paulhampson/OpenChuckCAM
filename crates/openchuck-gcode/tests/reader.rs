// SPDX-License-Identifier: AGPL-3.0-or-later
//! The reader, the modal replay and the static checks (GC-08, NFR-05, NFR-07).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use std::path::Path;

use openchuck_gcode::{
    CheckConfig, Dialect, Event, FindingKind, Motion, Step, read, replay, static_check,
};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/gcode")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

fn config() -> CheckConfig {
    CheckConfig {
        travel: [(-10.0, 200.0), (-200.0, 10.0), (-50.0, 100.0)],
        a_range: None,
        max_feed: 1200.0,
        safe_z: 20.0,
        spindle_off_at_tool_change: true,
        require_work_offset: true,
        allow_inverse_time: false,
    }
}

#[test]
fn req_gc_08_the_post_subset_fixture_is_read_with_positions() {
    let program = read(&fixture("linuxcnc_post_subset.ngc")).unwrap();
    assert!(program.blocks.len() > 40);
    let n160 = program
        .blocks
        .iter()
        .find(|b| {
            b.words
                .first()
                .is_some_and(|w| w.letter == 'N' && w.value == 160.0)
        })
        .unwrap();
    assert_eq!(n160.line, 22);
    let x = n160.words.iter().find(|w| w.letter == 'X').unwrap();
    assert_eq!((x.value, x.col), (10.5, 9));
    // Long unwrapped A keeps full precision (SP-5: f32 was 2.4e-3 off).
    let a = program
        .blocks
        .iter()
        .flat_map(|b| &b.words)
        .find(|w| w.letter == 'A' && w.text == "-12345.6789")
        .unwrap();
    assert_eq!(a.value, -12345.6789);
    // Comments of both kinds are kept apart from the words.
    assert!(
        program
            .blocks
            .iter()
            .any(|b| b.comments.iter().any(|c| c.contains("semicolon")))
    );
    assert!(
        program
            .blocks
            .iter()
            .any(|b| b.comments.iter().any(|c| c.starts_with("MSG")))
    );
}

#[test]
fn req_gc_08_the_post_subset_replays_with_every_code_known() {
    let program = read(&fixture("linuxcnc_post_subset.ngc")).unwrap();
    let replay = replay(&program, Dialect::Linuxcnc).unwrap();
    assert!(replay.moves().any(|m| m.motion == Motion::ArcCw));
    assert!(replay.moves().any(|m| m.inverse_time));
    // N250 G90 after the relative G91 move: the last absolute position is reached.
    let n240 = program
        .blocks
        .iter()
        .find(|b| b.words.iter().any(|w| w.letter == 'N' && w.value == 240.0))
        .unwrap()
        .line;
    let relative = replay.moves().find(|m| m.line == n240).unwrap();
    assert_eq!(relative.to[0], relative.from[0] + 1.0);
}

#[test]
fn req_nfr_07_features_outside_the_subset_are_refused_by_name() {
    let source = fixture("linuxcnc_dialect_extra.ngc");
    let errors = read(&source).unwrap_err();
    let text: Vec<String> = errors.iter().map(ToString::to_string).collect();
    assert!(
        text.iter()
            .any(|e| e.contains("line 3") && e.contains("parameters")),
        "{text:?}"
    );
    assert!(
        text.iter()
            .any(|e| e.contains("line 5") && e.contains("needs a number")),
        "{text:?}"
    );
    assert!(
        read("[1 + 2]\n").unwrap_err()[0]
            .to_string()
            .contains("expressions")
    );
    assert!(text.iter().any(|e| e.contains("O-words")), "{text:?}");
    // The valid lines of the file are not lost: lowercase and spaced words read.
    let ok = read("g1 x1.0 y2.0 a3.0\nG 1 X 1 . 5 A 2 0\n/ G0 X99\n").unwrap();
    assert_eq!(ok.blocks[0].words[0].letter, 'G');
    assert_eq!(ok.blocks[1].words[1].value, 1.5);
    assert_eq!(ok.blocks[1].words[2].value, 20.0);
    assert!(ok.blocks[2].block_delete);
    // G38.2 is read as a word and refused by the replay by name.
    let g = read("G38.2 Z-10 F50\n").unwrap();
    assert!(
        replay(&g, Dialect::Linuxcnc)
            .unwrap_err()
            .message
            .contains("G38.2")
    );
}

#[test]
fn req_nfr_07_malformed_input_is_an_error_with_line_and_column() {
    for (source, expect) in [
        ("G1 X\n", "needs a number"),
        ("G1 X1.2.3\n", "needs a number"),
        ("G1 (open\n", "not closed"),
        ("G1 ((x))\n", "nested"),
        ("G1 X+-1\n", "needs a number"),
        ("G1 X1 $\n", "unexpected character"),
        ("% G1\n", "alone"),
    ] {
        let errors = read(source).unwrap_err();
        let e = errors[0].to_string();
        assert!(e.starts_with("line 1, column "), "{source}: {e}");
        assert!(e.contains(expect), "{source}: {e}");
    }
    let long = format!("G1 X1 ({})\n", "a".repeat(5000));
    assert!(read(&long).unwrap_err()[0].to_string().contains("longer"));
}

#[test]
fn req_nfr_07_random_bytes_never_panic() {
    // A small deterministic mutation fuzz over the subset file (the fuzz target runs longer).
    let seed = fixture("linuxcnc_post_subset.ngc").into_bytes();
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    for _ in 0..3000 {
        let mut bytes = seed.clone();
        for _ in 0..4 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let i = (state % bytes.len() as u64) as usize;
            bytes[i] = (state >> 32) as u8;
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if let Ok(program) = read(&text) {
            let _ = replay(&program, Dialect::Linuxcnc);
            let _ = replay(&program, Dialect::MakeraZ1);
        }
    }
}

const GOOD: &str = "G90 G21 G17\nG54\nM5\nT1 M6\nS10000 M3\nG0 X0 Y0 Z30 A0\nG1 Z10 F100\nX5 Y-3 Z8 A45 F300\nG0 Z30\nM5\nM2\n";

fn findings(source: &str, config: &CheckConfig) -> Vec<FindingKind> {
    let program = read(source).unwrap();
    let replay = replay(&program, Dialect::Linuxcnc).unwrap();
    static_check(&replay, config)
        .into_iter()
        .map(|f| f.kind)
        .collect()
}

#[test]
fn req_gc_08_gate_1_passes_a_good_program() {
    assert_eq!(findings(GOOD, &config()), []);
}

#[test]
fn req_gc_08_gate_1_fails_on_seeded_faults() {
    let c = config();
    // Missing feed.
    let no_feed = GOOD.replace("G1 Z10 F100", "G1 Z10");
    assert_eq!(findings(&no_feed, &c), [FindingKind::MissingFeed]);
    // A beyond its limit.
    let limited = CheckConfig {
        a_range: Some((-90.0, 30.0)),
        ..config()
    };
    let a = findings(GOOD, &limited);
    assert!(
        !a.is_empty() && a.iter().all(|k| *k == FindingKind::AxisLimit),
        "{a:?}"
    );
    // X beyond the travel.
    assert!(findings(&GOOD.replace("X5 ", "X500 "), &c).contains(&FindingKind::AxisLimit));
    // Wrong units.
    assert!(findings(&GOOD.replace("G21", "G20"), &c).contains(&FindingKind::Units));
    assert!(findings(&GOOD.replace("G90 G21 G17\n", ""), &c).contains(&FindingKind::Units));
    // No work offset, relative mode, feed too high, spindle off, no end.
    assert!(findings(&GOOD.replace("G54\n", ""), &c).contains(&FindingKind::WorkOffset));
    assert!(findings(&GOOD.replace("G90 ", "G91 "), &c).contains(&FindingKind::DistanceMode));
    assert!(findings(&GOOD.replace("F300", "F3000"), &c).contains(&FindingKind::FeedTooHigh));
    assert!(findings(&GOOD.replace("S10000 M3\n", ""), &c).contains(&FindingKind::SpindleOff));
    assert!(findings(&GOOD.replace("M2\n", ""), &c).contains(&FindingKind::NoEnd));
    // G93 where it is not allowed, and a G93 move without an F in its own block.
    let g93 = GOOD.replace("G0 X0", "G93 G0 X0");
    assert!(findings(&g93, &c).contains(&FindingKind::InverseTime));
    // An unsafe tool change: second change below the safe Z.
    let low = "G90 G21\nG54\nT1 M6\nS1000 M3\nG0 Z5\nM5\nT2 M6\nM2\n";
    assert!(findings(low, &c).contains(&FindingKind::UnsafeToolChange));
    let running = "G90 G21\nG54\nG0 Z30\nS1000 M3\nT2 M6\nM2\n";
    assert!(findings(running, &c).contains(&FindingKind::UnsafeToolChange));
}

#[test]
fn req_gc_09_the_z1_drop_turns_command_changes_a_without_motion() {
    let source = "G54\nG21 G90\nG0 X0 Y0 Z30 A1080\nG92.4 A0 S0\nG1 A10 F100\n";
    let program = read(source).unwrap();
    let replay = replay(&program, Dialect::MakeraZ1).unwrap();
    let events: Vec<_> = replay
        .steps
        .iter()
        .filter_map(|s| {
            if let Step::Event(e) = s {
                Some(e)
            } else {
                None
            }
        })
        .collect();
    assert!(
        matches!(events[0], Event::DropTurns { from, to, .. } if *from == 1080.0 && *to == 0.0)
    );
    // LinuxCNC does not know the command.
    assert!(
        openchuck_gcode::replay(&program, Dialect::Linuxcnc)
            .unwrap_err()
            .message
            .contains("G92.4")
    );
    // Only the exact form is read.
    let bad = read("G92.4 A5 S0\n").unwrap();
    assert!(openchuck_gcode::replay(&bad, Dialect::MakeraZ1).is_err());
}

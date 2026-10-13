//! Post definitions as data: a template plus options (GC-03, GC-04). Throwaway shapes.

use serde::Serialize;

pub const ROTARY_POST: &str = include_str!("../posts/rotary.post");

#[derive(Clone, Debug, Serialize)]
pub struct Decimals {
    pub pos: usize,
    pub a: usize,
    pub f: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct LineNumbers {
    pub enabled: bool,
    pub start: i64,
    pub step: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToolChange {
    pub stop_spindle_first: bool,
    pub length_offset: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Options {
    pub comments: bool,
    pub header: Vec<String>,
    pub footer: Vec<String>,
    pub decimals: Decimals,
    pub line_numbers: LineNumbers,
    pub tool_change: ToolChange,
    /// "g94_linuxcnc", "g94_makera", "g93" or "naive".
    pub feed_mode: String,
    pub max_feed: Option<f64>,
    /// +1 or -1: multiplies the A word.
    pub a_direction: f64,
    /// "none" or "modulo360".
    pub wrap: String,
    /// "rapid" (an A-only G0 move) or "g92_4" (controller drops whole turns).
    pub unwind: String,
}

#[derive(Serialize)]
pub struct Ctx<'a> {
    pub program: &'a crate::program::Program,
    pub opt: &'a Options,
}

/// The SP-3 post, as options (byte-identical to SP-3's hard-coded Rust post).
pub fn sp3(feed_mode: &str) -> Options {
    Options {
        comments: true,
        header: vec!["G21 G90 G17 G54 G94".into()],
        footer: vec!["M5".into(), "M30".into()],
        decimals: Decimals { pos: 4, a: 4, f: 3 },
        line_numbers: LineNumbers {
            enabled: false,
            start: 10,
            step: 10,
        },
        tool_change: ToolChange {
            stop_spindle_first: false,
            length_offset: false,
        },
        feed_mode: feed_mode.into(),
        max_feed: None,
        a_direction: 1.0,
        wrap: "none".into(),
        unwind: "rapid".into(),
    }
}

/// A Makera Z1 style variant with the conventions of decision 0007 (all derived, unconfirmed).
pub fn makera() -> Options {
    Options {
        comments: true,
        header: vec!["G90 G21 G17".into(), "G54".into()],
        footer: vec!["M5".into(), "G53 G0 Z-3".into(), "G28".into(), "M2".into()],
        decimals: Decimals { pos: 3, a: 3, f: 1 },
        line_numbers: LineNumbers {
            enabled: false,
            start: 10,
            step: 10,
        },
        tool_change: ToolChange {
            stop_spindle_first: true,
            length_offset: false,
        },
        feed_mode: "g94_makera".into(),
        max_feed: Some(1200.0),
        a_direction: 1.0,
        wrap: "none".into(),
        unwind: "g92_4".into(),
    }
}

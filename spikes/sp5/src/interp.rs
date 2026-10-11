// SPDX-License-Identifier: AGPL-3.0-or-later
//! Minimal modal replay over the neutral word model: enough to turn words into reached
//! XYZA positions with the feed mode in force, and to name any code it does not know.
//! This is the part `openchuck-gcode` would own whatever reader is chosen.

use crate::model::{Parsed, ReadError};

/// G codes a rotary post is expected to emit (the hand-written fixture covers each).
pub const KNOWN_G: &[&str] = &[
    "0", "1", "2", "3", "4", "17", "18", "19", "20", "21", "28", "40", "43", "49", "53", "54", "55", "56", "57",
    "58", "59", "61", "61.1", "64", "90", "90.1", "91", "91.1", "93", "94",
];
pub const KNOWN_M: &[&str] = &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "30"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    Rapid,
    Linear,
    ArcCw,
    ArcCcw,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub line: usize,
    pub motion: Motion,
    pub to: [f64; 4],
    pub feed: Option<f64>,
    pub inverse_time: bool,
    pub metric: bool,
}

fn code_key(v: f64, text: &str) -> String {
    // Read from the source text so G01, G1 and G1.0 agree and G61.1 stays distinct.
    let v = text.parse::<f64>().unwrap_or(v);
    if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") }
}

pub fn replay(p: &Parsed) -> Result<Vec<Move>, ReadError> {
    let mut pos = [0.0f64; 4];
    let mut motion = Motion::Rapid;
    let mut absolute = true;
    let mut inverse = false;
    let mut metric = true;
    let mut feed: Option<f64> = None;
    let mut moves = Vec::new();
    for b in &p.blocks {
        let mut target: [Option<f64>; 4] = [None; 4];
        let mut non_modal = false; // G4, G28, G53 consume the block's words differently
        let mut g53 = false;
        for w in &b.words {
            let err = |m: String| ReadError { line: w.line, col: w.col, message: m };
            match w.letter {
                'G' => {
                    let k = code_key(w.value, &w.text);
                    if !KNOWN_G.contains(&k.as_str()) {
                        return Err(err(format!("G{k} is not in the post subset")));
                    }
                    match k.as_str() {
                        "0" => motion = Motion::Rapid,
                        "1" => motion = Motion::Linear,
                        "2" => motion = Motion::ArcCw,
                        "3" => motion = Motion::ArcCcw,
                        "4" | "28" => non_modal = true,
                        "53" => g53 = true,
                        "90" => absolute = true,
                        "91" => absolute = false,
                        "93" => inverse = true,
                        "94" => inverse = false,
                        "20" => metric = false,
                        "21" => metric = true,
                        _ => {}
                    }
                }
                'M' => {
                    let k = code_key(w.value, &w.text);
                    if !KNOWN_M.contains(&k.as_str()) {
                        return Err(err(format!("M{k} is not in the post subset")));
                    }
                }
                'X' => target[0] = Some(w.value),
                'Y' => target[1] = Some(w.value),
                'Z' => target[2] = Some(w.value),
                'A' => target[3] = Some(w.value),
                'F' => feed = Some(w.value),
                'N' | 'S' | 'T' | 'H' | 'P' | 'Q' | 'I' | 'J' | 'K' | 'R' | 'D' | 'L' => {}
                other => return Err(err(format!("word `{other}` is not in the post subset"))),
            }
        }
        if non_modal || target.iter().all(Option::is_none) {
            continue;
        }
        let from = pos;
        for (i, t) in target.iter().enumerate() {
            if let Some(v) = t {
                pos[i] = if absolute || g53 { *v } else { from[i] + v };
            }
        }
        if g53 {
            // Machine coordinates: recorded as reached, but outside the work frame.
        }
        let line = b.words.first().map_or(b.line, |w| w.line);
        moves.push(Move {
            line,
            motion: if g53 { Motion::Rapid } else { motion },
            to: pos,
            feed: if motion == Motion::Rapid { None } else { feed },
            inverse_time: inverse,
            metric,
        });
    }
    Ok(moves)
}

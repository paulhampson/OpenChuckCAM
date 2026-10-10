//! Replay reader built on the `gcode` 0.7.0 crate (SP-5 overlap).
//!
//! Finding: the crate stores every numeric word as `f32`. We use it for structure and spans,
//! then re-parse the number text from the span as f64. The helper `f32_value` is kept so the
//! precision loss can be measured.

use gcode::{Code, Value};

#[derive(Clone, Copy, Debug, Default)]
pub struct Pos {
    pub xyz: [f64; 3],
    pub a: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MoveKind {
    Rapid,
    Feed,
}

#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub line: usize,
    pub kind: MoveKind,
    pub from: Pos,
    pub to: Pos,
    /// F word in force, and whether the program was in G93 when the move ran.
    pub f: Option<f64>,
    pub inverse_time: bool,
    /// Same word read through the crate's f32 value (for the precision finding).
    pub to_f32: Pos,
}

#[derive(Debug)]
pub struct Replay {
    pub moves: Vec<Move>,
    pub codes_seen: Vec<String>,
    pub comments: usize,
}

fn word_f64(src: &str, span: gcode::core::Span) -> Option<f64> {
    let t = src.get(span.start..span.end())?;
    t[1..].trim().parse().ok() // drop the letter
}

fn f32_value(v: &Value) -> Option<f64> {
    match v {
        Value::Literal(n) => Some(f64::from(*n)),
        _ => None,
    }
}

pub fn replay(src: &str) -> Result<Replay, String> {
    let prog = gcode::parse(src).map_err(|d| format!("parse diagnostics: {d:?}"))?;
    let mut pos = Pos::default();
    let mut pos32 = Pos::default();
    let mut kind = MoveKind::Rapid;
    let mut inverse = false;
    let mut f: Option<f64> = None;
    let mut moves = Vec::new();
    let mut codes_seen: Vec<String> = Vec::new();
    let mut comments = 0;
    for b in &prog.blocks {
        comments += b.comments.len();
        let mut words: Vec<(char, f64, f64)> = Vec::new(); // letter, f64, f32 view
        let mut has_motion_code = false;
        for c in &b.codes {
            let (num, args) = match c {
                Code::General(g) => {
                    let n = g.number.to_string();
                    codes_seen.push(format!("G{n}"));
                    match n.as_str() {
                        "0" => { kind = MoveKind::Rapid; has_motion_code = true; }
                        "1" => { kind = MoveKind::Feed; has_motion_code = true; }
                        "93" => inverse = true,
                        "94" => inverse = false,
                        _ => {}
                    }
                    (n, &g.args)
                }
                Code::Miscellaneous(m) => { codes_seen.push(format!("M{}", m.number)); (m.number.to_string(), &m.args) }
                Code::ToolChange(t) => { codes_seen.push(format!("T{}", t.number)); (t.number.to_string(), &t.args) }
                _ => continue,
            };
            let _ = num;
            for a in args {
                if let (Some(v), Some(v32)) = (word_f64(src, a.span), f32_value(&a.value)) {
                    words.push((a.letter, v, v32));
                }
            }
        }
        // Words that are not attached to a G/M/T code on the line (modal motion).
        for w in &b.word_addresses {
            if let (Some(v), Some(v32)) = (word_f64(src, w.span), f32_value(&w.value)) {
                words.push((w.letter, v, v32));
            }
        }
        let _ = has_motion_code;
        let from = pos;
        let mut moved = false;
        for (l, v, v32) in &words {
            match l {
                'X' => { pos.xyz[0] = *v; pos32.xyz[0] = *v32; moved = true; }
                'Y' => { pos.xyz[1] = *v; pos32.xyz[1] = *v32; moved = true; }
                'Z' => { pos.xyz[2] = *v; pos32.xyz[2] = *v32; moved = true; }
                'A' => { pos.a = *v; pos32.a = *v32; moved = true; }
                'F' => f = Some(*v),
                _ => {}
            }
        }
        if moved {
            moves.push(Move { line: b.span.line + 1, kind, from, to: pos, f, inverse_time: inverse, to_f32: pos32 });
        }
    }
    Ok(Replay { moves, codes_seen, comments })
}

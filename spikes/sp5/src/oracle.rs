// SPDX-License-Identifier: AGPL-3.0-or-later
//! Independent truth for the comparison, and generated inputs.

use crate::model::Word;

/// For files where every word is separated by whitespace (our posts' output and the
/// hand-written subset file): strip comments, split on whitespace, letter + number.
/// Returns None when a file is outside that shape.
pub fn split(src: &str) -> Option<Vec<Word>> {
    let mut out = Vec::new();
    for (li, line) in src.lines().enumerate() {
        let mut chars: Vec<(usize, char)> = line.chars().enumerate().collect();
        // Remove comments, keeping column numbers.
        let mut in_paren = false;
        chars.retain(|(_, c)| {
            if in_paren {
                if *c == ')' {
                    in_paren = false;
                }
                return false;
            }
            if *c == '(' {
                in_paren = true;
                return false;
            }
            true
        });
        if let Some(p) = chars.iter().position(|(_, c)| *c == ';') {
            chars.truncate(p);
        }
        let mut i = 0;
        while i < chars.len() {
            if chars[i].1.is_whitespace() {
                i += 1;
                continue;
            }
            let start = i;
            while i < chars.len() && !chars[i].1.is_whitespace() {
                i += 1;
            }
            let tok: String = chars[start..i].iter().map(|(_, c)| *c).collect();
            if tok == "%" {
                continue;
            }
            let mut it = tok.chars();
            let letter = it.next()?;
            let text: String = it.collect();
            if !letter.is_ascii_alphabetic() {
                return None;
            }
            let value = text.parse::<f64>().ok()?;
            out.push(Word { letter: letter.to_ascii_uppercase(), value, text, line: li + 1, col: chars[start].0 + 1 });
        }
    }
    Some(out)
}

pub fn compare(truth: &[Word], got: &[Word]) -> Result<(), String> {
    if truth.len() != got.len() {
        return Err(format!("{} words, expected {}", got.len(), truth.len()));
    }
    for (t, g) in truth.iter().zip(got) {
        if t.letter != g.letter || t.value.to_bits() != g.value.to_bits() || t.line != g.line || t.col != g.col {
            return Err(format!(
                "line {} col {}: expected {}{} at {}:{}, got {}{} at {}:{}",
                t.line, t.col, t.letter, t.value, t.line, t.col, g.letter, g.value, g.line, g.col
            ));
        }
    }
    Ok(())
}

/// Values where f32 storage would show: long unwrapped A, large linear values, fine feeds.
pub fn stress_program() -> String {
    [
        "G1 X600.0001 Y-299.9999 Z0.0001 A-12345.6789 F0.123",
        "X123.4567 A98765.4321",
        "A0.0001",
        "X-0.00005 A359.99995",
        "Y1000.0001 A-1080.0001",
        "F12345.678",
    ]
    .join("\n")
}

/// Inputs a reader must refuse (or read correctly) without crashing.
pub fn malformed() -> Vec<(&'static str, &'static str)> {
    vec![
        ("sign then sign", "G1 X+-1.0"),
        ("double minus", "G1 X--1"),
        ("letter with no number", "G1 X"),
        ("two decimal points", "G1 X1.2.3"),
        ("unclosed comment", "G1 X1 (no close"),
        ("non-ASCII in a comment", "G1 X1 (Ä°)"),
        ("non-ASCII outside a comment", "G1 X1 Ä"),
        ("CRLF line endings", "G1 X1\r\nA2\r\n"),
        ("huge number", "G1 X1e400"),
        ("very long number", "G1 X123456789012345678901234567890.5"),
        ("lowercase", "g1 x1.5 a-2"),
        ("no space between words", "G1X1.5A-2"),
        ("spaces inside a word", "G 1 X 1 . 5"),
        ("two-digit G minor", "G1.25 X1"),
        ("huge G number", "G99999999999 X1"),
        ("huge line number", "N99999999999 G1 X1"),
        ("leading zeros", "G01 X01.500"),
        ("file ending in % and a newline", "%\nG1 X1\n%\n"),
        ("file ending in % without a newline", "%\nG1 X1\n%"),
        ("block delete", "/G0 X99"),
    ]
}

/// SP-3-style program of `n` lines.
pub fn big_program(n: usize) -> String {
    let mut s = String::with_capacity(n * 32);
    s.push_str("G21 G90 G17 G54 G94\nG0 X0 Y0 Z5 A0\nG1 Z-0.2 F100\n");
    for i in 0..n {
        let t = i as f64 * 0.001;
        s.push_str(&format!("X{:.4} A{:.4} F{:.3}\n", 30.0 + 10.0 * t.sin(), -(i as f64) * 0.5, 200.0 + 40.0 * t.cos()));
    }
    s.push_str("M30\n");
    s
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! The three candidates behind one interface:
//! - `gcode` 0.7.0, values as the crate gives them (f32),
//! - `gcode` 0.7.0 with an f64 layer (numbers re-read from the span text),
//! - `g-code` 0.6.0 (rust_decimal values),
//! - a small in-house reader for the subset our posts emit.

use crate::model::{Block, LineIndex, Parsed, ReadError, Reader, Word};

fn strip_ws(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

// ---------------------------------------------------------------- gcode 0.7.0

pub struct GcodeCrate {
    /// true: re-read the number text as f64; false: use the crate's f32 value.
    pub f64_layer: bool,
}

impl Reader for GcodeCrate {
    fn name(&self) -> &'static str {
        if self.f64_layer { "gcode 0.7.0 + f64 layer" } else { "gcode 0.7.0 (f32 values)" }
    }

    fn read(&self, src: &str) -> Result<Parsed, Vec<ReadError>> {
        let idx = LineIndex::new(src);
        let prog = match gcode::parse(src) {
            Ok(p) => p,
            Err(d) => {
                return Err(d
                    .iter()
                    .map(|d| {
                        let (line, col) = idx.locate(src, d.span.start);
                        ReadError { line, col, message: format!("{}", d.kind) }
                    })
                    .collect());
            }
        };
        let mut out = Parsed::default();
        let mut errs = Vec::new();
        for b in &prog.blocks {
            let (line, _) = idx.locate(src, b.span.start);
            let mut block = Block { line, ..Block::default() };
            block.comments = b.comments.iter().map(|c| c.value.clone()).collect();
            // (offset, word) so the words come back in source order.
            let mut words: Vec<(usize, Word)> = Vec::new();
            if let Some(n) = b.line_number {
                // The AST keeps N as a number only, with no span of its own; use the block start.
                let (l, c) = idx.locate(src, b.span.start);
                words.push((b.span.start, Word { letter: 'N', value: f64::from(n), text: n.to_string(), line: l, col: c }));
            }
            let push = |words: &mut Vec<(usize, Word)>, errs: &mut Vec<ReadError>, letter: char, start: usize, len: usize, val: Option<f64>| {
                let raw = src.get(start..start + len).unwrap_or("");
                // Span text starts with the letter; drop it.
                let text = strip_ws(raw.get(1..).unwrap_or(""));
                let (l, c) = idx.locate(src, start);
                let value = if self.f64_layer { text.parse::<f64>().ok() } else { val };
                match value {
                    Some(value) => words.push((start, Word { letter: letter.to_ascii_uppercase(), value, text, line: l, col: c })),
                    None => errs.push(ReadError { line: l, col: c, message: format!("no numeric value in `{raw}`") }),
                }
            };
            for code in &b.codes {
                let (letter, number, span, args) = match code {
                    gcode::Code::General(g) => ('G', g.number, g.span, &g.args),
                    gcode::Code::Miscellaneous(m) => ('M', m.number, m.span, &m.args),
                    gcode::Code::ToolChange(t) => ('T', t.number, t.span, &t.args),
                    _ => continue,
                };
                let num_text = number.to_string();
                let (l, c) = idx.locate(src, span.start);
                words.push((
                    span.start,
                    Word { letter, value: num_text.parse().unwrap_or(f64::NAN), text: num_text, line: l, col: c },
                ));
                for a in args {
                    let v32 = match &a.value {
                        gcode::Value::Literal(v) => Some(f64::from(*v)),
                        _ => None,
                    };
                    push(&mut words, &mut errs, a.letter, a.span.start, a.span.length, v32);
                }
            }
            for w in &b.word_addresses {
                let v32 = match &w.value {
                    gcode::Value::Literal(v) => Some(f64::from(*v)),
                    _ => None,
                };
                push(&mut words, &mut errs, w.letter, w.span.start, w.span.length, v32);
            }
            words.sort_by_key(|(o, _)| *o);
            block.words = words.into_iter().map(|(_, w)| w).collect();
            out.blocks.push(block);
        }
        if errs.is_empty() { Ok(out) } else { Err(errs) }
    }
}

// ---------------------------------------------------------------- g-code 0.6.0

pub struct GCodeCrate;

impl Reader for GCodeCrate {
    fn name(&self) -> &'static str {
        "g-code 0.6.0"
    }

    fn read(&self, src: &str) -> Result<Parsed, Vec<ReadError>> {
        use g_code::parse::ast::Spanned;
        let idx = LineIndex::new(src);
        let file = match g_code::parse::file_parser(src) {
            Ok(f) => f,
            Err(e) => {
                let (line, col) = idx.locate(src, e.location.offset);
                return Err(vec![ReadError { line, col, message: format!("expected {}", e.expected) }]);
            }
        };
        let mut out = Parsed::default();
        let mut errs = Vec::new();
        for l in file.iter() {
            let (line, _) = idx.locate(src, l.span().0);
            let mut block = Block { line, ..Block::default() };
            block.comments = l.iter_inline_comments().map(|c| format!("{c:?}")).collect();
            for f in l.iter_fields() {
                let sp = f.span();
                let (fl, fc) = idx.locate(src, sp.0);
                let ef = g_code::emit::Field::from(f);
                let mut letters = ef.letters.chars();
                let (Some(letter), None) = (letters.next(), letters.next()) else {
                    errs.push(ReadError { line: fl, col: fc, message: format!("multi-letter word `{}`", ef.letters) });
                    continue;
                };
                let text = strip_ws(src.get(sp.0 + 1..sp.1).unwrap_or(""));
                let value = match &ef.value {
                    g_code::emit::Value::Rational(d) => d.to_string().parse::<f64>().ok(),
                    g_code::emit::Value::Integer(i) => Some(*i as f64),
                    g_code::emit::Value::Float(f) => Some(*f),
                    g_code::emit::Value::String(_) => None,
                };
                match value {
                    Some(value) => block.words.push(Word { letter: letter.to_ascii_uppercase(), value, text, line: fl, col: fc }),
                    None => errs.push(ReadError { line: fl, col: fc, message: "non-numeric value".into() }),
                }
            }
            for fl in l.iter_flags() {
                let (a, b) = idx.locate(src, fl.span().0);
                errs.push(ReadError { line: a, col: b, message: "letter with no value".into() });
            }
            out.blocks.push(block);
        }
        // Line comments (`;`) are stored per line but not reachable per line from outside the crate.
        if let Some(first) = out.blocks.first_mut() {
            first.comments.extend(file.iter_comments().map(|c| format!("{c:?}")));
        }
        if errs.is_empty() { Ok(out) } else { Err(errs) }
    }
}

// ---------------------------------------------------------------- in-house

/// Reads the subset our posts emit, as LinuxCNC reads it: case-insensitive letters, spaces
/// allowed inside words, `(...)` and `;` comments, `%` delimiters, `/` block delete, `N` numbers.
/// Parameters (`#`), expressions (`[`) and O-words are refused with an error naming the place.
pub struct InHouse;

impl Reader for InHouse {
    fn name(&self) -> &'static str {
        "in-house subset reader"
    }

    fn read(&self, src: &str) -> Result<Parsed, Vec<ReadError>> {
        let mut out = Parsed::default();
        let mut errs = Vec::new();
        for (li, raw_line) in src.split('\n').enumerate() {
            let line_no = li + 1;
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            match read_line(line, line_no) {
                Ok(Some(b)) => out.blocks.push(b),
                Ok(None) => {}
                Err(e) => errs.push(e),
            }
        }
        if errs.is_empty() { Ok(out) } else { Err(errs) }
    }
}

fn read_line(line: &str, line_no: usize) -> Result<Option<Block>, ReadError> {
    let chars: Vec<char> = line.chars().collect();
    let err = |i: usize, m: String| ReadError { line: line_no, col: i + 1, message: m };
    let mut block = Block { line: line_no, ..Block::default() };
    let mut i = 0;
    let skip_ws = |i: &mut usize| {
        while chars.get(*i).is_some_and(|c| *c == ' ' || *c == '\t') {
            *i += 1;
        }
    };
    skip_ws(&mut i);
    if chars.get(i) == Some(&'%') {
        if chars[i + 1..].iter().all(|c| c.is_whitespace()) {
            return Ok(None);
        }
        return Err(err(i, "`%` must be alone on its line".into()));
    }
    if chars.get(i) == Some(&'/') {
        // Block delete: kept, and visible to the caller as a comment marker.
        block.comments.push("/".into());
        i += 1;
    }
    while i < chars.len() {
        let Some(&c) = chars.get(i) else { break };
        match c {
            ' ' | '\t' => i += 1,
            '(' => {
                let start = i;
                let Some(end) = chars[i..].iter().position(|c| *c == ')') else {
                    return Err(err(start, "comment not closed with `)`".into()));
                };
                let body: String = chars[i + 1..i + end].iter().collect();
                if body.contains('(') {
                    return Err(err(start, "nested `(` in a comment".into()));
                }
                block.comments.push(body);
                i += end + 1;
            }
            ';' => {
                block.comments.push(chars[i + 1..].iter().collect());
                break;
            }
            '#' => return Err(err(i, "parameters (`#`) are not part of the post subset".into())),
            '[' => return Err(err(i, "expressions (`[`) are not part of the post subset".into())),
            'o' | 'O' => return Err(err(i, "O-words are not part of the post subset".into())),
            c if c.is_ascii_alphabetic() => {
                let start = i;
                let letter = c.to_ascii_uppercase();
                i += 1;
                let mut text = String::new();
                loop {
                    skip_ws(&mut i);
                    match chars.get(i) {
                        Some(&d) if d.is_ascii_digit() || d == '.' => text.push(d),
                        Some(&s) if (s == '+' || s == '-') && text.is_empty() => text.push(s),
                        _ => break,
                    }
                    i += 1;
                }
                let digits = text.chars().filter(|c| c.is_ascii_digit()).count();
                let dots = text.chars().filter(|c| *c == '.').count();
                if digits == 0 || dots > 1 {
                    return Err(err(start, format!("`{letter}` needs a number, found `{text}`")));
                }
                let value: f64 = text
                    .parse()
                    .map_err(|_| err(start, format!("`{letter}{text}` is not a number")))?;
                if !value.is_finite() {
                    return Err(err(start, format!("`{letter}{text}` is out of range")));
                }
                block.words.push(Word { letter, value, text, line: line_no, col: start + 1 });
            }
            other => return Err(err(i, format!("unexpected character `{other}`"))),
        }
    }
    if block.words.is_empty() && block.comments.is_empty() {
        return Ok(None);
    }
    Ok(Some(block))
}

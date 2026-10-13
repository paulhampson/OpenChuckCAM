// SPDX-License-Identifier: AGPL-3.0-or-later
//! The in-house G-code reader (decision record 0005; slice 6 item 7).
//!
//! Reads the subset our posts emit, the way LinuxCNC reads it: letters in
//! either case, spaces allowed inside a word, `(...)` and `;` comments, `%`
//! delimiters, `/` block delete and `N` numbers. Values are `f64` from the
//! source text. Every error names the line and column. Parameters (`#`),
//! expressions (`[`) and O-words are refused by name.

/// One address word (`X10.5`, `G90.1`, `N120`) with its source position.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    /// The letter, upper case.
    pub letter: char,
    /// The value, read from the source text as `f64`.
    pub value: f64,
    /// The number as written, spaces removed.
    pub text: String,
    /// One-based line.
    pub line: usize,
    /// One-based column of the letter.
    pub col: usize,
}

/// One line of the program.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Block {
    /// One-based line.
    pub line: usize,
    /// The words in order.
    pub words: Vec<Word>,
    /// Comment texts, without their delimiters.
    pub comments: Vec<String>,
    /// The line started with `/`.
    pub block_delete: bool,
}

/// A read program.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Program {
    /// Blocks with words or comments; blank lines and `%` are left out.
    pub blocks: Vec<Block>,
}

/// A reader error that names where it happened.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("line {line}, column {col}: {message}")]
pub struct ReadError {
    /// One-based line.
    pub line: usize,
    /// One-based column.
    pub col: usize,
    /// What is wrong.
    pub message: String,
}

/// Longest line read, characters (NFR-07).
pub const MAX_LINE_CHARS: usize = 4096;

/// Reads a program. All errors are collected, one per bad line.
pub fn read(source: &str) -> std::result::Result<Program, Vec<ReadError>> {
    let mut program = Program::default();
    let mut errors = Vec::new();
    for (index, raw) in source.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        match read_line(line, index + 1) {
            Ok(Some(block)) => program.blocks.push(block),
            Ok(None) => {}
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() {
        Ok(program)
    } else {
        Err(errors)
    }
}

fn read_line(line: &str, line_no: usize) -> std::result::Result<Option<Block>, ReadError> {
    let chars: Vec<char> = line.chars().collect();
    let err = |i: usize, message: String| ReadError {
        line: line_no,
        col: i + 1,
        message,
    };
    if chars.len() > MAX_LINE_CHARS {
        return Err(err(
            MAX_LINE_CHARS,
            format!("the line is longer than {MAX_LINE_CHARS} characters"),
        ));
    }
    let mut block = Block {
        line: line_no,
        ..Block::default()
    };
    let mut i = 0;
    let skip_space = |i: &mut usize| {
        while chars.get(*i).is_some_and(|c| *c == ' ' || *c == '\t') {
            *i += 1;
        }
    };
    skip_space(&mut i);
    if chars.get(i) == Some(&'%') {
        if chars.iter().skip(i + 1).all(|c| c.is_whitespace()) {
            return Ok(None);
        }
        return Err(err(i, "`%` must be alone on its line".into()));
    }
    if chars.get(i) == Some(&'/') {
        block.block_delete = true;
        i += 1;
    }
    while let Some(&c) = chars.get(i) {
        match c {
            ' ' | '\t' => i += 1,
            '(' => {
                let rest = chars.get(i + 1..).unwrap_or_default();
                let Some(end) = rest.iter().position(|c| *c == ')') else {
                    return Err(err(i, "comment not closed with `)`".into()));
                };
                let body: String = rest.iter().take(end).collect();
                if body.contains('(') {
                    return Err(err(i, "nested `(` in a comment".into()));
                }
                block.comments.push(body);
                i += end + 2;
            }
            ';' => {
                block.comments.push(chars.iter().skip(i + 1).collect());
                break;
            }
            '#' => {
                return Err(err(
                    i,
                    "parameters (`#`) are not part of the post subset".into(),
                ));
            }
            '[' => {
                return Err(err(
                    i,
                    "expressions (`[`) are not part of the post subset".into(),
                ));
            }
            'o' | 'O' => return Err(err(i, "O-words are not part of the post subset".into())),
            c if c.is_ascii_alphabetic() => {
                let start = i;
                let letter = c.to_ascii_uppercase();
                i += 1;
                let mut text = String::new();
                loop {
                    skip_space(&mut i);
                    match chars.get(i) {
                        Some(&d) if d.is_ascii_digit() || d == '.' => text.push(d),
                        Some(&s) if (s == '+' || s == '-') && text.is_empty() => text.push(s),
                        _ => break,
                    }
                    i += 1;
                }
                let digits = text.chars().filter(char::is_ascii_digit).count();
                let dots = text.chars().filter(|c| *c == '.').count();
                if digits == 0 || dots > 1 {
                    return Err(err(
                        start,
                        format!("`{letter}` needs a number, found `{text}`"),
                    ));
                }
                let value: f64 = text
                    .parse()
                    .map_err(|_| err(start, format!("`{letter}{text}` is not a number")))?;
                if !value.is_finite() {
                    return Err(err(start, format!("`{letter}{text}` is out of range")));
                }
                block.words.push(Word {
                    letter,
                    value,
                    text,
                    line: line_no,
                    col: start + 1,
                });
            }
            other => return Err(err(i, format!("unexpected character `{other}`"))),
        }
    }
    if block.words.is_empty() && block.comments.is_empty() {
        return Ok(None);
    }
    Ok(Some(block))
}

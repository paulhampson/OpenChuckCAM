// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reader-neutral word model, so the three candidates can be compared on the same terms.

/// One address word (`X10.5`, `G90.1`, `N120`) with its source position.
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub letter: char,
    /// Value as the reader delivers it to its caller.
    pub value: f64,
    /// The exact source text of the number, whitespace removed (for the precision check).
    pub text: String,
    /// One-based line and column of the letter.
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    pub line: usize,
    pub words: Vec<Word>,
    pub comments: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    pub blocks: Vec<Block>,
}

impl Parsed {
    pub fn words(&self) -> impl Iterator<Item = &Word> {
        self.blocks.iter().flat_map(|b| b.words.iter())
    }
    pub fn comment_count(&self) -> usize {
        self.blocks.iter().map(|b| b.comments.len()).sum()
    }
}

/// A reader error that names where it happened.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}, column {}: {}", self.line, self.col, self.message)
    }
}

pub trait Reader {
    fn name(&self) -> &'static str;
    fn read(&self, src: &str) -> Result<Parsed, Vec<ReadError>>;
}

/// Byte offset to one-based (line, column), column counted in characters.
pub struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(src: &str) -> Self {
        let mut starts = vec![0];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self { starts }
    }
    pub fn locate(&self, src: &str, offset: usize) -> (usize, usize) {
        let line = match self.starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let start = self.starts.get(line).copied().unwrap_or(0);
        let col = src.get(start..offset).map_or(1, |s| s.chars().count() + 1);
        (line + 1, col)
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! The read-only preview of the program files the export would write
//! (GC-05, GC-07, NFR-06, NFR-07; decision 0327, issue 312).
//!
//! A preview file holds no copy of the text: it points at the program text
//! of the run, which is also what the write takes its bytes from, so what is
//! shown is what is written. A line index (one offset per line) gives random
//! access to any line without scanning, which keeps a million-line program
//! cheap to scroll.

use std::sync::Arc;

use crate::output_names::file_hash;
use crate::{AppError, Regeneration};

/// Where each line of a text starts. Lines end at `\n`; a `\r` before it is
/// not part of the line, as with [`str::lines`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineIndex {
    starts: Vec<u32>,
}

impl LineIndex {
    /// Indexes `text`. A text over 4 GiB is refused (NFR-07).
    pub fn new(text: &str) -> Result<Self, AppError> {
        if u32::try_from(text.len()).is_err() {
            return Err(AppError::Post(
                "A program file over 4 GiB cannot be previewed.".to_owned(),
            ));
        }
        let bytes = text.as_bytes();
        let mut starts = Vec::with_capacity(bytes.len() / 24 + 1);
        let mut at = 0usize;
        for (i, b) in bytes.iter().enumerate() {
            if *b == b'\n' {
                if at < bytes.len() {
                    starts.push(u32::try_from(at).unwrap_or(u32::MAX));
                }
                at = i + 1;
            }
        }
        if at < bytes.len() {
            starts.push(u32::try_from(at).unwrap_or(u32::MAX));
        }
        starts.shrink_to_fit();
        Ok(Self { starts })
    }

    /// The number of lines.
    pub fn len(&self) -> usize {
        self.starts.len()
    }

    /// Whether the text is empty.
    pub fn is_empty(&self) -> bool {
        self.starts.is_empty()
    }

    /// The line (counting from 0) that holds byte `offset`.
    pub fn line_at(&self, offset: usize) -> usize {
        self.starts
            .partition_point(|start| *start as usize <= offset)
            .saturating_sub(1)
    }

    fn offset(&self, n: usize) -> Option<usize> {
        self.starts.get(n).map(|s| *s as usize)
    }

    /// Line `n` (counting from 0) of `text`, which must be the text indexed.
    pub fn line<'a>(&self, text: &'a str, n: usize) -> Option<&'a str> {
        let start = *self.starts.get(n)? as usize;
        let end = self
            .starts
            .get(n + 1)
            .map_or(text.len(), |next| *next as usize);
        let line = text.get(start..end)?;
        let line = line.strip_suffix('\n').unwrap_or(line);
        Some(line.strip_suffix('\r').unwrap_or(line))
    }
}

/// One program file as it would be written.
#[derive(Debug, Clone)]
pub struct PreviewFile {
    run: Arc<Regeneration>,
    source: usize,
    name: String,
    sha256: String,
    lines: LineIndex,
}

impl PreviewFile {
    /// The final name, numbered if it clashed with another (decision 0267).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The exact bytes the write puts in the file.
    pub fn bytes(&self) -> &[u8] {
        self.text().as_bytes()
    }

    /// The size in bytes.
    pub fn size(&self) -> usize {
        self.text().len()
    }

    /// The SHA-256 of the bytes, 64 hexadecimal digits, as the setup sheet
    /// lists it (decision 0267 item 5).
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// The number of lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Line `n` counting from 0, without its line ending.
    pub fn line(&self, n: usize) -> Option<&str> {
        self.lines.line(self.text(), n)
    }

    /// The next line (counting from 0) after `from`, or before it when
    /// `forward` is false, that holds `needle`; the search wraps round the
    /// file. Letter case counts. `from` may be `None` to start at the top
    /// (or the bottom, going back). One plain scan of the text: about 25 ms
    /// for a million lines (decision 0327).
    pub fn find(&self, needle: &str, from: Option<usize>, forward: bool) -> Option<usize> {
        if needle.is_empty() || self.lines.is_empty() {
            return None;
        }
        let text = self.text();
        let last = self.lines.len() - 1;
        let start_line = match (from, forward) {
            (Some(n), true) => n.saturating_add(1),
            (Some(n), false) => n,
            (None, true) => 0,
            (None, false) => self.lines.len(),
        };
        let offset_of = |n: usize| self.lines.offset(n).unwrap_or(text.len());
        let hit = |offset: usize| self.lines.line_at(offset).min(last);
        if forward {
            let begin = offset_of(start_line.min(self.lines.len()));
            let ahead = text
                .get(begin..)
                .and_then(|t| t.find(needle).map(|i| begin + i));
            let found = ahead.or_else(|| text.get(..begin).and_then(|t| t.find(needle)));
            found.map(hit)
        } else {
            let end = offset_of(start_line.min(self.lines.len()));
            let behind = text.get(..end).and_then(|t| t.rfind(needle));
            let found = behind.or_else(|| {
                text.get(end..)
                    .and_then(|t| t.rfind(needle).map(|i| end + i))
            });
            found.map(hit)
        }
    }

    fn text(&self) -> &str {
        self.run
            .gcode
            .get(self.source)
            .map_or("", |(_, text)| text.as_str())
    }
}

/// The program files of a run as the write would name and fill them.
#[derive(Debug, Clone, Default)]
pub struct OutputPreview {
    files: Vec<PreviewFile>,
}

impl OutputPreview {
    pub(crate) fn new(
        run: &Arc<Regeneration>,
        names: impl IntoIterator<Item = (usize, String)>,
    ) -> Result<Self, AppError> {
        let mut files = Vec::new();
        for (source, name) in names {
            let text = run
                .gcode
                .get(source)
                .map(|(_, text)| text.as_str())
                .ok_or_else(|| AppError::Post(format!("no G-code for '{name}'")))?;
            files.push(PreviewFile {
                run: Arc::clone(run),
                source,
                sha256: file_hash::hex(text.as_bytes()),
                lines: LineIndex::new(text)?,
                name,
            });
        }
        Ok(Self { files })
    }

    /// The files, in write order.
    pub fn files(&self) -> &[PreviewFile] {
        &self.files
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn lines(text: &str) -> Vec<&str> {
        let index = LineIndex::new(text).unwrap();
        (0..index.len())
            .map(|n| index.line(text, n).unwrap())
            .collect()
    }

    #[test]
    fn req_gc_07_the_line_index_agrees_with_str_lines() {
        for text in [
            "",
            "G0\n",
            "G0",
            "G0\nG1\n",
            "G0\r\nG1\r\n",
            "\n\nG1\n\n",
            "a\nb",
            "é\nü\n",
        ] {
            assert_eq!(lines(text), text.lines().collect::<Vec<_>>(), "{text:?}");
        }
    }

    #[test]
    fn req_nfr_07_a_line_past_the_end_is_none() {
        let index = LineIndex::new("a\nb\n").unwrap();
        assert!(index.line("a\nb\n", 2).is_none());
        assert!(LineIndex::new("").unwrap().is_empty());
    }
}

#[cfg(test)]
mod measurements {
    //! Cost of a million-line program (OI-1300, decision 0327). Run with
    //! `cargo test -p openchuck-app --release -- --ignored --nocapture measure_`.
    #![allow(clippy::unwrap_used, clippy::print_stderr)]
    use super::*;
    use std::time::Instant;

    fn program(lines: usize) -> String {
        let mut text = String::with_capacity(lines * 36);
        for i in 0..lines {
            use std::fmt::Write;
            let _ = writeln!(
                text,
                "G1 X{:.3} Y{:.3} Z{:.3} A{:.3} F800",
                i as f64 * 0.001,
                (i % 977) as f64 * 0.01,
                -1.5,
                (i % 360) as f64
            );
        }
        text
    }

    #[test]
    #[ignore = "measurement, not a check"]
    fn measure_a_million_line_program() {
        let text = program(1_000_000);
        let t = Instant::now();
        let index = LineIndex::new(&text).unwrap();
        let build = t.elapsed();
        let t = Instant::now();
        let hash = file_hash::hex(text.as_bytes());
        let hashed = t.elapsed();
        let t = Instant::now();
        let mut sum = 0usize;
        for n in (0..index.len()).step_by(997) {
            sum += index.line(&text, n).unwrap().len();
        }
        let access = t.elapsed();
        let t = Instant::now();
        let found = text.matches("Z-1.500 A359").count();
        let search = t.elapsed();
        eprintln!(
            "lines {} text {} MB index {} MB | index {:?} sha256 {:?} ({}) {} lookups {:?} (sum {sum}) | substring scan {:?} ({found} hits)",
            index.len(),
            text.len() / 1_000_000,
            index.len() * 4 / 1_000_000,
            build,
            hashed,
            &hash[..8],
            index.len() / 997,
            access,
            search,
        );
    }
}

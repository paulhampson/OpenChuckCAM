// SPDX-License-Identifier: AGPL-3.0-or-later
//! Repeated notes about consecutive moves, grouped where the results are
//! shown so they can be read (issue 255; decision record 0218).
//!
//! The engine reports one note per move ("operation 'Roughing': move 2: the
//! machine's feed limits slow this move: the contact feed is 972.9 mm/min,
//! not 1000.0"), and those notes stay as they are (GC-07, NFR-06): the
//! grouping reads them and changes none. Consecutive notes of the same
//! operation and the same wording, about consecutive moves, whose values are
//! close, become one group. Nothing is hidden (NFR-12): the group names the
//! first and last move and the count, gives each value or its range, and
//! keeps every note it holds.

/// How close the values of notes must be to group: within this share of the
/// largest of them (1 %). A value further off starts a new group, so a real
/// outlier is never buried in a range.
pub const CLOSE: f64 = 0.01;

/// One line of grouped notes.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteGroup {
    /// The line shown: the note itself when the group holds one, else the
    /// moves, their count and the values or their ranges.
    pub text: String,
    /// The notes in the group, in order, one per move.
    pub notes: Vec<String>,
}

/// Groups `notes`, in order. Notes that name no move stay on their own.
pub fn group_notes(notes: &[String]) -> Vec<NoteGroup> {
    let mut out = Vec::new();
    let mut open: Option<Run> = None;
    for note in notes {
        let parsed = parse(note);
        if let (Some(run), Some(p)) = (&mut open, &parsed)
            && run.takes(p)
        {
            run.add(p, note);
            continue;
        }
        if let Some(run) = open.take() {
            out.push(run.group());
        }
        match parsed {
            Some(p) => open = Some(Run::new(p, note)),
            None => out.push(NoteGroup {
                text: note.clone(),
                notes: vec![note.clone()],
            }),
        }
    }
    if let Some(run) = open {
        out.push(run.group());
    }
    out
}

/// A note about one move, read apart: what comes before the move, the move,
/// and the sentence after it with its values taken out.
struct Parsed {
    prefix: String,
    number: usize,
    /// The sentence after the move number, each value replaced by
    /// [`SLOT`].
    wording: String,
    /// The values, in order, as numbers and as written.
    values: Vec<(f64, String)>,
}

/// Where a value stood in [`Parsed::wording`].
const SLOT: char = '\u{0}';

/// Reads "<prefix>move <n>: <sentence>" or "<prefix>move <n> <sentence>",
/// where the prefix is empty or ends with ": ".
fn parse(note: &str) -> Option<Parsed> {
    let mut from = 0;
    while let Some(found) = note.get(from..)?.find("move ") {
        let at = from + found;
        from = at + 1;
        let before = note.get(..at)?;
        if !(before.is_empty() || before.ends_with(": ")) {
            continue;
        }
        let after = note.get(at + "move ".len()..)?;
        let digits = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let rest = after.get(digits..)?;
        if digits == 0 || !(rest.starts_with(':') || rest.starts_with(' ')) {
            continue;
        }
        let number = after.get(..digits)?.parse().ok()?;
        let (wording, values) = values(rest);
        return Some(Parsed {
            prefix: before.to_owned(),
            number,
            wording,
            values,
        });
    }
    None
}

/// `text` with each value replaced by [`SLOT`], and the values. A number
/// inside a word ("G54", "mm3") is part of the wording.
fn values(text: &str) -> (String, Vec<(f64, String)>) {
    let chars: Vec<char> = text.chars().collect();
    let mut wording = String::new();
    let mut values = Vec::new();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        let previous = i.checked_sub(1).and_then(|p| chars.get(p));
        let in_word = previous.is_some_and(|p| p.is_alphanumeric() || *p == '.' || *p == '-');
        let digit_at = |k: usize| chars.get(k).is_some_and(char::is_ascii_digit);
        let starts = !in_word && (c.is_ascii_digit() || (c == '-' && digit_at(i + 1)));
        if !starts {
            wording.push(c);
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while digit_at(i) {
            i += 1;
        }
        if chars.get(i) == Some(&'.') && digit_at(i + 1) {
            i += 1;
            while digit_at(i) {
                i += 1;
            }
        }
        let written: String = chars.get(start..i).unwrap_or_default().iter().collect();
        match written.parse::<f64>() {
            Ok(v) => {
                wording.push(SLOT);
                values.push((v, written));
            }
            Err(_) => wording.push_str(&written),
        }
    }
    (wording, values)
}

/// A group being gathered.
struct Run {
    prefix: String,
    wording: String,
    first: usize,
    last: usize,
    /// Each value's lowest and highest, as numbers and as written.
    ranges: Vec<((f64, String), (f64, String))>,
    notes: Vec<String>,
}

impl Run {
    fn new(p: Parsed, note: &str) -> Self {
        Self {
            ranges: p.values.into_iter().map(|v| (v.clone(), v)).collect(),
            prefix: p.prefix,
            wording: p.wording,
            first: p.number,
            last: p.number,
            notes: vec![note.to_owned()],
        }
    }

    /// Whether `p` continues the group: the same operation and wording, the
    /// next move, and every value within [`CLOSE`] of the group's.
    fn takes(&self, p: &Parsed) -> bool {
        p.prefix == self.prefix
            && p.wording == self.wording
            && Some(p.number) == self.last.checked_add(1)
            && p.values.len() == self.ranges.len()
            && self.ranges.iter().zip(&p.values).all(|((lo, hi), (v, _))| {
                let (lo, hi) = (lo.0.min(*v), hi.0.max(*v));
                hi - lo <= CLOSE * lo.abs().max(hi.abs())
            })
    }

    fn add(&mut self, p: &Parsed, note: &str) {
        self.last = p.number;
        for ((lo, hi), v) in self.ranges.iter_mut().zip(&p.values) {
            if v.0 < lo.0 {
                *lo = v.clone();
            }
            if v.0 > hi.0 {
                *hi = v.clone();
            }
        }
        self.notes.push(note.to_owned());
    }

    fn group(self) -> NoteGroup {
        let text = match self.notes.as_slice() {
            [one] => one.clone(),
            _ => {
                let mut ranges = self.ranges.iter();
                let mut sentence = String::new();
                for c in self.wording.chars() {
                    if c != SLOT {
                        sentence.push(c);
                    } else if let Some((lo, hi)) = ranges.next() {
                        if lo.1 == hi.1 {
                            sentence.push_str(&lo.1);
                        } else {
                            sentence.push_str(&format!("{} to {}", lo.1, hi.1));
                        }
                    }
                }
                let sentence = match sentence.strip_prefix(':') {
                    Some(after) => format!(":{}", after.replace("this move", "these moves")),
                    None => format!(": each{sentence}"),
                };
                format!(
                    "{}moves {} to {} ({} moves){sentence}",
                    self.prefix,
                    self.first,
                    self.last,
                    self.notes.len()
                )
            }
        };
        NoteGroup {
            text,
            notes: self.notes,
        }
    }
}

/// What the machine's feed limits did to an operation's feed in its last
/// run, read from the run's notes (FIRSTRUN item 5; NFR-12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedLimit {
    /// How many moves were slowed.
    pub moves: usize,
    /// The lowest feed any of them was cut to, in mm/min.
    pub lowest: f64,
    /// The feed the operation asked for, in mm/min.
    pub programmed: f64,
}

/// The feed-limit notes of `operation` in `notes`, as one summary; `None`
/// when none slowed it. The notes are read, never changed (GC-07).
pub fn feed_limit_summary(notes: &[String], operation: &str) -> Option<FeedLimit> {
    let start = format!("operation '{operation}': move ");
    let mut found: Option<FeedLimit> = None;
    for note in notes {
        if !note.starts_with(&start) || !note.contains("the machine's feed limits slow") {
            continue;
        }
        let Some(parsed) = parse(note) else { continue };
        let [(effective, _), (programmed, _)] = parsed.values.as_slice() else {
            continue;
        };
        found = Some(match found {
            None => FeedLimit {
                moves: 1,
                lowest: *effective,
                programmed: *programmed,
            },
            Some(f) => FeedLimit {
                moves: f.moves + 1,
                lowest: f.lowest.min(*effective),
                programmed: f.programmed,
            },
        });
    }
    found
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    fn feed(moves: std::ops::RangeInclusive<usize>, feed: f64) -> Vec<String> {
        moves
            .map(|m| {
                format!(
                    "operation 'Roughing': move {m}: the machine's feed limits slow this \
                     move: the contact feed is {feed:.1} mm/min, not 1000.0"
                )
            })
            .collect()
    }

    #[test]
    fn req_nfr_12_repeated_notes_about_consecutive_moves_become_one_group() {
        // The owner's example: ten lines that differ only by the move.
        let notes = feed(2..=11, 972.9);
        let groups = group_notes(&notes);
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].text,
            "operation 'Roughing': moves 2 to 11 (10 moves): the machine's feed limits slow \
             these moves: the contact feed is 972.9 mm/min, not 1000.0"
        );
        // Every note is kept, in order, to be shown when the group is opened.
        assert_eq!(groups[0].notes, notes);
    }

    #[test]
    fn req_nfr_12_close_values_give_a_range_and_a_value_far_off_starts_a_new_group() {
        // The owner's answer 1: slightly different values group into a range;
        // a value far from the others starts a new group.
        let mut notes = feed(2..=3, 972.9);
        notes.extend(feed(4..=4, 975.1));
        notes.extend(feed(5..=6, 500.0));
        let groups = group_notes(&notes);
        assert_eq!(groups.len(), 2, "{groups:?}");
        assert!(
            groups[0].text.contains(
                "moves 2 to 4 (3 moves): the machine's feed limits slow these moves: \
                     the contact feed is 972.9 to 975.1 mm/min, not 1000.0"
            ),
            "{}",
            groups[0].text
        );
        assert!(groups[1].text.contains("moves 5 to 6 (2 moves)"));
        assert!(groups[1].text.contains("is 500.0 mm/min"));
        assert_eq!(groups.iter().map(|g| g.notes.len()).sum::<usize>(), 5);
    }

    #[test]
    fn req_nfr_12_only_consecutive_moves_of_one_operation_with_the_same_wording_group() {
        let mut notes = feed(2..=3, 972.9);
        // A gap in the moves.
        notes.extend(feed(5..=5, 972.9));
        // Another operation.
        notes.push(
            notes[0]
                .replace("'Roughing'", "'Finishing'")
                .replace("move 2", "move 6"),
        );
        // Other wording.
        notes.push("operation 'Roughing': move 7: the move leaves the travel by 2.0 mm".to_owned());
        // A note with no move, and a single note, are shown as they are.
        notes.push("operation 'Roughing': the feed comes from the operation".to_owned());
        let groups = group_notes(&notes);
        let texts: Vec<&str> = groups.iter().map(|g| g.text.as_str()).collect();
        assert_eq!(groups.len(), 5, "{texts:?}");
        assert!(texts[0].contains("moves 2 to 3 (2 moves)"));
        assert_eq!(texts[1], notes[2]);
        assert_eq!(texts[2], notes[3]);
        assert_eq!(texts[3], notes[4]);
        assert_eq!(texts[4], notes[5]);
        assert!(groups[1..].iter().all(|g| g.notes.len() == 1));
    }

    #[test]
    fn req_nfr_12_a_note_with_the_move_in_its_sentence_groups_too() {
        // The rapid moves that cut material: "move 12 is a rapid that ...".
        let notes: Vec<String> = (12..=14)
            .map(|m| {
                format!(
                    "operation 'Roughing': move {m} is a rapid that removes 0.100 mm3 of material"
                )
            })
            .collect();
        let groups = group_notes(&notes);
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].text,
            "operation 'Roughing': moves 12 to 14 (3 moves): each is a rapid that removes \
             0.100 mm3 of material"
        );
    }

    #[test]
    fn req_nfr_12_numbers_inside_words_are_wording_not_values() {
        // "G54" and "mm3" are words; a negative value is a value.
        let notes: Vec<String> = (1..=2)
            .map(|m| format!("move {m}: G54 offset -0.5 mm3"))
            .collect();
        let groups = group_notes(&notes);
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].text,
            "moves 1 to 2 (2 moves): G54 offset -0.5 mm3"
        );
        assert!(group_notes(&[]).is_empty());
    }

    #[test]
    fn req_nfr_12_a_range_runs_from_the_lowest_to_the_highest_value() {
        // Falling values: the range still reads low to high.
        let mut notes = feed(2..=2, 975.1);
        notes.extend(feed(3..=3, 972.9));
        let groups = group_notes(&notes);
        assert_eq!(groups.len(), 1);
        assert!(
            groups[0].text.contains("is 972.9 to 975.1 mm/min"),
            "{}",
            groups[0].text
        );
    }

    #[test]
    fn req_nfr_12_a_note_with_move_in_its_wording_but_no_move_number_is_not_grouped() {
        // "remove 2" is not a move, nor is "move the part"; the second note
        // names its move after those words.
        let notes = [
            "operation 'Roughing': remove 2 tabs".to_owned(),
            "operation 'Roughing': remove 2 tabs".to_owned(),
            "move the part: move 4: the feed is slowed".to_owned(),
        ];
        let groups = group_notes(&notes);
        assert_eq!(groups.len(), 3);
        assert!(groups.iter().zip(&notes).all(|(g, n)| g.text == *n));
    }

    #[test]
    fn req_nfr_12_req_op_11_the_feed_summary_names_the_lowest_effective_feed() {
        // FIRSTRUN item 5: the inspector says what the machine's limits did
        // to the feed the user typed, from the notes of the last run.
        let mut notes = feed(2..=4, 972.9);
        notes.push(
            "operation 'Roughing': move 9: the machine's feed limits slow this move: the \
             contact feed is 673.4 mm/min, not 1000.0"
                .to_owned(),
        );
        notes.push(
            "operation 'Finishing': move 1: the machine's feed limits slow this move: the \
             contact feed is 100.0 mm/min, not 500.0"
                .to_owned(),
        );
        let s = feed_limit_summary(&notes, "Roughing").unwrap();
        assert_eq!(s.moves, 4);
        assert!((s.lowest - 673.4).abs() < 1e-9);
        assert!((s.programmed - 1000.0).abs() < 1e-9);
        assert_eq!(feed_limit_summary(&notes, "Other"), None);
        assert_eq!(feed_limit_summary(&[], "Roughing"), None);
    }
}

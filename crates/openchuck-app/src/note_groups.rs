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
/// run, read from the run's notes (FIRSTRUN item 5; NFR-12; issue 432).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedLimit {
    /// How many moves were slowed.
    pub moves: usize,
    /// The lowest feed any of them was cut to, in mm/min.
    pub lowest: f64,
    /// The feed the operation asked for, in mm/min.
    pub programmed: f64,
    /// The feed asked for on the move that lost the most (target minus
    /// result; the first such move on a tie), in mm/min.
    pub worst_target: f64,
    /// The feed that move was cut to, in mm/min.
    pub worst_feed: f64,
}

/// The start of the note the engine writes for a move its feed limits slow.
const FEED_NOTE: &str = "the machine's feed limits slow";

/// What a feed-limit note says, as (feed the move got, feed asked for), in
/// mm/min; `None` for any other note. The note may or may not have its
/// operation's name in front: only the engine's own wording is read, so a
/// number in the name is not taken for a feed.
fn read_feed_note(note: &str) -> Option<(f64, f64)> {
    let (_, found) = values(note.get(note.find(FEED_NOTE)?..)?);
    match found.as_slice() {
        [(effective, _), (programmed, _)] => Some((*effective, *programmed)),
        _ => None,
    }
}

/// Whether `note` is one the feed-limit summary reads, so the Results window
/// may leave it out in favour of the summary. A note this cannot read is
/// not one, and stays in the list (NFR-12).
pub fn is_feed_limit_note(note: &str) -> bool {
    read_feed_note(note).is_some()
}

impl FeedLimit {
    /// The line for under the feed field, in plain words. It says what
    /// happened and that the feed set is unchanged (NFR-12).
    pub fn describe(&self) -> String {
        format!(
            "Last run: the machine's feed limits slowed {} to as low as {:.0} mm/min, \
             below the {:.0} mm/min set here. The feed set here is unchanged; the \
             machine's limits lower it only where it cannot keep up.",
            if self.moves == 1 {
                "1 move".to_owned()
            } else {
                format!("{} moves", self.moves)
            },
            self.lowest,
            self.programmed
        )
    }

    /// The one informational line the Results window shows for the operation
    /// called `operation` (issue 432). Only the move that lost the most is
    /// given; the per-move notes stay in the run's notes.
    pub fn describe_for(&self, operation: &str) -> String {
        let (moves, was) = if self.moves == 1 {
            ("1 move".to_owned(), "was")
        } else {
            (format!("{} moves", self.moves), "were")
        };
        let case = if self.moves == 1 {
            "The target was"
        } else {
            "In the worst case the target was"
        };
        format!(
            "{moves} during '{operation}' {was} limited by the machine's feed limits. \
             {case} {:.1} mm/min, but the move was reduced to {:.1} mm/min to stay within \
             the machine's capabilities.",
            self.worst_target, self.worst_feed
        )
    }
}

/// The feed-limit summary of one operation's path notes (the engine's own
/// wording, before the operation's name is put in front); `None` when none
/// slowed it. The notes are read, never changed (GC-07). The caller holds
/// the operation's id, so a rename or a shared name cannot mix operations
/// up (issue 418).
pub fn feed_limit_of<S: AsRef<str>>(messages: &[S]) -> Option<FeedLimit> {
    let mut found: Option<FeedLimit> = None;
    for message in messages {
        let message = message.as_ref();
        let Some((effective, programmed)) = read_feed_note(message) else {
            continue;
        };
        found = Some(match found {
            None => FeedLimit {
                moves: 1,
                lowest: effective,
                programmed,
                worst_target: programmed,
                worst_feed: effective,
            },
            Some(f) => {
                let bigger = programmed - effective > f.worst_target - f.worst_feed;
                FeedLimit {
                    moves: f.moves + 1,
                    lowest: f.lowest.min(effective),
                    programmed: f.programmed,
                    worst_target: if bigger { programmed } else { f.worst_target },
                    worst_feed: if bigger { effective } else { f.worst_feed },
                }
            }
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

    fn engine_note(effective: f64, target: f64) -> String {
        format!(
            "the machine's feed limits slow this move: the contact feed is {effective:.1} \
             mm/min, not {target:.1}"
        )
    }

    #[test]
    fn req_nfr_12_req_op_11_the_feed_summary_names_the_lowest_effective_feed() {
        // FIRSTRUN item 5: the inspector says what the machine's limits did
        // to the feed the user typed, from the notes of the last run.
        let mut notes = vec![
            engine_note(972.9, 1000.0),
            engine_note(972.9, 1000.0),
            engine_note(972.9, 1000.0),
        ];
        notes.push(engine_note(673.4, 1000.0));
        notes.push("a note about something else".to_owned());
        let s = feed_limit_of(&notes).unwrap();
        assert_eq!(s.moves, 4);
        assert!((s.lowest - 673.4).abs() < 1e-9);
        assert!((s.programmed - 1000.0).abs() < 1e-9);
        assert_eq!(feed_limit_of::<String>(&[]), None);
        assert_eq!(feed_limit_of(&["rapid cut".to_owned()]), None);
        let line = s.describe();
        assert!(
            line.contains("4 moves") && line.contains("673 mm/min"),
            "{line}"
        );
        assert!(line.contains("1000 mm/min set here"), "{line}");
    }

    #[test]
    fn req_nfr_12_req_ui_12_the_worst_case_is_the_greatest_delta_first_on_a_tie() {
        // Issue 432 (owner's decision): the worst move is the one that lost
        // the most, not the one with the lowest result. Here 1000 -> 900
        // loses 100, while 300 -> 250 loses 50 but has the lowest result.
        let notes = [
            engine_note(250.0, 300.0),
            engine_note(900.0, 1000.0),
            engine_note(800.0, 900.0),
        ];
        let s = feed_limit_of(&notes).unwrap();
        assert_eq!(s.moves, 3);
        assert_eq!((s.worst_target, s.worst_feed), (1000.0, 900.0));
        assert!((s.lowest - 250.0).abs() < 1e-9);
        // Equal deltas: the first move wins.
        let tie = [engine_note(900.0, 1000.0), engine_note(400.0, 500.0)];
        let t = feed_limit_of(&tie).unwrap();
        assert_eq!((t.worst_target, t.worst_feed), (1000.0, 900.0));
        // The same notes in the same order always give the same summary.
        assert_eq!(feed_limit_of(&notes), feed_limit_of(&notes));
    }

    #[test]
    fn req_ui_12_the_results_line_is_one_plain_sentence_per_operation() {
        let many = FeedLimit {
            moves: 6,
            lowest: 999.1,
            programmed: 1000.0,
            worst_target: 1000.0,
            worst_feed: 999.1,
        };
        assert_eq!(
            many.describe_for("Roughing"),
            "6 moves during 'Roughing' were limited by the machine's feed limits. In the \
             worst case the target was 1000.0 mm/min, but the move was reduced to 999.1 \
             mm/min to stay within the machine's capabilities."
        );
        let one = FeedLimit { moves: 1, ..many };
        let line = one.describe_for("Finishing");
        assert!(
            line.starts_with("1 move during 'Finishing' was limited"),
            "{line}"
        );
        assert!(line.contains("The target was 1000.0 mm/min"), "{line}");
    }
}

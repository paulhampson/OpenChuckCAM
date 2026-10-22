# 0219: Repeated notes about consecutive moves are grouped in the results (lane MSGPROG; issue 255)

- **Status:** proposed (22 Oct 2026). The grouping into a range and the closed default are the owner's answers on issue 255; the closeness rule (item 2) and the wording are the lane's, and wait for the owner's acceptance.
- **What it amends:** nothing in the documents. It changes how the results window lists the notes of a run; the engine's notes are unchanged. No requirement changes; it serves NFR-12 (the user verifies the program from this list) and NFR-04.
- **Date:** 22 Oct 2026.
- **Open items:** OI-891 (a move's line has no link to the move in the view or the G-code) is new.

## Context

Trial feedback F-84 (issue 255): the results list had one line per move for the same warning, for example ten lines "operation 'Roughing': move N: the machine's feed limits slow this move: the contact feed is 972.9 mm/min, not 1000.0" that differ only by the move. The owner asked for one entry with the range and the count, opened to see each move, nothing hidden, the same for every repeated warning, computed from the messages the engine reports without changing them. The owner's answers: slightly different values group into a range, the lane decides how close and says so, a value far from the others starts a new group; a group is shown closed, with the range, the count and the values in its one line.

The notes reach the results as sentences (`Regeneration::notes`): the operation, "move N", then the strategy's or the simulation's sentence with its values.

## Decision

1. **Grouped where shown.** `note_groups::group_notes` reads the run's notes in order and changes none. Consecutive notes group when they have the same operation, the same wording once their values are taken out, and consecutive move numbers. A note that names no move, or that matches no neighbour, stays a line of its own, as it was.
2. **Close values group into a range; a value far off starts a new group.** Each value of the notes in a group stays within 1 % of the largest of them (`note_groups::CLOSE`): 972.9 and 975.1 mm/min group (0.2 %), 972.9 and 500.0 do not. The group shows a value that does not change once and a value that does as "972.9 to 975.1". A number inside a word ("G54", "mm3") is part of the wording.
3. **The group's line.** "operation 'Roughing': moves 2 to 11 (10 moves): the machine's feed limits slow these moves: the contact feed is 972.9 mm/min, not 1000.0": the first and last move, the count, and the values or their ranges. A note with the move in its sentence ("move 12 is a rapid that removes …") reads "moves 12 to 14 (3 moves): each is a rapid that removes …".
4. **Shown closed, opened on each move.** In the results window a group is one line, closed, that opens on its notes, one per move, as they were. Pointing at it says the moves have the same note with values within 1 % and that it opens to show each.

## Alternatives considered

- **Exact matches only**: the owner chose ranges.
- **Grouping in the engine** (one note per run of moves): the issue keeps the engine's notes one per move, so tests and the G-code checks can still count them (GC-07, NFR-06).
- **A closeness rule in written units** (a difference of one in the last place written): too tight for feeds that drift by a few tenths, and meaningless for values written with three places.

## Consequences

- A run with many slowed moves lists a line per run of moves, not per move, and draws less (issue 200).
- The words "the machine's" in the feed-limit note are issue 189's to change; the grouping keeps whatever the engine writes.
- Changelog: `openchuck-app` gains the module `note_groups`. It is not in the library family, so the version is not bumped (decision 0048).

## Tests

- `note_groups::tests` (`crates/openchuck-app/src/note_groups.rs`): `req_nfr_12_repeated_notes_about_consecutive_moves_become_one_group` (the owner's ten lines), `req_nfr_12_close_values_give_a_range_and_a_value_far_off_starts_a_new_group`, `req_nfr_12_only_consecutive_moves_of_one_operation_with_the_same_wording_group`, `req_nfr_12_a_note_with_the_move_in_its_sentence_groups_too`, `req_nfr_12_numbers_inside_words_are_wording_not_values`.
- `run_panel::tests::req_nfr_12_repeated_notes_are_one_line_that_opens_on_each_move` (`apps/openchuck-ui`).

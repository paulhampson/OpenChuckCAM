# 0206: Math in number boxes, and how an unreadable entry shows (lane NUMBOX; issue #183, trial feedback F-27)

- **Status:** accepted (21 Oct 2026, by the owner directly in the lane NUMBOX session: "Accept 0206"). The design was first agreed as written (option a) through the coordinator. The owner's earlier answers on issue 183 (operators, show the result, round to the field's precision) and the decision on the error display are built in.
- **What it amends:** nothing yet. It proposes requirement **UI-26** (P2, below); `docs/requirements.md` and the verification plan are not edited here (the owner adds them).
- **Date:** 21 Oct 2026.
- **Open items:** OI-800, OI-801, OI-802.

## Context

Number boxes took a plain number only. The owner asked (F-27) that they accept simple sums such as `12.5 + 3`, `60 / 2` or `20 * 0.5`. The open points on the issue were the operators, what the box shows afterwards, how an unreadable entry is reported (NFR-07), the unit, and the rounding.

## Decision

1. **Syntax.** Numbers (digits, a point, an optional exponent), `+ - * /`, brackets, a sign before a number or bracket, and spaces. `×`, `÷` and the minus sign `−` read as `*`, `/` and `-`. Multiplication and division bind tighter than addition and subtraction; equal operators go left to right. Nothing else: no percent, functions, variables or units.
2. **Units.** The entry is read in the unit shown beside the box (millimetres, degrees, mm/min; NFR-05). A unit word typed in the entry is an error that says the unit is already shown ("5 mm"). A comma is an error that asks for a point, so a decimal comma is never guessed.
3. **When.** A sum is worked out when the entry is confirmed (Enter, or the box loses focus), as a plain number is today. The box then shows the result and the expression is not kept.
4. **What is stored.** A plain number is applied exactly as typed, so nothing that worked before changes. A sum is worked out in double precision and rounded to the places of the field's unit (`field_row::places`: millimetres 3, degrees 2, feeds and speeds whole, a plain number as it is), then applied as that number only. The project never contains an expression, and the same entry always gives the same number (GC-07, NFR-06). Division by zero and results too large to be a number are errors.
5. **An unreadable entry** (owner decision): the box gets a red outline and a short plain line under it that quotes the entry and says what is expected, for example "Cannot read '12*'; expected a number or a sum." The last good value stays applied (nothing reaches the project) until the entry is fixed or Esc puts the last value back. The step bar marks the job "Error to fix" while a number box in the open step holds such an entry, with a line in the badge's tooltip ("Fix Feed: it cannot be read"); the mark goes with the entry. Entries longer than 200 characters or nested more than 32 deep are refused the same way, so no input can crash or stall the application (NFR-07).
6. **Where.** One evaluator, `arith` in `openchuck-ui`, used by every typed number box: the shared field row (`TextRow::number`: the Setup, operation, model, tool and tool-number fields, the workholding tab fields), and the two boxes that keep their own text (the results cell size and the tool library's cutting data). The tool numbers (a whole number with an optional T) keep their own reading and take no sum. The egui drag controls are not typed boxes and are unchanged. The widget belongs to this lane; lane OPCREATE only sets `number: true` on rows it adds, if it adds any.
7. **Dependency.** None added: the evaluator is about 150 lines, so no NFR-14 justification is needed. A crate such as `meval` or `evalexpr` offers more than was asked for and is a larger risk.
8. **Proposed requirement UI-26 (P2), for the owner to add to `docs/requirements.md`:** "Number boxes accept simple arithmetic (`+ - * /` and brackets), worked out when the entry is confirmed, shown as its result rounded to the field's precision, with an entry that cannot be read reported under the box and on the step bar, and the last good value kept." Verification plan row: unit tests of the grammar, rounding and every fault, and shell tests of the message, the step bar mark and the kept value. Until UI-26 is in the requirements document the tests are named for the requirements that exist (UI-07, UI-11, NFR-07), because `cargo xtask trace` rejects an ID the document does not have (OI-802).

## Tests

`arith::tests::req_ui_26_*` (renamed from `req_ui_07_*` by lane GATES, OI-802; grammar, precedence, signs, exponents, determinism, every fault and its message, no panic over every short string and over hostile ones, rounding, plain numbers unchanged, the boxes with their own text) and `shell::numbox_tests::req_ui_26_*` (the reason under the box, the step bar mark until fixed or Esc, a sum shown as its result and not kept, the project unchanged while an entry is unreadable). The existing `req_ui_16_req_ui_07_typing_in_an_entry_box_moves_nothing` still passes: the reason line replaces the one a refusal already used.

## Alternatives considered

- **Units and percent in the entry** ("5 mm", "10%"): more to teach and to get wrong (percent of what?). Deferred (OI-800).
- **An existing expression crate:** rejected above.
- **Keeping the expression in a tooltip:** the owner chose to show the result only.

## Consequences

The step bar mark follows the box that is drawn: it shows while the step with the unreadable entry is open and comes back when the user returns to it with the entry still in the box. It does not mark another step from a distance.

The step bar badge now reads the number boxes drawn in the last frames, so an unreadable entry counts even though the project has not changed. Boxes that refuse a value for another reason (a range) are unchanged.

# 0005: SP-5 result, G-code reader

- **Status:** proposed. The reader choice (OI-05) is for the project owner at slice 6. The LinuxCNC check of the hand-written fixture is outstanding (OI-31).
- **Date:** 11 Oct 2026.
- **Spike branch:** `spike/sp-5-gcode-reader` (throwaway, never merged). The full write-up is in `spikes/sp5/RESULT.md` on that branch, and the raw output is in `spikes/sp5/out/report.md`.

## Context

SP-5 (work breakdown, Spikes) asked: does the `gcode` crate read A words and the LinuxCNC dialect? Pass criterion: every code our posts emit is parsed with positions. Named fallback: the `g-code` crate, or a small in-house reader for the subset our posts emit. It runs before slice 6 and closes the reader half of decision 0001 item 5 (OI-05).

Requirements touched: GC-08 (checks on exported G-code), SIM-09 (P3), OP-13 (feed round trip), GC-07 and NFR-06 (determinism), NFR-05 (double precision), NFR-07 (no crash on bad input) and NFR-14 (dependency records).

Architecture sections touched:

- Section 3: `openchuck-gcode` is the G-code reader and interpreter.
- Section 9: the post round trip and the kinematic oracle.
- Section 7 / crate selection: the "G-code reading" row, which picks `gcode` (75) and asks for a spike to confirm A-word parsing.
- Section 12: a fuzz target for the G-code reader.

Spike input: decision 0001 item 5 (SP-3 found that `gcode` 0.7.0 stores numbers as `f32`).

## Method and numbers

Four readers ran behind one interface and fed one neutral word model (letter, f64 value, source text, line, column) and one small modal replay to XYZA positions:

1. `gcode` 0.7.0 with its own `f32` values.
2. `gcode` 0.7.0 with an f64 layer (numbers re-read from the span text).
3. `g-code` 0.6.0.
4. An in-house reader for the post subset.

The inputs were the three SP-3 outputs (312 moves each, 310 A words) and two hand-written files:

- **Post subset.** Every code a rotary post is expected to emit: G0 to G4, G17 to G21, G28, G40, G43, G49, G53 to G59, G61, G61.1, G64, G90, G90.1, G91, G91.1, G93 and G94, plus M0 to M9 and M30. It also has N numbers, `%`, both comment forms, and unwrapped A down to -12345.6789.
- **Dialect extras.** LinuxCNC features no post plans to emit: `#` parameters, expressions, O-words, spaces inside words, block delete, G38.2, G33.1 and G10.

An independent split-on-whitespace oracle gave the true words, values and positions. Each reader was compared with it bit for bit.

| Check | `gcode`, f32 | `gcode` + f64 layer | `g-code` 0.6.0 | In-house |
| --- | --- | --- | --- | --- |
| SP-3 outputs and the subset file parsed, with line and column | yes | yes (N has no span; block start used) | **no**: a file ending in `%` and a newline is rejected | yes |
| Values against the source text | up to 2.4e-3 off (long unwrapped A) | exact | exact | exact |
| Replay of SP-3 files against in-house | positions off by up to 1.25e-5 | bit-identical | bit-identical | reference |
| A code it cannot store (`G1.25`, `G99999999999`, huge `N`) | **silently dropped**, parse returns success | same | read | read; the replay refuses unknown codes |
| Unclosed comment | accepted | accepted | error | error |
| `X+-1` | error | error | **panic** | error |
| Mutation fuzz, 20,000 inputs | 0 panics | 0 panics | **8 panics** | 0 panics |
| `#`, `[...]`, O-words | error | error | error | error naming the feature |
| LinuxCNC spacing (`G 1 X 1 . 5`), block delete `/` | error | error | error | read |
| 1M lines (31.8 MB), best of three | 1.65 s | 1.27 s | 2.79 s | 0.53 s |
| Dependencies | `serde`, two proc-macro crates (one brings `proc-macro2` 0.4 and `quote` 0.6) | same | `peg`, `nom`, `rust_decimal`, `codespan` | none |

Cause of the silent drop: `gcode` 0.7.0's diagnostics collector implements only two of its four diagnostic hooks. Number and integer parse errors fall through to empty defaults, so `gcode::parse` returns `Ok` without the code.

All candidates are deterministic on repeat reads, and all licences are AGPL-compatible.

## Decision (proposed)

1. **Answer.** `gcode` 0.7.0 reads A words and every code our posts emit with positions, so the literal pass criterion is met with an f64 layer. It is not fit for `openchuck-gcode`, though. A reader whose job is to check exported G-code (GC-08) must not:
   - lose precision (`f32`, 2,400 times the 1e-6 degree limit of verification plan section 4.1);
   - drop a code silently and report success;
   - accept an unclosed comment.

   With the f64 layer, `gcode` would only be supplying tokens and spans.
2. **`g-code` 0.6.0 is rejected.** It panics on bad input (NFR-07) and rejects the usual LinuxCNC file ending.
3. **Proposed choice: a small in-house reader in `openchuck-gcode`** for the subset our posts emit, with no third-party dependency. This is the fallback the work breakdown names.
   - It reads case-insensitive letters, LinuxCNC spacing inside words, both comment forms (an unclosed one is an error), `%`, block delete and N numbers.
   - Values are parsed from the source text as f64.
   - Every error names line and column.
   - Parameters, expressions and O-words are refused by name. If a later post or SIM-09 needs them, its slice widens the reader.

   The spike's lexer was about 100 lines, and the modal interpreter is ours whichever lexer is used.
4. **Known-code list.** The reader's list of known G and M codes is the post subset. A post that adds a code adds it to the reader and to the subset fixture in the same change. SP-10 and SP-12 are expected to add Z1-specific codes.

## Alternatives considered

- **`gcode` 0.7.0 with an f64 layer** (decision 0001's first option). It meets the literal criterion, but it still needs our own number parsing. It also needs a custom visitor to catch the dropped codes and its own unclosed-comment check. That is more code than the in-house lexer, it adds a dependency, and it is slower.
- **`gcode` raw values.** Rejected for the precision loss.
- **`g-code` 0.6.0.** Rejected: it panics, rejects valid files, and is the slowest of the four.
- **Reporting the two crate bugs upstream** (the silent drop in `gcode`, the panic in `g-code`). This is worth doing and is independent of the choice. It is the project owner's call, because it is outward-facing.

## Consequences

If the proposal is accepted:

- **Crate selection:** the "G-code reading" row of architecture section 7 and `docs/crate-selection.md` would name an in-house reader in place of `gcode`. This is an architecture edit, so it needs the owner's agreement and is **not made here**.
- **Dependency register:** the `gcode` row in `docs/dependencies.md` moves from "Recorded but not yet declared" to "Tried and not adopted". Until then its note points here. `g-code` is recorded as tried and not adopted now, because it failed the criterion outright.
- **Slice 6:** builds the reader and its modal interpreter in `openchuck-gcode`, with:
  - the subset file and the malformed cases as `testdata/` fixtures and fuzz seeds;
  - tests named for GC-08, NFR-05 and NFR-07;
  - a `fuzz/` target, as architecture section 12 already plans.
- **Unchanged:** decision 0001's other items (simulator run, golden files, Z1 conventions, feed rule).

No rule exception and no `xtask` allow-list entry are needed. The proposal adds no dependency.

## Open items

Tracked in [docs/open-items.md](../open-items.md).

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 (OI-05) | Reader choice: in-house reader proposed. | Slice 6 | Project owner |
| 2 (OI-31) | Run `linuxcnc_post_subset.ngc` through LinuxCNC's own interpreter (`rs274` standalone or the simulator) to confirm it is valid LinuxCNC before it becomes a fixture. It was checked by hand against the G-code reference only; LinuxCNC is not installable in the working environment (as in OI-01). | Before slice 6 adds the fixture to `testdata/` | Someone with LinuxCNC 2.9 or later (can be done with OI-01) |

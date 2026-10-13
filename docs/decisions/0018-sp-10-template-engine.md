# 0018: SP-10 result, template engine for posts: minijinja 3.0.0

- **Status:** proposed. The spike met its pass criteria for `minijinja`; items 1 to 8 need the project owner's agreement. Until then this record is guidance, not authority. Item 5 also answers OI-38 as a recommendation.
- **Date:** 13 Oct 2026.
- **Spike branch:** `spike/sp-10-template-engine` (throwaway, never merged). Code in `spikes/sp-10-template-engine/`, a separate Cargo project with an empty `[workspace]`; the write-up is `spikes/sp-10-template-engine/RESULT.md` on that branch.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-77 to OI-80 are new; OI-38 has a recommendation.

## Context

SP-10 (work breakdown, Spikes, 2 days) asks: can a candidate engine express rotary posts (modal state, A wrap, line numbers, feed mode, conditionals) deterministically? Pass criteria: the SP-3 post and a Makera-style variant render identically on repeat runs; errors name the line; fixed-decimal formatting checked (minijinja reached 3.0 on 8 Oct 2026, so pin it); NFR-14 assessment recorded. Fallback: a second engine, or a minimal in-house template language. It runs before slice 6.

Requirements: GC-01 to GC-04, GC-07, GC-09, GC-10, OP-13, NFR-05 to NFR-07, NFR-09, NFR-13, NFR-14. Architecture sections 5.3 and 7 (post templates row). Decisions 0001 (SP-3 post), 0005 (reader subset) and 0007 (Z1 conventions, OI-38).

## Method

`minijinja =3.0.0`, one template reading post-definition data and a candidate neutral machine program with machine positions already computed. SP-3's three committed outputs are the reference for the LinuxCNC post; the Makera variant (decision 0007 conventions, all derived and unconfirmed) is judged against an independent Rust implementation and by replaying its G-code under the Z1 timing rule. 29 tests; the formatting filter is checked against an integer-arithmetic oracle.

## Results

- The template reproduces SP-3's `g94`, `g93` and `naive` files byte for byte. The Makera variant equals the independent implementation, and its 310 feed moves replay to 44.242 s against the program's 44.242 s.
- Modal state, line numbers, A direction and wrap, tool change and header conditionals, max feed, both feed rules, G93 and `G92.4` unwinding are expressed and each option changes the output as specified.
- Identical bytes over repeats, fresh environments, 1 to 8 threads, debug and release, and 12 processes (including a `HashMap` context). Not yet run on other platforms.
- Errors name the template and line in all 10 cases. Caveats: debug mode must be switched on; an unclosed block is reported at end of input; `is none` and `is defined` hide a mistyped option name.
- `fixed` formatting: 1,245,566 oracle comparisons with no mismatch. The engine's own number display is not fixed width, so a post must use the filter.
- Cost: about 21 µs and 323 fuel per move for the full template (100,000 moves 2.1 s; 1,000,000 moves 22 s); formatting alone is 2.4 µs per move.
- minijinja's only dependency is `serde`. Include, extends and import find nothing without templates registered; no file, network or clock access.

## Decision (proposed)

1. **`minijinja =3.0.0` is the post template engine** (GC-03), in `openchuck-post` only, behind the `PostProcessor` trait. Features `builtins macros multi_template debug fuel serde`, default features off (`fuel` does not build without `multi_template` in 3.0.0). No fallback engine is adopted.
2. **The engine's numeric surface is IEEE-exact only.** The engine registers `fixed` (ties to even on the exact value, no `-0`, errors on NaN and infinity), `rounded`, `sqrt`, `floor`, `trunc` and `pi`, and nothing that calls the platform maths library (no `hypot`, `sin`, `cos`), so templates give the same bytes on all three platforms (NFR-06). Transcendental work stays in Rust.
3. **Environment settings:** strict undefined, no auto-escaping, `trim_blocks`, `lstrip_blocks`, `keep_trailing_newline`, debug on, recursion limit 64, fuel set per render in proportion to the program (about 400 per move plus a constant), no loader.
4. **The post definition is validated before render.** Every option key must be present (`null` for unset) and typed, because strict undefined does not catch a mistyped name under `is none` or `is defined` (OI-79). Comments written from project text have parentheses replaced.
5. **Where the per-controller feed rule lives (OI-38), recommended:** in Rust, as a small closed set of named rules (`linuxcnc`, `makera`, `inverse_time`, `contact`) selected by the post definition, applied in the neutral machine program stage, with modal suppression kept in the template. Reasons: the template can carry the rule (shown above) but about 90 % of the render time is that logic; the rule gets Rust unit tests and the oracle; a user cannot silently change a safety-relevant formula by editing a template. Cost: a user-written post cannot invent a new rule without Rust (GC-13 scripting, P2, would cover it). OI-38 stays open until you decide; the template version is the alternative and works.
6. **Template loading normalises newlines to LF** before compile, and `posts/**` is marked `-text` or `eol=lf` in `.gitattributes` (OI-78).
7. **Dependency (NFR-14):** `minijinja` 3.0.0, survey 94, Apache-2.0, one dependency already in the workspace, no build script, 751 dependents, 19 releases in 12 months, 3.0.0 released 8 Oct 2026 after four alphas. Pinned exactly, upgraded deliberately with the spike tests re-run. Recorded in `docs/dependencies.md` as tried and not yet declared; slice 6 declares it.
8. **Spike fixtures kept for slice 6:** the three SP-3 outputs, the integer oracle for `fixed`, the independent Makera implementation and the replay check.

## Alternatives considered

- **Compute the feed rule in the template (the spike's version):** works and passes; slower and harder to test (item 5).
- **`handlebars` or `tera`:** not tried; no criterion failed.
- **An in-house template language:** not needed.

## Consequences

- **Slice 6:** declares `minijinja`, builds the environment of item 3, the loader of item 6 and the schema check of item 4, and writes the Z1 and LinuxCNC posts with golden tests.
- **Work breakdown:** SP-10 Status updated in this change. No edit to `requirements.md` or `architecture.md`.
- **Dependencies:** `minijinja` row added.

## Open items

- **OI-77**: byte-identical template output on Windows, macOS and arm64 (the spike ran on Linux x86-64 only). Slice 6 CI.
- **OI-78**: loader rules: newline normalisation, `.gitattributes` for `posts/`, comment sanitising and non-ASCII text.
- **OI-79**: the post option schema and its validation before render, and the post file layout.
- **OI-80**: render limits: the fuel budget per move, error text size, and whether a time budget for large programs is wanted (21 µs per move in the template, 22 s for 1,000,000 moves).

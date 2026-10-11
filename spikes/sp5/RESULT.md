# SP-5 result: G-code reader

- **Question (work breakdown, Spikes):** does the `gcode` crate read A words and the LinuxCNC dialect?
- **Pass criterion:** every code our posts emit is parsed with positions.
- **Fallback named:** the `g-code` crate, or a small in-house reader for the subset our posts emit.
- **Requirements touched:** GC-08 (checks on exported G-code), SIM-09 (P3, G-code simulation), OP-13 (feed round trip), GC-07 and NFR-06 (determinism), NFR-05 (double precision), NFR-07 (no crash on bad input), NFR-14 (dependency records). Architecture sections 3 (`openchuck-gcode`), 9 (post round trip, kinematic oracle) and the crate selection row "G-code reading".
- **Timebox:** 1 day. Throwaway code: this branch is never merged.

## Method

Four readers behind one interface (`src/readers.rs`), feeding one neutral word model (letter, f64 value, source text, line, column) and one small modal replay (`src/interp.rs`) that turns words into reached XYZA positions with the feed mode in force:

1. `gcode` 0.7.0, values as the crate gives them (`f32`).
2. `gcode` 0.7.0 with an f64 layer: structure and spans from the crate, numbers re-read from the span text (the SP-3 approach).
3. `g-code` 0.6.0 (`rust_decimal` values), the named fallback crate.
4. An in-house reader for the post subset (99 non-blank lines), the other named fallback.

Inputs (`fixtures/`):

- The three SP-3 outputs (`sp3_g94.ngc`, `sp3_g93.ngc`, `sp3_naive.ngc`; 312 moves each, 310 A words, A from +90 to -450).
- `linuxcnc_post_subset.ngc`, hand-written: every code a rotary post is expected to emit. That is G0 to G3 (with IJK and R, in G17, G18 and G19), G4, G20/G21, G28, G40, G43 H, G49, G53, G54 to G59, G61, G61.1, G64 P, G90/G91, G90.1/G91.1, G93/G94, and M0 to M9 and M30. It also has N numbers, `%` delimiters, paren and semicolon comments, `(MSG,...)`, A alone, negative and unwrapped A (-12345.6789), `.5`, `+2.75` and `0.`.
- `linuxcnc_dialect_extra.ngc`: LinuxCNC features our posts do not plan to emit: `#` parameters, named parameters, `[...]` expressions, O-words, spaces inside words, block delete, lowercase, G38.2, G33.1 and G10 L2.

Truth: for files where every word is separated by whitespace (the post outputs and the subset file), an independent split-on-whitespace oracle (`src/oracle.rs`) gives each word's letter, correctly rounded f64 value, line and column. Each reader is compared with it bit for bit.

`cargo run --release` prints every number below (full output in `out/report.md`); `cargo test --release` pins each finding (11 tests).

## Numbers

### Pass criterion: every emitted code parsed with positions

| Reader | SP-3 files (3) | Subset file | Values vs text | Positions (line, column) | Replay |
| --- | --- | --- | --- | --- | --- |
| `gcode`, f32 values | parsed | parsed | off by up to 1.9e-4 (5.9e-5 on SP-3 files) | match | 312 moves each, positions off by up to 1.25e-5 |
| `gcode` + f64 layer | parsed | parsed | exact | match (N has no span of its own: block start used) | identical to in-house |
| `g-code` 0.6.0 | parsed | **rejected**: a file ending in `%` and a newline fails ("expected EOF") | exact | match | identical where it parses |
| in-house | parsed | parsed | exact | match | reference |

Replay of the SP-3 files: positions and feeds bit-identical across `gcode` + f64 layer, `g-code` and in-house. Repeat reads are identical for every reader.

### Precision on stress values (A-12345.6789, A98765.4321, X600.0001, F0.123 and others)

| Reader | Worst \|value - exact\| |
| --- | --- |
| `gcode`, f32 values | **2.4e-3** (at A98765.4321): 2,400 times the 1e-6 degree limit of verification plan 4.1 |
| others | 0 |

### Malformed and edge-case input (selected; full list in `out/report.md`)

| Input | `gcode` 0.7.0 | `g-code` 0.6.0 | in-house |
| --- | --- | --- | --- |
| `G1 X+-1.0` | error, line and column | **panic** (`unreachable!` in its grammar) | error, line and column |
| `G1.25 X1` | **silently drops G1.25**, returns `Ok` | reads G1.25 | reads G1.25 (replay refuses it: not in the subset) |
| `G99999999999 X1` | **silently drops the G word** | reads it | reads it (replay refuses it) |
| `N99999999999 G1 X1` | silently drops N | reads it | reads it |
| `G1 X1 (no close` | accepted | error | error |
| `%\nG1 X1\n%\n` | accepted | **error** | accepted |
| `G 1 X 1 . 5` (legal in LinuxCNC) | error | error | read as G1 X1.5 |
| `/G0 X99` (block delete) | error | error | read, marked as block delete |
| `#1=5`, `[2*3]`, `o100 sub` | error | error | error naming the feature |
| `G38.2`, `G33.1`, `G10 L2 P1`, lowercase, `G1X1.5A-2`, CRLF | read | read | read |

Cause of the silent drop: `gcode` 0.7.0's `Diagnostics` type implements only `emit_unknown_content` and `emit_unexpected`. The number and integer parse errors fall through to empty default methods, so `gcode::parse` returns `Ok` without the code. A custom visitor over its core API could catch them.

### Robustness (mutation fuzz, 20,000 inputs from the fixtures, deterministic seed)

| Reader | Panics |
| --- | --- |
| `gcode` 0.7.0 | 0 |
| `g-code` 0.6.0 | **8** (all sign-then-sign, such as `A+-399.375`) |
| in-house | 0 |

### Speed (1,000,000-line SP-3-style program, 31.8 MB, best of three, release build, Linux container)

| Reader | Time |
| --- | --- |
| `gcode`, f32 values | 1.65 s |
| `gcode` + f64 layer | 1.27 s |
| `g-code` 0.6.0 | 2.79 s |
| in-house | 0.53 s |

### Dependencies

- `gcode` 0.7.0: MIT OR Apache-2.0; pulls `serde` by default and two proc-macro crates (`doc-cfg` 0.1.0 brings `proc-macro2` 0.4 and `quote` 0.6, an old second copy).
- `g-code` 0.6.0: MIT OR Apache-2.0; pulls `peg`, `nom` 8, `rust_decimal`, `codespan` and `codespan-reporting` by default.
- In-house: standard library only.

All licences are AGPL-compatible.

## Decision (proposed; the project owner decides at slice 6, OI-05)

**Answer to the question:** yes, partly. `gcode` 0.7.0 reads A words and every code our posts emit, with line and column, so the literal pass criterion is met *with an f64 layer*. But it fails what the reader exists for (GC-08, and the round trip in architecture section 9):

- values are `f32` (2.4e-3 error on long unwrapped A), so the f64 layer must re-parse every number itself;
- it **silently drops** a code it cannot store and returns success, which a checking tool must never do;
- it accepts an unclosed comment;
- the N word has no span.

`g-code` 0.6.0 (the named fallback crate) fails outright: it rejects a file ending in `%` and a newline (the usual LinuxCNC form) and panics on `X+-1` (NFR-07).

**Proposed:** take the other named fallback, **a small in-house reader in `openchuck-gcode`** for the subset our posts emit, with no third-party dependency:

- Lexing: case-insensitive letters, LinuxCNC spacing inside words, paren and semicolon comments (an unclosed comment is an error), `%` delimiters, `/` block delete, N numbers. Values are parsed straight from the text as f64.
- It refuses `#` parameters, `[...]` expressions and O-words with an error naming line and column, because no post we plan emits them. If a later post or SIM-09 needs them, that is a scope change for its slice.
- The modal interpreter (which codes are known, G90/G91, G93/G94, units, A unwrapped) is ours whatever the lexer. It was most of the spike's code with either crate.
- It costs about 100 lines for the lexer, it is the fastest of the four, and it needs no NFR-14 record. It gets the `fuzz/` target the architecture already plans for the G-code reader.

## Fallback status

Both fallbacks were tried. `g-code` is rejected. The in-house reader passes everything above.

## Limits

- **No LinuxCNC run.** The hand-written subset file was checked against the LinuxCNC G-code reference by hand, not run through LinuxCNC's own interpreter (none is installable here, as in SP-3, OI-01). LinuxCNC's `rs274` standalone interpreter should read `linuxcnc_post_subset.ngc` without error before the file becomes a fixture in `testdata/`.
- **The post subset is a forecast.** SP-10 (template engine) and SP-12 (Z1 conventions) may add codes, for example a Z1-specific header, tool change or M-codes. Each one added to a post must be added to the reader's known list and to the fixture, in the same change.
- Arcs are replayed to their end point only; arc interpolation for limit checks is slice 6 work.
- The fuzz is a mutation loop in the spike, not cargo-fuzz.

## Fixtures to keep

`fixtures/linuxcnc_post_subset.ngc` and `fixtures/linuxcnc_dialect_extra.ngc` (with the malformed cases in `src/oracle.rs`) become `testdata/` fixtures and fuzz seeds in slice 6, after the LinuxCNC check above. The SP-3 outputs are not golden files (OI-02).

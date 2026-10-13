# SP-10 result: template engine for rotary posts

Spike branch `spike/sp-10-template-engine` (throwaway, never merged). Run `cargo test --release` (29 tests), `cargo run --release` (writes `out/`), `cargo run --release -- bench`.

## Question and pass criteria

Can a candidate engine express rotary posts (modal state, A wrap, line numbers, feed mode, conditionals) deterministically? Pass: the SP-3 post and a Makera-style variant render identically on repeat runs; errors name the line; fixed-decimal formatting checked (minijinja reached 3.0 on 8 Oct 2026, so pin it); NFR-14 assessment recorded.

## Method

`minijinja =3.0.0` (features `builtins macros multi_template debug fuel serde`). One template, `posts/rotary.post` (96 lines), reads post-definition data (`opt`: header and footer lines, decimals, line numbers, tool change, feed mode, max feed, A direction, wrap, unwind) and a candidate neutral machine program (`program`: machine XYZA positions already computed by the shared kinematics, durations, tool, spindle). SP-3's path, kinematics and hard-coded post are copied in as the reference. The engine adds six numeric helpers, all IEEE-exact: filters `fixed`, `rounded`, `sqrt`, `floor`, `trunc` and the global `pi`.

## Results

**Expressiveness (pass).**
- The template reproduces SP-3's three committed outputs (`g94`, `g93`, `naive`; 318, 319, 318 lines) byte for byte, and SP-3's Rust post's output.
- The Makera-style variant (decision 0007 conventions, all derived) equals an independent Rust implementation written from the decision text. Read back and timed under the Z1 rule, its 310 feed moves total 44.242 s against the program's 44.242 s.
- Modal state (`namespace` plus string comparison at output precision), line numbers (a macro mutating the outer namespace), A direction, A wrap, tool change conditionals, max feed clamp, both feed rules and G93, and `G92.4` unwinding each change the output as specified (tests `option_*`).
- Awkward: no list `append` (lists are rebuilt with `+`), all state lives in one `namespace`, and values are floats.

**C1, deterministic (pass).** 20 repeats, fresh environments, 1/2/4/8 concurrent threads, debug and release builds, and separate processes give identical bytes. A Rust `HashMap` context renders the same in 12 processes (maps are iterated in sorted key order). Not run on Windows, macOS or arm64.

**C2, errors name the line (pass, with caveats).** All 10 cases report template name and correct line, including inside a macro, a loop and a CRLF source. Caveats:
- Runtime errors in a release build omit the variable name and source view unless `set_debug(true)`; with it: "`opt.deciamls` is undefined", the source lines, and the referenced values (3.6 kB for a 20,000-move program).
- An unclosed block is reported at end of input, not at the opening tag.
- `is none` and `is defined` swallow the strict-undefined error, so `opt.max_fead is not none` silently reads as set. Options must be validated against a complete schema before render.

**C3, fixed decimals (pass, with a custom filter).** Built-in display is not fixed width (`{{ 3.0 }}` gives `3.0`, `0.1 + 0.2` gives `0.30000000000000004`; `round` in 3.0 is exact but returns a float, so no trailing zeros). The `fixed` filter (Rust `format!`, ties to even on the exact binary value, never `-0.000`, NaN and infinity are errors) matched an integer-arithmetic oracle in 1,245,566 comparisons and through the engine on 5,000 values at four precisions. 3.0 API change: contexts go in as `Value::from(Serde(&ctx))`.

**NFR-07 and NFR-09, limits and isolation.** Nested loops stop at the fuel limit, macro recursion at the recursion limit (64), `range` is capped at 10,000, string repeat is bounded. With no loader and no registered templates, `include`, `extends` and `import` find nothing, and no file, network, environment or clock function exists. `fuel` does not compile without `multi_template` in 3.0.0, so the include machinery stays compiled in but inert.

**Cost.** About 323 fuel and 21 µs per move for the full template (1,000 moves 21 ms; 100,000 moves 2.1 s; 1,000,000 moves 22 s, linear). A formatting-only template is 2.4 µs per move, so about 90 % of the time is the modal and feed logic. A fixed fuel limit cannot work: it must scale with the program (about 400 per move).

**Other findings.**
- G-code characters pass untouched (`#<_a>`, `[#5220 + 1]`, `%`, `;`, `< &` in comments). Literal `{{` needs `{{ '{{' }}`.
- A CRLF template source changes the output bytes; the loader must normalise newlines.
- `hypot` goes through the platform maths library, so the engine offers none (the first draft used it; output unchanged after replacing it with `sqrt(y*y + z*z)`).

**NFR-14.** minijinja 3.0.0, Apache-2.0, survey 94, one dependency (`serde`, already in the workspace), no build script. 751 dependents; 19 releases in 12 months; 3.0.0 released 8 Oct 2026 after four alphas. No `AGENTS.md` or `CLAUDE.md` at the repository root (404 on `main`). No fallback engine was tried: nothing failed.

## Decision input

minijinja passes. Per-controller feed rule (OI-38): the template can carry it, but a Rust stage would be about ten times faster and unit-testable; see decision 0018.

## Not done

Windows, macOS and arm64 runs; the user-facing post file layout; the `PostProcessor` trait; handlebars and tera were not tried.

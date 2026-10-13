# 0020: Slice 6 design refinement: posts, G-code reader and round trip

- **Status:** proposed. Built on branch `claude/nice-fermi-lyjx5n` as the slice progresses; this record grows with each step and is accepted at slice close. It relies on accepted records 0001, 0005, 0007, 0009 and 0019. The Z1 conventions stay derived and unconfirmed until the air cut (OI-37).
- **Date:** 13 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-84 and OI-85 progress, OI-88 to OI-90 are new.

## Context

Work breakdown slice 6 (GC-02, GC-03, GC-04, GC-07, GC-09, GC-10, GC-11, GC-14; GC-01 end to end; NFR-08). Architecture section 5.3. Decision 0019 chose the engine and put the feed rule in Rust (item 5). This record covers items 1 to 6 of the slice (post stage); the reader, replay, simulator, CLI, gates and tool numbers are added below as they are built.

## Decision (proposed)

1. **Neutral machine program (slice item 1).** `openchuck_post::to_machine_program` turns the IR into `Step`s in the work frame (`RotaryFrame`): comment, tool change, spindle on, rapid, fed move with its `FeedWord`, and unwind. Machine XYZ comes from the shared kinematics. A rapid at an unchanged tip whose A changes by whole turns is an `Unwind`. Moves with no motion are dropped. A fed move with motion and no duration is an error (the feed cannot be worked out). Tool numbers and spindle speeds come from a `ToolSetting` per IR tool tag (GC-14); a tag with none is refused naming the group and operation.
2. **Post definition and options (items 3 and 4; OI-85).** A post is a folder: `post.toml` (`schema`, `name`, `template`, `feed_rule`, `[options]`) and one template. `PostOptions` is typed (units, A letter, direction, unwind mode and command, header, footer, work offset, tool change, coolant, line numbers, decimals, maximum feed, feed mode, comments); unknown keys, wrong types and out-of-range values are refused at load naming the option, the feed mode must agree with the feed rule, and the template is compiled at load so a syntax error names its line. The template sees every option (unset is `none`). Environment as decision 0019 item 3; fuel is 100,000 plus 400 per step. A direction and unwinding are applied in Rust before the template runs: with `drop_turns` the controller keeps the remainder of A with its sign, so later A values are shifted by whole turns to keep every machine position, which the template then writes unchanged.
3. **Loader rules (OI-84).** Newlines normalised to LF; `posts/** -text` in `.gitattributes`; files over 1 MiB refused; the template must be a plain file name in the post folder; comment text from the project has parentheses replaced by brackets, control characters by spaces and non-ASCII by `?`.
4. **`PostProcessor` and selection (item 2; GC-02).** `PostProcessor::post(&Toolpath, &PostJob) -> PostOutput` (text, clamped feed count, estimated seconds); `TemplatePost` is the first implementation. The two posts ship in `posts/` and are embedded (`builtin`, `builtin_ids`), and load from a folder identically. Tying a machine profile to a post is OI-88.
5. **Shipped posts (items 5 and 6).** `posts/linuxcnc` (GC-10) and `posts/makera-z1` (GC-09) share one template (a test keeps the copies identical) and differ by options and feed rule. Z1: SP-12 conventions, `makera` rule, `G92.4 A0 S0` unwind, no G93, no line numbers, `M5` before `T1 M6`, footer `M5`, `G53 G0 Z-3`, `G28`, `M2`. Numbers go through `fixed` (round half to even, no `-0`). Golden files `testdata/golden/post_*_small_part.ngc` (a 6 mm, three-turn helical groove on a 20 mm cylinder, 45 degree steps, with a three-turn unwind) are reviewed line by line: positions match hand calculation (for example A 45 gives Y -7.071, Z 7.071), the Z1 feed 696.2 is `45 * (2 pi 10 + 30) / 360` mm in 1 s, and LinuxCNC's 459.5 is the 7.658 mm chord in 1 s.
6. **Narrower than GC-04 reads (OI-89).** Millimetres only and continuous A only; other values are refused, not approximated.

## Alternatives considered

- **Feed rule in the template:** decision 0019 item 5 chose Rust.
- **Options as an untyped table:** a mistyped key would pass through `none` checks silently; the typed struct refuses it.
- **A rotary wrap in the template (the spike's `modulo360`):** breaks a cut that crosses the seam; not offered (OI-89).

## Verification

`crates/openchuck-post/tests/machine_program.rs` and `posts.rs`: GC-04/OP-13 rule values by hand, GC-01 conversion and unwind detection, GC-07 repeat and four-thread identity, GC-03 editing a post file changes output with no rebuild and a CRLF definition gives the same bytes, an unknown variable names its line, option errors name the option, NFR-07 comment safety, GC-09 Z1 conventions, GC-14 missing tool, NFR-08 two golden files.

## Open items

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | Non-ASCII in comments becomes `?`; a transliteration is the alternative. (OI-84) | Owner's review | |
| 2 | Agree the post layout and option names. (OI-85) | Owner's review | |
| 3 | Post per machine profile. (OI-88) | Slice 6 | Owner's choice |
| 4 | Inches and wrapped A. (OI-89) | When a machine needs them | |
| 5 | Post `max_feed` duplicates the profile's. (OI-90) | Slice 6 | OI-39 |

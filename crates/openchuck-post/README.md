# openchuck-post

Reusable post-processing library: neutral toolpath IR to G-code through user-editable post definitions.

Part of [OpenChuckCAM](https://github.com/paulhampson/OpenChuckCAM). Architecture section 5.3; requirements GC-02 to GC-10, GC-13, INT-03.

It has the neutral machine program stage (`to_machine_program`), the named feed rules (`FeedRule`: `linuxcnc`, `makera`, `inverse_time`, `contact`; decision 0019 item 5), the template engine, post definitions and their options (`PostDefinition`, `PostOptions`), and the two built-in posts (`builtin`): the Makera Z1 post and the LinuxCNC verification post. The built-in posts are data files in this crate's `posts/` folder (decision 0103), so the published crate carries them; a user can copy one, edit it and load it from disk without recompiling.

GC-17 uses the canonical `PostJob::block_time` settings, bridges them to `ShortBlockSettings` in `ConvertOptions`, and accepts optional per-field overrides in `PostOptions`. Report leaves G-code unchanged and returns per-operation demand and predicted added time. Slow down lowers feeds to meet sliding-window duration targets while retaining coordinates. `PostOutput.verification_path` carries the adjusted durations for independent replay comparison. Use the machine's measured threshold/window; zero or Off retains legacy behaviour. Active analysis requires a controller timing rule (Makera, LinuxCNC or inverse time), and rejects the legacy Contact rule.

Generated G-code is yours to verify before you run it on a machine (NFR-12). The Makera Z1 conventions are derived and unconfirmed until they are checked on the machine.

This crate belongs to the reusable library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`). It depends only on other members of that family and on third-party crates, never on the application, UI, GPU, Git or the C++ kernel (architecture sections 4 and 12).

Licence: AGPL-3.0-or-later.

### Minimum block-time Report (GC-17, decision 0222)

`PostJob::block_time` carries the machine profile’s `min_block_ms`, `block_window` and `ShortBlockMode`. Its default is 0 ms, window 1, Off, preserving existing output. A post definition may override each independently in `[options]`; for example `min_block_ms = 7.0`, `block_window = 32`, `short_block_mode = "report"`. These values are the measured built-in Makera Z1 choice, not defaults for another controller. A zero time or Off disables reporting.

`PostOutput::block_time_reports` retains the original-IR Report compatibility API. It analyses complete windows of consecutive fed blocks, resetting at rapid moves; an incomplete window is explicitly unexamined. The report gives the affected-block count, longest affected run, lowest complete-window mean, effective contact feed in the longest run and estimated added seconds. It is omitted during effective Slow down. The standalone `block_time_reports` function preserves these original-IR semantics.

`PostOutput::short_blocks` contains the managed analysis used by application exports: actual posted coordinates and feed rounding, short runs checked with their own length, and predicted or applied added time. Its metrics differ from the compatibility report. Slow down uses this analysis to lower feeds; Merge remains deferred (decision 0245). Generated G-code remains the user’s responsibility to verify before running.

# Archive: decision 0247 (AROT)

`compare-firstcut-counter.py` is the independent first-cut comparison reproducer from the AROT lane (#337, PR #364), moved here from `docs/arot-pass-resets-verification.md` on 1 Nov 2026 (DOCS-TIDY). Usage: `python3 -I compare-firstcut-counter.py <old-golden.nc> <new-golden.nc>`. It decodes both Makera programs with Decimal arithmetic, rebuilds physical A from the non-moving counter resets and asserts that every programmed coordinate, motion, feed and spindle or tool-control record is identical. Its results are in `docs/decisions/0247-firstcut-counter-evidence.json` (cited by [0247](../../decisions/0247-long-operation-rotary-accumulation.md)). It is an analysis tool, not product code; it does not infer G28 homing, acceleration or hardware behaviour.

## Raw measurement evidence moved from `docs/decisions/` on 1 Nov 2026 (DOCS-TIDY)

- `0247-rotary-accumulation-evidence.md` and `0247-rotary-accumulation-raw.json`: the AROT phase 1 raw measurements of 26 Oct 2026 (commands, source, settings, timings, output hashes, refusals), cited by [decision 0247](../../decisions/0247-long-operation-rotary-accumulation.md).
- `0247-firstcut-counter-evidence.json`: hashes and results of the first-cut comparison produced by the script above.

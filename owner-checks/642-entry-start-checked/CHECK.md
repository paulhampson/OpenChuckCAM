# Check for issue #642: the move to a lead or ramp entry's start is checked against the stock

**What was fixed** (pull request #663, lane RAMPENTRY, [decision 0544](../../docs/decisions/0544-a-lead-on-a-pass-that-turns-a-stays-in-the-plane-of-the-cut.md) item 6): a link ends above the pass's first cut point at a height checked against the stock under the tool along the link. A lead-in or a ramp then moved sideways at that height to its own start, which is not over the first cut point, and nothing checked that move. Under a low link (decisions 0496, 0521) or roughing's local link, that height is only the approach distance above the stock along the link, so the sideways move could pass through stock beside the first cut point. Now the stock under the tool's footprint is probed along the whole of that move; where it is not clear by the approach distance, the tool rises first (never above the retract height). Where even the retract height is not clear, the operation is refused:

> Entry: the move to an entry's start would pass through stock that reaches *R* mm from the rotary axis, at or above the retract height of *H* mm. Raise the setup's clearance, or set Entry to Plunge.

Where the tool rises first, the operation gets an information note, and only then:

> Entry (operation '*name*'): 1 entry starts higher than the link before it ends, so the move to the entry's start stays clear of the stock beside the first cut point.

or, for several, "Entry (operation '*name*'): *N* entries start higher than the links before them end, so the move to each entry's start stays clear of the stock beside the first cut point." No test asserts this note's text yet (lane RAMPENTRY is adding one); the tests below assert that no rapid sweeps through the stock.

**There is no project to open for this check.** Lane RAMPENTRY found no public model that reproduces the old fault: it needs a raised piece of stock beside a pass's first cut point, outside the footprint the link checks, under a low link. The fix is shown by the lane's tests on synthetic stock, in [`crates/openchuck-path/tests/entry_link_stock.rs`](https://github.com/paulhampson/OpenChuckCAM/blob/main/crates/openchuck-path/tests/entry_link_stock.rs) on `main`:

- `req_op_14_req_op_11_the_rapid_to_a_lead_in_start_never_enters_the_stock`
- `req_op_14_req_op_11_the_rapid_to_a_ramp_start_never_enters_the_stock`
- `req_op_14_a_plunge_entry_on_the_same_stock_stays_clear` (the control)

They place a block of stock 19.5 mm from the axis beside the second pass's first cut point, link that pass low, and check that no rapid sweeps the tool through the stock. On `main` at `1f3675d52` (before the fix) the lead-in's sideways rapid entered the block at X 19.16 mm and a 10° ramp's at X 18.81 mm (decision 0544, Context); the plunge stayed clear.

**Time:** about 2 minutes, reading only.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. The tests check one kind of move against the stock the program knows; they say nothing about clamps, fixtures or stock that differs from the setup. Generated G-code must be verified by you before it is run on a machine.

## Short list (what to look at)

1. Read the three test names above and decision 0544 item 6.
2. Optional, if you build from source: `cargo test -p openchuck-path --test integration entry_link_stock` passes on `main` at `fa1c2de1` or later.
3. On your own projects with Entry Lead or Ramp and low links: if one used to show a rapid through stock at the start of an entry, regenerate it and look at that place.

## Expected result

- The three tests pass on `main` at `fa1c2de1` or later (lane RAMPENTRY ran them before the merge; they failed on `1f3675d52` as written, the control passing).
- No change in programs where the move to an entry's start was already clear: the tool rises only where the probe finds stock, and only then does the "Entry (operation '...'): ... entr... start higher than the link..." note appear.
- If one of your own projects shows that note, look at the entries it counts: the tool now rises before moving across to their start.

## Report back

Reply on issue #642 with: **PASS** if you are content with the tests as the check (or your own project now enters clear), or **FAIL and what you saw**. Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Lane OWNERCHECKS2 tried to find a public case and did not: the public example projects and the STEP files of `testdata/step/valid` have no raised stock beside a first cut point under a low link. The refusal and the note texts above are read from the code on `main` at `fa1c2de1` (`crates/openchuck-path/src/assemble.rs`, the note at about lines 855 to 871, the rise in `clear_traverse`). We did not produce the rise-first case on a public model, so the note has not been seen in a program here. We did not run the tests here; they are part of the continuous-integration suite of #663. The rest of #642 (a roughing lead arc swinging through uncut stock) belongs to the lead work of #640 and is checked with it.

# 0231: The machine profile's spindle speed range, a limit that blocks Post and export (lane OPSPEED; OI-754, issue 249)

- **Status:** proposed (23 Oct 2026). The choice is the owner's option A on OI-754, relayed by the coordinator session on 22 Oct 2026: a machine limit only, now; a limit, never a default; the built-in Makera Z1 figures from Makera's published specification marked unconfirmed; an out-of-range speed blocks Post and export with an actionable message; on project schema 18 and machine profile schema 4, with a library minor bump. The owner did not give the Z1's figures; the lane took them from Makera's document (item 3).
- **What it amends:** [decision 0196](0196-spindle-speed-belongs-to-the-operation.md) "Not decided here" (OI-754) and the machine profile of [decision 0011](0011-slice-3-machine-stock-setup.md). Decision 0196 item 1, no machine default, stands. Requirements GC-14, SET-01 and NFR-12 are unchanged.
- **Date:** 23 Oct 2026.
- **Open items:** OI-754 is closed by this record. OI-982 (the owner confirms the Z1's spindle range on the machine) is new; the specification entry below joins OI-981.

## Context

Issue 249 asked for the spindle speed to be checked against the machine's limit and the tool's. Decision 0196 found that the machine profile had no spindle speed figure and a tool had no speed limit, so only "a number above 0" was checked (OI-754). The owner chose a machine limit only (option A); a tool's own limit was not wanted (option B), since most tool makers give a recommended range rather than a hard limit.

## Decision

1. **The profile may give a range.** `MachineProfile` gains the optional `min_spindle_rpm` and `max_spindle_rpm`, each written only when set. They are a limit, never a default: an operation's spindle speed still comes from the operation, else the tool's cutting data for its preset (decisions 0196 and 0230). Each value must be above 0, and a lowest speed must be below the highest. A profile without them checks nothing more than before. `ProfileField::SpindleRange` records where the range came from; a profile without a range does not list it among its unconfirmed values, so the count of unconfirmed values for such a profile is unchanged.
2. **An out-of-range speed blocks Post and export.** The speed an operation runs at, its own or the tool data's, is checked against the range of the project's machine. Outside it, the operation lists the spindle speed as a required value ("Bring the spindle speed within the machine's range"), its Spindle speed row is marked "Outside the machine's range" with the message under it, the step bar shows it, Post and export is blocked, and posting refuses with: "cannot post the G-code: The operation "Finish" runs at 15000 rpm, above the Makera Z1's highest spindle speed, 13000 rpm. Lower the spindle speed for this operation, or choose a material preset whose speed is within the machine's range." Below the lowest speed it says "below" and "Raise". The operation still generates and simulates. No G-code with an S word outside the range is ever written (NFR-12).
3. **The built-in Makera Z1: 13 000 rpm, unconfirmed.** Makera's Z1 Safety Guidelines, V1.3 (makera.com, file `MakeraZ1-Safety_Guidelines-V1.3-multi-lan-260709-1.pdf`), give the Z1's spindle as 0 to 13 000 rpm with closed-loop speed control. CNX Software's review (1 Sep 2026) and other reviews give the same figure, and one launch page gives 15 000, which the manufacturer's document does not. So the built-in Z1 gives `max_spindle_rpm = 13000` and no lowest speed, marked unconfirmed with that source. The lane could not download the PDF from its container (the network proxy refused it) and took the figure from the document's published text as search results quote it; the owner confirms it on the machine (OI-982).
4. **Existing project copies are not changed.** A project's machine copy keeps its values (decision 0011): a copy of the Z1 made before this change has no range and checks nothing until the user updates it from the library with the machine panel's "Update the copy from the library", which is offered when the copy differs from its source ("spindle speed range" is now one of the differences it names). This follows "migrate, never guess" and keeps every older project posting as before.
5. **The machine section shows it.** The Setup inspector's machine values list "Spindle speed" ("Up to 13000 rpm", or "2000 to 13000 rpm") with the others in the collapsed part, with its confirmation mark, its source and how to confirm it ("Check the spindle's lowest and highest speed in the machine's documents or controller settings.").
6. **Formats.** Machine profile files go to their schema 4; the project's machine copy takes project schema 18 (shared with decision 0230; claimed in the work breakdown). Both steps are additive: they change only the number and refuse an older file that already holds the range.
7. **Library family 0.18.0.** `MachineProfile` gains two public fields and `ProfileField` a variant, breaking changes (decision 0048): the workspace minor version goes to 0.18.0, with changelog entries in each crate of the family. Lane STEPALL-E (unmerged) also takes 0.18.0; whichever merges second takes 0.19.0.

## Specification entry proposed

For the owner to agree with those of decision 0230 (OI-981); the specification is not edited by this record.

- **Section 11.3, the machine values:** after the maximum feeds, in the collapsed part, "Spindle speed" with the profile's range, shown only when the profile gives one; and in section 11.6, an operation whose spindle speed is outside the machine's range blocks Post and export like a missing value.

## G-code and golden files

A project whose machine gives no range, which is every project saved before this change, posts byte for byte as before. A project with a range posts exactly as before when its speeds are within it. **No golden file changes.**

## Tests

- `crates/openchuck-types/tests/data.rs`: `req_gc_14_req_nfr_12_a_profile_spindle_range_is_a_limit`, `req_nfr_07_a_bad_spindle_range_is_refused`.
- `crates/openchuck-project/src/machines.rs`: `req_gc_14_req_set_01_the_z1_spindle_range_is_unconfirmed_and_needs_schema_4`; `req_set_01_builtin_z1_reads_and_marks_every_value` now lists the range as unconfirmed.
- `crates/openchuck-project/tests/persistence.rs`: `req_gc_14_req_prj_04_the_machine_copys_spindle_range_round_trips_and_needs_schema_18`.
- `crates/openchuck-app/tests/posting.rs`: `req_gc_14_req_nfr_12_a_spindle_speed_outside_the_machines_range_is_refused`.
- `crates/openchuck-app/src/machine_view.rs`: `req_ui_13_each_value_says_whether_it_is_confirmed_its_source_and_how_to_confirm` now includes the spindle speed.

## Alternatives considered

- **B: a tool's own speed limit too.** Not chosen by the owner.
- **C: no limit** (the tooltip already says to check the speed). Not chosen.
- **A warning instead of a block.** The owner chose a block.
- **Filling the range into existing Z1 copies by migration.** It would block posting in an older project that posted before; item 4.

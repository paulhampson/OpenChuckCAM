# Check for issue #765: the held bar's own end is no longer named as a shoulder

**What was fixed** (pull request #769, lane BALUSTER-WARN, [decision 0551](../../docs/decisions/0551-the-held-regions-own-end-is-not-a-shoulder.md)): on the tutorial project, **Finish** warned that "the tool does not turn at a step across the rotary axis (a shoulder facing along X) at X 8.589". There is no shoulder of the part there: X 8.589 is one tool radius past X 7, the end face of the held bar itself (the chuck holds 8 mm of stock from X -1). The face search found the held region's own end face as if it were a face of the part. Now only faces of the part itself are named; the held region's own end, and a tab's, are not. No turn is added or removed, so **every program is byte-identical**; only the warning list changes.

The other Finish warning, "nothing within one tool radius of the held region at the start of the stock is cut", is right by design and stays, as does the setup's warning that the grip covers 7 mm of the part. So the project still reads **Needs attention**, as the tutorial's "Hold the work" page says.

**Time:** about 3 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the warnings only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): an unchanged copy of the user guide's tutorial project, [`testdata/examples/baluster/baluster-project`](../../testdata/examples/baluster/baluster-project/) (the public baluster in 63 mm of 24 mm stock from X -1, the chuck holding 8 mm, Rough and Finish), the project the warning was found on.

## Short list (what to click)

1. Open the project, **Operations**, **Generate**.
2. Read **Messages** for **Finish**: no "does not turn at a step ... at X 8.589"; the "nothing within one tool radius of the held region" warning is still there.
3. The status badge still reads **Needs attention** (expected).

## Steps

Use the release candidate, or a build from `main` at commit `55e6348d` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`765-held-end-not-a-shoulder.zip`](765-held-end-not-a-shoulder.zip) (or take it from the raw link: https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/765-held-end-not-a-shoulder/765-held-end-not-a-shoulder.zip), unzip it anywhere, and open the folder `765-held-end-not-a-shoulder/project` with **File > Open project**.
2. Click **Operations**, then **Generate**.
3. Read the **Messages** for **Finish**, and the status badge.

## Expected result

As the command-line tool prints them on `main` at `55e6348d`:

- **Finish** has two warnings:
  - "Free ends (operation 'Finish'): the end face at X 61.000 is faced from 12.000 mm, the top of the setup's own stock with no earlier operation in this setup: the real blank must match it."
  - "Held region (operation 'Finish'): nothing within one tool radius of the held region at the start of the stock is cut: no part of the tool reaches over the gripped length of the stock."
- No "Held region (operation 'Finish'): the tool does not turn at a step across the rotary axis (a shoulder facing along X) at X 8.589: ...".
- **Rough** has no warnings.
- The setup keeps its warning: "Held region (setup 'Setup 1'): the grip at the start of the stock covers 7.000 mm of the part (X 0.000 to 7.000), which is never machined; shorten the grip if that length should be cut."
- The badge reads **Needs attention** (the warnings above).

**Old behaviour (`main` at `a3e1ad22`, just before the fix):** Finish had a third warning, "Held region (operation 'Finish'): the tool does not turn at a step across the rotary axis (a shoulder facing along X) at X 8.589: there it would reach over the gripped length of the stock, so that shoulder keeps what earlier operations left."

## Report back

Reply on issue #765 with: **PASS**, or **FAIL and what you saw** (the warnings for Finish). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT`, each operation on its own and both together; `openchuck-cli setup project "Setup 1"`), Linux x86_64, release builds each in a target folder of its own, 10 November 2026, on `main` at `55e6348d` (`--version` `55e6348d`, no uncommitted changes) and at its parent `a3e1ad22`:

| | `main` at `a3e1ad22` (before) | `main` at `55e6348d` (after) |
| --- | --- | --- |
| Rough: warnings | none | none |
| Finish: warnings | Free ends; Held region "does not turn at a step ... X 8.589"; Held region "nothing within one tool radius ..." | Free ends; Held region "nothing within one tool radius ..." |
| Setup 1: checks | Held region, the grip covers 7 mm of the part | the same |
| Rough on its own, G-code SHA-256 | `26de6f3a8843b923…` | the same, byte for byte |
| Finish on its own, G-code SHA-256 | `ae176c45c21db0f9…` | the same, byte for byte |
| Both together, the two files | `26de6f3a…`, `e06552bb…` | the same, byte for byte |

**Issue #633 is changed by this fix too.** #633 reported the same "does not turn at a step" warning at an X outside both the selection and the stock, on three owner-check projects of the first set. Run on both builds:

| Project (owner-checks/) | `a3e1ad22` (before) | `55e6348d` (after) | G-code |
| --- | --- | --- | --- |
| `555-axial-into-tab`, Tenon side | "... does not turn at a step ... at X -15.001 ..." | no such warning | byte-identical |
| `548-tab-length-limited-op`, Vase only and Whole part | "... at X -16.001 ..." on each | no such warning | byte-identical |
| `547-flute-check-outside-selection`, Finish vase | "... at X -3.411 ..." | no such warning | byte-identical |

In those projects the X is one tool radius beyond an end of the held bar (stock from X -13), the same cause as here. So the three cases #633 names no longer show the warning; whether #633 has any case left (a real face of the part outside a selection) is for its own triage. The interface (the badge, Messages) cannot be run without a display here; the badge follows from the warnings listed.

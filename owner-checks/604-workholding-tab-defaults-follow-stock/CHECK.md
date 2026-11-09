# Check for issue #604: the default tab and "Add default tabs" follow a stock change at once

**What was fixed** (pull request #675, lane TABFRESH): after the stock was changed on the Setup step, the Workholding section's zone lines followed the change, but the default-tab line ("No room for a default tab at the head ...") and the greyed **Add default tabs** button stayed as they were until the project was saved and opened again. Now the section works the defaults out again from the setup as edited, so the offered tab, the "No room" line and the button all follow the stock straight away. They come from the same calculation as before; the button is enabled only when a tab is offered.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the workholding section only. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)) is an unchanged copy of the user guide's tutorial project, [`testdata/examples/baluster/baluster-project`](../../testdata/examples/baluster/baluster-project/), written by [`generate.py`](generate.py): the public baluster (X 0 to 61) in 63 mm of 24 mm stock from X -1, the chuck holding 8 mm, no tabs. The grip reaches the part, so there is no room for a default tab at the head; at the tail the baluster comes to a point at X 61.

## Short list (what to click)

1. Open the project. **Setup**, **Setup 1**, Inspector **Workholding**: "No room for a default tab at the head ..." and **Add default tabs** greyed.
2. **Stock**: Length `78` (Enter), Start X `-13` (Enter). Look at Workholding at once: a head tab is offered and **Add default tabs** is enabled.
3. Set the stock back (Length `63`, Start X `-1`): "No room ..." and the button greyed again.

## Steps

Use the release candidate, or a build from `main` at commit `07ec6990` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`604-workholding-tab-defaults-follow-stock.zip`](604-workholding-tab-defaults-follow-stock.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/604-workholding-tab-defaults-follow-stock/604-workholding-tab-defaults-follow-stock.zip)), unzip it anywhere, and open the folder `604-workholding-tab-defaults-follow-stock/project` with **File > Open project**.
2. Click **Setup** at the top and **Setup 1** in the project panel. Scroll the Inspector to **Workholding** and note the default-tab lines and whether **Add default tabs** is greyed.
3. Under **Stock**, set **Length** to `78` and press <kbd>Enter</kbd>, then **Start X** to `-13` and press <kbd>Enter</kbd>.
4. Look at **Workholding** again straight away, without saving or clicking anything else.
5. Press **Add default tabs**, look at the tab it adds, then press **Undo** (<kbd>Ctrl</kbd>+<kbd>Z</kbd>) until it is gone (or **Revert** in the section).
6. Set **Length** back to `63` and **Start X** to `-1` (<kbd>Enter</kbd> after each) and look at **Workholding** again.

## Expected result

- **Step 2:** "No room for a default tab at the head: the held region reaches the part." and, for the tail, "The part comes to a point at X 61.000 mm, so there is no diameter to base a default tab on; add the tab by hand." **Add default tabs** is greyed.
- **Step 4, straight away:**
  - the zone line "Head zone X -5.000 to 0.000 mm (5.000 mm)";
  - the offered tab "Default tab, not added · Head · Frustum · X -5.000 to 0.000 · Ø 14.000 to Ø 7.000 mm";
  - no "No room for a default tab at the head";
  - the tail line about the point is still there (the baluster still comes to a point);
  - **Add default tabs** is enabled.
- **Step 5:** one tab is added at the head (the offered one); none at the tail.
- **Step 6:** "No room for a default tab at the head: the held region reaches the part." again, and **Add default tabs** greyed again, without saving or reopening.

**Old behaviour (not fixed, from the issue, `main` at `69ec676` underneath):** at step 4 the head zone line changed but the section still said "No room for a default tab at the head: the held region reaches the part." and **Add default tabs** stayed greyed; clicking other items did not refresh it; only **Save**, quit and reopen did.

## Report back

Reply on issue #604 with: **PASS**, or **FAIL and what you saw** (the default-tab lines and whether the button is enabled, at steps 2, 4 and 6). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

**Nobody has clicked this yet.** The section is drawn only in the interface, which cannot be run without a display here, and lane TABFRESH did not run it either. What backs the expected result:

- **The code** on `main` at `07ec6990` (built and checked out here, `--version` `07ec6990`, no uncommitted changes): the offered defaults and the "No room ..." line come from the same part-facts calculation, which now runs again on the setup as edited; **Add default tabs** is enabled only when the offered list is not empty, and adds exactly the offered tabs. The texts above are the code's (`crates/openchuck-app/src/workholding_view.rs`, `apps/openchuck-ui/src/workholding_panel.rs`).
- **The regression test** `req_ui_25_the_default_tabs_follow_a_change_to_the_stock` (#675), on a test cube, not on the baluster: after a longer stock is handed to the section in the same frame, the tail's "No room" line goes, the tail zone line appears and two tabs are offered. It checks the change in one direction only; step 6 (back again) is not covered by it.
- **The command line** cannot show the section. Its `setup --default-tabs` on this project is refused with or without the longer stock ("invalid workholding: the part comes to a point at X 61.000 mm, so there is no diameter to base a default tab on; add the tab by hand"), because it asks for both ends; the section offers the head tab alone. Reported to the coordinator; not part of this check.

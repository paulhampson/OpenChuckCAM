# Check for issue #604: the default tab and "Add default tabs" follow a stock change at once

> [!NOTE]
> **Draft, to run once a fix is merged.** No fix for #604 is on `main` at `1f3675d52` (8 Nov 2026), and no pull request names it. This file gives the issue's own steps on a public project and today's behaviour. When a fix merges, the expected result is checked against that build, the fix's pull request is named here, and this note is removed.

**What is to be fixed:** after the stock is changed on the Setup step, the Workholding section's zone lines follow the change, but the default tab's text ("No room for a default tab at the head ...") and the greyed **Add default tabs** button stay as they were until the project is saved and opened again.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the workholding section only. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)) is an unchanged copy of the user guide's tutorial project, written by [`generate.py`](generate.py): the public baluster (X 0 to 61) in 63 mm of 24 mm stock from X -1, the chuck holding 8 mm, no tabs. The grip reaches the part, so there is no room for a default tab at the head.

## Steps

Use the release candidate, or a build from `main` that holds the fix (**Help > About OpenChuckCAM** shows the commit).

1. Download [`604-tab-defaults-follow-stock.zip`](604-tab-defaults-follow-stock.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/604-tab-defaults-follow-stock/604-tab-defaults-follow-stock.zip)), unzip it anywhere, and open the folder `604-tab-defaults-follow-stock/project` with **File > Open project**.
2. Click **Setup** at the top and **Setup 1** in the project panel. Scroll the Inspector to **Workholding** and note what it says about a default tab at the head, and whether **Add default tabs** is greyed.
3. Under **Stock**, set **Length** to `78` and press <kbd>Enter</kbd>, then **Start X** to `-13` and press <kbd>Enter</kbd>.
4. Look at the **Workholding** section again, without saving or clicking anything else.

## Expected result

- **Step 2**: "No room for a default tab at the head: the held region reaches the part." and **Add default tabs** greyed (as today).
- **Step 4**: the section shows "Head zone X -5.000 to 0.000 mm (5.000 mm)", **and** the default tab line now offers a tab at the head, "Default tab, not added · Head · Frustum · X -5.000 to 0.000 · Ø 14.000 to Ø 7.000 mm", **and Add default tabs** is enabled, straight away.
- Pressing **Add default tabs** then adds that tab.

**Old behaviour (today, from the issue, `main` 69ec676 underneath):** at step 4 the head zone line changes but the section still says "No room for a default tab at the head: the held region reaches the part." and **Add default tabs** stays greyed. Clicking other items does not refresh it; only **Save**, quit and reopen does.

## Report back

Reply on issue #604 with: **PASS**, or **FAIL and what you saw** (the default tab line and whether the button is enabled). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Not run: the section's defaults are worked out in the interface, which cannot be run without a display here, and the command line has no equivalent of the stale state. `openchuck-cli setup project "Setup 1" --stock cylinder=24,78,-13` on `main` at `1f3675d52` stores the new stock and reports "checks: no problems found", so the stock values of step 3 are valid.

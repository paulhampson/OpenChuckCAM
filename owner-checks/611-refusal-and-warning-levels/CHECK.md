# Check for issue #611 and decision 0539: a refusal and a warning in the workholding section each keep their own level

**What was fixed** (pull request #623 for #611, and #591 for [decision 0539](../../docs/decisions/0539-workholding-apply-warns-on-a-tab-with-no-room-or-tabs-that-overlap.md)):

- When **Apply** is refused because a tab is outside the end zones, only that tab's row has the red Error mark. A warning found at the same time (here, a tab shorter than the tool) keeps its amber Warning mark. Before, every finding was drawn red. The command line now prints the warning as `warning:` before the refusal; before, it was folded into the error.
- An automatic tab with no room is stored by **Apply** with a warning (decision 0539). Before, Apply refused it.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the workholding section's marks and messages only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's baluster (X 0 to 61) in 78 mm of 24 mm stock from X -13, the chuck holding 8 mm (X -13 to -5), and **tab 1**, placed automatically at the head: a cylinder 14 mm across that fills the head zone, X -5 to 0 (5 mm long). One axial finishing operation, **Whole part**, uses a 6 mm ball (T6), wider than the tab is long, so the section warns that tab 1 is shorter than the tool.

## Steps

Use a nightly or release-candidate build made from commit `3b99ea23e` or later (**Help > About OpenChuckCAM** shows the commit), or a build from `main`. You do not need to generate anything.

1. Download [`611-refusal-and-warning-levels.zip`](611-refusal-and-warning-levels.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/611-refusal-and-warning-levels/611-refusal-and-warning-levels.zip)), unzip it anywhere, and open the folder `611-refusal-and-warning-levels/project` with **File > Open project**.
2. Click **Setup** at the top, click **Setup 1** in the project panel, and scroll the Inspector to **Workholding**.
3. **A refusal and a warning together (#611).** Press **Add tab**. On the new tab 2, untick **Place automatically** if it is ticked, set **Shape** to Cylinder, **Start X** `30`, **Length** `5`, **Diameter** `10` (press <kbd>Enter</kbd> after each). Press **Apply**.
4. Look at the rows of tab 1 and tab 2, their marks and text colour, and at the tabs in the view.
5. On tab 2's row press **...** and **Remove**. Press **Apply**.
6. **An automatic tab with no room (0539).** Set **Grip length** (under **Held at the start**) to `13` and press <kbd>Enter</kbd>. Press **Apply**.
7. Look at tab 1's row and at the **Messages** list.

## Expected result

- **After step 3** Apply is refused and nothing is stored (the project panel still shows one tab).
  - **Tab 2's** row has the red **Error** mark and red text, and tab 2 is drawn as failing in the view. The command-line tool prints its text as:

    > Tab 2 (X 30.000 to 35.000 mm) is not wholly in the head or the tail zone. Tabs lie between the held region (or the end of the stock) and the part, or between the part and the other end.

  - **Tab 1's** row has the amber **Warning** mark and is **not** red; tab 1 is not drawn as failing. Its text:

    > Tab 1 is 5.000 mm long along X, shorter than the tool T6 6 mm ball (check 611) (6.000 mm) of 1 Whole part. Make the tab longer or use a smaller tool.

- **After step 5** Apply stores the workholding; tab 1 keeps its amber Warning.
- **After step 6** Apply stores the 13 mm grip (it is not refused). Tab 1's row is amber with the Warning mark, the section says the head zone has no room for a tab, and the operation becomes stale. The Messages list has this warning:

  > Tab 1 is placed automatically at the head of the part, but there is no room there: the held region at the start (or the start of the stock) reaches the part. It keeps its last position; move the part or shorten the held region, or untick "Place automatically" and place it by hand.

**Old behaviour (not fixed):** in step 3 both rows were red with the Error mark, although tab 1's finding is only a warning; in step 6 Apply was refused with the "no room" text as an error, and nothing was stored.

## Report back

Reply on issue #611 with: **PASS**, or **FAIL and what you saw** (the step, each row's mark and colour, and the text). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

The interface cannot be run without a display here, so steps 3 to 7 were not run in the interface. The command-line tool makes the same check as **Apply** (`openchuck-cli setup project "Setup 1" ...`, which stores nothing when it refuses). Linux x86_64, release builds, on 8 November 2026.

| Build | `--tab cylinder=30,5,10` (step 3) | `--held-start 13` (step 6) |
| --- | --- | --- |
| Before both fixes: `1fccd5847` (0.49.0), the parent of #584, before #591 and #623 | Exit 1. One `Error:` holding both texts, the tab 2 refusal and, on the next line, the tab 1 text: the warning is reported as part of the error. Nothing stored. | Exit 1. `error: tab 1 is placed automatically at the head of the part, but there is no room there: ...`. Nothing stored. |
| `main` at `1f3675d52` (0.50.0) | Exit 1. `warning: Tab 1 is 5.000 mm long along X, shorter than the tool ...` first, then `error: Tab 2 (X 30.000 to 35.000 mm) is not wholly in the head or the tail zone. ...`. Nothing stored (the project files are unchanged). | Exit 0. `warning: tab 1 is placed automatically at the head of the part, but there is no room there: ...`; the setup is stored with a 13 mm grip and tab 1 still at X -5 to 0. |

The panel's colours and marks are covered by the tests named in #623 (`workholding_panel.rs`, "A refused Apply marks only what refuses (issue #611, decision 0539)"); the look on screen is what this check asks you to confirm. `openchuck-cli validate project` on `main` reports the project valid, with the tab-length and "cannot cut down beside tab 1" warnings expected for this setup.

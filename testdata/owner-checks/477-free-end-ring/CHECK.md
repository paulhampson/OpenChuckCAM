# Check for issue #477: a free-end ring that would part the part from the chuck

**What was fixed** (pull request #490, [decision 0500](../../../docs/decisions/0500-free-end-ring-warns-when-the-part-is-held-by-nothing.md), your option A): with **Face free ends** on and a **Stepdown**, finishing cuts a ring beside a free end face down to the rotary axis. When nothing holds the part on its own side of that ring (no held region at its other end and no tab joining it to one), the last layer parts the part from the stock the chuck grips. The program now warns of it and still generates (no move changes). Before, it said nothing.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. Do not run the G-code of this project on a machine, not even as an air cut: in this layout the part would be cut free. Generated G-code must be verified by you before it is run on a machine.

## The project

[`project/`](project/), written by [`generate.py`](generate.py) from public material only: the user guide's baluster (X 0 to 61) in 24 mm stock from X -13 to X 61. The chuck holds 8 mm (X -13 to -5); between the chuck and the part's start there are 5 mm of free stock and **no tab**, so the end face at X 0 is a free end. The stock ends at the part's other end (X 61), where **nothing holds it**. One operation, **1 Finish with free ends**: helical finishing with a 3.175 mm ball, **Face free ends** on, **Stepdown** 2 mm.

## Steps

Use the nightly build of 8 November 2026 (commit 76796fb24) or later, or a build from `main`.

1. Copy the folder `project` to a folder of your own and open the copy with **File > Open project**.
2. Click **Operations** at the top, then **Generate**. It takes a few seconds.
3. Click **1 Finish with free ends** and read the **Messages** list.
4. Now hold the far end too. Click **Setup** at the top; in the project panel, under **Setup 1**, click **Stock** and set **Length** to `80` (press <kbd>Enter</kbd>). Click **Workholding**, tick **Held** under **Held at the end**, set **Grip length** to `4`, and click **Apply**.
5. Click **Operations**, then **Generate stale**, and read the **Messages** list again.

You can ignore lines saying "the machine's feed limits slow this move".

## Expected result

- After step 2, the operation is generated with this warning (as the command-line tool prints it; the Messages list shows it on the operation, with **Show setting** pointing at Face free ends):

  > the end face at X 0.000 is faced down to the rotary axis: the tool cuts a ring 3.175 mm wide from X -3.176 to -0.001 down to the axis, and nothing in this setup holds the part on the other side of the ring (no held region at its other end, and no tab joining it to one), so the part can be cut free there and thrown while the rotary axis turns. Before running this program, check the stock and the workholding on the machine

  It also says "the end face at X 0.000 has nothing beyond it: the tool goes down beside it in 7 layers of 2 mm to the rotary axis, ..." (the ring itself, decision 0344) and "the stock ends at the part's end at X 61.000: there is no stock past this end to face ...".
- After step 5, with the far end held, the "nothing in this setup holds the part" warning is **gone**; the "7 layers of 2 mm" line stays.

**Old behaviour (not fixed):** after step 2 there was no warning that the part could be cut free; only the "7 layers of 2 mm" line.

## Report back

Reply on issue #477 with: **PASS**, or **FAIL and what you saw** (the message text after steps 2 and 5).

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project`), Linux x86_64, 8 November 2026. "Held far end" is the project as after step 4 (stock 80 mm, 4 mm held at the end).

| Build | Project as written | Held far end |
| --- | --- | --- |
| Before the fix: `a8b64368c` (0.35.0), the parent of the fix's commit `a8c88d23` | Generated; no "holds the part" warning (the "7 layers of 2 mm" line only) | Generated; no "holds the part" warning |
| `main` at `ae608e28f` (0.50.0) | Generated; the "nothing in this setup holds the part on the other side of the ring" warning above, and the "stock ends at the part's end at X 61.000" line | Generated; no "holds the part" warning |

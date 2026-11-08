# Check for issue #547: the flute-length check and stock outside the selected faces

**What was fixed:**

- pull request #551 ([decision 0522](../../../docs/decisions/0522-the-flute-check-warns.md)): the flute check warns instead of refusing, and the warning names the operation and the tool;
- pull request #554 ([decision 0524](../../../docs/decisions/0524-flute-check-reads-the-stock-in-the-tools-reach.md)): the depth the check uses is taken from the stock within the tool's reach of the path, not from the stock farthest from the axis anywhere in the setup (the held region and the tabs).

(The tab part of the issue, pull request #552, has its own check: [548](../548-tab-length-limited-op/CHECK.md).)

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. Do not run the G-code of this project on a machine, not even as an air cut: it is made only to show the messages. No flute warning does not mean the shank or holder clears the stock. Generated G-code must be verified by you before it is run on a machine.

## The project

[`project/`](project/), written by [`generate.py`](generate.py) from public material only: the user guide's baluster (X 0 to 61) in 24 mm stock from X -13, the chuck holding 8 mm (X -13 to -5), a tab at each end (X -5 to 0, 14 mm across; X 61 to 65, 6 mm across). Two operations:

- **1 Rough**: rotary roughing of the whole part with a 3.175 mm flat end mill, leaving 0.3 mm;
- **2 Finish vase**: helical finishing with **Short-flute ball 3.175x5 (check 547)**, a ball with only **5 mm** of flute, limited to the selection set **Vase** (the 1,440 triangles from X 29 to 44, the fluted vase). After roughing, the stock next to the vase is shallow; the deep stock is in the held region and round the tabs, which this operation never reaches.

## Steps

Use the nightly build of 8 November 2026 (commit 76796fb24) or later, or a build from `main`.

1. Copy the folder `project` to a folder of your own and open the copy with **File > Open project**.
2. Click **Operations** at the top, then **Generate**. It takes under a minute.
3. Click **2 Finish vase** and read its status and the **Messages** list.
4. Now make the flutes too short for the stock the path really reaches. With **2 Finish vase** selected, under **Tool** click **Edit tool...**, set **Flute length** to `3`, click **Close**, then **Generate**.
5. Read the **Messages** list again.

You can ignore lines saying "the machine's feed limits slow this move".

## Expected result

- After step 2, **2 Finish vase** is generated with **no** line starting "Flute length". Its only note is "the tool does not turn at a step across the rotary axis (a shoulder facing along X) at X -3.411: ...", which is not about the flutes.
- After step 4, there is one warning (the path is still made). The command-line tool prints it as below; the Messages list starts it "Flute length (operation 'Finish vase', tool 'Short-flute ball 3.175x5 (check 547)'): ...":

  > Flute length: the stock may stand up to 4.005 mm above the tool's tip along its axis (a bound, from how far the stock reaches from the rotary axis or across a step between readings: that stock is not measured exactly across the tool) (at X 44.883, A 275.5), more than the tool's 3.000 mm of flute, so the shank may meet the stock; the stock is what the earlier operations in this setup left. The path was made: check it before running it. Use a tool with longer flutes, or rough the part first in this setup, or turn on Finishing layers

  This shows the check still warns where the path does reach stock deeper than the flutes (at the end of the vase, beside the step down to the neck).

**Old behaviour (not fixed):** at step 2, **2 Finish vase** was refused, with no operation named and nothing generated for it:

> parameter finishing_layers: the stock may stand up to 5.523 mm above the tool's tip along its axis (a bound, ...) (at X 44.875, A 270.0), more than the tool's 5.000 mm of flute, so the shank would meet the stock; ...

**What this does and does not show.** On your carving the point named was in a tab outside the selection. On this public part the old refusal names a point at the end of the selection (X 44.9), but its depth (5.523 mm) comes from the stock farthest from the axis, not from the stock beside that point: with the fix, the same place reads 4.005 mm. The check shows that stock the path cannot reach no longer sets the depth; it cannot show your project's numbers.

## Report back

Reply on issue #547 with: **PASS**, or **FAIL and what you saw** (step, operation, message text). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project`), Linux x86_64, on 8 November 2026.

| Build | Step 2 (5 mm flutes) | Step 4 (3 mm flutes) |
| --- | --- | --- |
| Before the fix: `eaeba831d` (0.44.0), the parent of pull request #551's merge commit `bbf00f3a` | `error: parameter finishing_layers: the stock may stand up to 5.523 mm above the tool's tip ... (at X 44.875, A 270.0), more than the tool's 5.000 mm of flute, so the shank would meet the stock; ...` | not run |
| `main` at `ae608e28f` (0.50.0) | Generated; no flute warning | Generated; the "Flute length: the stock may stand up to 4.005 mm ... (at X 44.883, A 275.5), more than the tool's 3.000 mm of flute ..." warning |

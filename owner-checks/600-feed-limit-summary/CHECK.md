# Check for issue #600: the feed-limit lines, and the "0.0 mm/min" at the rotary axis

**What was fixed** (pull request #625, [decision 0536](../../docs/decisions/0536-message-levels-operation-reference-and-setting-links.md) build step 2, with [decision 0430](../../docs/decisions/0430-results-feed-limit-summary-and-copyable-results.md)): the moves the machine's feed limits slow are told as one information line per operation, not one warning per move, in the command line as in the Results tab. A turn of A with the contact point at the rotary axis no longer reads as "the contact feed is 0.0 mm/min": it has its own note that says what the move is. The G-code does not change.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at messages and feed words only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)) is an unchanged copy of the user guide's tutorial project, [`testdata/examples/baluster/baluster-project`](../../testdata/examples/baluster/baluster-project/), written by [`generate.py`](generate.py): the public baluster, a **Rough** and a **Finish** operation. The Finish faces the free end at X 61 in five rings down to the rotary axis (moves 19077 to 19148), which is where the "0.0 mm/min" lines came from.

## Steps

Use a nightly or release-candidate build made from commit `1f3675d52` or later (**Help > About OpenChuckCAM** shows the commit), or a build from `main`.

1. Download [`600-feed-limit-summary.zip`](600-feed-limit-summary.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/600-feed-limit-summary/600-feed-limit-summary.zip)), unzip it anywhere, and open the folder `600-feed-limit-summary/project` with **File > Open project**.
2. Click **Operations** at the top, then **Generate**. It takes under a minute.
3. Open the **Results** tab in the lower dock. Read the lines about the machine's feed limits.
4. Click **Finish** in the project panel and read the line under its feed field in the Inspector ("Last run: ...").
5. Optional, for the program itself: **File > Post and export...** with the Makera Z1 post, and in the **Preview** use **Find** for `A41760`. Read the F words of the last five rings.
6. In the same **Preview**, read the head of the program (its first lines), for the safety notice of [decision 0533](../../docs/decisions/0533-safety-notice-post-option.md) (#570).

## Expected result

> [!NOTE]
> Since decision 0536 step 4 (#659, on `main` from `0d640692`) the text after the topic starts in lower case and every line ends with a full stop; the quotes below are as `0d640692` prints them. Builds before that print the same sentences with a capital after the lead-in or without the last full stop; either form passes.

> [!NOTE]
> The texts below are those of the release candidate's form: since decision 0536 step 3 (#648, on `main` from `645256c3`) every line starts with a topic and the operation, such as "Feed limit (operation 'Rough'): ...". A build from `1f3675d52` up to that change prints the same sentences without the lead-in ("9536 moves during 'Rough' were limited ...", "operation 'Finish': moves 19077 to 19148 ..."); either form passes.

- **Results** has one line per operation about the feed limits, and none per move. As the command-line tool prints them:

  > Feed limit (operation 'Rough'): 9536 moves were limited by the machine's feed limits. In the worst case the target was 1000.0 mm/min, but the move was reduced to 22.0 mm/min to stay within the machine's capabilities.

  > Feed limit (operation 'Finish'): 7818 moves were limited by the machine's feed limits. In the worst case the target was 500.0 mm/min, but the move was reduced to 0.7 mm/min to stay within the machine's capabilities.

- The rings at the free end have their own note, with no "0.0 mm/min" in it:

  > Rotary axis (operation 'Finish'): moves 19077 to 19148 (72 moves): A turns with the contact point at the rotary axis during these moves, so the contact point barely moves and the rotary axis's speed limit sets the time taken (0.083 s); the feed set is 500.0 mm/min.

- No line anywhere says "the contact feed is 0.0 mm/min" or "as low as 0 mm/min".
- These lines are information (no warning mark on **Rough** or **Finish** because of them).
- Step 5: the last five rings are written `A41760.000 F551.3`, `A42120.000 F488.5`, `A42480.000 F425.7`, `A42840.000 F362.8` and `A43200.000 F362.8`; no block has `F0`.
- Step 6: the program starts with these lines (the notice is the third and fourth):

  ```text
  (Baluster - Setup 1)
  (OpenChuckCAM <the version of your build>)
  (This program has not been run on your machine. Check it before you run it.)
  (Simulate it and air cut it first. You are responsible for what the machine does.)
  ```

**Old behaviour (not fixed):** the command line printed 17,426 warnings, one per slowed move, such as `warning: operation 'Finish': move 19077: the machine's feed limits slow this move: the contact feed is 0.0 mm/min, not 500.0` (72 of them read 0.0 mm/min). Results said the Finish move "was reduced to 0.0 mm/min", and the Inspector "slowed 7890 moves to as low as 0 mm/min".

**Not part of this check:** the other Finish warnings (the shoulder at X 8.589, and nothing cut within one tool radius of the held region) are expected for this job. The "worst case 0.7 mm/min" is a move near the axis at the finial's point, not a zero feed; tell us if you think a value that low should be said differently.

## Report back

Reply on issue #600 with: **PASS**, or **FAIL and what you saw** (the line's text, or the F word). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post makera-z1 --tool-numbers keep --out OUT`), Linux x86_64, release builds, on 8 November 2026.

| Build | Result |
| --- | --- |
| Before the fix: `9dae85d30` (0.50.0), the parent of the fix's merge commit `1f3675d52` | 17,438 lines on standard error, 17,426 of them `warning: operation '...': move N: the machine's feed limits slow this move: ...`; 72 read `the contact feed is 0.0 mm/min, not 500.0` (Finish, moves 19077 to 19148). |
| `main` at `1f3675d52` (0.50.0) | 17 lines on standard error: the two `note:` summary lines and the `note:` for moves 19077 to 19148 quoted above; no per-move line, no "0.0 mm/min". |
| `main` at `81bade8a3` (0.51.0), after decision 0536 steps 3 and 5 | The same three notes, now with their lead-ins ("Feed limit (operation 'Rough'): ...", "Rotary axis (operation 'Finish'): ..."); no per-move line, no "0.0 mm/min". The program differs from `1f3675d52`'s only in the version comment on line 2. |

Both programs start with the two notice lines of step 6 (the safety notice is in both builds; it came with #570, before either). The two programs are byte-identical (`cmp`), so the fix changed only the messages. The F words in step 5 are those of both programs; a search for `F0` or a negative F finds nothing. The UI's Results tab and Inspector were not run here (they need a display); they read the same summary, so their numbers should match the command line's.

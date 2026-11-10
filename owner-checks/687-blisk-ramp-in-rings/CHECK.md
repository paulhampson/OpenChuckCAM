# Check for issue #687: a ramp in Rings on the public blisk generates

**What was fixed** (pull request #761, lane RAMPENTRY, [decision 0543](../../docs/decisions/0543-a-ramp-entry-follows-the-toolpath.md) as amended for #687): two things refused a ramp entry on the blisk.

- **The ramp's own check.** A ramp was checked by dropping the tool along its axis at each ramp position. On a ring beside the disc's end face the tool's edge touches the face sideways, the drop scored the face, and the pass's own points failed ("pass 3 ramps over its own path, yet the tool would cut 2.404 mm into the finished surface ... please report it as a defect"). Now each ramp move is checked against the pass beneath it. That refusal can no longer be reached; it is kept as a defect report.
- **The shortest ramp** (decision 0543 D3). A ramp that goes back and forth over a short stretch needed a stretch of at least the tool radius; between the blades many rings start with less ("pass 438 starts with a stretch of 1.484 mm, shorter than the 1.500 mm (the tool radius) ..."). You chose to lower the minimum to ten path tolerances; each leg is still at least the tool radius wherever the stretch allows. A pass ramped over less than the tool radius is listed with the plunged passes, so a tool that cannot cut straight down is refused there.

Your own project from the issue is for you to re-run (numbers only). This check uses the public blisk example instead.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at whether the operation generates and at its notes. A ramp over a short stretch, many times reversing A, says nothing about chip load or tool suitability. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): a copy of the public blisk example, [`testdata/examples/blisk-start/blisk-project`](../../testdata/examples/blisk-start/blisk-project/) (the repository's `blisk.step`, X 10 to 60, in 42 mm stock from X 9; the 3 mm flat end mill), with one operation set as lane RAMPENTRY's public-blisk test sets it:

- **Rough rings, ramp**: continuous rotary roughing in **Rings**, 3 mm flat end mill, Stepover 1 mm, Stepdown 1 mm, Stock to leave 0, Entry **Ramp** at **3°**, Allow single-point plunge on, Steeper ramp near the axis on with Maximum ramp angle 20°.

## Short list (what to click)

1. Open the project, **Operations**, **Generate** (a few seconds).
2. **Rough rings, ramp** generates; read its three Ramp entry notes.

## Steps

Use the release candidate, or a build from `main` at commit `70457312` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`687-blisk-ramp-in-rings.zip`](687-blisk-ramp-in-rings.zip) (or take it from the raw link: https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/687-blisk-ramp-in-rings/687-blisk-ramp-in-rings.zip), unzip it anywhere, and open the folder `687-blisk-ramp-in-rings/project` with **File > Open project**.
2. Click **Operations**, then **Generate**.
3. Read the status and the **Results** notes of **Rough rings, ramp**. Optionally zoom on the disc's end face (X 10) and between the blades.

## Expected result

As the command-line tool prints them on `main` at `a3e1ad22` and `55e6348d` (both after #761):

- **Rough rings, ramp** generates (199,165 moves), with:
  - "Ramp entry (operation 'Rough rings, ramp'): 1769 passes, from pass 2 to pass 1804 (2, 3, 4 ... 1802, 1803, 1804) start with too short a stretch to ramp along without turning the part, so their ramps go back and forth over each pass's own path, reversing A 20795 times in all during the entries."
  - "Ramp entry (operation 'Rough rings, ramp'): 56 passes, from pass 9 to pass 1793 (9, 242, 428 ... 1768, 1771, 1793) drop more steeply than the ramp angle just before their first point, so their ramps follow that drop there, as the passes themselves do."
  - "Ramp entry (operation 'Rough rings, ramp'): 34 passes, from pass 22 to pass 1784 (22, 53, 133 ... 1670, 1747, 1784) are single points with no path for a ramp to follow, so they are entered by a straight plunge, as Allow single-point plunge asks."
- No "please report it as a defect".

**Old behaviour (`main` at `5677b10`, just before #761):** refused: "Ramp entry (operation 'Rough rings, ramp'): pass 438 starts with a stretch of 1.484 mm, shorter than the 1.500 mm (the tool radius) a ramp needs to go back and forth over. Set Entry to Plunge or Helix."

## Report back

Reply on issue #687 with: **PASS**, or **FAIL and what you saw** (the status and the notes), and the numbers from re-running your own project. Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT`), Linux x86_64, release builds each in a target folder of its own, 10 November 2026, on `main` at `5677b10` (before), `a3e1ad22` and `55e6348d` (after; `--version` matching, no uncommitted changes):

| | `5677b10` (before #761) | `a3e1ad22` and `55e6348d` (after) |
| --- | --- | --- |
| Rough rings, ramp | refused at pass 438 (stretch 1.484 mm < tool radius 1.500 mm) | generates, 199,165 moves; the same program on both builds (SHA-256 `f6f2ae3f05ff8236…`) |
| The same with the stock from X 7 or X 5 | refused at pass 441, the same reason | generates (199,606 and 199,653 moves) |
| The same in line passes (Passes not Rings) | generates, 342,150 moves | the same program |

- **The "finished surface" refusal of the issue (pass 3) did not appear through the application on the public blisk**, in Rings or in line passes, with the stock from X 9, 7 or 5: before the fix this project is refused first by the shortest-ramp rule. Lane RAMPENTRY's test shows that case on the public blisk mesh through the pathing library (`req_op_11_req_op_03_roughing_the_blisk_in_rings_ramps_beside_its_end_face_and_over_short_steps` in `crates/openchuck-path/tests/ramp_follows.rs`, 182 s, listed as long). Your re-run of your own project covers it through the application.
- The programmed time of the generated program is 79,622 s (about 22 hours; the replay's figure, much of it in feed-limited A moves), as a plain fact; it is not part of the check.
- The view cannot be run without a display here.

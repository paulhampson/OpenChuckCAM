# Check for issue #637: the mark on areas a strategy could not reach is comfortable to look at

> [!NOTE]
> **Draft, to run on the release candidate.** [Decision 0549](https://github.com/paulhampson/OpenChuckCAM/pull/651/files) (lane VIEWHATCH, accepted by the owner on 8 Nov 2026, pattern A) is in docs pull request #651; the code is in draft pull request #652 and not merged. This file gives today's behaviour, read from `main` at `1f3675d52` and checked again at `81bade8a3`, and what the record says the fix does. When it merges, its pull request is named here and this note is removed.

**What is to be fixed:** the view marks the areas a strategy could not reach with dots. The dots stay still on the screen while the part turns, instead of being painted on the surface, which is uncomfortable to look at and may trigger trypophobia. The issue asks for a better pattern.

**Time:** about 3 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the view only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)) is the project of the #512 check, which you attached to the issue, written again by [`generate.py`](generate.py) with that check's generator: the user guide's blisk starting project with face-flow finishing (**Floor finish**) on the hub floor between two blades. It leaves one strip it cannot reach next to the far blade.

## Steps

Use the release candidate, or a build from `main` that holds the fix (**Help > About OpenChuckCAM** shows the commit).

1. Download [`637-unreachable-pattern.zip`](637-unreachable-pattern.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/637-unreachable-pattern/637-unreachable-pattern.zip)), unzip it anywhere, and open the folder `637-unreachable-pattern/project` with **File > Open project**.
2. Click **Operations**, then **Generate**. Click **1 Floor finish**.
3. Look at the unreached strip on the hub floor next to the far blade (the Messages line "Unreached areas (operation 'Floor finish'): face-flow finishing cannot reach 1 area ..." has a link that shows it). Turn the part with the mouse, zoom in and out.
4. Do the same with the view's light and dark themes if you use both.

## Expected result

- The unreached strip is **hatched**: black diagonal lines in a white casing, 1 mm apart, painted on the surface. They move, turn and foreshorten with the part when you turn and zoom; they do not stay still on the screen.
- From far away, or at a steep angle, the lines fade into a flat dark tint instead of shimmering. No small circles remain anywhere.
- The part's grey and any deviation colours show between the lines. The legend row "Unreachable (Floor finish)" shows a swatch of two hatch lines.
- The strip reads as one closed area along its length (the owner asked for the area's edge to be cleaned up): no single ticks across a narrow strip with gaps between them.
- The Messages line still names the area: "Unreached areas (operation 'Floor finish'): face-flow finishing cannot reach 1 area of the selected faces, which stay uncut: X 20.000 to 60.000 near A 282 (208.6 mm2), a neighbouring face is in the way of the tool; check them in the simulation before running the program" (as `main` at `81bade8a3` prints it).

**Old behaviour (today):** dots that stay still on the screen while the part turns (the issue's screenshot).

## Report back

Reply on issue #637 with: **PASS**, or **FAIL and what you saw** (what the pattern looks like, and whether it moves with the part). A screenshot of the public blisk is fine. Numbers and messages only otherwise; no project files from your own work.

## What we saw when preparing this

The view cannot be drawn without a display here, so nothing in this check was run; the old behaviour is from the issue. The project is byte-identical to the #512 check's project (`diff -r`). `openchuck-cli validate` reports it valid on `main` at `1f3675d52`, with the STEP reader built from that commit.

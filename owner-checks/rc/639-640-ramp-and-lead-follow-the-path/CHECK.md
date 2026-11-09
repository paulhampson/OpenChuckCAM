# Check for issues #639 and #640: a ramp follows the pass, and a lead on a ring stays in the plane of the cut

> [!NOTE]
> **Draft, to run on the release candidate. The code is not merged.** [Decision 0543](https://github.com/paulhampson/OpenChuckCAM/pull/646/files) (a ramp entry follows the toolpath, with its points found while building) and decision 0544 (a lead on a pass that turns A) are in docs pull request #646. The ramp is built on lane RAMPENTRY's branch `claude/lane-rampentry-wip`, the lead on lane RAMPLEAD's `claude/lane-rampentry-lead-wip`; no pull request is merged. This file gives `main` at `81bade8a3`, what those branches did on 8 Nov 2026, and what the records say. When the code merges, the expected texts are copied from the merged build, the pull request is named here, and this note is removed.

**What is to change:**

- **Ramp (0543):** a ramp follows the pass's own path, raised along the tool axis, from the approach distance above the stock to the pass's own depth at its first point. On a ring it comes down around the ring at the ring's X while A turns the Rotation way; on a line it goes back and forth over the start of the pass. Where a pass starts with too short a stretch, it goes back and forth over the pass's own path, A reversing during the entry only.
- **New settings** (Entry, links and retract; each off by default):
  - **Allow single-point plunge**: a pass that is a single point has no path for a ramp; off, the operation is refused naming this setting; on, the tool plunges there and a note names the passes.
  - **Steeper ramp near the axis**, with **Maximum ramp angle** (default 15°): a ring ramp that would need more than 10 turns at the Ramp angle uses a steeper angle, just enough for 10 turns, never above the maximum.
  - **Plunge to remove face nubs**, with **Face nub diameter** (roughing only).
- **A ring with its tip on the rotary axis** (an end face): the tool descends there while the part turns, as plunge moves at the plunge feed; a note gives the turns.
- **Lead (0544):** on a ring or a spiral, the lead-in and lead-out are arcs in the plane of the cut at the ring's X, A and Z moving together; they no longer run along X. A held region at the starting end no longer refuses it.

**Time:** about 15 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at where the entry moves go and at the messages. A ramp that follows the path says nothing about chip load or tool suitability; the first ring of a layer is still entered at full tool width. Do not run this project's G-code on a machine, not even as an air cut. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): the user guide's tutorial project (the public baluster in 63 mm of 24 mm stock from X -1, the chuck holding 8 mm) with its two operations replaced by five, all with a 3° ramp unless named otherwise, run in this order:

1. **Rough rings, ramp**: rotary roughing in Rings, Entry Ramp, Allow single-point plunge on.
2. **Rough spiral, ramp**: rotary roughing (spiral), Entry Ramp, Allow single-point plunge on. Expected to be **refused** (the 10-turn bound).
3. **Rough spiral, steeper**: as 2, with Steeper ramp near the axis on (Maximum ramp angle 15°).
4. **Finish rings, ramp**: helical finishing in Rings, Entry Ramp, Allow single-point plunge on.
5. **Finish rings, lead**: helical finishing in Rings, Entry Lead, on **Setup 2, no grip**: the same stock with no held region. With the grip, this pass starts with a short cut along X before its first ring, so it keeps today's lead and is refused at the held region; that case waits for the owner's decision (decision 0544 L2) and is not part of this check.

## Steps

Use the release candidate, or a build from `main` that holds the fix (**Help > About OpenChuckCAM** shows the commit).

1. Download [`639-640-ramp-and-lead-follow-the-path.zip`](639-640-ramp-and-lead-follow-the-path.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/639-640-ramp-and-lead-follow-the-path/639-640-ramp-and-lead-follow-the-path.zip)), unzip it anywhere, and open the folder `639-640-ramp-and-lead-follow-the-path/project` with **File > Open project**.
2. Click **Operations**, then **Generate**. Read each operation's status and its Messages and Results lines.
3. Click **1 Rough rings, ramp** and **4 Finish rings, ramp**, and look at where each pass comes down: over the pass, around the ring at its X; nothing runs along X towards the chuck.
4. Click **5 Finish rings, lead** (on Setup 2) and look at its first lead-in at the head end (X 0).
5. Open the Entry, links and retract section of any operation and read the three new settings and their tooltips.
6. Optional: **File > Post and export...** with the LinuxCNC post, one file per operation, and in the **Preview** read the first `G1` moves of **Finish rings, ramp**: the ramp's Z falls while A turns, at one X.

## Expected result

As lane RAMPENTRY's branch prints them (`claude/lane-rampentry-wip` at `dd9357f07`, 8 Nov 2026), to be checked against the merged build:

- **1 Rough rings, ramp** generates, with the note "Ramp entry (operation 'Rough rings, ramp'): passes 31, 32, ... 86, 87 are single points with no path for a ramp to follow, so they are entered by a straight plunge, as Allow single-point plunge asks", and one note per pass that starts with a short stretch ("pass 4 starts with too short a stretch to ramp along without turning the part, so its ramp goes back and forth over the pass's own path, reversing A 5 times during the entry").
- **2 Rough spiral, ramp** is refused: "Ramp entry: pass 85 turns round the part with its tip 0.864 mm from the rotary axis, so at 3.00 degrees a ramp round it would take 17.6 turns, more than the 10 allowed. Raise Ramp angle, or turn on Steeper ramp near the axis, or set Entry to Plunge or Helix."
- **3 Rough spiral, steeper** generates, with a note that the pass near the axis was ramped more steeply ("... so its ramp is steeper, up to 3.69 degrees, as Steeper ramp near the axis asks"; 5.26° when it is generated on its own).
- **4 Finish rings, ramp** generates, with "pass 4 starts on the rotary axis, where a ramp has no path to follow, so the tool descends there while the part turns 3.64 turns".
- **5 Finish rings, lead** generates; its lead-in is an arc that turns with the part at the ring's X (A and Z moving together at X 0), not a quarter arc in the X-Y plane coming in along X. Without a grip, Setup 2 also gives two "Part cut free" warnings at the end faces, which need acknowledging before export; they are expected here and are not part of the check. *(Not yet seen on public material: see below.)*
- With Allow single-point plunge off, **1** and **3** are refused naming it: "Ramp entry: pass 33 is a single point, so there is no path for the ramp to follow. Turn on Allow single-point plunge, or set Entry to Plunge or Helix."

**Old behaviour (today, `main` at `81bade8a3`):** **1** to **4** are refused, for the new setting: "operation 'Rough rings, ramp' cannot be generated: parameter allow_single_point_plunge: this strategy has no parameter of that name". Without the new settings, the ramp runs along X from the first cut point and is refused where it reaches the held region, for example "pass 1: the ramp needs a run of 100.270 mm to descend 5.255 mm at 3.00 degrees, and it would run over the held region at the start of the stock at X 7.156 mm ...". **5**, on Setup 2 with no grip, generates with today's lead: a quarter arc in the X-Y plane from `X-1.588 Y1.587` to `X0.000 Y0.000` at Z 7, then the ring turning A. (On Setup 1, with the grip, it is refused: "pass 1: the lead-in needs an arc of radius 1.587 mm, and it would run over the held region at the start of the stock at X 7.604 mm ...".)

## Report back

Reply on issue #639 (operations 1 to 4) and #640 (operation 5) with: **PASS**, or **FAIL and what you saw** (the operation, where its entry goes, and the message). Numbers and messages only; no project files from your own work. The records also ask you to re-run your own projects behind OI-553 and OI-790 and report numbers only.

## What we saw when preparing this

Run with the command-line tool, Linux x86_64, release builds, 8 November 2026, each operation on its own and then 1, 3 and 4 together (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT`):

| Build | 1 | 2 | 3 | 4 | 5 |
| --- | --- | --- | --- | --- | --- |
| `main` at `81bade8a3` | refused (no such setting) | refused (no such setting) | refused (no such setting) | refused (no such setting) | generated, old X-Y lead |
| `claude/lane-rampentry-wip` `dd9357f07` | 11,951 moves | refused, 17.6 turns | 6,941 moves | 19,098 moves (after 1 and 3) | (no lead change on this branch) |
| `claude/lane-rampentry-lead-wip` `f7b27bcc3` | as above | as above | as above | as above | generated, old X-Y lead (program identical to `main`'s apart from the version line) |

In the LinuxCNC program of **4** the ramp reads `G0 X9.137 ... Z40.000 A0.000`, `Z9.829`, then three short moves along X to X 9.409 (the back and forth of a short stretch), then `Z9.797 A2.500`, `Z9.779 A5.000`, ... : Z falls while A turns, at one X.

**Not yet seen working on public material:**

- **The lead on rings (#640).** Lane RAMPLEAD reports that on `f7b27bcc3`, without a held region, the lead is the new one (A and Z together at X 0). Our build of that commit (`openchuck-cli --version`: `f7b27bcc3`, no uncommitted changes) gives, for **5** on Setup 2 (`regenerate project --post linuxcnc --tool-numbers keep --acknowledge-collision-warnings`), a program identical to `main`'s apart from the version line, with the old X-Y quarter arc. The difference is reported to the coordinator; this check expects the new lead and will be confirmed on the merged build.
- **With the grip**, the pass starts with a 0.5 mm cut along X before the first ring, keeps the line lead and is refused at the held region; the owner decides that case (decision 0544 L2), so this check does not expect it to generate.
- **Plunge to remove face nubs.** On the baluster, roughing in Rings with the setting on gives a program identical to the one without it, with 1 mm and with 5 mm of stock past the free end, so no public stand-in shows it yet; this check will gain one when the lane names a public case.
- The many per-pass "too short a stretch" notes (about fifty on **1**) and the wording "passes 1 drop ..." and "reversing A 1 times" were reported to the coordinator.

The interface (the view, the settings and tooltips) cannot be run without a display here.

# Check for decision 0536 build step 6: every refusal, the Facing notes and the setup checks in the one form

> [!NOTE]
> **To run on the release candidate.** Build step 6 is merged (pull request #684, `main` at `ee190429`, lane MSGREVIEW, [decision 0644](../../../docs/decisions/0644-refusals-take-the-one-form.md)); the texts below are those of that commit. This check is posted on the tracking issue for decision 0536 (#668).

**What changed** (decision 0644, which extends [decision 0536](../../../docs/decisions/0536-message-levels-operation-reference-and-setting-links.md) with build step 6):

- **Refusals.** An operation that cannot be generated used to read "operation 'X' cannot be generated: ..." followed by whatever the check wrote ("End facing: ...", "parameter stepdown: ...", or nothing). Now every refusal reads like every other message: "**Topic (operation 'X'): ...**", the topic naming the check that refused (End facing, Stepdown, Links, Lead entry, Ramp entry, ...), lower case after it, one full stop at the end. Where the refusal names one setting to change, **Show setting** opens the operation at that field.
- **The Facing notes** take the topic Facing: "Facing (operation 'X'): facing covered the end faces it was asked to." and "Facing (operation 'X'): intentionally unfaced: ...".
- **The setup checks on the command line** (`openchuck-cli setup`, `report` and the warnings before a run) read "Topic (setup 'X'): ....".
- **Passes are named alike**: a refusal that lists passes names them as the notes do ("88 passes, from pass 1 to pass 88 (1, 2, 3 ... 86, 87, 88), are entered ..."), not "88 passes, the first pass 1, are entered ...".
- **A count of one is singular**: "1 layer", "1 operation, 1 move".
- **No message names a requirement, decision or open item**: the links refusal lost its "(OP-14)".
- **OI-360** (a read of every message a dialog can show) is closed by your decision; that read was not done.

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at messages and links only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's tutorial project (the public baluster in 63 mm of 24 mm stock from X -1, the chuck holding 8 mm), with the public `cylinder.step` (radius 10, X 0 to 50) added as a second model, three setups and seven operations, each made to give one kind of message:

| # | Operation | Setup | What it is set to show |
| --- | --- | --- | --- |
| 1 | Face end | Setup 1 | Facing of the free end: the Facing note |
| 2 | Face end, wide stepover | Setup 1 | Stepover (slice thickness along X) of 5 mm, wider than the 3.175 mm tool: an End facing refusal |
| 3 | Rough, deep stepdown | Setup 1 | rotary roughing with a Stepdown of 30 mm, deeper than the 25 mm flutes: a Stepdown refusal |
| 4 | Rough rings, ramp | Setup 1 | rotary roughing in Rings, Entry Ramp, Allow single-point plunge off: a Ramp entry refusal |
| 5 | Finish | Setup 1 | the tutorial's Finish, which generates (its notes and warnings in the one form) |
| 6 | Finish, low clearance | Setup 2, low clearance | the same with the setup's safe cylinder at radius 10 mm, inside the 24 mm stock: a Links refusal, and two Clearance checks on the setup |
| 7 | Axial lead, through stock | Setup 3, cylinder in long stock | axial finishing of the cylinder, Entry Lead, with **O-flute, not centre cutting** (the tutorial's flat end mill marked as not centre cutting): a Lead entry refusal naming 88 passes |

## Steps

Use the release candidate, or a build from `main` at commit `ee190429` or later (**Help > About OpenChuckCAM** shows the commit; the library family is 0.55.0).

1. Download [`0536-step6-refusals-in-one-form.zip`](0536-step6-refusals-in-one-form.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/0536-step6-refusals-in-one-form/0536-step6-refusals-in-one-form.zip)), unzip it anywhere, and open the folder `0536-step6-refusals-in-one-form/project` with **File > Open project**.
2. Click **Operations**, then **Generate**. Read **Messages** and the **Results** tab.
3. On each refusal that offers it, click **Show setting**: operation 2 should open at **Stepover (slice thickness along X)**, 3 at **Stepdown**, 4 at **Allow single-point plunge**, 7 at **Entry**.
4. Open **Setup 2, low clearance** and read its checks.
5. Optional, on the command line: `openchuck-cli setup <project> "Setup 2, low clearance"`.
6. Optional, for the singular count: in the [`638b-rings-start-at-held-edge`](../../638b-rings-start-at-held-edge/CHECK.md) project, generate both operations together and read the **Free ends** note of **Finish rings, blended**.

## Expected result

As the command-line tool prints them on `main` at `ee190429`, each operation generated on its own:

- **1 Face end**: "Facing (operation 'Face end'): facing covered the end faces it was asked to."
- **2 Face end, wide stepover**: "End facing (operation 'Face end, wide stepover'): the Stepover (slice thickness along X) must be above zero and no greater than the cutting diameter." **Show setting** at Stepover (slice thickness along X).
- **3 Rough, deep stepdown**: "Stepdown (operation 'Rough, deep stepdown'): 30 mm is more than the tool's 25.000 mm of flute; a layer that deep would cut with the shank. Use a smaller Stepdown or a tool with longer flutes." **Show setting** at Stepdown.
- **4 Rough rings, ramp**: "Ramp entry (operation 'Rough rings, ramp'): pass 31 is a single point, so there is no path for the ramp to follow. Turn on Allow single-point plunge, or set Entry to Plunge or Helix." (the same words as before), now with **Show setting** at Allow single-point plunge.
- **5 Finish** generates; every note and warning starts with its topic and operation, for example "Free ends (operation 'Finish'): the end face at X 61.000 has nothing beyond it: the tool goes down beside it in 13 layers of 1 mm ...".
- **6 Finish, low clearance**: "Links (operation 'Finish, low clearance'): the stock reaches 12.000 mm from the rotary axis, beyond the retract height of 10.000 mm, so links could cross it. Enlarge the setup's clearance." (no "(OP-14)").
- **7 Axial lead, through stock**: "Lead entry (operation 'Axial lead, through stock'): 88 passes, from pass 1 to pass 88 (1, 2, 3 ... 86, 87, 88), are entered by a straight plunge into the stock (a single point, a ring on the rotary axis, where the tool descends while the part turns, a face nub, or a lead whose descent to its arc goes down through the stock), and the tool 'O-flute, not centre cutting' cannot cut straight down (it is not centre cutting). Set Entry to Helix, or choose a tool that can plunge."
- **Setup 2, low clearance** (step 4 and the command line): "Clearance (setup 'Setup 2, low clearance'): the safe cylinder (radius 10.000 mm) does not enclose the stock (radius 12.000 mm).", the same for the part (radius 11.000 mm), and "Held region (setup 'Setup 2, low clearance'): the grip at the start of the stock covers 7.000 mm of the part (X 0.000 to 7.000), which is never machined; shorten the grip if that length should be cut."
- **Step 6**: "... the tool goes down beside it in 1 layer of 1 mm to the rotary axis ...".
- No message names a requirement, decision or open item number.

**Old behaviour (`main` at `b12bfaed`, just before step 6):**

- 1: "operation 'Face end': Facing covered the end faces it was asked to."
- 2: "operation 'Face end, wide stepover' cannot be generated: End facing: the Stepover (slice thickness along X) must be above zero and no greater than the cutting diameter"
- 3: "operation 'Rough, deep stepdown' cannot be generated: parameter stepdown: 30 mm is more than the tool's 25.000 mm of flute; ..."
- 6: "operation 'Finish, low clearance' cannot be generated: the stock reaches 12.000 mm from the rotary axis, beyond the retract height of 10.000 mm, so links could cross it (OP-14); enlarge the setup's clearance"
- 7: "... 88 passes, the first pass 1, are entered ..." with no full stop at the end.
- The setup listing: "  - the safe cylinder (radius 10.000 mm) does not enclose the stock (radius 12.000 mm)", with no topic, no setup and no full stop.
- Step 6: "... in 1 layers of 1 mm ...".

## Report back

Tick **Step 6 passes** on the tracking issue (#668), or reply there with **FAIL and what you saw** (the operation, the message, and where Show setting went). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT`, each operation on its own; `openchuck-cli setup project SETUP`), Linux x86_64, release builds each in a target folder of its own, 10 November 2026, on `main` at `ee190429` (`--version` `ee190429`, no uncommitted changes) and at `b12bfaed` (the commit before it) for the old behaviour. All the texts above are copied from those runs.

- The project is written at schema 25. A build at schema 28 or later exchanges End facing's stored `stepover` and `stepdown` when it reads such a file (decision 0547 item 3), so operation 2's file says `stepdown = 5.0`: the interface shows it as Stepover (slice thickness along X) 5 mm.
- **Show setting** and the interface cannot be run without a display here: step 3 is yours to confirm. The settings the refusals carry are read from the code (`crates/openchuck-path/src/facing/end.rs`, the roughing parameter check, the ramp and lead entry refusals).
- `openchuck-cli report` still prints the machine profile's provenance notes with decision and open-item numbers ("(decision 0013)", "OI-471"); those are the text of the machine profile's own data, not messages, and are reported separately.

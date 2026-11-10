<!-- Title: Release candidate: owner checks for the message work (0536 steps 3 to 6) -->
<!-- Posted as #668; this is its body as last posted (10 Nov 2026, steps 3 to 6). The posting route wraps the step 4 zip link in backticks; it stays readable. -->
<!-- Body for the coordinator to post (lane OWNERCHECKS2). Everything below the line is the issue body. -->

---

**Owner checks on the release candidate** for the message work of [decision 0536](https://github.com/paulhampson/OpenChuckCAM/blob/main/docs/decisions/0536-message-levels-operation-reference-and-setting-links.md), build steps 3 to 6, all merged on `main` (#648, #659, #653, #684; `main` at `0d640692` holds steps 3 to 5, `ee190429` step 6). One section per step: what changed, the steps on a public project, and what to expect. Tick each box when it passes, or reply with **FAIL and what you saw** (numbers and messages only).

Each project is public (the user guide's baluster tutorial job, with small changes; step 6 adds the public `cylinder.step`). Download the zip, unzip it anywhere and open its `project` folder with **File > Open project**. Use the release candidate, or a build from `main` at `0d640692` or later (`ee190429` or later for step 6) (**Help > About OpenChuckCAM** shows the commit).

> [!WARNING]
> Nothing in these checks shows that a program is safe to run. They look at messages, links and labels only. Do not run their G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

The checks were prepared with the command-line tool on `main` (before and after each step); the interface steps could not be run without a display, so the clicks below are yours to confirm.

## Step 3 (#648): every message in one form, and Show setting by key

**What changed.** Every message starts with a topic and the item it is about, "Topic (operation '...', tool '...'): ...", and ends with what to change. **Show setting** opens the operation and marks the named field, whatever kind it is (number, tick box, list).

- Folder: https://github.com/paulhampson/OpenChuckCAM/tree/owner-checks/2026.1-alpha/owner-checks/rc/0536-step3-lead-in-show-setting
- Zip: https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/0536-step3-lead-in-show-setting/0536-step3-lead-in-show-setting.zip
- CHECK.md: https://github.com/paulhampson/OpenChuckCAM/blob/owner-checks/2026.1-alpha/owner-checks/rc/0536-step3-lead-in-show-setting/CHECK.md

**Steps** (about 10 minutes): open the project, **Operations**, **Generate**, read **Messages** and **Results**, and click each **Show setting**.

**Expect:**
- "Flute length (operation 'Short flutes', tool 'Short-flute ball 3.175x5 (check 0536)'): the stock stands up to 9.335 mm above the tool's tip ... or turn on Finishing layers." **Show setting** marks **Finishing layers** (a tick box) on Short flutes.
- "Part cut free (operation 'Free ends'): the end face at X 0.000 is faced down to the rotary axis: ..." **Show setting** marks **Face free ends** (a tick box) on Free ends.
- "Held region (operation 'Free ends'): nothing within one tool radius of the held region ..." (no setting; a link to the operation only).
- No line names a requirement or decision number.

**Before:** only the flute-length line had this form; the others began "Operation 'Free ends': ..." or "operation 'Rough': ...".

- [ ] Step 3 passes

## Step 4 (#659): setup, model, tool and export messages in one form; Show setting for setups and tools; plain help

**What changed.** Setup, model, tool and export messages take the same form, in Messages, the Results tab, the export checks and the command line. The text after the topic starts in lower case and every message ends with a full stop. **Show setting** also opens a setup's field (clearance, safe Z, the machine's short-block mode) or a tool's field in the tool library. A rapid that removes material and short blocks left in Report mode are listed in Messages. The command-line help names no requirement, decision or gate, and the command line prints the "verify" notice when it refuses for a missing acknowledgement flag (#645).

- Folder: https://github.com/paulhampson/OpenChuckCAM/tree/owner-checks/2026.1-alpha/owner-checks/rc/0536-step4-setup-export-cli-messages
- Zip (download from its page): https://github.com/paulhampson/OpenChuckCAM/blob/owner-checks/2026.1-alpha/owner-checks/rc/0536-step4-setup-export-cli-messages/0536-step4-setup-export-cli-messages.zip
- CHECK.md: https://github.com/paulhampson/OpenChuckCAM/blob/owner-checks/2026.1-alpha/owner-checks/rc/0536-step4-setup-export-cli-messages/CHECK.md

**Steps** (about 10 minutes; the setup's safe cylinder is set inside its minimum clearance on purpose): open the project and read **Messages**; click **Show setting** on the Clearance message; **Generate**, **File > Post and export...**, read **Checks before writing**. Optionally, on the command line, read `openchuck-cli --help` and run `regenerate --out` without `--acknowledge-collision-warnings`.

**Expect:**
- "Clearance (setup 'Setup 1'): the safe cylinder (radius 30.000 mm) is inside the minimum clearance (radius 35.000 mm, set for this setup): links may hit the chuck, jaws, headstock or tailstock. Raise the setup's clearance or switch on Clearance from stock, and check the clearance on your machine before running." **Show setting** opens the Setup step with the clearance field marked.
- In the export, the same text followed by "A possible collision: acknowledge it before the files are written".
- Every line: topic, lower case after it, full stop at the end ("Held region (operation 'Finish'): nothing within ...").
- The command line's help names no requirement, decision or gate; the refused run writes nothing and still prints "Generated G-code must be verified by the user before it is run on a machine. ...".

**Before:** "the safe cylinder (radius 30.000 mm) is inside ..." with no topic; the export read "Setup 'Setup 1' has a possible collision, which must be acknowledged ..."; 20 help lines named requirements or decisions; no notice on that refusal.

- [ ] Step 4 passes

## Step 5 (#653): the lost status lines, error dialogs, and the NFR-12 wording

**What changed.** What you did (Saved, Opened, Added, Copied) is a finished entry in the **Jobs** tab, never a message. Changed tool numbers and a recovered checkpoint are warnings in **Messages** with **Save** (and **Undo**) until saved or undone. Importing with no project open is an error dialog whose title names the action. The status badge reads **Nothing to report** instead of "Good to go". The Facing line no longer says "verified path geometry".

- Folder: https://github.com/paulhampson/OpenChuckCAM/tree/owner-checks/2026.1-alpha/owner-checks/rc/0536-step5-status-and-wording
- Zip: https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/0536-step5-status-and-wording/0536-step5-status-and-wording.zip
- CHECK.md: https://github.com/paulhampson/OpenChuckCAM/blob/owner-checks/2026.1-alpha/owner-checks/rc/0536-step5-status-and-wording/CHECK.md

**Steps** (about 10 minutes): with no project open, **File > Import model...**; open the project and look at **Jobs**; **Generate** and read the badge and the **Face end** line in **Results**; **Simulate**; **File > Post and export...** with a changed tool number; **Ctrl+S**.

**Expect:**
- An error dialog **Import model failed**: "Import model: there is no project open. Open or create a project first."
- A finished "Opened ..." entry in Jobs, nothing in Messages.
- The badge never reads "Good to go". Face end reads "Facing covered the end faces it was asked to." and, after the simulation, "...; the simulation shows what it removed." Nowhere "verified path geometry", "certify" or "certificate".
- After the export: "Tool numbers: the tool numbers were changed for the program, but the project keeps them only once it is saved: ... Save the project to keep them, or undo the change" with **Save** and **Undo**; after Ctrl+S it goes and Jobs has "Saved ...".

**Before:** the status texts were never shown; the badge read "Good to go"; Face end read "Facing coverage complete (verified path geometry); ...".

- [ ] Step 5 passes

## Step 6 (#684): every refusal, the Facing notes and the setup checks in the one form

**What changed** ([decision 0644](https://github.com/paulhampson/OpenChuckCAM/blob/main/docs/decisions/0644-refusals-take-the-one-form.md)). An operation that cannot be generated reads like every other message, "Topic (operation '...'): ...", the topic naming the check that refused, with one full stop and **Show setting** at the setting it names. The Facing notes take the topic Facing. The command line's setup checks read "Topic (setup '...'): ....". A refusal that lists passes names them as the notes do. A count of one is singular ("1 layer"). No message names a requirement, decision or open item. OI-360 is closed by your decision (the read of every dialog's messages is not done).

- Folder: https://github.com/paulhampson/OpenChuckCAM/tree/owner-checks/2026.1-alpha/owner-checks/rc/0536-step6-refusals-in-one-form
- Zip: https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/0536-step6-refusals-in-one-form/0536-step6-refusals-in-one-form.zip
- CHECK.md: https://github.com/paulhampson/OpenChuckCAM/blob/owner-checks/2026.1-alpha/owner-checks/rc/0536-step6-refusals-in-one-form/CHECK.md

**Steps** (about 10 minutes; seven operations, each set up to give one kind of message): open the project, **Operations**, **Generate**, read **Messages** and **Results**, click each **Show setting**, and read the checks of **Setup 2, low clearance**.

**Expect:**
- "Facing (operation 'Face end'): facing covered the end faces it was asked to."
- "End facing (operation 'Face end, wide stepover'): the Stepover (slice thickness along X) must be above zero and no greater than the cutting diameter." **Show setting** at Stepover (slice thickness along X).
- "Stepdown (operation 'Rough, deep stepdown'): 30 mm is more than the tool's 25.000 mm of flute; ... Use a smaller Stepdown or a tool with longer flutes." **Show setting** at Stepdown.
- "Ramp entry (operation 'Rough rings, ramp'): pass 31 is a single point, ..." **Show setting** at Allow single-point plunge.
- "Links (operation 'Finish, low clearance'): the stock reaches 12.000 mm from the rotary axis, beyond the retract height of 10.000 mm, so links could cross it. Enlarge the setup's clearance." (no "(OP-14)").
- "Lead entry (operation 'Axial lead, through stock'): 88 passes, from pass 1 to pass 88 (1, 2, 3 ... 86, 87, 88), are entered by a straight plunge into the stock ..." **Show setting** at Entry.
- "Clearance (setup 'Setup 2, low clearance'): the safe cylinder (radius 10.000 mm) does not enclose the stock (radius 12.000 mm)." (also on the command line: `openchuck-cli setup`).

**Before:** "operation '...' cannot be generated: End facing: ...", "... parameter stepdown: ...", "... so links could cross it (OP-14); enlarge the setup's clearance", "88 passes, the first pass 1, are entered ...", "operation 'Face end': Facing covered ...", setup checks with no topic or full stop, "1 layers".

- [ ] Step 6 passes

## Follow-up (#765 and #633, fixed in #769): no false shoulder warning beside the held bar

**What changed** ([decision 0551](https://github.com/paulhampson/OpenChuckCAM/blob/main/docs/decisions/0551-the-held-regions-own-end-is-not-a-shoulder.md)). The tutorial's Finish warned "the tool does not turn at a step across the rotary axis (a shoulder facing along X) at X 8.589": X 8.589 is one tool radius past the held bar's own end (X 7), not a shoulder of the part. Only faces of the part are named now. The programs are byte-identical; only the warning goes. The "nothing within one tool radius of the held region ... is cut" warning and the setup's grip warning stay, so the badge still reads **Needs attention**, by design.

- Folder: https://github.com/paulhampson/OpenChuckCAM/tree/owner-checks/2026.1-alpha/owner-checks/765-held-end-not-a-shoulder
- Zip: https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/765-held-end-not-a-shoulder/765-held-end-not-a-shoulder.zip
- CHECK.md: https://github.com/paulhampson/OpenChuckCAM/blob/owner-checks/2026.1-alpha/owner-checks/765-held-end-not-a-shoulder/CHECK.md

**Steps** (about 3 minutes; the tutorial project unchanged; a build from `55e6348d` or later): open the project, **Operations**, **Generate**, read **Messages** for **Finish** and the badge.

**Expect:**
- Finish: "Free ends (operation 'Finish'): the end face at X 61.000 is faced from 12.000 mm, ..." and "Held region (operation 'Finish'): nothing within one tool radius of the held region at the start of the stock is cut: no part of the tool reaches over the gripped length of the stock."
- No "... does not turn at a step across the rotary axis ... at X 8.589 ...".
- The badge reads **Needs attention**.

**Before** (`a3e1ad22`): the X 8.589 warning as well.

**#633** (the same warning at an X outside the selection and the stock) has the same cause and is fixed by the same change. Its own check:

- Folder: https://github.com/paulhampson/OpenChuckCAM/tree/owner-checks/2026.1-alpha/owner-checks/633-no-step-note-outside-selection
- Zip: https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/633-no-step-note-outside-selection/633-no-step-note-outside-selection.zip
- CHECK.md: https://github.com/paulhampson/OpenChuckCAM/blob/owner-checks/2026.1-alpha/owner-checks/633-no-step-note-outside-selection/CHECK.md

**Steps** (about 5 minutes; the three owner-check projects #633 names, as three setups of one project): open the project, **Operations**, **Generate**, open **Messages**, look for "Held region (operation '...'): the tool does not turn at a step across the rotary axis ... at X ...".

**Expect:** none. The only Held region warning left is on **Whole part**, "nothing within one tool radius of the held region ... is cut" (by design). **Before** (`a3e1ad22`): on Tenon side at X -15.001, on Vase only and Whole part at X -16.001, on Finish vase at X -3.411. Programs byte-identical.

- [ ] #765 passes
- [ ] #633 passes (its check above: no "does not turn at a step" warning on any of the five operations)

## Known, not part of these checks

- The Facing line and the setup listing of the command line, noted here under steps 3 to 5, take the one form with step 6. The doubled full stop in the acknowledgement refusal ("... before running.. Read them, ...") is fixed in #670 (merged on `main` at `576ef9c9`); builds before that commit still show it.
- `openchuck-cli report` prints the machine profile's own provenance notes, with decision and open-item numbers ("(decision 0013)", "OI-471"). This is by design: you decided on 10 Nov 2026 that they are the profile's data, not messages under decision 0644, and stay as they are.
- Several earlier checks quote these texts and pass in either form: #600, #622 and #567 (and #611, whose workholding texts are unchanged).

---
_Generated by [Claude Code](https://claude.ai/code)_

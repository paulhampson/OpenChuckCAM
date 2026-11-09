<!-- Title: Release candidate: owner checks for the message work (0536 steps 3 to 5) -->
<!-- Body for the coordinator to post (lane OWNERCHECKS2). Everything below the line is the issue body. -->

---

**Owner checks on the release candidate** for the message work of [decision 0536](https://github.com/paulhampson/OpenChuckCAM/blob/main/docs/decisions/0536-message-levels-operation-reference-and-setting-links.md), build steps 3 to 5, all merged on `main` (#648, #659, #653; `main` at `0d640692` holds all three). One section per step: what changed, the steps on a public project, and what to expect. Tick each box when it passes, or reply with **FAIL and what you saw** (numbers and messages only).

Each project is public (the user guide's baluster tutorial job, with small changes). Download the zip, unzip it anywhere and open its `project` folder with **File > Open project**. Use the release candidate, or a build from `main` at `0d640692` or later (**Help > About OpenChuckCAM** shows the commit).

> [!WARNING]
> Nothing in these checks shows that a program is safe to run. They look at messages, links and labels only. Do not run their G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

The checks were prepared with the command-line tool on `main` (before and after each step); the interface steps could not be run without a display, so the clicks below are yours to confirm.

## Step 3 (#648): every message in one form, and Show setting by key

**What changed.** Every message starts with a topic and the item it is about, "Topic (operation '...', tool '...'): ...", and ends with what to change. **Show setting** opens the operation and marks the named field, whatever kind it is (number, tick box, list).

**Steps** ([folder](https://github.com/paulhampson/OpenChuckCAM/tree/owner-checks/2026.1-alpha/owner-checks/rc/0536-step3-lead-in-show-setting), [zip](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/0536-step3-lead-in-show-setting/0536-step3-lead-in-show-setting.zip), [CHECK.md](https://github.com/paulhampson/OpenChuckCAM/blob/owner-checks/2026.1-alpha/owner-checks/rc/0536-step3-lead-in-show-setting/CHECK.md); about 10 minutes): open the project, **Operations**, **Generate**, read **Messages** and **Results**, and click each **Show setting**.

**Expect:**
- "Flute length (operation 'Short flutes', tool 'Short-flute ball 3.175x5 (check 0536)'): the stock stands up to 9.335 mm above the tool's tip ... or turn on Finishing layers." **Show setting** marks **Finishing layers** (a tick box) on Short flutes.
- "Part cut free (operation 'Free ends'): the end face at X 0.000 is faced down to the rotary axis: ..." **Show setting** marks **Face free ends** (a tick box) on Free ends.
- "Held region (operation 'Free ends'): nothing within one tool radius of the held region ..." (no setting; a link to the operation only).
- No line names a requirement or decision number.

**Before:** only the flute-length line had this form; the others began "Operation 'Free ends': ..." or "operation 'Rough': ...".

- [ ] Step 3 passes

## Step 4 (#659): setup, model, tool and export messages in one form; Show setting for setups and tools; plain help

**What changed.** Setup, model, tool and export messages take the same form, in Messages, the Results tab, the export checks and the command line. The text after the topic starts in lower case and every message ends with a full stop. **Show setting** also opens a setup's field (clearance, safe Z, the machine's short-block mode) or a tool's field in the tool library. A rapid that removes material and short blocks left in Report mode are listed in Messages. The command-line help names no requirement, decision or gate, and the command line prints the "verify" notice when it refuses for a missing acknowledgement flag (#645).

**Steps** ([folder](https://github.com/paulhampson/OpenChuckCAM/tree/owner-checks/2026.1-alpha/owner-checks/rc/0536-step4-setup-export-cli-messages), [zip](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/0536-step4-setup-export-cli-messages/0536-step4-setup-export-cli-messages.zip), [CHECK.md](https://github.com/paulhampson/OpenChuckCAM/blob/owner-checks/2026.1-alpha/owner-checks/rc/0536-step4-setup-export-cli-messages/CHECK.md); about 10 minutes; the setup's safe cylinder is set inside its minimum clearance on purpose): open the project and read **Messages**; click **Show setting** on the Clearance message; **Generate**, **File > Post and export...**, read **Checks before writing**. Optionally, on the command line, read `openchuck-cli --help` and run `regenerate --out` without `--acknowledge-collision-warnings`.

**Expect:**
- "Clearance (setup 'Setup 1'): the safe cylinder (radius 30.000 mm) is inside the minimum clearance (radius 35.000 mm, set for this setup): links may hit the chuck, jaws, headstock or tailstock. Raise the setup's clearance or switch on Clearance from stock, and check the clearance on your machine before running." **Show setting** opens the Setup step with the clearance field marked.
- In the export, the same text followed by "A possible collision: acknowledge it before the files are written".
- Every line: topic, lower case after it, full stop at the end ("Held region (operation 'Finish'): nothing within ...").
- The command line's help names no requirement, decision or gate; the refused run writes nothing and still prints "Generated G-code must be verified by the user before it is run on a machine. ...".

**Before:** "the safe cylinder (radius 30.000 mm) is inside ..." with no topic; the export read "Setup 'Setup 1' has a possible collision, which must be acknowledged ..."; 20 help lines named requirements or decisions; no notice on that refusal.

- [ ] Step 4 passes

## Step 5 (#653): the lost status lines, error dialogs, and the NFR-12 wording

**What changed.** What you did (Saved, Opened, Added, Copied) is a finished entry in the **Jobs** tab, never a message. Changed tool numbers and a recovered checkpoint are warnings in **Messages** with **Save** (and **Undo**) until saved or undone. Importing with no project open is an error dialog whose title names the action. The status badge reads **Nothing to report** instead of "Good to go". The Facing line no longer says "verified path geometry".

**Steps** ([folder](https://github.com/paulhampson/OpenChuckCAM/tree/owner-checks/2026.1-alpha/owner-checks/rc/0536-step5-status-and-wording), [zip](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/0536-step5-status-and-wording/0536-step5-status-and-wording.zip), [CHECK.md](https://github.com/paulhampson/OpenChuckCAM/blob/owner-checks/2026.1-alpha/owner-checks/rc/0536-step5-status-and-wording/CHECK.md); about 10 minutes): with no project open, **File > Import model...**; open the project and look at **Jobs**; **Generate** and read the badge and the **Face end** line in **Results**; **Simulate**; **File > Post and export...** with a changed tool number; **Ctrl+S**.

**Expect:**
- An error dialog **Import model failed**: "Import model: there is no project open. Open or create a project first."
- A finished "Opened ..." entry in Jobs, nothing in Messages.
- The badge never reads "Good to go". Face end reads "Facing covered the end faces it was asked to." and, after the simulation, "...; the simulation shows what it removed." Nowhere "verified path geometry", "certify" or "certificate".
- After the export: "Tool numbers: the tool numbers were changed for the program, but the project keeps them only once it is saved: ... Save the project to keep them, or undo the change" with **Save** and **Undo**; after Ctrl+S it goes and Jobs has "Saved ...".

**Before:** the status texts were never shown; the badge read "Good to go"; Face end read "Facing coverage complete (verified path geometry); ...".

- [ ] Step 5 passes

## Step 6: every strategy's refusals in the one form

To come (not built yet). A section and a check will be added here when it is merged.

- [ ] Step 6 passes

## Known, not part of these checks

- The Facing line is the one note without a topic in front ("operation 'Face end': Facing covered ..."); it is your wording, parked for your decision.
- `openchuck-cli setup` still lists setup and workholding findings in the older form; reported to the message lane. (The doubled full stop in the command line's acknowledgement refusal, "... before running.. Read them", is fixed by #670.)
- Several earlier checks quote these texts and pass in either form: #600, #622 and #567 (and #611, whose workholding texts are unchanged).

---
_Generated by [Claude Code](https://claude.ai/code)_

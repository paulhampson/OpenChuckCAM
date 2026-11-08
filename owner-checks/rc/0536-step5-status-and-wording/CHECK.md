# Check for decision 0536 build step 5: the lost status lines, and the NFR-12 wording

> [!NOTE]
> **To run on the release candidate.** Build step 5 is merged (pull request #653, `main` at `81bade8a3`, lane MSGSTEP5); the texts below are those of that commit. This check stays in `rc/` until the coordinator opens the tracking issue for decision 0536 steps 3 to 5, where it is posted.

**What changed** ([decision 0536](../../../docs/decisions/0536-message-levels-operation-reference-and-setting-links.md) section 8, OI-2097 and OI-2098):

- The shell's status line was never drawn, so its texts reached nobody. Now what you did ("Saved ...", "Opened ...", "Added ...", "Copied n models ...") is a finished entry in the **Jobs** tab with how long ago, never a message. Changed tool numbers and a recovered checkpoint are warnings in **Messages** with **Save** (and **Undo**) until saved or undone. Importing with no project open is an error dialog whose title names the action. A dropped pick is an error on what it was for, with **Reselect**.
- Wording: the overall status badge's "Good to go" is **"Nothing to report"** ("Needs attention" and "Error to fix" stay). Facing's "Facing coverage complete (verified path geometry)" is **"Facing covered the end faces it was asked to."**, with "; the simulation shows what it removed" added only for a simulated run. "Verified", "certify" and "certificate" are gone from the Facing check's texts.

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at messages and labels only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): the user guide's tutorial project with a Facing operation added first, **Face end**: the unheld end of the stock (the far end, X 62) faced with the tutorial's 3.175 mm flat end mill, in 0.5 mm layers.

## Steps

Use the release candidate, or a build from `main` at commit `81bade8a3` or later (**Help > About OpenChuckCAM** shows the commit).

1. With no project open, choose **File > Import model...**.
2. Download [`0536-step5-status-and-wording.zip`](0536-step5-status-and-wording.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/0536-step5-status-and-wording/0536-step5-status-and-wording.zip)), unzip it anywhere, and open the folder `0536-step5-status-and-wording/project` with **File > Open project**. Look at the **Jobs** tab and at **Messages**.
3. Click **Operations**, **Generate**. Look at the overall status badge, and at the **Results** line for **Face end**.
4. Click **Simulate** and run the simulation. Look at the **Face end** line in the Simulate results.
5. **File > Post and export...**: change a tool number in the tool numbers table, write the file, and close the dialog. Look at **Messages**.
6. Press <kbd>Ctrl</kbd>+<kbd>S</kbd>. Look at **Messages** and the **Jobs** tab.

## Expected result

- **Step 1**: an error dialog titled **Import model failed**, reading "Import model: there is no project open. Open or create a project first."
- **Step 2**: the Jobs tab has a finished "Opened ..." entry with how long ago; nothing about it in Messages.
- **Step 3**: the badge reads **Needs attention** (this job has warnings) or **Nothing to report**; never "Good to go". The Face end line reads "Facing covered the end faces it was asked to." with nothing about a simulation. No "verified path geometry", "certify" or "certificate" anywhere.
- **Step 4**: after the simulation, the line reads "Face end: Facing covered the end faces it was asked to; the simulation shows what it removed."
- **Step 5**: Messages has a warning, with **Save** and **Undo**: "Tool numbers: the tool numbers were changed for the program, but the project keeps them only once it is saved: it would reopen with the old numbers, and the next program would not match the tool table. Save the project to keep them, or undo the change".
- **Step 6**: that warning goes; the Jobs tab has a finished "Saved ..." entry.

**Old behaviour (before step 5, `main` at `1f3675d52`):** the status texts were set but never drawn, so steps 1, 2, 5 and 6 showed nothing; the badge read "Good to go"; the Facing line read "Facing coverage complete (verified path geometry); simulation remains resolution-limited and whole-part deviation is unchanged".

## Report back

Reply with: **PASS**, or **FAIL and what you saw** (the step and the text as it reads). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project`), Linux x86_64, release builds, 8 November 2026:

| Build | Face end's note |
| --- | --- |
| Before step 5: `1f3675d52` (0.50.0) | `note: operation 'Face end': Facing coverage complete (verified path geometry); simulation remains resolution-limited and whole-part deviation is unchanged` |
| `main` at `81bade8a3` (0.51.0) | `note: operation 'Face end': Facing covered the end faces it was asked to.` |

The other texts above (the dialog, the badge, the simulated Facing line, the tool-numbers and recovery warnings) are read from the code at `81bade8a3` and its tests (`reminder_tests.rs` pins the dialog's title and text); the command line has no status line, badge or Results tab, and the interface cannot be run without a display here. `openchuck-cli validate` reports the project valid.

Seen while preparing this, reported to the project and not part of the check: on `81bade8a3` the Facing note is the one line without a topic in front ("operation 'Face end': Facing covered ...", where every other line reads "Topic (operation '...'): ...").

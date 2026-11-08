# Check for decision 0536 build step 5: the lost status lines, and the NFR-12 wording

> [!NOTE]
> **Draft, to run on the release candidate.** Build step 5 of [decision 0536](../../docs/decisions/0536-message-levels-operation-reference-and-setting-links.md) (lane MSGSTEP5) is not merged yet (8 Nov 2026). This file gives today's behaviour, read from `main` at `1f3675d52`, and what step 5 is to do (decision 0536 section 8, OI-2097 and OI-2098). When it merges, the expected texts are copied from that build, its pull request is named here, and this note is removed.

**What is to change:**

- The shell's status line is never drawn, so its texts reach nobody (OI-2098). Each gets a place: "Saved ...", "Opened ...", "Added ..." and "Copied n models ..." become finished entries in the **Jobs** tab only; "Tool numbers changed ..." and "Recovered checkpoint ..." become warnings in **Messages** with **Save** (and **Undo**) until saved; "Open or create a project before importing a model." becomes an error dialog; "The selected geometry changed; select it again." becomes an error on the selection set with **Reselect**.
- Wording (OI-2097): the overall status badge's "Good to go" becomes **"Nothing to report"** ("Needs attention" and "Error to fix" stay); Facing's "Facing coverage complete (verified path geometry)" becomes **"Facing covered the end faces it was asked to; the simulation shows what it removed"**; "certify" and "certificate" in the Facing check's texts become "check" and "check result".

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at messages and labels only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): the user guide's tutorial project with a Facing operation added first, **Face round** (the stock turned down to 23 mm across with the 3.175 mm flat end mill, 0.5 mm layers).

## Steps

Use the release candidate, or a build from `main` that holds step 5 (**Help > About OpenChuckCAM** shows the commit).

1. With no project open, choose **File > Import model...**. Note what tells you that a project is needed.
2. Download [`0536-step5-status-and-wording.zip`](0536-step5-status-and-wording.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/0536-step5-status-and-wording/0536-step5-status-and-wording.zip)), unzip it anywhere, and open the folder `0536-step5-status-and-wording/project` with **File > Open project**. Look at the **Jobs** tab.
3. Click **Operations**, **Generate**. Look at the overall status badge, and at the **Results** line for **Face round**.
4. **File > Post and export...**: change a tool number in the tool numbers table so the numbers change, write the file, and close the dialog. Look at **Messages**.
5. Press <kbd>Ctrl</kbd>+<kbd>S</kbd>. Look at **Messages** and the **Jobs** tab.

## Expected result

- **Step 1**: an error dialog says that a project must be opened or created before importing a model.
- **Step 2**: the Jobs tab has a finished entry "Opened ..."; nothing about it in Messages.
- **Step 3**: the badge reads **Nothing to report** when there is nothing to report, or **Needs attention** with this project's warnings; never "Good to go". The Face round line reads "Face round: Facing covered the end faces it was asked to; the simulation shows what it removed" (exact text from the merged build); no "verified path geometry", "certify" or "certificate" anywhere.
- **Step 4**: Messages has a warning on the tools that the tool numbers changed, with **Save** and **Undo**.
- **Step 5**: that warning goes away; the Jobs tab has a finished "Saved ..." entry.

**Today (`main` at `1f3675d52`, by reading the code):** the status texts are set but never drawn, so steps 1, 2, 4 and 5 show nothing (`apps/openchuck-ui/src/shell.rs` and `export_panel.rs`); the badge reads "Good to go" (`crates/openchuck-app/src/overview.rs`); the Facing line reads "Face round: Facing coverage complete (verified path geometry)." (`apps/openchuck-ui/src/results_panel.rs`).

## Report back

Reply with: **PASS**, or **FAIL and what you saw** (the step and the text as it reads). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

`openchuck-cli regenerate project` on `main` at `1f3675d52` (Linux x86_64, release build, 8 November 2026) generates all three operations (Face round, Rough, Finish); the command line has no status line, badge or Results tab, so the texts above are read from the code, not seen. The interface cannot be run without a display here. `openchuck-cli validate` reports the project valid.

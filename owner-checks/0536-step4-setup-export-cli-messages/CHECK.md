# Check for decision 0536 build step 4: setup and export messages in one form, and plain command-line help

> [!NOTE]
> **Draft, to run on the release candidate.** Build step 4 of [decision 0536](../../docs/decisions/0536-message-levels-operation-reference-and-setting-links.md) (lane MSGREVIEW) is not merged yet (8 Nov 2026). This file gives today's texts, run on `main` at `1f3675d52`, and what step 4 is to do (decision 0536 section 7, item 4). When it merges, the expected texts are copied from that build, its pull request is named here, and this note is removed.

**What is to change:**

- Setup, model, post and export messages take the same form as operation messages: `<Topic> (setup '<name>'): ...`, with the Inspector's field labels in place of internal names (OI-360), and a link to the field.
- The command-line help text uses the same plain words as the messages and the user guide's Command line page, with no requirement numbers or internal terms.
- A rapid that removes material, and short blocks left in the program in Report mode, are listed in **Messages** as warnings on their operation (OI-373).

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at messages only. Do not run this project's G-code on a machine, not even as an air cut: its clearance is set wrong on purpose. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): the user guide's tutorial project with one change, the setup's safe cylinder set to radius 30 mm inside a minimum clearance of 35 mm. That gives a setup message that is a possible collision (decision 0538).

## Steps

Use the release candidate, or a build from `main` that holds step 4 (**Help > About OpenChuckCAM** shows the commit).

1. Download [`0536-step4-setup-export-cli-messages.zip`](0536-step4-setup-export-cli-messages.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/0536-step4-setup-export-cli-messages/0536-step4-setup-export-cli-messages.zip)), unzip it anywhere, and open the folder `0536-step4-setup-export-cli-messages/project` with **File > Open project**.
2. Click **Setup** and **Setup 1**. Read the setup's messages in the Inspector and in **Messages**. Click the message's link.
3. Click **Operations**, **Generate**, then **File > Post and export...** (Makera Z1). Read **Checks before writing**.
4. Optional, if a stock simulation finds a rapid through material or the post leaves short blocks in Report mode on your own work: look for them in **Messages**.
5. If you use the command-line tool: run `openchuck-cli --help` and `openchuck-cli <command> --help` for each command, and read them.

## Expected result

- **Step 2**: the clearance message starts with its topic and the setup, for example "Clearance (setup 'Setup 1'): ...", names the fields as the Inspector labels them, ends with what to change, and its link goes to the clearance fields.
- **Step 3**: the export lists the same text (a possible collision, with its tick).
- **Step 5**: no help line has a requirement number (such as `SET-01`, `GC-14`, `SIM-03`), a decision number or "gate 3".
- The exact texts will be copied here from the merged build.

**Today (`main` at `1f3675d52`, before step 4):**

- The setup message, as `openchuck-cli setup project "Setup 1"` prints it, has no topic and starts in lower case: "the safe cylinder (radius 30.000 mm) is inside the minimum clearance (radius 35.000 mm, set for this setup): links may hit the chuck, jaws, headstock or tailstock. Raise the setup's clearance or switch on Clearance from stock, and check the clearance on your machine before running".
- The export check reads "Setup 'Setup 1' has a possible collision, which must be acknowledged before the files are written: the safe cylinder (radius 30.000 mm) is inside the minimum clearance ...".
- The help text has 20 lines with a requirement or decision number, for example `setup ... (SET-03, SET-05, SET-06)`, `simulate ... (SIM-03, SIM-05 to SIM-07)`, `gcode-sim ... (gate 3)`, `--min-clearance ... (decision 0495)`.

## Report back

Reply with: **PASS**, or **FAIL and what you saw** (the message or help line as it reads). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Linux x86_64, release build of `main` at `1f3675d52`, 8 November 2026. `openchuck-cli setup` prints the two setup lines above (and the tutorial's own "the grip at the start of the stock covers 7.000 mm of the part (X 0.000 to 7.000) ..."). `openchuck-cli regenerate project --post makera-z1 --tool-numbers keep --out OUT` lists the setup's possible collision under "checks before writing the G-code" and writes nothing without `--acknowledge-collision-warnings`. The help lines were counted with `grep -cE '[A-Z]{2,4}-[0-9]{2}|decision [0-9]{4}|OI-[0-9]+'` over the help of every command: 20. The interface cannot be run without a display here. `openchuck-cli validate` reports the project valid.

# Check for decision 0536 build step 4: setup, model and export messages in one form, Show setting for setups and tools, and plain command-line help

> [!NOTE]
> **To run on the release candidate.** Build step 4 is merged (pull request #659, `main` at `0d640692`, lane MSGREVIEW); the texts below are those of that commit. This check stays in `rc/` for the tracking issue of decision 0536 steps 3 to 5, where it is posted.

**What changed** ([decision 0536](../../../docs/decisions/0536-message-levels-operation-reference-and-setting-links.md) section 7 item 4):

- **One form everywhere.** Setup, model, tool and export messages read "Topic (setup '...'): ..." as operation messages do, in Messages, the Results tab, the export checks and the command line. The text after the lead-in starts in lower case and every message ends with a full stop.
- **Show setting for setups and tools.** A message about a setup's clearance or safe Z opens the Setup step at that field; a tool too long to clear the workholding opens the tool library at its **Flute length**; short blocks open the setup's machine section. The field is marked until a value is entered there.
- **Two warnings reach Messages** (OI-373): a rapid that removes material ("Rapid cut (operation '...'): move N is a rapid that removes V mm³ of material", to acknowledge before export) and, with the short-block mode on Report, the short blocks left in the program ("Short blocks (operation '...'): ...").
- **Plain words.** A refused parameter is named by its Inspector label ("Tolerance: must be above 0 mm"), and no requirement or decision number is left in texts or in the command-line help.
- **The command line prints the "verify" notice when it refuses** for a missing `--acknowledge-collision-warnings` or `--acknowledge-partial-path` (issue #645).
- **The deviation note under a Facing line** in Results shows only after a simulation.

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at messages only. Do not run this project's G-code on a machine, not even as an air cut: its clearance is set wrong on purpose. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): the user guide's tutorial project with one change, the setup's safe cylinder set to radius 30 mm inside a minimum clearance of 35 mm set for the setup. That gives a setup message that is a possible collision (decision 0538).

## Steps

Use the release candidate, or a build from `main` at commit `0d640692` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`0536-step4-setup-export-cli-messages.zip`](0536-step4-setup-export-cli-messages.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/0536-step4-setup-export-cli-messages/0536-step4-setup-export-cli-messages.zip)), unzip it anywhere, and open the folder `0536-step4-setup-export-cli-messages/project` with **File > Open project**.
2. Read the **Messages** list. Click **Show setting** on the Clearance message.
3. Click **Operations**, **Generate**, then **File > Post and export...** (Makera Z1). Read **Checks before writing**.
4. If you use the command-line tool: run `openchuck-cli --help` and `openchuck-cli <command> --help` for a few commands; then run `openchuck-cli regenerate <project> --post makera-z1 --tool-numbers keep --out <folder>` without `--acknowledge-collision-warnings`.

## Expected result

- **Step 2:** the message reads "Clearance (setup 'Setup 1'): the safe cylinder (radius 30.000 mm) is inside the minimum clearance (radius 35.000 mm, set for this setup): links may hit the chuck, jaws, headstock or tailstock. Raise the setup's clearance or switch on Clearance from stock, and check the clearance on your machine before running." **Show setting** opens the Setup step at Setup 1 with the clearance section open and the clearance field marked.
- **Step 3:** the export lists it as a possible collision to tick: "Clearance (setup 'Setup 1'): ... before running. A possible collision: acknowledge it before the files are written". Every line in Messages and the export starts with a topic, continues in lower case and ends with a full stop (for example "Held region (operation 'Finish'): nothing within one tool radius of the held region ...").
- **Step 4:** no help page names a requirement (such as `SET-01`), a decision or "gate 3". The refused `regenerate` writes nothing, ends with the error, and also prints "Generated G-code must be verified by the user before it is run on a machine. ..." (issue #645).

**Old behaviour (before step 4, `main` at `81bade8a3`):** the setup message had no topic ("the safe cylinder (radius 30.000 mm) is inside ..."); the export check read "Setup 'Setup 1' has a possible collision, which must be acknowledged ..."; 20 help lines named requirements or decisions; the refusal for the missing flag printed no notice; some lines started with a capital after the lead-in or ended without a full stop.

## Report back

Reply on the tracking issue with: **PASS**, or **FAIL and what you saw** (the message as it reads, and where Show setting went). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool, Linux x86_64, release builds each in a target folder of its own:

| | `main` at `81bade8a3` (before) | `main` at `0d640692` (after) |
| --- | --- | --- |
| Export check of the clearance | "Setup 'Setup 1' has a possible collision, which must be acknowledged before the files are written: the safe cylinder ..." | "Clearance (setup 'Setup 1'): the safe cylinder ... before running. A possible collision: acknowledge it before the files are written" |
| Help lines naming a requirement, decision or gate | 20 | 0 |
| Notice on the refusal for the missing flag (the #567 check's project) | not printed | printed once |
| "Held region (operation '...'): ..." | "Nothing within ..." | "nothing within ..." |

**Seen, reported to the coordinator, not part of the check:**

- The refusal for a missing acknowledgement read "... before running.. Read them, ..." on `0d640692` (a doubled full stop); fixed by #670, on `main` from `576ef9c`: on `07ec6990` it reads "... or make the tab longer. Read them, then acknowledge them to write the files."
- `openchuck-cli setup` still lists a setup's checks and workholding findings without the topic, starting in lower case and some without a full stop ("the safe cylinder (radius 30.000 mm) is inside ... before running", "tab 1 is placed automatically at the head of the part, but there is no room there: ..."); `regenerate` and the export use the new form.
- A rapid that removes material and short blocks in Report mode were not produced on public material here (the tutorial job has neither); their texts are from the merged pull request.
- The Show setting links and the Facing deviation note need the interface, which cannot be run without a display here.

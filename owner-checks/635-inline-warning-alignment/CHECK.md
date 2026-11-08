# Check for issue #635: the warning line under a setting in the Inspector uses the Inspector's width

> [!NOTE]
> **Draft, to run on the release candidate.** The fix for #635 is not merged yet (8 Nov 2026). This file gives the issue's own steps on a public project and today's behaviour. When the fix merges, the result is described here from that build, its pull request is named, and this note is removed.

**What is to be fixed:** when the last path was made with a warning about one of an operation's settings, the Inspector shows a line under that setting. Today that line is poorly aligned (the issue's screenshot). It should use the whole width of the Inspector, consistently, under the setting it is about.

**Time:** about 3 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the Inspector's layout only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)) is the project of the #442 check, written again by [`generate.py`](generate.py) with that check's generator: the user guide's blisk starting project, the face set **Blade wall** (face 19) and one swarf operation, **Blade swarf**, on it with every setting at its default. That is the issue's own recipe (a single blade wall set, swarf at its defaults).

## Steps

Use the release candidate, or a build from `main` that holds the fix (**Help > About OpenChuckCAM** shows the commit).

1. Download [`635-inline-warning-alignment.zip`](635-inline-warning-alignment.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/635-inline-warning-alignment/635-inline-warning-alignment.zip)), unzip it anywhere, and open the folder `635-inline-warning-alignment/project` with **File > Open project**.
2. Click **Operations**, then **Generate**. Click **1 Blade swarf**.
3. In **Messages**, click **Show setting** on the swarf's overcut warning. The Inspector opens the section and marks the setting it names (**Tolerance**).
4. Look at the warning line under the setting. Make the Inspector wider and narrower by dragging its edge.

## Expected result

- The line under the setting starts at the Inspector's left edge (or the same indent as the other rows), runs across its whole width and wraps there, at every width; it is not squeezed into the value column or pushed to the left of the label column.
- It sits directly under the setting it is about, with the warning mark and the text: "The last path was made with a warning about this setting (see Messages): change it to accept or remove the warning."

**Old behaviour (today, from the issue's screenshot):** the line is drawn out of line with the Inspector's columns.

## Report back

Reply on issue #635 with: **PASS**, or **FAIL and what you saw** (a screenshot of the public blisk is fine). Numbers and messages only otherwise; no project files from your own work.

## What we saw when preparing this

The Inspector cannot be drawn without a display here, so nothing in this check was run in the interface. The line's text is `WARNED_SETTING` in `apps/openchuck-ui/src/form_panel.rs` on `main` at `1f3675d52`. The project is byte-identical to the #442 check's project (`diff -r`); for the warnings that check lists for **Blade swarf** at its defaults (the swarf cuts face 1 by about 0.014 mm and face 20 by about 0.023 mm, beyond the 0.01 mm tolerance), see [its CHECK.md](../442-swarf-past-blade-end/CHECK.md).

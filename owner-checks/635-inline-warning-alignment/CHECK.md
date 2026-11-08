# Check for issue #635: the warning line under a setting in the Inspector uses the Inspector's width

**What was fixed** (pull request #648, lane MSGREVIEW, with [decision 0536](../../docs/decisions/0536-message-levels-operation-reference-and-setting-links.md) step 3): when the last path was made with a warning about one of an operation's settings, the Inspector shows a line under that setting. Today that line is poorly aligned (the issue's screenshot). The warning shape was drawn after an indent the width of the label column, and the text wrapped to the next line at the left edge, leaving the shape alone above it. Now the warning is one block the whole width of the Inspector under the field, the shape just before its first line and every line starting at the same place; a refusal line under a field takes the same width. It holds for every kind of field.

**Time:** about 3 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the Inspector's layout only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)) is the project of the #442 check, written again by [`generate.py`](generate.py) with that check's generator: the user guide's blisk starting project, the face set **Blade wall** (face 19) and one swarf operation, **Blade swarf**, on it with every setting at its default. That is the issue's own recipe (a single blade wall set, swarf at its defaults).

## Steps

Use the release candidate, or a build from `main` at commit `645256c3` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`635-inline-warning-alignment.zip`](635-inline-warning-alignment.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/635-inline-warning-alignment/635-inline-warning-alignment.zip)), unzip it anywhere, and open the folder `635-inline-warning-alignment/project` with **File > Open project**.
2. Click **Operations**, then **Generate**. Click **1 Blade swarf**.
3. In **Messages**, click **Show setting** on the warning that starts "Overcut (operation 'Blade swarf'): Swarf on face 19 cuts into the finished surface by 0.0158 mm". The Inspector opens the section and marks the setting it is about (**Tolerance**). If another of the operation's warnings also has **Show setting** (the two about faces 1 and 20 name **Tip extension**), look at its line as well.
4. Look at the warning line under the setting. Make the Inspector wider and narrower by dragging its edge.

## Expected result

- The warning under the setting is one block across the whole width of the Inspector: the warning shape just before its first line, and every line, the first included, starting at the same place. It wraps at the Inspector's edge, at every width; the shape is never left alone on a line above the text.
- It sits directly under the setting it is about, with the warning mark and the text: "The last path was made with a warning about this setting (see Messages): change it to accept or remove the warning."

**Old behaviour (not fixed, the issue's screenshot):** the warning shape sat alone after an indent the width of the label column, and the text started on the next line at the left edge.

## Report back

Reply on issue #635 with: **PASS**, or **FAIL and what you saw** (a screenshot of the public blisk is fine). Numbers and messages only otherwise; no project files from your own work.

## What we saw when preparing this

The Inspector cannot be drawn without a display here, so the layout was not seen; the fix's test `req_ui_12_an_inline_warning_spans_the_inspector_under_its_field` (#648) fails on the old layout. The line's text is `WARNED_SETTING` in `apps/openchuck-ui/src/form_panel.rs`, unchanged by the fix. The project is byte-identical to the #442 check's project (`diff -r`) and valid on `main` at `81bade8a3` with the STEP reader built from `1f3675d52`. `openchuck-cli regenerate project` on `81bade8a3` gives **Blade swarf** three warnings:

> Overcut (operation 'Blade swarf'): Swarf on face 19 cuts into the finished surface by 0.0158 mm at (50.509, -1.120, 10.440), more than this operation's tolerance of 0.01 mm. ... To accept this cut, raise the Tolerance of this operation to at least 0.0158 mm (in the operation inspector's parameters, where the field is marked) ...

> Overcut (operation 'Blade swarf'): Swarf on face 19 cuts 0.014 mm into face 1, which meets it along the ends of its rulings, ... or set a negative Tip extension to pull the tool up the face (in the operation inspector's parameters, where the field is marked). The program is made with this cut in it: check the program and the simulation before running it.

and the same for face 20 (0.023 mm).

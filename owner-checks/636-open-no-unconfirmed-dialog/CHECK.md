# Check for issue #636: File > Open project with a pasted path opens the project, with no Keep/Discard dialog

**What was fixed** (pull request #653, lane MSGSTEP5): in the **Open project** dialog (and **New project**), typing or pasting a folder into the field and clicking **OK** at once brings up an "Unconfirmed changes" dialog with **Keep**, **Discard** and **Cancel**, because the folder field counts as a value not yet confirmed with <kbd>Enter</kbd>. Now **OK** accepts the dialog's own fields and **Cancel** abandons them, in both folder dialogs; the guard still asks about real unconfirmed edits before the dialog opens.

**Time:** about 3 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at a dialog only. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)) is an unchanged copy of the user guide's tutorial project, written by [`generate.py`](generate.py). Any project does; this one is public.

## Steps

Use the release candidate, or a build from `main` at commit `81bade8a3` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`636-open-no-unconfirmed-dialog.zip`](636-open-no-unconfirmed-dialog.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/636-open-no-unconfirmed-dialog/636-open-no-unconfirmed-dialog.zip)) and unzip it. Copy the full path of the folder `636-open-no-unconfirmed-dialog/project` (in Explorer: select it, <kbd>Shift</kbd> + right-click, **Copy as path**, then remove the quotes).
2. Start OpenChuckCAM with no project open. Choose **File > Open project...**.
3. Click in the **Project folder** field, paste the path with <kbd>Ctrl</kbd>+<kbd>V</kbd>, and click **OK** straight away (do **not** press <kbd>Enter</kbd> first).
4. Choose **File > Open project...** again, paste the same path, and this time press <kbd>Enter</kbd> in the field. (Enter may open the project at once, or only confirm the field; either is fine.)
5. Choose **File > New project...**, type a parent folder and a project name, and click **OK** without pressing <kbd>Enter</kbd> (then close the new project; it can be deleted).
6. Choose **File > Open project...**, paste the path, and click **Cancel** without pressing <kbd>Enter</kbd>. Do the same with **File > New project...** (type a folder and a name, then **Cancel**).

## Expected result

- **Step 3**: the project opens ("Baluster" in the title bar). No "Unconfirmed changes" dialog appears.
- **Step 4**: the project opens, with no "Unconfirmed changes" dialog.
- **Step 5**: the new project is made, with no "Unconfirmed changes" dialog.
- **Step 6**: each dialog closes and what you typed is abandoned: nothing is opened or made, and no "Unconfirmed changes" dialog appears.
- An "Unconfirmed changes" dialog still appears where it should: change a number in the Inspector without pressing Enter, then choose **File > Open project...**: the dialog lists that field (this guards against losing an edit; it is not part of the bug).

**Old behaviour (not fixed; `main` at `1f3675d52`):** after step 3 a dialog titled **Unconfirmed changes** says "You have changed these fields but not confirmed them:", lists **Project folder**, asks "Keep the changes or discard them?" and offers **Keep**, **Discard** and **Cancel**. By reading the code (`apps/openchuck-ui/src/shell.rs`, the Open and New project dialog), the folder field and the project name field are made with the same pending-edit control as the Inspector's fields, so **New project** is expected to do the same.

## Report back

Reply on issue #636 with: **PASS**, or **FAIL and what you saw** (the step and the dialog's title and buttons). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

The interface cannot be run without a display here, so nothing in this check was run. The old behaviour is from the issue and from the code on `main` at `1f3675d52`. At `81bade8a3` the shell's dialog code says "New project and Open: OK accepts the dialog's own fields and Cancel abandons them, so neither asks about them (#636)", and the fix's commit adds that File > Open still asks about real unconfirmed edits before the dialog opens (the last bullet of "Expected result"). `openchuck-cli validate` reports the project valid.

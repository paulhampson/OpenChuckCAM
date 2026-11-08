Owner check for issue #636 (fixed in #653): File > Open project with a
pasted path opens the project, with no Keep/Discard dialog
Build: commit 81bade8a3 or later (Help > About OpenChuckCAM), or main.

1. With no project open: File > Open project...
2. Paste the full path of this folder's "project" folder; click OK without
   pressing Enter.
Expected: the project opens. No "Unconfirmed changes" dialog.
3. File > New project...: type a folder and a name, click OK without Enter.
Expected: the project is made, no "Unconfirmed changes" dialog.
4. Open project / New project again, type, then Cancel.
Expected: the dialog closes, nothing is opened or made, no dialog.
Old behaviour: "Unconfirmed changes ... Project folder ... Keep the changes
or discard them?" with Keep / Discard / Cancel.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.

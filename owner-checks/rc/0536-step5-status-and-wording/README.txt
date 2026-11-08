Owner check for decision 0536 build step 5: the lost status lines, and
the NFR-12 wording
Build: commit 81bade8a3 or later (Help > About OpenChuckCAM), or main.

1. No project open: File > Import model...: an error dialog "Import model
   failed": "Import model: there is no project open. Open or create a
   project first."
2. Open the folder "project": a finished "Opened ..." entry in Jobs.
3. Operations, Generate: the badge never reads "Good to go" ("Nothing to
   report" or "Needs attention"); Face end's line reads "Facing covered
   the end faces it was asked to."; no "verified path geometry",
   "certify", "certificate".
4. Simulate: the line adds "; the simulation shows what it removed".
5. Export with a changed tool number: a "Tool numbers: ..." warning in
   Messages with Save and Undo. Ctrl+S: it goes, and Jobs has "Saved ...".
Before step 5: none of the status texts was shown; the badge read "Good
to go"; "Facing coverage complete (verified path geometry); ...".

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.

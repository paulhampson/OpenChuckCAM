Owner check for decision 0536 build step 5: the lost status lines, and
the NFR-12 wording
DRAFT: to run on the release candidate, once the fix is merged.

1. No project open: File > Import model...: an error dialog.
2. Open the folder "project": a finished "Opened ..." entry in Jobs.
3. Operations, Generate: the badge never reads "Good to go" ("Nothing to
   report" or "Needs attention"); Face round's Results line reads "Facing
   covered the end faces it was asked to; the simulation shows what it
   removed"; no "verified path geometry", "certify", "certificate".
4. Export with changed tool numbers: a warning in Messages with Save and
   Undo. Ctrl+S: it goes, and Jobs has "Saved ...".
Today: none of the status texts is shown; the badge reads "Good to go".

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.

Owner check for issue #634: the tooltip of "Exact STEP surface contact"
DRAFT: to run on the release candidate, once the fix is merged.

1. Open the folder "project" with File > Open project.
2. Operations: hover "Exact STEP surface contact" on "1 Axial, tapered
   ball", "2 Pencil, flat" and "3 Helical, ball".
Expected: short tooltips ending at "More accurate and much slower." (axial,
pencil) or "More accurate and can be slower." (helical); no measurements,
no decision numbers.
Old behaviour: "... Measured once on one test part (decision 0338): pencil
clean-up took about 31 times as long ..." and "... (decision 0277) ...".

Nothing in this check shows that a program is safe to run. Generated
G-code must be verified by you before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.

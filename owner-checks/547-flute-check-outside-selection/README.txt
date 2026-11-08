Owner check for issue #547: flute check and stock outside the selected faces
Build: the nightly of 8 Nov 2026 (76796fb24) or later, or main.

1. Open the folder "project" with File > Open project.
2. Click Operations, then Generate (under a minute).
3. Click "2 Finish vase" and read Messages.
4. With it selected, Tool > Edit tool..., set Flute length to 3, Close,
   Generate; read Messages again.
   (Ignore "the machine's feed limits slow this move" lines.)

Expected: after step 2 no "Flute length" line on "2 Finish vase". After step
4 one warning (path still made): "Flute length (operation 'Finish vase', ...):
the stock may stand up to 4.005 mm above the tool's tip ... (at X 44.883,
A 275.5), more than the tool's 3.000 mm of flute ...".
Old behaviour: at step 2 it was refused: "parameter finishing_layers: the
stock may stand up to 5.523 mm ... more than the tool's 5.000 mm of flute ...".

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.

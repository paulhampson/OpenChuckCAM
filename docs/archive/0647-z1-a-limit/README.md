# Evidence for decision record 0647

Raw evidence cited by [decision record 0647](../../decisions/0647-the-z1-turns-a-at-most-1800-deg-per-min.md) (lane ALIMIT, issue #793): the Makera Z1's A limit becomes 1800 deg/min. The old programs are those of `main` at `bc1df28`; the new ones are the golden files of the record's pull request.

| File | What it is |
| --- | --- |
| `fdiff.py` | The F-word diff check: for each changed golden G-code file, strips the F words and the estimates in operation comments and compares the rest line by line; prints the fed blocks, those whose F in force changed, the changed estimates and any merged pure-A blocks, and exits non-zero on any other change. Run from the repository root: `python3 docs/archive/0647-z1-a-limit/fdiff.py bc1df28`. |
| `fdiff-output.txt` | Its output for this change: no other change in any file. |
| `fwtime.py` | Times a Z1 program with the firmware's rule (decision 0007) and A capped at a given rate. Fed moves follow the firmware; rapids use a rough model, for comparison only. `python3 docs/archive/0647-z1-a-limit/fwtime.py 1800 bc1df28:<path> <path>`. |
| `fwtime-output.txt` | Its output for the changed Z1 programs, old (rc2, `bc1df28`) against new, at 1800 deg/min: the same fed time to 0.02 %. |

Generated G-code is the user's to verify before it is run on a machine (NFR-12); these checks compare programs with a model and do not verify them.

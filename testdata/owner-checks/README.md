# Owner checks

Small projects and step-by-step checks for the project owner, one folder per issue whose fix is merged and waits for the owner to confirm it (label `awaiting-fix-feedback`). Written by lane OWNERCHECKS from public material only: the user guide's baluster ([`../examples/baluster`](../examples/baluster/)), the blisk starting project ([`../examples/blisk-start`](../examples/blisk-start/)) and the public STEP files of [`../step/valid`](../step/valid/). Nothing here comes from a private project.

These folders are not part of the examples zip (only `testdata/examples/` is packaged). Each one has:

- `generate.py`, which writes its project with fixed ids (run it from anywhere with `python3`);
- `project/`, the written project, where it is small enough to keep (otherwise the script writes it into a folder you name);
- `CHECK.md`, the steps, the expected result, the old behaviour, and what we saw on the build before the fix and on `main`;
- `COMMENT.md`, the short text posted on the issue.

> [!WARNING]
> Nothing in these checks shows that a program is safe to run. They look at messages and paths only. Do not run their G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

| Issue | What to check | Time | Folder |
| --- | --- | --- | --- |
| #555 | Axial finishing limited to a selection stops at the selection's edge and no longer runs down onto the tab | 5 min | [`555-axial-into-tab`](555-axial-into-tab/CHECK.md) |
| #558 | Shortening the free-end tab and pressing Apply makes the operations stale, even if you click away at once; an unapplied draft is listed in Messages | 10 min | [`558-tab-edit-makes-stale`](558-tab-edit-makes-stale/CHECK.md) |
| #556 | Zooming stays smooth with a selection set of about 470,000 triangles shown | 10 min | [`556-zoom-large-selection`](556-zoom-large-selection/CHECK.md) |
| #548 | A tab shorter than the tool no longer stops an operation that cannot reach the tab; a whole-part operation gets a "Tab length" warning | 5 min | [`548-tab-length-limited-op`](548-tab-length-limited-op/CHECK.md) |
| #547 | The flute check no longer refuses because of stock the operation cannot reach, and still warns where it can | 5 min | [`547-flute-check-outside-selection`](547-flute-check-outside-selection/CHECK.md) |
| #477 | A free-end ring that would part the part from the chuck is warned of | 5 min | [`477-free-end-ring`](477-free-end-ring/CHECK.md) |
| #442 | Swarf stops short of the faces at the ends of the blade (Keep off the end faces), and warns when the setting is off | 10 min | [`442-swarf-past-blade-end`](442-swarf-past-blade-end/CHECK.md) |
| #512 | Face-flow finishing on the blisk's hub floor, with a guide edge | 15 min | [`512-face-flow-finishing`](512-face-flow-finishing/CHECK.md) |
| #351, #257 | Exact STEP surface contact, on and off, for axial finishing with a tapered ball and pencil clean-up with a flat end | 15 min | [`351-257-exact-step-contact`](351-257-exact-step-contact/CHECK.md) |

Shared helpers: [`ownercheck.py`](ownercheck.py) (writes the project files), [`gcode_extent.py`](gcode_extent.py) (reads where a program's fed moves go) and [`blake3_pure.py`](blake3_pure.py) (the model hash, for the one project too large to keep).

Reply on each issue with **PASS**, or **FAIL and what you saw**. Numbers and messages only: never paste or attach files from a private project.

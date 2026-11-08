# Owner checks

Projects and step-by-step checks for the project owner, one folder per issue whose fix is merged on `main` and waits for the owner to confirm it (label `awaiting-fix-feedback`, milestone 2026.1-alpha). Written by lane OWNERCHECKS from public material only: the user guide's baluster ([`testdata/examples/baluster`](../testdata/examples/baluster/)), the blisk starting project ([`testdata/examples/blisk-start`](../testdata/examples/blisk-start/)) and the public STEP files of [`testdata/step/valid`](../testdata/step/valid/). Nothing here comes from a private project.

This folder lives only on the branch `owner-checks/2026.1-alpha`, which is never merged. Each check folder has:

- `<folder>.zip`: the finished project and a one-page `README.txt`; unzip it anywhere and open `<folder>/project` with **File > Open project**. Nothing needs to be run first;
- `CHECK.md`: the numbered steps, the expected result, the old behaviour, and what we saw on the build before the fix and on `main`;
- `project/`: the same project as files, to read here (not for #556, whose model is about 25 MB: it is in the zip only);
- `generate.py`, which writes `project/` with fixed ids, and `COMMENT.md`, the text posted on the issue.

> [!WARNING]
> Nothing in these checks shows that a program is safe to run. They look at messages and paths only. Do not run their G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

| Issue | What to check | Time | Steps | Project zip |
| --- | --- | --- | --- | --- |
| #555 | Axial finishing limited to a selection stops at the selection's edge and no longer runs down onto the tab | 5 min | [`555-axial-into-tab`](555-axial-into-tab/CHECK.md) | [`555-axial-into-tab.zip`](555-axial-into-tab/555-axial-into-tab.zip) (62 KB) |
| #558 | Shortening the free-end tab and pressing Apply makes the operations stale, even if you click away at once; an unapplied draft is listed in Messages | 10 min | [`558-tab-edit-makes-stale`](558-tab-edit-makes-stale/CHECK.md) | [`558-tab-edit-makes-stale.zip`](558-tab-edit-makes-stale/558-tab-edit-makes-stale.zip) (13 KB) |
| #556 | Zooming stays smooth with a selection set of about 470,000 triangles shown | 10 min | [`556-zoom-large-selection`](556-zoom-large-selection/CHECK.md) | [`556-zoom-large-selection.zip`](556-zoom-large-selection/556-zoom-large-selection.zip) (9.4 MB) |
| #548 | A tab shorter than the tool no longer stops an operation that cannot reach the tab; a whole-part operation gets a "Tab length" warning | 5 min | [`548-tab-length-limited-op`](548-tab-length-limited-op/CHECK.md) | [`548-tab-length-limited-op.zip`](548-tab-length-limited-op/548-tab-length-limited-op.zip) (67 KB) |
| #547 | The flute check no longer refuses because of stock the operation cannot reach, and still warns where it can | 5 min | [`547-flute-check-outside-selection`](547-flute-check-outside-selection/CHECK.md) | [`547-flute-check-outside-selection.zip`](547-flute-check-outside-selection/547-flute-check-outside-selection.zip) (66 KB) |
| #477 | A free-end ring that would part the part from the chuck is warned of | 5 min | [`477-free-end-ring`](477-free-end-ring/CHECK.md) | [`477-free-end-ring.zip`](477-free-end-ring/477-free-end-ring.zip) (62 KB) |
| #442 | Swarf stops short of the faces at the ends of the blade (Keep off the end faces), and warns when the setting is off | 10 min | [`442-swarf-past-blade-end`](442-swarf-past-blade-end/CHECK.md) | [`442-swarf-past-blade-end.zip`](442-swarf-past-blade-end/442-swarf-past-blade-end.zip) (69 KB) |
| #512 | Face-flow finishing on the blisk's hub floor, with a guide edge | 15 min | [`512-face-flow-finishing`](512-face-flow-finishing/CHECK.md) | [`512-face-flow-finishing.zip`](512-face-flow-finishing/512-face-flow-finishing.zip) (70 KB) |
| #351, #257 | Exact STEP surface contact, on and off, for axial finishing with a tapered ball and pencil clean-up with a flat end | 15 min | [`351-257-exact-step-contact`](351-257-exact-step-contact/CHECK.md) | [`351-257-exact-step-contact.zip`](351-257-exact-step-contact/351-257-exact-step-contact.zip) (10 KB) |

Shared helpers: [`ownercheck.py`](ownercheck.py) (writes the project files), [`make_zips.py`](make_zips.py) (writes the zips: sorted entries, fixed times, so the same projects give the same bytes), [`gcode_extent.py`](gcode_extent.py) (reads where a program's fed moves go) and [`blake3_pure.py`](blake3_pure.py) (the model hash, when the `blake3` package is not installed).

Each zipped project was checked with `openchuck-cli validate` on `main` at `ae608e28f` (all valid).

Reply on each issue with **PASS**, or **FAIL and what you saw**. Numbers and messages only: never paste or attach files from a private project.

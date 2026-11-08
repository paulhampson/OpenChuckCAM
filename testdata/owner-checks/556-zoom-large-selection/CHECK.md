# Check for issue #556: zooming with a large selection set shown

**What was fixed** (pull request #560, merge commit `e1d5b516`): the yellow outline under the pointer has its own layer in the view, so moving the pointer or zooming no longer rebuilds the large selection's highlight, and a set's outline is worked out once. The lane measured a zoom frame with your carving's 496,172-triangle set shown at a median of 873 ms before and 0.5 ms after (on the issue). Showing or editing a large set still costs a pause once (about 0.3 to 0.8 s); making that wait-free is open item OI-2050, for you to prioritise.

**Time:** about 10 minutes, plus about a minute to write the project.

## The project

The model is too big to keep in the repository (about 25 MB), so [`generate.py`](generate.py) writes the project into a folder you choose. It is made from public material only: the user guide's baluster with 4,500 points round each ring instead of 48, **504,000 triangles**, and one selection set, **Everything but the ends**, of **468,000 triangles** (every triangle from X 0.5 to 60.5), about the size of your carving's set. There are no operations.

Run, with a folder outside the repository that does not exist yet:

```text
python3 testdata/owner-checks/556-zoom-large-selection/generate.py C:\checks\556
```

(On Windows the command may be `py` instead of `python3`.) It needs nothing beyond Python 3.8; it prints "working out the model's hash (about a minute)" unless the `blake3` package is installed, and ends with "wrote ...: 504000 triangles, 468000 in the set 'Everything but the ends'".

## Steps

Use the nightly build of 8 November 2026 (commit 76796fb24) or later, or a build from `main`.

1. Open the written folder with **File > Open project**.
2. Click **Operations** at the top, then **Front** and **Fit** above the view.
3. In the project panel, under **Selection sets**, click **Everything but the ends**. The set is drawn on the part. (A pause of up to a second or so here is expected: OI-2050.)
4. With the pointer over the part, zoom in and out with the mouse wheel for a few seconds, then move the pointer about over the part.
5. Click away from the set (so it is no longer shown) and zoom again, to compare.

## Expected result

- Zooming and moving the pointer with the set shown are as smooth as with it hidden: the view follows the wheel at once, with no pause of about a second on each step.
- The set stays drawn the same; nothing about what is selected changes.

**Old behaviour (not fixed):** with the set shown, each zoom step froze the view for about a second (your report: 873 ms a frame on the carving), so zooming came in jerks.

**What this does and does not show.** This is a public stand-in of about the same size as your set, not your carving. If it is smooth here and on your carving, the issue is fixed for you. If it is smooth here but not on your carving, say so: something else in that project is slow.

## Report back

Reply on issue #556 with: **PASS**, or **FAIL and what you saw** (on this project and on your carving: smooth, or a pause of about how long on each zoom step; your computer and graphics card if it fails). Numbers only; no project files from your own work.

## What we saw when preparing this

- `generate.py` was run twice on Linux (once with the `blake3` package, once without it): both wrote the same project, and `openchuck-cli validate` on `main` (`ae608e28f`) finds it valid, with one setup warning that the grip covers 7 mm of the part (expected; no operations use it).
- The zoom itself was **not** measured here: the container this was prepared in has no display or graphics card, so neither the build before the fix (`2f898cfb8`, the parent of `e1d5b516`) nor `main` could be run with a view. The before and after numbers above are the fixing lane's, on your carving and on the 1,000,000-triangle public benchmark model (1,783 ms to 0.3 ms a zoom frame, pull request #560).

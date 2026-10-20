# 0165: STEP in the model chooser, hidden helper programs, window title and project folder (lane BUG-IMP; #220, #221, #218, #219; OI-581 to OI-583)

- **Status:** proposed. Items 1 and 2 are bug fixes that follow requirements; items 3 and 4 wait for the owner's answers (OI-581, OI-582).
- **Date:** 20 Oct 2026.
- **Open items:** OI-581, OI-582, OI-583.

## Decision

1. **The model chooser offers STEP (#220, IMP-01, IMP-02, IMP-06).** The Browse dialog's filter is "3D models (STL, STEP)" with `stl`, `step` and `stp` in either case; the path box hint says "model file (STL or STEP)". A STEP file chosen is imported by the STEP worker of decision 0027. A build with no STEP reader says so in plain words: "this build cannot read STEP files: the STEP reader program … was not found next to the application" (NFR-07).
2. **Every helper program starts without a console window (#221, NFR-17).** The only helper program the application starts that is not a deliberate file browser is the STEP worker (`openchuck-import`, `run_worker`); it was started without `CREATE_NO_WINDOW`, so each import of a STEP, including the re-imports a STEP project makes when it is opened or checked, flashed a console window on Windows. It now starts with that flag on Windows and with none elsewhere; its standard error goes to a file in its scratch folder, and the tail of it is added to the error message the application writes to the log file (#155). The Open log folder and Open folder buttons start the system file browser on purpose and keep their window. The `git` probe of the build script runs at build time only. The check on a real Windows desktop is the owner's: OI-582.
3. **Window title (#218).** Waits for the owner's format (OI-581).
4. **New project folder (#219).** Waits for the owner's rules and for a ruling on whether the file layout changes (OI-583); no schema is claimed until then.

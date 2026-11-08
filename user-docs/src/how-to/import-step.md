# Import a STEP file

A STEP file (`.step` or `.stp`) holds a model's exact faces and edges, not just triangles. Swarf, face flow finishing and radial drilling need it, and edge following and pencil clean-up use its exact edges. Other strategies cut a mesh made from it, 0.01 mm and 0.1 radians from the exact surface.

## Import it

1. Open a project, then choose **File > Import model...** (<kbd>Ctrl</kbd>+<kbd>I</kbd>).
2. Type the file's path, or click **Browse...**, and click **Check and preview**. Nothing is added to the project yet.
3. Under **1 UNITS**, check the unit. A STEP file states its own length unit, which the program uses; choose another only if the file is wrong.
4. Under **2 PLACEMENT**, choose how to place the model on the rotary axis: **Keep as in the file**, **Centre on the rotary axis**, or turn one of the file's axes onto the rotary axis. The Model step's placement steps can change it later.
5. Read **3 MESH CHECK** and the notes under it.
6. Leave **Copy the file into the project folder** ticked, so the project keeps its own copy and can be moved or shared. Untick it to link the file where it is instead; the project then breaks if the file moves.
7. Click **Add to project**.

The notes include one that says which strategies use the exact faces and which use the mesh. Each operation's notes say the same for that operation.

## If it does not import

The STEP reader is a separate program, `openchuck-step-worker`, which ships beside the others. The import stops with a message in red when:

"STEP import failed: this build cannot read STEP files: the STEP reader program 'openchuck-step-worker' was not found next to the application ..."
: The reader is missing. Keep `openchuck-step-worker` in the same folder as `openchuck-ui`; on macOS, use the copy inside `OpenChuckCAM.app`. If you built the program from source, build the reader too (see [Install and first run](../start/install.md#build-from-source)).

"not a valid STEP file: ..."
: The file is damaged, or holds no faces. Export it again from your CAD program as a solid.

"the STEP reader did not finish within 120 s and was stopped"
: The file is very large or very complex. Simplify it, or export only the part you need.

"the STEP reader stopped without an answer ..."
: The reader stopped on the file. The program itself carries on. Export the file again from your CAD program and try once more.

STEP files written by CAD programs other than OpenCASCADE have had little testing. Check the imported model against your design before you go on.

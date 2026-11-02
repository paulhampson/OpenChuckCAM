# 0010: Slice 2 design refinement: STL import, mesh checks and model placement

**Status:** Accepted (11 Oct 2026). Built and merged in pull request #12. Item 4 (units) was chosen by the owner after a look at OrcaSlicer, Cura and FreeCAD; the owner accepted the rest of the record, including the items marked **(owner)**, when closing the slice. The waiver of the first nightly fuzz run is recorded under "Slice close".


Numbering: drafted as 0009 with open items OI-46 to OI-49, renumbered to 0010 with OI-49 to OI-52 when `main` took 0009 (LinuxCNC fixture checks) and OI-46 to OI-48.

## Context

Work breakdown slice 2 (STL import and mesh checks, headless) covers IMP-01, IMP-03, IMP-04 and IMP-05, with NFR-07 for the importer. Its design refinement asks for the imported model data and the deterministic face and edge numbering scheme that STEP will share later, the `stl_io` adapter behind `ModelImporter`, and where the unit boundary sits. Architecture sections 3, 5.4, 5.6, 7 and 8; verification plan sections 4 (IMP-01 to IMP-05, NFR-07) and 5 (import, geometry, STL tolerance statement). Decision 0003 (STEP numbering) and decision 0008 (project folder, open item OI-44) bear on it.

| Slice 2 item | Where it is decided |
| --- | --- |
| 1. `ModelImporter` trait with an STL adapter, ASCII and binary (IMP-01) | Items 1 to 3 |
| 2. Units from the file, user override in the project, millimetres at the import boundary (IMP-03) | Item 4 |
| 3. `openchuck-geom` adjacency and edge map; checks and a report the UI can show (IMP-04) | Items 2, 5 |
| 4. Transforms and orientation to the rotary axis, stored as project data (IMP-05) | Items 6, 7 |
| 5. Corpus tests and fuzzing of the STL reader (NFR-07) | Item 10 |
| 6. CLI `inspect` | Item 9 |
| 7. The STL tessellation statement | Item 11 |

## Decision

1. **Importer interface.** `openchuck-import` defines `ModelImporter` (`format`, and `import` from a file's bytes with `ImportOptions` and a `JobContext` for cancellation and progress) and `ImportedModel`: format, geometry, the units used, the normals the file states (for a mesh) and notes for the user. Geometry is the shared `openchuck_types::model::ModelGeometry` (a `TriangleMesh` or a `BrepModel`), so STEP fills the same type in slice 11. The importer is chosen by the file name's extension (`ModelFormat::from_file_name`: `.stl`; `.step` and `.stp`, which report "not available yet" until slice 11). Importers read bytes, not paths, so the application can hash and import one read of the file (PRJ-02), and the fuzz target needs no files. Errors are `ImportError` with messages that name the facet or triangle and say what is wrong (NFR-07).
2. **Numbering (shared with STEP).** For a mesh, triangle `i` is the `i`th facet of the file (across every `solid` of an ASCII file), so a triangle selection (PRJ-06) is stable for the same file. Vertices are numbered in order of first use. Edges are numbered by `openchuck-geom`'s edge map in ascending `(low vertex, high vertex)` order. All three depend only on the file. A B-rep keeps decision 0003's face and edge numbering (OpenCASCADE `TopExp_Explorer` order); its tessellation's triangles are never selectable.
3. **The STL adapter over `stl_io` 0.11.0 (architecture section 7).** `stl_io` types stay in `openchuck-import::stl`. Around it:
   - ASCII or binary is decided here, not by `stl_io`: a file whose size is exactly what its header's triangle count says (84 + 50 bytes per triangle) is binary even if its header starts with `solid`, as some exporters write it (`stl_io` would try ASCII and fail); otherwise a file starting with `solid` that is valid text is ASCII; anything else is binary and must be at least as long as its count says ("it is truncated" otherwise). Extra bytes after the last triangle are ignored with a note.
   - A nameless `solid` line and Windows line endings are accepted. An ASCII file with several solids is read as one model, with a note (`stl_io` stops at the first `endsolid`). A solid with no `endsolid` is reported as truncated.
   - Vertices are joined only when their coordinates are identical in the file, with `-0` and `0` the same (`stl_io` joins by bit pattern, which keeps them apart). Nothing is merged by distance: the checks report what the file holds, and repair is not in scope (IMP-04 asks for a report).
   - A non-finite binary coordinate is an error (`stl_io` passes it through); a file with no triangles is an error.
   - STL stores single precision, so ASCII and binary copies of a part give identical meshes; values are widened to `f64` exactly and then scaled (NFR-05).
4. **The unit boundary (IMP-03); chosen by the owner after a look at OrcaSlicer, Cura and FreeCAD.** Conversion to millimetres happens in the importer and nowhere else; geometry is always millimetres internally. `LengthUnit` (um, mm, cm, m, in) is in `openchuck-types`.
   - **Millimetres by default.** STL has no unit field, so a file is read in millimetres unless the user has set a unit for the model. OrcaSlicer, Cura and FreeCAD all read STL this way; none reads a unit from an STL header.
   - **A unit the file names is confirmed, never applied silently.** A binary header or ASCII `solid` line holding `unit` or `units`, then `=` or `:`, then a unit word (`UNITS=in`), ignoring case, is read as a hint. If it is not millimetres the import raises a `UnitQuestion` ("the file says its unit is in, but it was read in mm; convert it from in?"). An unknown word is noted.
   - **A size check asks the user to choose.** Read in millimetres, a part whose largest bounding-box side is below 2 mm (`SMALL_PART_MM`) raises a question suggesting in, cm or m (m first below 0.2 mm), and one above 2000 mm (`LARGE_PART_MM`) suggests um. A unit the file names takes precedence. OrcaSlicer asks the same way (by volume: under 8 mm³ inches, under 0.008 mm³ metres); Cura scales silently by a power of ten, which is not safe for G-code.
   - **The answer is stored per model only when needed.** A front end asks the question and stores the answer on the model reference (`units = "in"`) with `SetModelUnits`; answering "keep millimetres" stores `mm`, so the question is not asked again. A model nobody asked about has no `units` field. With a unit set, nothing is asked. The CLI asks on a terminal at `import` (Enter converts from a unit the file names, and keeps mm for a size question), and otherwise adds the model unanswered with a warning naming `openchuck-cli units`; `inspect` and `validate` show an open question. Inch display remains a UI setting (architecture section 8).
5. **Mesh checks (IMP-04)** in `openchuck-geom::check`, over the edge map (`openchuck-geom::edges`), reporting without changing the mesh:
   - holes: edges used by one triangle, grouped into one hole per connected run;
   - non-manifold edges: used by more than two triangles;
   - flipped triangles: wound against their neighbours. Each piece joined through two-triangle edges is oriented from its lowest triangle; a closed piece is expected to enclose a positive volume, so a closed piece that is inside out has every triangle flipped (and counts as an inside-out shell), and an open piece takes its majority winding as correct;
   - normal mismatches: a stated normal (STL) pointing against its triangle's winding, reported apart from flipped triangles; zero or non-finite stated normals are ignored;
   - degenerate triangles: a repeated vertex, or `|ab x ac| <= 1e-12 x (longest side)^2`.
   Every list is sorted, so the report is identical on every run (NFR-06). `MeshReport::problems()` gives one line per kind for the user; the lists give the edges and triangles for the UI to highlight (slice 8). The enclosed volume is reported for a closed mesh. In-house over an edge map, as architecture section 7 says (`halfedge` is not selected).
6. **Placement as data (IMP-05).** `openchuck_types::transform::TransformStep` is one of: translate (x, y, z), rotate (about the X, Y or Z axis through the origin, right-handed, degrees), uniform scale (factor above zero), mirror (negates one coordinate), and centre on the rotary axis (moves the bounding box centre to Y = Z = 0, keeping X). A model's placement is an ordered list of steps on its model reference, written one value per line (`kind = "rotate"`, `axis = "y"`, `angle = 90.0`), so an edit is a small diff. The model frame's X axis is the rotary axis (decision 0004 item 5). **(owner)** Orientation to the rotary axis is stored as intent, a rotation step plus the centring step, not as resolved numbers, so it still centres the part if the model file is re-exported; `orient_to_rotary_axis(+z)` produces those steps. **(owner)** Only axis rotations and uniform scale for now (OI-50).
7. **Evaluating a placement** (`openchuck-geom::transform`): the steps become one affine map (the centring step uses the bounding box of the mesh as placed so far). Quarter turns use exact sines and cosines. A map that mirrors reverses each triangle's winding, so normals stay outward; stated normals are mapped by the linear part, which is correct because every step keeps angles. Checks run on the placed mesh, so the report and bounds are in the model frame.
8. **Project schema 2.** Model references gain the optional `units` and `transform` fields, so the project folder schema becomes 2, with the step 1 to 2 changing only the number (a schema 1 file has neither field). A model with neither is written as before. The first real migration fixture (`crates/openchuck-project/tests/fixtures/schema-1/`) loads through it (decision 0008 item 12). **OI-44:** a model's kind follows from its format, so `Project` refuses triangle selections on a STEP model and face or edge selections on an STL model, including when a model's path is changed under an existing selection. No `kind` field is stored. Slice 1 tests that selected faces on `.stl` paths now use `.step`.
9. **Application and CLI.** Commands `SetModelUnits` and `SetModelTransform` (undoable, UI-08, and in the random undo property test). `Application::add_model` imports and checks a file before adding it, so a file that cannot be read is refused and nothing changes; mesh problems are reported, not refused. It references the file by its path relative to the project folder (`..` allowed, as decision 0008 item 2 says). `inspect_model` and `inspect_file` run as jobs on the worker pool. `openchuck-app::model` re-exports what a front end needs. The CLI gains `inspect <file or project> [--units]`, `import <project> <file> [--units]`, `units <project> <model> <unit|file>` and `transform <project> <model> <steps…> [--clear]`; `validate` also imports every unchanged model, fails if one cannot be read and warns about mesh problems.
10. **Fuzzing and corpus (NFR-07).** `testdata/stl/` holds 27 small files written by `testdata/stl/generate.py`: the same cube in ASCII, binary, binary with a `solid` header and nameless with CRLF; unit-tagged cubes (in, m, an unknown word); small, tiny and huge cubes for the size check; two solids in one file; one file per defect kind; and eight damaged files. A `stl_import` fuzz target imports, checks and places any bytes; it runs for 1 minute on pull requests and 1 hour nightly, seeded from `testdata/stl/`. Property tests feed random and damaged bytes in every test run. `.gitattributes` keeps test data and `.stl` files byte-exact on Windows checkouts, since they are hashed.
11. **The STL statement.** `openchuck_import::STL_TESSELLATION_NOTICE` ("STL holds triangles only: the tessellation was fixed when the file was exported, and gouge tolerance on this part is measured against this mesh, not the original surface.") is a note on every STL import; `openchuck-cli inspect` prints it, and tests check it appears (verification plan section 5). The UI shows it in slice 8 (OI-51).

## Alternatives considered

- **Letting `stl_io` decide ASCII or binary and join vertices.** It misreads binary files whose header starts with `solid`, stops at the first solid, and keeps `-0` and `0` apart. Wrapping it costs little and keeps the survey's pick.
- **Our own STL reader.** Small, but the architecture names `stl_io` (88) and it reads both encodings safely; the fallback stays recorded.
- **Merging vertices within a distance.** That is repair, it changes triangle adjacency silently, and the tolerance would be a guess; IMP-04 asks for a report.
- **Treating an STL with no unit as an error, or asking every time.** Most STL files carry no unit, and millimetres is the common case; asking only when the file or the size suggests otherwise follows OrcaSlicer.
- **Applying a header's unit without asking** (this record's first draft). No surveyed program reads STL headers for units, so the convention is unproven, and a wrong silent scale by 25.4 is the costly case for CAM output.
- **Scaling small parts automatically** (Cura's "scale tiny meshes"). Silent, and powers of ten only, so never inches.
- **A 4x4 matrix in the project.** Unreadable in a diff and loses intent (mirror, orient); steps are what a user does.
- **Storing the resolved orientation (a rotation and a translation by numbers).** Simpler, but stale if the file changes; the centring step is re-evaluated instead.
- **A `kind` field on the model reference (OI-44).** Redundant with the format, which already chooses the importer; it could disagree with the file name.
- **Keeping schema 1 with optional fields.** An older build would then fail on "unknown field" instead of saying the project is newer.

## Verification

Tests named after the requirement IDs (decision 0004 item 14):

- **IMP-01:** ASCII, binary, `solid`-header binary and nameless CRLF copies give identical meshes; a property test writes random facets both ways and gets identical meshes with exact coordinates; corpus counts and bounds match references; triangle numbering follows the file across solids; add a model through the application and the CLI; undo and redo.
- **IMP-03:** millimetres by default; every unit (um to m and in) converts when set; a header's unit (in, m) raises a question and is not applied; small, tiny and huge parts raise a question with the right suggestions, and the thresholds are exclusive; a set unit asks nothing; the answer (including "keep mm") is stored, survives save and load, and undo brings the question back; the CLI warns when it cannot ask, `--units` answers up front, and the terminal answers are parsed as described.
- **IMP-04:** seeded holes (one triangle, one face, two faces, a vertex gap), non-manifold fin, flipped triangle (including the seed), inside-out part, reversed stated normal, collinear and repeated-vertex degenerates, all at once; clean analytic parts give no report; a property test reverses random triangles and finds exactly those; the defect corpus files report the right counts; `validate` warns.
- **IMP-05:** every step against a hand-written reference matrix; exact quarter turns; step order; mirror on each axis keeps every analytic part clean with the same volume (and without the winding fix it would be inside out); orienting a cone drawn along -Y puts it on the axis; centring keeps X; stated normals follow the placement; property tests: any placement keeps a clean part clean with the volume scaled by the cube of the scale, and the combined map equals step by step.
- **Nightly fuzz (waived):** the owner waived waiting for the first 1-hour nightly run of `stl_import` before closing the slice (11 Oct 2026). A nightly failure is addressed when it occurs; the failing input is kept as a regression test.
- **NFR-07:** every damaged corpus file gives a message that names the problem; random bytes and damaged good files never panic (property tests); the `stl_import` fuzz target ran 705,864 inputs in 2 minutes locally without a crash.
- **NFR-06:** inspection is identical at 1, 2 and N threads.
- **UI-08 and PRJ-04:** the undo property test includes the two new commands; the schema 1 fixture migrates.
- **Performance (not a requirement of this slice):** importing the 1M-triangle benchmark model as binary STL takes about 0.33 s and checking it about 0.6 s on the development container (`benches/benches/stl.rs`).

## Consequences

- New dependency `stl_io` 0.11.0 (MIT, no dependencies of its own), recorded in `docs/dependencies.md`. It is the architecture's pick, so no rule exception.
- `openchuck-types` gains `units::LengthUnit`, `model` (`ModelFormat`, `ModelKind`, `ModelGeometry`) and `transform`. `openchuck-geom`, `openchuck-import` and the application gain the APIs above; `openchuck-geom` has its example (OI-30).
- Project folder schema 2 (item 8). No change to `requirements.md` or `architecture.md`.
- `fuzz/` gains `stl_import`; `ci.yml` and `nightly.yml` run it, and the fuzz path filter includes `openchuck-geom`, `openchuck-import` and `testdata/stl/`.
- `benches/` gains the STL import and check benchmark.

## Slice close, 11 Oct 2026

- **Record accepted** as a whole by the project owner, including the items marked **(owner)** (placement stored as intent, axis rotations and uniform scale only).
- **Nightly fuzz waived:** the owner waived waiting for the first 1-hour nightly run of `stl_import`. A nightly failure is addressed when it occurs, and the failing input is kept as a regression test.
- **Slice 2 is closed** in `xtask/traceability.toml`, so CI now fails if IMP-01, IMP-03, IMP-04 or IMP-05 loses its tests. IMP-02 and IMP-06 close with slice 11.
- **OI-44 closed.** OI-50, OI-51 and OI-52 stay open or deferred (see below).

## Open items

Tracked in [open-items.md](../open-items.md).

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 (OI-44) | Selections must match the model kind. **Closed** by item 8 on acceptance. | Slice 2 | Done |
| 2 (OI-49) | The STL header unit convention (item 4) is a guess at what exporters write. **Closed** by the owner's choice in item 4: the header's unit is only a hint the user confirms, so a wrong guess costs a question, not a wrong part. | Slice 2 | Done |
| 3 (OI-50) | Placement has axis rotations and uniform scale only. Add an arbitrary-axis rotation, alignment of a picked face or edge to the rotary axis, or non-uniform scale if users need them; each is a new step kind, a compatible schema addition. | When a need appears (slice 10 picking makes face alignment possible) | User feedback |
| 4 (OI-51) | The UI must show the STL tessellation statement, the check report and the unit question (item 4, as a dialog at import), with the listed triangles and edges highlighted (architecture section 5.4, verification plan section 5). | Slice 8 | The UI shell |
| 5 (OI-52) | The size thresholds (2 mm and 2000 mm) are fixed. Once machine profiles exist (slice 3, SET-01), the upper one could follow the machine's work envelope. | Slice 3 | The machine profile |

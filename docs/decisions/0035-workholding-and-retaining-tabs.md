# 0035: Workholding and retaining tabs (SET-09, OP-21)

- **Status:** proposed (15 Oct 2026). The two requirements were added to `docs/requirements.md` at the project owner's direction (SET-09 and OP-21, both P1; tab shapes cylinder and frustum). The design below is guidance until the owner accepts it. Nothing is built.
- **Date:** 15 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): new OI-158 to OI-162.

Numbering: decisions 0033, 0034 and 0036 and open items up to OI-157 are on `main` (lanes E, H and I), so this record keeps 0035 and takes OI-158 to OI-162. The retaining tabs requirement is OP-21 because OP-20 is lane I's P2 requirement.

## Context

A rotary part is held at one end, often with the rest of the stock hanging beyond the part, and has to stay held until it is cut free. Two things were missing from the documents. First, the setup knew the stock but not how much of it the chuck grips (chuck depth), so roughing would plan cuts into the gripped end. Second, nothing left a connection between the part and the stock, so a finished part would drop or shift when its last support was cut, and could not be removed from the stock easily. SET-03, SET-05 and SET-06 define stock, placement and clearance only; COL-02 to COL-04 (collision checks against workholding) are P2.

The new requirements: **SET-09** (a gripped length at either end of the stock, with an optional holding diameter; the held region is never machined, links keep clear of it, and the simulation keeps it) and **OP-21** (retaining tabs shaped as a cylinder or a frustum, set by axial position, diameter and, for tabs around the axis, count and angle; operations leave them uncut and the simulation keeps them). The work breakdown adds slice 21 with sub-slices 21a, 21b and 21c (lanes U, V and W) so slices 12, 13 and 14, already in progress, are not touched.

## Decision (proposed)

1. **The held region and each tab are part of the target.** The cleanest way to keep every strategy off them is to add them to the surface the strategies already treat as the finished part, so stock to leave, gouge protection, the independent checker, the simulation target and the deviation report all treat them as part without any strategy changing. The held region is a cylinder of the stock's radius (or the holding diameter, where one is set) over its gripped length. A tab is a cylinder or a frustum.
2. **A composite surface in `openchuck-path`.** A new type implements the existing surface port over the model's surface plus a list of analytic solids: a tool drop returns the highest contact, a distance the nearest, a ray the first hit. Cylinder and frustum are exact analytic evaluators (decision 0017 gives the pattern: pure Rust, exact point, normal, closest point, ray). The port and `StrategyInput` do not change, and no public struct gains a field (the semver check, decision 0026). Results are identical at any thread count (NFR-06).
3. **Where they are defined: the setup.** They belong to how the part is held, next to stock (SET-03) and clearance (SET-06). Fields go in the setup in `openchuck-types` and `openchuck-project` with undoable commands. This is a project format change, so lane U claims a schema number under "Project schema ordering" when it starts; numbers 8 and 9 are already contended (lanes E, H, O and S).
4. **Tab geometry.** Cylinder: axial position, diameter, length. Frustum: axial position, the two end diameters (the smaller end towards the part by default, so the tab narrows to a break point at the part), length. For tabs around the axis (rather than one on the axis): count and start angle, evenly spaced. Defaults are not chosen here; they are for the owner (OI-158).
5. **Validation (NFR-07).** A held region must lie inside the stock and not overlap the other end's; a tab must lie inside the stock and between the held region and the part's far end; tab diameters must be positive and, for a frustum, distinct enough to be a frustum. Impossible values are refused with a message naming the field.
6. **The stock to leave, the tool and the tab.** A tab is a part feature, so strategies cut around it with the tool's own geometry and leave stock to leave on its sides like any wall. A tab narrower than the tool's cutting radius cannot be cut to size by that tool; the generator refuses with a message rather than leaving a wrong shape (OI-159).
7. **Cut-off is not built in P1.** OP-21 keeps the part joined; removing the tab (by hand, with a saw or by a separate cut) is the user's step. A parting-off operation is a later decision (OI-161).
8. **Simulation and deviation.** The simulated stock starts as the stock definition and the tabs and held region are in the target, so the deviation report counts them as part and an operation that cuts one is overcut with a location (SIM-05).
9. **Sub-slices** (work breakdown slice 21): 21a data and setup (lane U), 21b composite target (lane V), 21c wiring, simulation, deviation, forms and viewport (lane W). 21a and 21b need nothing merged first and touch no file lanes E or H own; 21c needs both, and its roughing and finishing tests need slices 12 and 14.

## Alternatives considered

- **Each strategy keeps clear of the zones itself:** repeats the logic six or more times and changes lanes E, H, I and others mid-flight.
- **An operation that adds tabs after roughing:** hard to keep consistent with stock-aware roughing and rest machining, which must know the tab is never to be cut.
- **Put the zones in the machine profile (a chuck definition):** the gripped length changes with every job, so it is per setup. A chuck geometry for collision checks (COL-04, P2) can later supply defaults (OI-160).
- **Subtract the zones from the stock instead of adding them to the target:** roughing would then not clear around them, and the simulation would not hold them.

## Consequences

- `docs/requirements.md` gains SET-09 and OP-21 (done with this record); the verification plan gains rows for both (done); the work breakdown gains slice 21, lanes U, V and W, the traceability row, the P1 count of 83 of 126 (done).
- The architecture is updated at the project owner's direction: sections 5.2 (the composite `SurfaceSource`), 5.6 (workholding and tabs in the setup), the pipeline's setup step, the coverage table and section 13 (made with this record).
- New P1 requirements need tests named after them, in slice 21; the traceability rule applies when slice 21 closes. Slices 12 to 14 close without them.
- A project without a zone or tab must generate byte-identical G-code.
- Open items: OI-158 (tab defaults and layout), OI-159 (tool size against tab size), OI-160 (chuck defaults from the machine profile), OI-161 (a cut-off operation), OI-162 (composite surface cost on large meshes).

## Owner's answers (15 Oct 2026)

The record is still proposed; these answers settle open items without accepting the rest.

- **OI-158, default tab length.** At the end next to the chuck, a tab runs from the inner edge of the held region to the start of the model. At the other end it runs from the end of the model to the end of the stock. Any held region is honoured, so a default never enters one. Count, diameters and which end of a frustum faces the part are still open. The defaults need the part's extent, so the application computes them when a tab is added (slice 21c); the stored data stays explicit (decision 0040 item 2).
- **OI-159, tool against tab.** A tab narrower than the tool is not permitted. It is refused with a message, not left to produce a wrong shape (item 6 changes from "refuse when the generator cannot cut it" to a check when the tool is chosen or the tab edited).
- **OI-160.** Deferred until COL-04 (P2).
- **OI-161.** Not a P1 requirement; no cut-off operation is built.


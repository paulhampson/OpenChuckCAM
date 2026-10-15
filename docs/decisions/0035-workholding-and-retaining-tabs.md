# 0035: Workholding and retaining tabs (SET-09, OP-20)

- **Status:** proposed (15 Oct 2026). The two requirements were added to `docs/requirements.md` at the project owner's direction (SET-09 and OP-20, both P1; tab shapes cylinder and frustum). The design below is guidance until the owner accepts it. Nothing is built.
- **Date:** 15 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): new OI-153 to OI-157.

Numbering: decisions 0033 (claimed twice, on the slice 13 and slice 14 branches) and 0034 (the slice 12 branch and the `occt-install-cache` branch) are claimed by unmerged work; this record takes 0035. Open-item numbers run to OI-152 on unmerged branches; this record takes OI-153 to OI-157.

## Context

A rotary part is held at one end, often with the rest of the stock hanging beyond the part, and has to stay held until it is cut free. Two things were missing from the documents. First, the setup knew the stock but not how much of it the chuck grips (chuck depth), so roughing would plan cuts into the gripped end. Second, nothing left a connection between the part and the stock, so a finished part would drop or shift when its last support was cut, and could not be removed from the stock easily. SET-03, SET-05 and SET-06 define stock, placement and clearance only; COL-02 to COL-04 (collision checks against workholding) are P2.

The new requirements: **SET-09** (a gripped length at either end of the stock, with an optional holding diameter; the held region is never machined, links keep clear of it, and the simulation keeps it) and **OP-20** (retaining tabs shaped as a cylinder or a frustum, set by axial position, diameter and, for tabs around the axis, count and angle; operations leave them uncut and the simulation keeps them). The work breakdown adds slice 21 with sub-slices 21a, 21b and 21c (lanes U, V and W) so slices 12, 13 and 14, already in progress, are not touched.

## Decision (proposed)

1. **The held region and each tab are part of the target.** The cleanest way to keep every strategy off them is to add them to the surface the strategies already treat as the finished part, so stock to leave, gouge protection, the independent checker, the simulation target and the deviation report all treat them as part without any strategy changing. The held region is a cylinder of the stock's radius (or the holding diameter, where one is set) over its gripped length. A tab is a cylinder or a frustum.
2. **A composite surface in `openchuck-path`.** A new type implements the existing surface port over the model's surface plus a list of analytic solids: a tool drop returns the highest contact, a distance the nearest, a ray the first hit. Cylinder and frustum are exact analytic evaluators (decision 0017 gives the pattern: pure Rust, exact point, normal, closest point, ray). The port and `StrategyInput` do not change, and no public struct gains a field (the semver check, decision 0026). Results are identical at any thread count (NFR-06).
3. **Where they are defined: the setup.** They belong to how the part is held, next to stock (SET-03) and clearance (SET-06). Fields go in the setup in `openchuck-types` and `openchuck-project` with undoable commands. This is a project format change, so lane U claims a schema number under "Project schema ordering" when it starts; numbers 8 and 9 are already contended (lanes E, H, O and S).
4. **Tab geometry.** Cylinder: axial position, diameter, length. Frustum: axial position, the two end diameters (the smaller end towards the part by default, so the tab narrows to a break point at the part), length. For tabs around the axis (rather than one on the axis): count and start angle, evenly spaced. Defaults are not chosen here; they are for the owner (OI-153).
5. **Validation (NFR-07).** A held region must lie inside the stock and not overlap the other end's; a tab must lie inside the stock and between the held region and the part's far end; tab diameters must be positive and, for a frustum, distinct enough to be a frustum. Impossible values are refused with a message naming the field.
6. **The stock to leave, the tool and the tab.** A tab is a part feature, so strategies cut around it with the tool's own geometry and leave stock to leave on its sides like any wall. A tab narrower than the tool's cutting radius cannot be cut to size by that tool; the generator refuses with a message rather than leaving a wrong shape (OI-154).
7. **Cut-off is not built in P1.** OP-20 keeps the part joined; removing the tab (by hand, with a saw or by a separate cut) is the user's step. A parting-off operation is a later decision (OI-156).
8. **Simulation and deviation.** The simulated stock starts as the stock definition and the tabs and held region are in the target, so the deviation report counts them as part and an operation that cuts one is overcut with a location (SIM-05).
9. **Sub-slices** (work breakdown slice 21): 21a data and setup (lane U), 21b composite target (lane V), 21c wiring, simulation, deviation, forms and viewport (lane W). 21a and 21b need nothing merged first and touch no file lanes E or H own; 21c needs both, and its roughing and finishing tests need slices 12 and 14.

## Alternatives considered

- **Each strategy keeps clear of the zones itself:** repeats the logic six or more times and changes lanes E, H, I and others mid-flight.
- **An operation that adds tabs after roughing:** hard to keep consistent with stock-aware roughing and rest machining, which must know the tab is never to be cut.
- **Put the zones in the machine profile (a chuck definition):** the gripped length changes with every job, so it is per setup. A chuck geometry for collision checks (COL-04, P2) can later supply defaults (OI-155).
- **Subtract the zones from the stock instead of adding them to the target:** roughing would then not clear around them, and the simulation would not hold them.

## Consequences

- `docs/requirements.md` gains SET-09 and OP-20 (done with this record); the verification plan gains rows for both (done); the work breakdown gains slice 21, lanes U, V and W, the traceability row, the P1 count of 83 of 126 (done).
- The architecture needs a small change to be accepted before slice 21c: the setup (section 5.6) holds a workholding zone and tabs, and the surface port (section 5.2) has a composite implementation. That edit is proposed and waits for agreement.
- New P1 requirements need tests named after them, in slice 21; the traceability rule applies when slice 21 closes. Slices 12 to 14 close without them.
- A project without a zone or tab must generate byte-identical G-code.
- Open items: OI-153 (tab defaults and layout), OI-154 (tool size against tab size), OI-155 (chuck defaults from the machine profile), OI-156 (a cut-off operation), OI-157 (composite surface cost on large meshes).

# 0633: The viewport marks the surfaces simultaneous finishing cannot reach, and Messages goes to each (OP-20, UI-11, UI-12, NFR-12, NFR-15; issue #416; lane VIEW-OVERLAYS)

- **Status:** proposed. The owner's answers to questions 1 to 4 (1 Nov 2026) were relayed by the coordinator (decision 0049); the record waits for the owner's acceptance. Its number is provisional: under the owner's numbering rule (1 Nov 2026) it takes the next free number after `main`'s highest when it is released.
- **Date:** 2 Nov 2026.
- **Issue:** #416 (OI-1451).
- **Requirements:** OP-20, UI-11, UI-12, SIM-05, NFR-06, NFR-12, NFR-15, GC-07. No requirement changes.
- **What it amends:** [decision 0368](0368-simultaneous-finishing-of-hidden-surfaces.md) item 6 (the unreached areas as structured data; the viewport left to a later lane) and its open item 2; the interface specification ([decision 0038](0038-ui-specification-and-requirements.md)), sections 11.4 and 11.5 and the Show menu. It builds on [decision 0216](0216-selection-set-colours-and-deviation-scale.md) (the colours, hatches and outlines already taken), [decision 0238](0238-simulate-display-and-operation-navigation.md) item 3 (the Show menu) and [decision 0492](0492-deviation-colours-on-the-part-model.md) (the part's deviation colours, which the areas lie over).
- **Open items:** OI-1451 closes when this is built. OI-1457 (walls square to the axis under an overhang are not reported) stays open and is a known limit here: what the strategy does not report is not marked.
- **Library:** `openchuck-path` changes (a new field on a public struct), so the library family takes the next minor version at release (decision 0048; provisionally 0.36.0, the Merge Manager assigns it).

## Context

Simultaneous finishing reports the areas of the part it cannot reach (OP-20; decision 0368 item 6): a warning lists up to five, largest first, with their X range, the A at which each faces the tool, area and reason, and `GeneratedPath::unreached` carries all of them as data. The owner asked for them to be highlighted on the part, and for each to be reached from its warning, so the user can check them against the simulation before running the program (NFR-12).

An area is given as an X range by an A range. That box also takes in surfaces the tool did reach: under a dovetail's lip, the lip's outside lies in the same X and A range as the floor below it. Marking every triangle in the box would mark finished surfaces as unreachable.

## Decision

1. **Exact points (owner, question 1, option A).** `openchuck_path::Unreached` gains `points`: the surface points the strategy sampled and could not reach, in its own order, taken evenly to at most `MAX_UNREACHED_POINTS` (4,096) per area. The X range, A range, area, reason and the warning are unchanged. Output stays the same whatever the thread count (NFR-06, GC-07).
2. **The part's triangles.** The application keeps each operation's areas with its toolpath and, off the interface thread, finds the part triangles the points lie on (the nearest triangle within one sample spacing). Those triangles are what is marked. An area's marker sits at its point nearest the area's middle.
3. **Where and when (owner, question 2, option A).** On Operations and on Simulate, whenever the operation's toolpath is shown (its tree eye is on). The Show menu gets "Unreachable areas", on by default, menu-only like "Held regions and tabs" (decision 0238 item 3). Nothing is shown from a generation that is out of date or failed: the areas go with the toolpath they came from.
4. **Look (owner, question 3, option A).** A two-tone dot pattern on the area's triangles, drawn over whatever colours lie beneath them (the part's grey, the deviation colours of decision 0492), and a numbered marker per area: the numbers follow the warning's order, so "area 2" in Messages is "2" in the view. The legend gets a row "Unreachable (operation name)". Dots are a pattern no other role uses; the colours, dot size and spacing are measured against the checks of decision 0216 (3:1 against every band and the part's grey, each simulated colour vision type, 100% and 120% scale) and given under "As built".
5. **Show in view (owner, question 4, option A).** Each area the warning lists gets "Show in view N" in Messages. It goes to the operation's step and selects the operation, orbits the camera to look straight at the area (at the middle of its A range, from the side it faces), frames its X range with a margin, and pulses its marker. The part stays in its setup placement and Simulate's playback does not move. The move takes about 0.3 s, and is instant with reduced motion; it is one camera step that can be undone, like Fit. Areas past the fifth have their markers in the view only, as the warning counts them.

## Specification entries proposed

- **11.4, Viewport,** and **11.5, Viewport:** "**Unreachable areas** *(OP-20; decision 0633)*. The surfaces an operation could not reach are dotted (two-tone dots) with a numbered marker each, in the warning's order, while its toolpath is shown and its result is current; the legend names them 'Unreachable (operation name)'."
- **The Show menu:** "Unreachable areas", on by default.
- **12, Messages:** "A warning listing unreachable areas offers 'Show in view N' for each: the view turns to the area and frames it; the part and the playback do not move."

## Alternatives considered

- **Mark every triangle in the X and A box** (question 1, option C, or drawing the box as a wedge, option B). Rejected: it marks reached surfaces as unreachable (the lip above an undercut), or shows where an area is but not which surface.
- **Only on demand from Messages** (question 2, option B). Not chosen: easy to miss, and the part can look clean when it is not.
- **A fill or an outline** (question 3, options B and C). Not chosen: a fill hides the deviation colours and few colours remain; outlines and hatches already mean sets, avoid surfaces, held regions and stale sets.
- **Turn the part to the area, or move the playback** (question 4, options B and C). Not chosen: the part would sit in a pose the program never gives it on Operations, and the user would lose their place in the playback.

## Consequences

- `crates/openchuck-path`: `Unreached::points`, `MAX_UNREACHED_POINTS`; a minor version and a changelog entry.
- `crates/openchuck-app`: the regeneration keeps each operation's areas; the mapping to the part's triangles as a job.
- `crates/openchuck-viewport`: a dot pattern beside the single and cross hatch.
- `apps/openchuck-ui`: the areas in the scene, the markers and legend row, the Show menu tick, "Show in view" in Messages.

## As built

To be completed with the build.

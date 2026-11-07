# 0532: Swarf stops short of the faces at the ends of a face, and measures every face next to it (OP-07, OP-12, OP-16, GC-07, NFR-06, NFR-07, NFR-12; issue #442; lane SWARFEND)

- **Status:** accepted (7 Nov 2026, by the project owner, relayed by the coordinator; decision 0049). Proposed by lane SWARFEND on 7 Nov 2026. It builds the owner's decisions D1, D2 and D3 (option A each, 6 Nov 2026, relayed by the coordinator). OI-2079 stays open; the owner asked for it to be noted on issue #442.
- **What it amends:**
  - **accepted [decision 0061](0061-slice-17-swarf.md) item 7 (the gate measures the selected faces only, OI-237):** the faces that share an edge with the cut face are now measured too, and those at the ends of the face stop the pass short (items 1 to 3).
  - **accepted [decision 0491](0491-swarf-keeps-the-tip-off-a-floor.md) item 3 (OI-1622, the owner's option C: the tool running past the ends of the face is issue #442):** this record is that issue's change.

  OP-07, OP-12 and the other requirements are unchanged. OP-12 asks for gouge checks "where defined"; the faces next to the cut face are now defined by the B-rep's edges.
- **Date:** 7 Nov 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-2078 to OI-2081 are new; OI-237 is narrowed (item 6).

Numbering: 0532 and OI-2078 to OI-2081 were given by the coordinator on 6 Nov 2026. No project schema number: the new setting is a strategy parameter, and parameters are a free map (decision 0008 item 6). The workspace version is sequenced by the Merge Manager at release (0.48.0, or the next free one).

## Context

Issue #442: at each end of a swarf pass the tool ran past the end of the face into the faces beside it, by up to the tool radius, and nothing measured it. The swarf gate measures only the cut face (0061 item 7, OI-237). On 6 Nov 2026 the owner asked for swarf passes to stop short of the face's ends automatically, and for the faces next to the cut face to be checked, with a note saying so.

Records read: 0021, 0061, 0111, 0175, 0176, 0272 and 0491 (swarf), 0255 (the bounded checker), 0049 (who accepts), 0048 (API changes before 1.0), 0516 (tests), and issue #567 (warning or information).

### What was measured (the repository's blisk, `testdata/step/valid/blisk.step`)

The example `crates/openchuck-app/examples/swarf_ends.rs` runs swarf on blades 7, 9 and 13 (tip extension 0, tolerance 0.01 mm, no sets). It then checks every fed pose, and the quarter, middle and three-quarter points of every fed move on the machine's straight move in X, Y, Z and A, with the test kit's bounded checker (decision 0255; intervals 0.0005 mm). It checks against each face that shares an edge with the blade, and against every other face.

Before this change, the deepest cut (found; proven at most) was:

| Tool | Disc wall at X = 20 (faces 2 and 3) and its rim (face 1) | End plane at X = 60 (face 15) |
| --- | --- | --- |
| Flat end mill, 3 mm | 0.283 (blade 7) to 1.500; 1.500 | 0.012 |
| Flat end mill, 6 mm | 0.567 to 3.000; 3.000 | 0.022 to 0.026 |
| Ball end mill, 3 mm | 0.283 to 1.500; 1.500 | 0.93 to 1.16 (the ball at the hub corner) |
| Tapered ball, 0.5 mm tip, 3 degrees, 3 mm | 0.251 to 0.765; 0.765 | 0.23 to 0.35 |

**The cause.** The pass runs from the first ruling of the face to the last, with the flank on the face. The tool's body reaches past the end ruling into whatever face meets it there.

**Twisted blades.** Blades 9 and 13 twist into the wall, so the side of the tool sweeps into it over a stretch of the pass. The deepest cut is 2.5 to 4.6 mm from the end, not at the last pose. The last ruling of those blades at X = 20 is only a corner, 0.016 mm long: the wall meets the rulings obliquely, at their ends. To clear the wall there the pass must stop 6.7 mm short with the 3 mm tools, 4.6 mm with the tapered ball and 10.8 mm with the 6 mm flat end mill. So stopping one tool radius short (the owner's first direction) would still have cut the wall by about 1 to 2 mm. The owner chose the computed stop (D1, option A) with these numbers in front of them.

## Decision

1. **A pass stops short of a face that meets an end of the face (D1, option A, the owner's choice).**
   - **Which faces.** The faces next to the cut face are those that share an edge with it in the B-rep. The application passes them to the pathing library as `RegionConstraint::neighbours`: the faces that share an edge with any face of the face set. A face of the set is included where it meets another face of the set.
   - **End faces.** Swarf samples each of those faces as it samples an avoid face (decision 0176 item 6 and 0491 item 1: a grid of at most an eighth of the tool radius, plus its trimmed edge). A face *meets an end* when it meets one of the rulings at the two ends of the whole face, and none of the ends of the rulings a quarter, half and three quarters of the way along. Meeting a ruling means a sample within its own spacing and the tolerance of the ruling's ends or its quarter, half and three-quarter points.
   - **Side faces.** A floor, or the face along the top of the rulings, meets the ends of the rulings all along the pass, so it is not an end face. It is measured (item 3), not stopped short of.
   - **A twisted face.** Where a twisted face meets a wall obliquely, the wall still counts as an end face. On the blisk, the X = 20 wall of blades 9 and 13 meets only a corner ruling and no middle ruling.
   - **How far short.** Each end face cuts the pass short at its ends as an avoid set on Cut short does (decision 0176 item 4): a pose, or a move, that would cut into it deeper than the tolerance is left out. The stop is found by bisection, so it is the least length that keeps the tool off the face, whatever the tool's profile (flat, ball, bull nose, tapered ball). The face is never lifted: a lift (0491) clears what the tip meets, not a wall across the pass.
   - **What stops nothing.** An open end (no face beside it), an end face the tool does not reach (the end cap of a bar, where the tool goes past into the air), and a face carried on flush past the end (the flank lies on it).
   - **Corners within the face set.** Where two faces of the face set meet at an inside corner, each pass stops short of the other face. At a convex corner nothing stops.
   - **The strip left.** The strip left uncut at an end is left on purpose. The gate measures only the rulings that are cut, so it is neither material left nor a reason to refuse the face (as for a pass cut short by an avoid set).
   - **The note.** The face's note says it: "The pass stops short of face 2, which meets this face at an end, so the tool keeps off it: about 6.935 mm along the face is left uncut there." When the faces next to the face were given, the note also says: "The faces that share an edge with this face were checked against the tool's cutting part; the shank, the holder and other faces were not."
2. **Keep off the end faces (D2, option A, the owner's choice).**
   - Swarf has a new setting, **Keep off the end faces** (`keep_off_end_faces`), a flag that is on by default. Its description: "Stops each pass short of the faces that meet the ends of the face, such as a wall the face stands against, by the least length that keeps the tool off them; that part of the face is left uncut and the note says how much. Off, the pass runs to the ends and any cut into those faces is warned of. Only the faces that share an edge with the face are checked."
   - Off, the passes run to the ends as before this record. Each cut into an end face deeper than the tolerance is a warning, never silent, and points at the setting: "Swarf on face 0 cuts 3.000 mm into face 1, which meets it at an end of the pass, at (38.000, 11.000, 10.000), more than this operation's tolerance of 0.01 mm. Keep off the end faces is off for this operation: turn it on to stop the pass short of face 1 (in the operation inspector's parameters, where the field is marked), or put face 1 in an avoid set on Cut short. The program is made with this cut in it: check the program and the simulation before running it."
3. **Every face next to the cut face is measured (D3, option A, the owner's choice).**
   - After the path is made, each face next to the cut face is measured against the written motion. That is the same motion the gate measures (the poses and the machine's straight moves between them, decision 0272), sampled as an avoid face.
   - A cut deeper than the tolerance is a warning that names the face, the depth and the place. For a face along the ends of the rulings (a floor), the warning points at Tip extension: "Swarf on face 9 cuts 1.494 mm into face 8, which meets it along the ends of its rulings, at (…), more than this operation's tolerance of 0.01 mm. To keep the tool off it, put face 8 in an avoid set on Cut short, which lifts the tool clear of it, or set a negative Tip extension to pull the tool up the face (in the operation inspector's parameters, where the field is marked). The program is made with this cut in it: check the program and the simulation before running it."
   - If an end face is still cut beyond the tolerance with the setting on (between the points checked), the warning says so and advises an avoid set or a smaller tool; it points at no setting.
   - The warnings change no G-code. A face in an avoid, check or watched set is measured by its set (decisions 0176 and 0491), not again here.
   - These are warnings, not information, under the owner's review of messages (issue #567): each is a cut into the part that may cause a problem. The application lists each, naming the operation, as swarf's other warnings are (decision 0175 item 5).
4. **Refusal (NFR-07).** End faces that leave no position of the tool clear of them refuse the face, naming them and what to change: "swarf cannot cut face 0: the tool would cut into face 1 or face 2, which meet it at its ends, wherever it cuts it, so nothing of it is left to cut; use a tool with a smaller radius, or turn Keep off the end faces off to cut to the ends (the cut into those faces is then warned of), or take the face out of the face set".
5. **Order, and what is unchanged.**
   - **Order.** The boundary's limits come first (decision 0176), then the tip extension and the lift (decision 0491), then the end faces. The end faces cut short the poses as lifted, so a lift can shorten the stop. They do this together with the avoid and check sets on Cut short.
   - **Avoid and check sets.** An end face in an avoid or check set is kept off by its set (a check set with its stock to leave), and is not an end face again. A face in a watched set (Warn only) is only measured by that set, as before: Warn only does not stop the pass short of it.
   - **Not weakened.** The overcut warning (decision 0175), the tolerance check (decision 0272), the material-left refusal (decision 0061 item 7) and the lift (decision 0491) are unchanged.
   - **Held regions and tabs.** These are kept clear as before for every strategy (SET-09; decisions 0161, 0523).
6. **What is not claimed (NFR-12).**
   - Only the faces that share an edge with the cut face are checked, and only against the tool's cutting part, as the gate is: the shank above the flutes, the holder and the rest of the part are not. OI-237 stays open for those, narrowed by this record.
   - The faces are checked to the sampling of a guarded face (OI-636). On the blisk, the bounded checker finds the end faces cut by at most 0.016 mm with the tolerance at 0.01 mm (Verification). OI-2078 tracks the gap.
   - Nothing in the notes, warnings or the description says the program is safe or that the part is uncut. The user checks the path and the simulation before running it.
7. **Deterministic (GC-07, NFR-06).** The faces are sampled and taken in face-number order. The cut short uses the same bisection, in a fixed order, as an avoid set's. The only parallel work (the guard tests and the measurement) goes through `crate::parallel`, in order. A path stopped short of an end wall is the same at 1, 2 and 4 threads (tested).
8. **Library API (decision 0048).**
   - `RegionConstraint` gains `neighbours`.
   - `swarf::Settings` gains `keep_off_end_faces`.
   - `swarf::Limits` gains `neighbours_checked`, `end_stops` and `neighbour_cuts`, with the new `swarf::EndStop` and `swarf::NeighbourCut`.
   - New `swarf::FacePath::neighbour_warnings` and `swarf::parameter::KEEP_OFF_END_FACES`.
   - These break the library API, and the output changes, so the workspace goes to 0.48.0, or the next free version, which the Merge Manager sequences at release. Each library crate's changelog says so.

## Alternatives considered

- **D1, option B (the owner's first direction): stop one tool radius short of every end.** Simple, but on blades 9 and 13 it still cuts the wall about 1 mm (3 mm tools) to 2 mm (6 mm flat). It also leaves a strip one radius wide at every end, open ends included.
- **D1, option C: the larger of a radius and the computed stop.** Never less than a radius, so it leaves a strip where nothing needs one.
- **D2, option B: always on, with no setting.** The user could not cut to an end deliberately, for example where the end face is stock to be removed later.
- **D2, option C: off by default.** The default would still cut the wall.
- **D3, option B: a note, not a warning.** Under issue #567 a cut into the part is a warning. **D3, option C: leave the other faces to OI-237.** That would not check the faces next to the cut face, which the owner asked for.
- **Calling a face an end face when it lies along the end ruling only.** This was built first. On the blisk, blade 9's last ruling at X = 20 is a 0.016 mm corner. So the rim cylinder (face 1), which meets that corner, was taken for an end face, and the disc wall was not. The blade's own twist cuts the rim by 0.013 mm all along the top, so the pass was stopped short by 7 to 54 mm and split into pieces. The rule of item 1, no middle ruling met, takes the wall and leaves the rim to be measured.
- **Leaving out samples of a face next to the cut face that lie on their shared edge**, so the cut face's own overcut there is not counted again: not needed on the cases measured, and it would hide a real cut near the edge.

## Consequences

- **Tests**, named after the requirements, written first and seen to fail:
  - `crates/openchuck-path/tests/swarf.rs`:
    - a pass stops short of a square end wall by the tool radius, and of a wall leaning over the face at 60 degrees by the radius times the cotangent of 30 degrees (5.196 mm, more than the radius); the independent checker finds the wall cut by no more than the tolerance; the strip left is not material left, and the note says it;
    - an open end, a bar's end cap, a flush continuation and a floor stop nothing short;
    - a ball and a tapered ball stop short too;
    - with Keep off the end faces off, the pass runs to the end and the cut is warned of, naming the face, the depth and the setting (the setting is a flag, on by default);
    - a floor a ball cuts into is measured within 0.02 mm of the checker and warned of, pointing at Tip extension, with the poses unchanged; with the floor in an avoid set it is lifted clear (decision 0491) and not measured again;
    - end walls that leave nothing refuse the face, naming them;
    - the path is identical at 1, 2 and 4 threads.
  - `crates/openchuck-app/src/operation.rs`: on a STEP part, the end wall joined to the wall by an edge stops the pass a tool radius short with no set chosen. The same test checks that Keep off the end faces off warns, and that without the edge the path is unchanged. With the wiring turned off, the test fails.
  - `crates/openchuck-app/tests/step_strategies.rs` (kernel): on the blisk's blade 9 with a 3 mm flat end mill:
    - before this change, the wall at X = 20 is cut more than 1.4 mm (bounded checker);
    - now, the pass stops more than 6 mm short of it;
    - the bounded checker finds the wall and the X = 60 plane cut by at most 0.02 mm;
    - the hub is measured and reported, not stopped short of.
- **Times** (uninstrumented, `cargo nextest run`): the new path tests take 0.1 to 3 s each, the application test 0.4 s, the kernel test 2 s (debug). None is long-running. On the blisk, swarf takes 0.03 to 0.2 s longer a blade (release).
- **Project format:** unchanged (schema 27). The new setting is a parameter.
- **Interface:** one new checkbox, Keep off the end faces, in swarf's parameters, with its description as the tooltip. No layout change.
- **Saved projects.** A swarf operation on a STEP part whose faces meet another face at an end (a wall the face stands against) is made differently after the update: its passes stop short of that face. Its notes gain the checked sentence, and it may gain warnings for faces next to it that it cuts (a floor under a ball tool). The same project still gives the same G-code within a version (GC-07). The changelogs say so.
- **The reference job** (`testdata/projects/swarf`, a hexagonal bar). Its flats meet each other at convex corners and the end caps at the bar's ends. Its golden programs are unchanged: the four swarf tests of `step_pipeline.rs` pass. Its notes gain the checked sentence; the notes are not in its golden files.
- **OI-237** is narrowed: the faces next to the cut face are now measured, and those at its ends kept off. The shank, the holder and the rest of the part are still not checked.

## Verification

Measured after the change with `swarf_ends`, the faces next to each blade given, tolerance 0.01 mm. The stop is the length along the face left uncut; the cut is the bounded checker's deepest cut into the end faces after the change (found; proven at most).

| Blade | Tool | Stops short of (mm) | End faces cut after, by the checker (mm) | Warned of (mm) |
| --- | --- | --- | --- | --- |
| 7 | flat 3 mm | wall 0.471; end plane 0.833 | 0.010; 0.010 | rim 0.014; hub 0.023 |
| 7 | flat 6 mm | wall 0.965; end plane 0.833 | 0.010; 0.010 | rim 0.027; hub 0.042 |
| 7 | ball 3 mm | wall 0.471; end plane 2.157 | 0.012; 0.012 | rim 0.014; hub 1.190 |
| 7 | tapered ball | wall 0.406; end plane 0.818 | 0.015; 0.016 | rim 0.108; hub 0.393 |
| 9 | flat 3 mm | wall 6.935; end plane 0.825 | 0.010; 0.010 | hub 0.295; rim 0.013 |
| 9 | flat 6 mm | wall 11.137; end plane 0.825 | 0.010; 0.010 | hub 0.579; rim 0.014 to 0.025 |
| 9 | ball 3 mm | wall 6.935; end plane 0.970 | 0.011; 0.011 | hub 1.494; rim 0.013 |
| 9 | tapered ball | wall 4.626; end plane 0.484 | 0.012; 0.012 | hub 0.500; rim 0.105 to 0.107 |
| 13 | flat 3 mm | wall 6.936; rim at X = 60 (face 16) 2.084 | 0.011; 0.011 | hub 0.295; rim 0.014 |
| 13 | flat 6 mm | wall 11.139; end plane 0.728; face 16 1.741 | 0.011; 0.011 | hub 0.579; rim 0.025 |
| 13 | ball 3 mm | wall 6.936; end plane 0.825; face 16 1.385 | 0.010; 0.010 | hub 1.494; rim 0.014 |
| 13 | tapered ball | wall 4.628; end plane 1.649; face 16 0.834 | 0.011; 0.011 | hub 0.500; rim 0.107 |

- **Before and after.** Before, the walls were cut 0.25 to 3.00 mm. After, the checker finds them cut at most 0.016 mm (blade 7, tapered ball), against the tolerance of 0.01 mm. The 0.006 mm beyond it is the sampling of a guarded face and of the moves between poses (OI-636, OI-2078). The material left on the blades and the gate's cut into them are unchanged.
- **Face 16.** On blade 13, face 16 is the piece of the rim cylinder beside the X = 60 end. It meets the end ruling and no middle ruling, so it is an end face. The pass stops 0.8 to 2.1 mm short of it, where the tool would have cut it 0.013 to 0.064 mm by the blade's own twist (OI-2079).
- **The hub.** The hub cut is the same as decision 0491 measured with no set (0.03 to 1.50 mm), now reported as a warning. Putting the hub in an avoid set lifts the tool clear of it, as there.

After the merge the owner checks the bladed disk in the application: a swarf operation on the blades with no sets. The passes should stop short of the disc wall, the notes should say how much, and the hub should be warned of with a ball. Then the same with Keep off the end faces off.

## Open items

- **OI-2078** (new): the end faces are kept off, and the faces next to the cut face measured, to the sampling of a guarded face (an eighth of the tool radius, at least 0.05 mm) and three points of each move. On the blisk that leaves up to 0.016 mm with the tolerance at 0.01 mm. A bounded check (the test kit's checker, decision 0255) or a finer sampling near the tool would close the gap; this is the same limit as OI-636 for avoid and check sets.
- **OI-2079** (new): a face that meets the end ruling and no middle ruling is an end face, however little the tool cuts it. On the blisk, the piece of the rim at X = 60 (face 16) stops the pass 0.8 to 2.1 mm short, to keep off a 0.013 to 0.064 mm cut made by the blade's own twist. A depth below which an end face only warns is for the owner, if the strip matters.
- **OI-2080** (new): only faces that share an edge with the cut face are found. A face that comes near the cut face's end without sharing an edge with it (the next blade, or a separate body) is not checked. The whole-model check stays OI-237.
- **OI-2081** (new, issue #572): the entry and exit moves (lead, ramp, helix) are still checked against the face set only, in the shared entry code: a lead-out at a pass that ends at a wall runs on into it (3.000 mm on the library's fixture; not reached on the blisk, where those entries are refused on the blade). Present before this record and not changed by it; a follow-up.
- **OI-237** (narrowed, stays open): the shank above the flute, the holder and the faces that do not share an edge with the cut face.
- **OI-636** (unchanged): how finely a set is sampled.

# 0491: Swarf lifts the tool's tip off an avoid or check set, and takes a negative tip extension (OP-02, OP-07, OP-11, OP-12, GC-07, NFR-06, NFR-07, NFR-12; issue #435; lane SWARF-FLOOR)

- **Status:** accepted (1 Nov 2026, by the project owner, relayed by the coordinator; decision 0049), including the whole-face reading of item 2 and the effect on existing swarf operations with avoid or check sets (item 1). Proposed by lane SWARF-FLOOR on 1 Nov 2026. The owner's decisions, relayed by the coordinator on 1 Nov 2026: OI-1620 option A (item 1), OI-1621 option A (item 2), OI-1622 option C (item 3, tracked as issue #442).
- **What it amends:**
  - **accepted [decision 0176](0176-swarf-boundary-avoid-and-check-sets.md) item 4:** Cut short left out every tool position that broke an avoid or check set, so a floor under the whole face left nothing to cut and the face was refused. On swarf, Cut short now lifts the tip first (item 1).
  - **accepted [decision 0061](0061-slice-17-swarf.md) item 3:** the tip sat `tip_extension` beyond the end of the ruling, with a minimum of zero. A negative value is now allowed (item 2).

  OP-02, OP-07 and OP-12 and the requirements are unchanged: OP-02's avoid surface is "never cut; the tool may pass over it" ([decision 0032](0032-slice-10-mesh-selection.md)), which a lift does.
- **Date:** 1 Nov 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-1620, OI-1621 and OI-1622 are new and closed by the owner's decisions; OI-1623 is new and closed by item 1's changes to the guard. OI-237 and OI-636 stay open.

Numbering: written as 0450, from a range the coordinator gave the lane; renumbered 0491 at the release merge, the next number after the highest on `main` (0490), under the owner's rule of 1 Nov 2026 that decision numbers run on with no reserved ranges. OI-1620 to OI-1623 keep their numbers. No project schema number.

## Context

Issue #435: a swarf operation on the blades of a bladed disk cut into the floor below the blades (the hub), with no setting to stop it. The owner put the floor in an avoid set and generation failed, where the owner expected the tool to be lifted along the path to keep off the set. The owner also asked for a negative tip extension, to pull the tool up the face, which the operation refused.

Records read: 0021, 0061, 0111, 0175, 0176 and 0272 (swarf), 0032 (the roles of a set), 0043 and 0229 (rotary roughing's check set lifts the tool), 0048 (API changes before 1.0).

### What was measured (the repository's blisk, `testdata/step/valid/blisk.step`)

The hub is five cones (faces 8, 12, 17, 20 and 23), 15 mm in radius at X = 20 and 9 mm at X = 60. Swarf ran on blades 7, 9 and 13 at a tolerance of 0.01 mm, and the written motion was measured against every other face with the test kit's own tool outline (`openchuck_testkit::gouge::Outline`), since the gate measures the cut face only (OI-237). The example `crates/openchuck-app/examples/swarf_floor.rs` repeats it.

Cut into the hub with a tip extension of 0:

| Tool | Cut into the hub, mm |
| --- | --- |
| Flat end mill, 3 mm | 0.03 to 0.30 |
| Flat end mill, 6 mm | 0.05 to 0.59 |
| Ball end mill, 3 mm | 1.19 to 1.50 |
| Tapered ball, 0.5 mm tip, 3 degrees, 3 mm | 0.40 to 0.50 |

It comes from three places:

1. **The corner.** The flank starts at the end of each ruling, so the tip of a ball, bull nose or tapered ball sits its corner radius below the end of the rulings. On the library's flat wall and floor, a 1 mm corner cuts 1.00 mm into the floor and a 3 mm ball 3 mm.
2. **The end of the tool is square to the ruling.** Where the floor is not square to the blade (a cone, or a leaning or twisting blade), the end dips into it: a flat end mill on a wall leaning 10 degrees cuts 0.88 mm into the floor.
3. **The four-axis tilt** adds to both.

With the hub as an avoid set, the ball and the tapered ball were refused ("the tool would cut into the avoid surfaces wherever it cuts it, so nothing of it is left to cut"): this is the owner's error. The flat end mills kept only pieces: 9 to 53 mm along each blade was not cut. A negative tip extension (tried with the parameter's minimum bypassed) cleared the hub at -0.5 mm (3 mm flat), -1.0 mm (6 mm flat), -0.5 to -1.0 mm (tapered ball) and below -1.5 mm (3 mm ball), but the strip it left counted as material left and would have refused the face.

## Decision

1. **On swarf, Cut short lifts the tip first (OI-1620, option A, the owner's choice).**
   - Where the tool on a ruling is not kept off an avoid or check set on Cut short, the tip is lifted up the ruling, which is shortened at the tip end, by the least that keeps the tool off the set. A check set keeps the stock to leave clear, as before. The least lift is found by bisection (20 steps) at each pose, so it follows a floor that changes along the face.
   - "Kept off" allows for the set's sampling: at each sample the tool's clearance is at least the tolerance beyond the limit at which a position breaks the set (decision 0176 item 4), plus what the surface may come nearer between the samples. That is the spacing times the sine of the angle between the surface and the tool's end, plus, for a rounded end, the spacing squared over eight times its radius. So a flat end that only meets a flat floor is not lifted, and the positions, still tested against the set as before, stay within its tolerance between poses.
   - **The set's samples.** Besides the grid of its parameters (decision 0176 item 6), each face of the set is sampled where each grid line meets its trimmed edge, found by bisection: a floor is often highest at its edge (the blisk's hub at X = 20), and the grid alone missed it by up to a spacing. A sample is tested against a position when it is within half the ruling, the tool radius, the tip extension, the stock to leave and a millimetre of the contact, plus a further tool radius: the contact is on the near side of the tool and its far side is two radii from it, which on a short ruling (4.5 mm at the blade's front end) left floor under the far side unchecked. Both apply to Warn only and to a pass cut short too.
   - At least a quarter of each ruling stays in contact. A position no lift within that keeps off the set, such as a wall across the pass that the side of the tool meets, is cut short as before (decision 0176 item 4).
   - The strip left at the foot of the face is left on purpose: the gate measures the face above the lifted tip only (between two poses, above the larger of their lifts), so it is neither material left nor a reason to refuse. Each face's note says it: "The tool is lifted up to 1.200 mm along the rulings to keep clear of the avoid surfaces, so a strip up to about 1.200 mm wide is left at that end of the face." The note names avoid surfaces, check surfaces, or both, as they caused the lift.
   - Warn only and Ignore are unchanged. No new choice and no project schema change. In the operation inspector's Geometry section, swarf's Cut short tooltip says that the tool is lifted first, that a strip is left, and that a wall at an end of the face can be kept clear by putting it in an avoid set (item 3).
   - **Saved projects.** Every swarf operation with an avoid or check set may be made differently after the update, on Cut short and on Warn only alike: lifted where it was cut short or refused before, and measured against the set's trimmed edges and the far side of the tool, which the guard missed before (OI-1623). A swarf operation with no avoid or check set is made exactly as before. The changelog says so. The same project still gives the same G-code within a version (GC-07). The repository's swarf golden programs and reference job have no such set and are unchanged (Consequences).
2. **A negative tip extension (OI-1621, option A, the owner's choice).**
   - **Tip extension** has no minimum. A negative value starts the flank that far up the rulings: the rulings are shortened at the tip end, along the ruling, and the tool is placed on what is left with no extension.
   - A ruling no longer than the value is not cut, and the pass stops where such rulings start, as a ruling the boundary's limit B leaves nothing of is not cut (decision 0176 item 2). The note says how much: "About 5.426 mm along the face is not cut because its rulings there are no longer than the negative tip extension." A trimmed face has short rulings at its corners: the blisk's blades have one of 0.016 mm, so refusing a face for any such ruling would refuse every blade for any negative value.
   - A face with no ruling longer than the value is refused, naming the face, the value and the length: "swarf cannot cut face 9: a Tip extension of -9 mm lifts the tool off the whole of the face, whose longest ruling is 8.400 mm long: use a value above -8.400 mm" (NFR-07).
   - **The reading.** The owner's approved wording is that a value which "lifts the tool off the whole of face 9" is refused. It is read as the whole face, not any one ruling: rulings no longer than the value are left uncut and said, and the face is refused only when no ruling is longer. The coordinator accepted this reading on 1 Nov 2026, and the owner approved it with this record.
   - The strip it leaves is left on purpose: it is not counted against Most material left. The note says: "The tip sits 1.000 mm up the rulings (a negative tip extension), so a strip about 1.000 mm wide is left at that end of the face."
   - **Each face's note says where the tool's end is** against the end of the rulings, before any lift: "The tool's end reaches 3.000 mm beyond the end of the rulings at the tip (its corner radius and the tip extension), so it cuts into any face that meets them there, such as a floor, unless that face is in an avoid or check set." Or "is level with the end of the rulings at the tip", or "stays 1.000 mm short of the end of the rulings at the tip".
   - The tooltip of Tip extension says that a ball or corner radius reaches below the end of the rulings by that radius, that a negative value pulls the tool up the face and leaves a strip that wide, and that it must be above minus the length of the rulings.
   - Tip extension and a lift add: the lift is measured on the rulings the extension leaves. Limit B of a boundary (decision 0176 item 2) is applied first, as before.
3. **The tool running past the ends of the face (OI-1622, option C, the owner's choice).** At each end of a pass the tool reaches past the end of the face into the faces beside it, by up to the tool radius: on the blisk, the disc's wall at X = 20 and the end at X = 60, 0.28 to 1.50 mm with the 3 mm flat end mill and 0.57 to 3.00 mm with the 6 mm. It is not part of this lane: it is tracked as issue #442. Meanwhile, a wall at an end of the face can be kept clear by putting it in an avoid set on Cut short, which cuts the pass short there (the tooltip says so).
4. **What is not claimed (NFR-12).** The lift keeps the tool off a set only where the set is chosen, and only to the tolerance of its sampling and of the positions tested between poses (OI-636). Nothing in the notes or tooltips says the floor is safe or uncut. A floor that is not in an avoid or check set is still cut into, and the note says by how much the tool's end reaches below the rulings. The user checks the path and the simulation before running it.
5. **Deterministic (GC-07, NFR-06).** The lift is found by the same bisection in a fixed order for each pose, and the lifts are made to agree in passes over the run in order. Only the gate and the guard tests run in parallel, through `crate::parallel`, in order, as before.
6. **Library API (decision 0048).**
   - `swarf::Pose` gains `lift`; `swarf::Limits` gains `tip_lifted`, `lifted_clear_of`, `tip_up`, `too_short` and `end_reach`.
   - The `tip_extension` parameter has no minimum.
   - These break the library API, and the output changes (item 1), so the workspace goes to 0.33.0 with changelog entries, set from `main` at the release merge (lane CRANKEXCESS took 0.32.0 first; the coordinator sequences the versions).
   - The application's `SetOptionsShown` gains `cut_short_lifts_tip`, true for swarf.

## Alternatives considered

- **For item 1: a fourth choice "Lift clear" beside Cut short (option B):** needs project schema 26 and an inspector change, and the default would still refuse a bladed disk with a ball. **A separate floor role (option C):** a fourth role for one strategy, and schema 26, for what avoid already means. **Lifting clear of the whole part with no set (option D):** needs every face of the model tested at every pose (OI-237); not for this lane.
- **For item 2: keep the minimum at zero (option B):** the owner asked for the negative value, and it works without choosing the floor faces. **A minimum of minus the corner radius (option C):** clears only the corner, not a leaning blade or a cone.
- **Lifting to the tolerance's limit, not off the set:** tried first. The samples of the set lie differently under each position, so the motion between two poses lifted exactly to the limit broke it by a few microns, and was cut short.
- **Making the lifts of neighbouring poses agree** (raising the lower to the higher where the move between them broke the set): built while the lift was still aimed at the limit, then measured with each mechanism off. With the set's trimmed edges sampled and the sampling allowed for, it changed nothing on the blisk (0.000 mm with and without it) and no test reached it, so it was taken out. Without the edge samples, the hub was cut up to 0.100 mm on blade 9, so they stay.

## Consequences

- **Tests,** named after OP-02, OP-07, OP-12, NFR-06, NFR-07 and NFR-12:
  - `crates/openchuck-path/tests/swarf.rs`:
    - a ball with the floor in an avoid set is lifted by its radius, not refused, cuts the floor no more than the tolerance by the independent checker, cuts the whole pass, and says so;
    - the lift follows a floor that rises along the pass;
    - a check set lifts the tool the stock to leave clear;
    - a wall the side of the tool meets still cuts the pass short, with no lift;
    - a tip extension into an avoid floor is lifted back (the test that it was refused is replaced);
    - a negative tip extension pulls the tool up and leaves the strip without a refusal, and clears the floor with a ball;
    - a tip extension that lifts the tool off the whole face is refused, naming the face, value and length, and rulings no longer than it are left uncut and said;
    - the far side of the tool is kept off a floor rising away from a short wall;
    - a ball under a floor in both an avoid and a check set is lifted clear of both, and the note names both;
    - the note says where the tool's end is;
    - a lifted path is the same at 1, 2 and 4 threads.
  - `crates/openchuck-path/src/swarf.rs`: an avoid face is sampled along its trimmed edge (it fails without the edge samples).
  - `crates/openchuck-app/src/operation.rs`: a STEP part's wall and floor, with the floor in an avoid set, is lifted, and a negative tip extension goes through the application.
  - `crates/openchuck-app/tests/operation.rs` and `apps/openchuck-ui/src/shell/inspector.rs`: swarf's Cut short tooltip says the tip is lifted first, for swarf only.
- **Project format:** unchanged (schema 25).
- **Interface:** two tooltips change (Cut short on swarf's sets, and Tip extension). No layout change.
- **OI-237** (the gate measures only the cut face) and **OI-636** (how finely a set is sampled) stay open; issue #442 follows item 3.

## Verification

Measured after the change with the example `crates/openchuck-app/examples/swarf_floor.rs` on the blisk, blades 7, 9 and 13, the hub in an avoid set on Cut short, tolerance 0.01 mm, measured by the test kit's outline:

| Tool | Tip extension 0, no set: cut into the hub, mm | Tip extension 0, hub in an avoid set: most lift, cut into the hub, length not cut | Tip extension -1, no set: cut into the hub, length not cut (rulings of 1 mm or less) |
| --- | --- | --- | --- |
| Flat end mill, 3 mm | 0.027 to 0.299 | 0.069 to 0.388 mm; 0.000 mm; none | 0.000 mm; 0 to 1.92 mm |
| Flat end mill, 6 mm | 0.046 to 0.587 | 0.132 to 0.759 mm; 0.000 mm; none | 0.000 mm; 0 to 1.92 mm |
| Ball end mill, 3 mm | 1.190 to 1.499 | 1.266 to 1.629 mm; 0.000 mm; none | 0.217 to 0.572 mm; 0 to 1.92 mm |
| Tapered ball, 0.5 mm tip, 3 degrees, 3 mm | 0.395 to 0.502 | 0.443 to 0.589 mm; 0.000 mm; none | 0.000 mm; 0 to 1.92 mm |

Nothing is refused. The material left on the blades is unchanged (at most 0.088 mm, the tapered ball's own on blade 7, before and after). A uniform negative tip extension must suit the worst place on the blade, so -1 mm is not enough for the 3 mm ball, whose lift with the hub in an avoid set is 1.27 to 1.63 mm; the 1.92 mm not cut on blades 9 and 13 is the corner of the face where its rulings are 1 mm long or less.

The swarf reference job (`testdata/projects/swarf`) has no avoid or check set: its golden programs are unchanged (`crates/openchuck-app/tests/step_pipeline.rs`, 19 of 19 kernel tests pass). After the merge the owner checks the bladed disk in the application: the hub in an avoid set on Cut short, and Tip extension negative, with a ball or tapered ball.

## Open items

- **OI-1620** (closed): how an avoid or check set keeps the tip off it on swarf. The owner's option A, item 1.
- **OI-1621** (closed): a negative tip extension. The owner's option A, item 2.
- **OI-1622** (closed): the tool running past the ends of the face. The owner's option C: issue #442, item 3.
- **OI-1623** (new, closed): with the floor in an avoid set on Cut short before this change, the pieces of the pass that were kept still cut 0.04 to 0.15 mm into the hub at its edge at X = 20 (flat end mills on blades 7 and 9), above the tolerance. The causes were the guard's samples missing the trimmed edge and its reach missing the far side of the tool on short rulings; item 1 samples the edge, reaches the far side and allows for the sampling, and the blisk's hub is now cut 0.000 mm (the table above). What sampling cannot see stays OI-636.

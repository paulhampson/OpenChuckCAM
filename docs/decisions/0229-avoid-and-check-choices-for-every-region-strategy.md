# 0229: Cut short, Warn only and Ignore for the avoid and check sets of every strategy that takes a region (lane SWARF3; closes OI-637)

- **Status:** accepted by the project owner on 23 Oct 2026 ("accept 0229"), relayed through the coordinator at the owner's instruction (decision 0049). Proposed 23 Oct 2026 by lane SWARF3. The owner chose option A on OI-637, relayed by the coordinator on 23 Oct 2026: avoid and check choices on rotary roughing and helical, axial, indexed and simultaneous finishing, and no boundary options for them. On 23 Oct 2026 the owner chose to document OI-970 and defer reliable detection, relayed by the coordinator: keep the advisory approximation, show its limitation whenever Warn only is selected, and keep OI-970 open. The acceptance above was given separately after the owner reviewed the full record.
- **What it amends:**
  - **[decision 0176](0176-swarf-boundary-avoid-and-check-sets.md) item 3** (accepted). Its avoid and check choices were swarf's only, and another strategy kept the options without using them. Now every strategy that takes a region uses an avoid or check set's choice. The boundary's two limits stay swarf's.
  - **Decision 0176 item 6.** Another strategy no longer refuses a region that watches a surface: the shared generation measures the watched surfaces for it. A boundary limit turned off is still refused for it.
  - **[decision 0210](0210-step-face-sets-for-region-strategies.md) item 3** (accepted): "set options stay swarf's" now holds for the boundary's options only.
  - **The interface specification, section 8** (accepted with [decision 0038](0038-ui-specification-and-requirements.md)): the Geometry section shows each avoid or check set's choice for every strategy that takes a region. The line is edited in this change.

  OP-02 and the requirements are unchanged. The project file is unchanged: schema 14's `[avoid_options]` and `[check_options]` tables already hold a choice for any operation (decision 0176 item 3).
- **Date:** 23 Oct 2026.
- **Open items** ([docs/open-items.md](../open-items.md)): closes OI-637 and opens OI-970.

Numbering: the coordinator gave lane SWARF3 records 0228 and 0229 and OI-970 to OI-979; record 0228 says why they were free on 23 Oct 2026. No project schema number: the file does not change.

## Context

Decision 0176 gave each avoid or check set of a swarf operation one choice of three: Cut short (the default), Warn only or Ignore. Its boundary set got two tick boxes, Limits the pass and Limits the tip. OI-637 asked whether the other strategies that take these sets should take the options too, and what each would mean for them.

Five strategies other than swarf take boundary, avoid and check sets: rotary roughing and helical, axial, indexed and simultaneous finishing. They take them on a mesh part (decision 0032) and as face sets on a STEP part (decision 0210). What their sets do today:

- **Helical, axial, indexed and simultaneous finishing.** A tool position is not cut where its contact lies on an avoid set, or where the tool touches a check set (`RegionConstraint::allows`). The pass stops there and starts again after it. This is already swarf's Cut short.
- **Rotary roughing.** An avoid set is not cut, as above. A check set keeps the tool above it, by the stock to leave, rather than stopping the pass ([decision 0043](0043-slice-14-roughing-containment.md) item 3).
- **A boundary** on all five is a window of X and angle, and the contact must lie inside it. Swarf's two limits depend on swarf cutting a whole ruling with the side of the tool. These strategies cut at one contact point, so neither limit has anything separate to act on.

Records read: 0032, 0043, 0048, 0049, 0055, 0114, 0175, 0176, 0209, 0210, 0228. Requirements: OP-02, OP-03 to OP-06, OP-23, UI-05, UI-07, UI-09, INT-04, NFR-06, GC-07, NFR-07, NFR-12.

## Decision

1. **Each avoid and check set takes one choice of three on all five strategies (the owner's option A).** The choice is shown beside the set in the operation inspector's Geometry section, as for swarf.
   - **Cut short** (the default) is the behaviour above, unchanged. An operation whose sets are all on Cut short gives the same G-code as before (GC-07). For a check set of rotary roughing, the choice's tooltip says that the tool is lifted to stay the stock to leave above the set, and the material over it is still roughed.
   - **Warn only.** The path is made as if the set were not there. Then each move between two cut points of each pass is followed as the machine makes it (straight in the machine's axes) and sampled with a nominal spacing of an eighth of the tool's cutting radius (at least 0.01 mm). An eight-chord estimate of the move's length chooses the sample count, capped at 100,000 intervals per move; this is not a guaranteed maximum spacing or a complete intersection test. At each sample the tool, grown by the operation's stock to leave, is dropped along its axis onto the set alone. The tool breaks the set there when:
     - it rests on the set at its tip or above it: the tool, where it is, touches or reaches into the set; or
     - the tool rides on the part (it rests on the part within the toolpath tolerance of where it is) and the set is where it rests. This is the test Cut short makes at a cut point, made between the cut points too, where a straight move lies up to the tolerance off the part.

     A piece between two samples counts whole when both break the set and half when one does. A warning (decision 0175) names each set broken and the length, in swarf's words, and is said first in the operation's notes:

     > The tool cuts into the avoid set 'Rib' along about 3.000 mm of the toolpath. The set only warns, so the path is kept: set it to cut the pass short, or check the cut in the simulation before using the path.

     A check set is "touched" with no stock to leave, and "comes closer than the stock to leave to" with some. Roughing's upper layers ride on no part, so they break only a set they reach into, not one that lies under them. A set with no detected break gives no intersection warning; a narrow set can be crossed between samples without being detected (OI-970).
   - **Ignore.** The set is checked as any set is (the model, the version, its contents), then left out of the path. The warning that it limits nothing names it, in decision 0176's words: "The avoid set 'Rib' limits nothing, because it is set to Ignore: the tool is not kept clear of it. ..."

   The detected-break warning for Warn only, the limits-nothing warning for Ignore, and both choices' tooltips say plainly that the tool is not kept clear of the set (NFR-12). For these five strategies, an amber line below each set on Warn only is always shown, before generation and whether or not a break is detected: "Warn only samples the path and can miss a narrow set. No warning does not establish clearance; verify the cut before running G-code." Swarf keeps its existing options and measurement; OI-636 tracks its separate sampling limitation.
2. **The boundary takes no options on these strategies.** The Geometry section shows none under their boundary set. Boundary options stored on such an operation (for example from when it was a swarf operation) are kept but not used, as before. The library still refuses a boundary limit turned off for a strategy other than swarf, now saying "takes no options on its boundary set". A tick box "Limits the cut" (option B) was not chosen: turning it off would do no more than taking the set out of the operation.
3. **Where it is built.**
   - **Library (`openchuck-path`).** The shared generation (`generate_in_region`, `generate_in_zones`) measures the watched surfaces of `RegionConstraint::watch` for any strategy that does not honour restriction options, after its passes are made (`region::watched_warnings`). So the five strategies take Warn only without a change of their own, and so will any later strategy that honours a region. The measurement checks the job context between surface queries so cancellation stops this phase too (UI-09, INT-04; architecture section 5.1). Swarf measures its own, as decision 0176 says. No public item changes, so the library's version is unchanged ([decision 0048](0048-pre-1-0-api-changes.md)).
   - **Application (`openchuck-app`).** `selection::resolve`, which builds the region of these strategies on a mesh part and on a STEP part, reads each avoid and check set's choice: the Cut short sets of a role are one surface, as before; each Warn only set is a watched surface with its name; each Ignore set gives the warning. `Application::strategy_set_options` (with a new `SetOptionsShown`) says which options the Geometry section shows, and replaces `strategy_takes_set_options`.
4. **Deterministic.** The moves are measured through `crate::parallel::map_in_order` and summed in their order, and the warnings come in the order of the sets. The same project gives the same path and the same warnings at any thread count (GC-07, NFR-06).

## Alternatives considered

- **Option B: A, plus a "Limits the cut" tick box for the boundary.** Not chosen (item 2).
- **Option C: no options on the other strategies.** Not chosen: the owner wanted the same three choices wherever avoid and check sets are taken.
- **Option D: A on the four finishing strategies only, since roughing's check set lifts the tool.** Not chosen: Warn only and Ignore mean the same for roughing, and Cut short's tooltip says what roughing does.
- **Measuring Warn only inside each strategy, at its own cut points.** That would give the exact test of each strategy, but would mean five changes, and the cut points are thinned by the chord limit, so a long move across a set could hold no cut point on it. Measuring the moves in the shared generation covers what is cut between the points too.
- **Warn only measured with the toolpath tolerance as slack everywhere.** Tried first. It counted the tool as touching a set up to about 0.35 mm before its edge (a 6 mm ball at 0.01 mm), 12 % too long on the test cylinder. The second test of item 1 keeps the tolerance only for the question "does the tool ride on the part".

## Consequences

- **Tests**, named after OP-02 (and NFR-06):
  - `crates/openchuck-path/tests/region.rs`: Warn only keeps helical finishing's path, names the avoid or check set and gives the length within 2 % of the strip's; a set the tool never reaches says nothing; a check set with stock to leave says so; the warning is the same at 1, 2 and 8 threads; cancellation during a watched-surface query stops further queries on one worker; axial, indexed and simultaneous finishing take Warn only; a boundary limit turned off is still refused.
  - `crates/openchuck-path/tests/roughing_region.rs`: Warn only keeps roughing's path in both forms, does not lift the tool over a check set, and warns.
  - `crates/openchuck-app/tests/selection.rs`: a helical operation's avoid and check sets on Cut short, Warn only and Ignore.
  - `apps/openchuck-ui/src/shell/frame_tests.rs`: the Geometry section shows the avoid and check choices and no boundary options for helical finishing, and both for swarf; both Avoid and Check explain the sampling limitation whenever Warn only is selected, without generation or a detected hit, and remove it when another choice is selected.
- **Library API.** Unchanged.
- **Project format.** Unchanged (schema 14).
- **Interface.** The choices show for every strategy that takes a region (section 8 edited); Warn only's tooltip no longer speaks of a face.

## Open items

- **OI-637** (closed): the owner's option A, items 1 and 2.
- **OI-970** (open, reliable detection deferred by the owner's choice on 23 Oct 2026, relayed by the coordinator): Warn only uses the nominal sampling described in item 1 and can miss a narrow set entirely. Bisection at a detected change can refine an edge but cannot discover a crossing between two non-hit samples. Reliable detection needs a separate design, such as conservative geometric bounds on the swept tool, and verification of narrow crossings. The always-visible explanation and its regression document the current limitation; they do not close this item or establish clearance.

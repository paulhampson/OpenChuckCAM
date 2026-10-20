# 0163: Proposed requirement SIM-15: choose which deviation bands the result views show (lane BUG-RESULTS; issue 216)

- **Status:** proposed (lane BUG-RESULTS, 20 Oct 2026). A requirement proposal for the owner, sent through the coordinator. `docs/requirements.md` and `docs/verification-plan.md` are not edited, and nothing is built until the owner agrees the wording.
- **Date:** 20 Oct 2026.
- **Requirements:** proposes SIM-15 (P2); relates to SIM-05, SIM-11, SIM-14, NFR-12.
- **Open items:** OI-565 (new).

## Context

Issue 216 (trial feedback F-60, owner's request): the user chooses the visibility of undercut and overcut by band, so the part shows only the deviation in the chosen ranges. The owner's answer: a band could be both the histogram's bins and ranges the user sets, to be decided when the requirement is agreed. Not yet answered: whether the choice is remembered, and the wording. The histogram's bins are defined by [decision 0162](0162-simulation-results-histogram-and-probe.md) (proposed): the surface band and the colour thresholds 0.25, 0.5 and 1.0 mm, mirrored for the overcut.

SIM-14 is the last simulation requirement, and no unmerged branch takes SIM-15, so SIM-15 is the next free ID in the group.

## Proposal

**Requirement (simulation group, P2):**

| ID | Requirement | Priority |
| --- | --- | --- |
| SIM-15 | Choose which deviation bands the result views show, as the histogram's bins and as a range the user sets, so the part shows only the overcut or the material left of interest; the colour scale, the legend and the histogram follow the choice, and the worst values and counts stay those of the whole part. | P2 |

**Verification plan row:**

| ID | Priority | Method | Level | Pass criteria |
| --- | --- | --- | --- | --- |
| SIM-15 | P2 | T | System | On analytic cases, with a bin or a range switched off, exactly the triangles whose values lie in it keep the part's colour, and the histogram, legend and colour scale mark it hidden; a user-set range shows exactly the triangles within it; the worst values, the share within tolerance and the overcut count are unchanged by any choice |

## Options for what a band is (the owner chooses)

- **A. The histogram's bins only.** A tick box on each histogram row. Simplest; the bins are fixed by the colour thresholds, so "only the overcut deeper than 0.5 mm" is two ticks.
- **B. A range the user sets only.** "Show from … to … mm" fields, so any limit can be set, for example only the overcut beyond 0.2 mm.
- **C. Both (recommended).** The tick boxes of A, and one range of B that narrows them further, which is what the owner's answer describes. The requirement text above is written for C; for A or B, drop the other half.

Recommended for all options:
- The choice is per session, not saved in the project. That needs no schema claim. Saving it with the project would need one; the owner decides.
- The views say when a band is hidden ("Some bands are hidden"), so a hidden overcut is never mistaken for none (NFR-12).
- Wording: "Show" for the tick boxes, "Show only from … to … mm" for the range (lane X7b, decision 0114).

## Alternatives considered

Options A, B and C above. Fixed steps (for example every 0.1 mm) were also considered and left out: the histogram's bins already follow the colour thresholds, and a set range covers any other limit.

## Consequences

When the owner agrees, a lane adds SIM-15 to `docs/requirements.md` and the row to `docs/verification-plan.md`, with a specification 11.5 entry, and builds it in `openchuck-ui` (the band filter that `ColourBy` already applies in `playback_panel.rs`). Until then, OI-565 stays open.

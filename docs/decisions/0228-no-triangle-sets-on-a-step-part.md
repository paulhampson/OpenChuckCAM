# 0228: Avoid and check sets on a STEP part stay face sets (lane SWARF3; closes OI-634)

- **Status:** proposed (23 Oct 2026, lane SWARF3). The owner chose option A on OI-634, relayed by the coordinator on 23 Oct 2026: close OI-634 and build nothing. Only the owner accepts this record (decision 0049).
- **What it amends:** nothing. It confirms [decision 0176](0176-swarf-boundary-avoid-and-check-sets.md) item 5 (the owner's 3b: face sets only) and rule R5 of [decision 0209](0209-step-geometry-for-every-strategy.md) (a triangle set cannot be used on a STEP part). OP-02, OP-07 and OP-23 are unchanged.
- **Date:** 23 Oct 2026.
- **Open items** ([docs/open-items.md](../open-items.md)): closes OI-634.

Numbering: the coordinator gave lane SWARF3 records 0228 and 0229 and OI-970 to OI-979. On 23 Oct 2026 the highest record on `main` was 0219, the unmerged branches held 0221, 0222, 0227 and 0230 to 0232, and the highest `OI-` number in use was OI-982 (lane OPSPEED), with none from OI-970 to OI-979. So these were free.

## Context

Decision 0176 item 5 made swarf's avoid and check sets on a STEP part face sets only. It left open whether a set of triangles of the part's tessellation could be one too, for example to protect part of one large face (OI-634).

Swarf runs only on a STEP part: on an STL part it is disabled (decision 0174). So for swarf, a "mesh" avoid or check set can only be a set of a STEP part's triangles. Two things stand in the way:

- Triangles are picked only on a mesh (decision 0111 item 2).
- Decision 0209 rule R5 refuses a triangle set on a STEP part: "a STEP model is selected by face and edge, not by triangle, because its triangles change with the tessellation tolerance".

Records read: 0032, 0066, 0111, 0174, 0176, 0209, 0210. Requirements: OP-02, OP-07, OP-23, PRJ-06, NFR-07.

## Decision

1. **Face sets only.** An avoid or check set on a STEP part stays a face set, for swarf and for every other strategy. Triangle picking on a STEP part's tessellation is not built. The reasons:
   - The triangles of a STEP part change whenever its tessellation tolerance changes (OI-831). A set of them would quietly cover a different area, or name triangles that no longer exist.
   - Swarf measures the tool against the exact faces (decision 0176 item 4). A triangle set would be accurate only to the tessellation (0.01 mm by default).
2. **Protecting part of a large face.** The user splits the face in their CAD model, so the part to protect is a face of its own, and makes a face set of it.
3. **Nothing changes in the code, the project format or the interface.** The refusal of a triangle set on a STEP part (R5) stays as it is.

## Alternatives considered

- **Triangle picking on a STEP part, for avoid and check sets on swarf only (option B).** The set would keep the tessellation tolerance it was made at and be marked in error when the tolerance changed. Swarf would measure against the triangles, not the exact faces. This would have amended R5 and is a large build (picking, the set check, the operation build, the wording). Not chosen.
- **Leaving OI-634 open with no build (option C).** Not chosen: the owner closed it.

## Consequences

- OI-634 is closed. No tests, no API change and no schema number.

## Open items

- **OI-634** (closed): the owner's option A, item 1.

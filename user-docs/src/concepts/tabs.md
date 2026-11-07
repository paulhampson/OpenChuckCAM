# Tabs

When a rotary job cuts the whole length of a part, the part can come free of the stock before the job ends, and a loose part turning in the machine can be thrown. A [tab](../glossary.md#tab) is a short piece of material left between the part and the stock, so the part stays joined until you cut it away by hand, after the job. [Stock and the held region](stock-and-held-region.md) explains the held region at the chuck, which the tabs sit beside.

> [!WARNING]
> A tab that is too thin, or a part that is not held at both ends, can let the part break free while the stock turns. Check every tab against your material and your cutting forces. The program checks the tabs' positions and lengths, not whether they are strong enough.

## Where tabs go

Tabs lie in the **head zone** or the **tail zone**: the gap between the part and the held region (or the end of the stock) at each end of the part. The Workholding section of the setup shows each zone, for example "Head zone X -1.000 to 0.000 mm", or says when there is no room for a tab. A tab that is not wholly in a zone is an error, and operations that can reach it cannot be generated.

Every operation leaves the tabs uncut, and the simulation keeps them, so they show as part of the finished surface.

## The shape of a tab

A tab is a **Cylinder** or a **Frustum** (a cone with a flat end), set by:

Start X and Length
: Where it starts along the rotary axis and how long it is.

Diameter
: Its diameter; a frustum has a **Diameter at start X** and a **Diameter at end X**.

Placement
: **On the axis**, one tab on the rotary axis; or **Around the axis**, several tabs evenly spaced round it, set by **Count**, **Start angle** and **Offset from axis**.

A tab must be at least as long along X as the tools that cut beside it are wide; otherwise the tool cannot cut the gap beside it to shape. The program warns when a tab is shorter, with "Tab length (operation ..., tool ...): ...", but makes the path.

## Adding tabs

In the setup's **Workholding** section:

- **Add default tabs** adds one tab at the head and one at the tail: frustums with the small end on the part, sized from the part.
- **Head tab** or **Tail tab** adds a tab that fills the gap at that end of the part, a cylinder a quarter of the stock's diameter across. With **Place automatically** it follows the part, the stock and the held regions when they change. Where the part narrows to a small end, check the tab is not wider than the part there.
- **Add tab** adds one you place by hand.

Edits are a draft until you press **Apply**, which checks them against the part and the tools and stores them in the setup; **Revert** drops them. While a draft is not applied, Messages warns that the toolpaths, the simulation and the G-code still use the stored tabs. **Check against the part and tools** runs the same check without storing anything.

Apply refuses a tab outside the zones, an automatic tab with no room, and tabs that overlap, and marks the row with the reason. It stores a tab that is shorter than the tools are wide, and keeps the warning on its row.

## After the job

The tabs are still there when the program ends: cut through them with a saw or a knife, and finish the marks by hand. Plan for that when you choose their size and place.

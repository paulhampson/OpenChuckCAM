# Helical, axial, indexed and simultaneous finishing

A finishing strategy decides how the cutter travels over the part while it cuts the final surface. On a rotary machine the cutter can follow the part in a spiral, along it, from a few fixed angles, or while the part turns to face it. Each leaves different marks and suits different shapes. [Roughing and finishing](roughing-and-finishing.md) and [How 4-axis rotary machining works](rotary-machining.md) cover the ideas this page builds on; [Operations and strategies](../reference/operations.md) lists every setting.

## Helical: a spiral round the part

The part turns continuously while the cutter moves slowly along it, so the tool traces a spiral, like the thread of a screw. Each turn of the spiral is one [stepover](../glossary.md#stepover) further along.

Good for
: Round, turned shapes such as the tutorial's baluster: one continuous cut with few retracts, and even marks all round.

Watch for
: Faces across the axis, such as a shoulder: the spiral cannot cut them by itself. Helical finishing adds a turn at such faces, and **Face free ends** steps down beside an end of the part with nothing beyond it. Selections and gaps in the part split the spiral into separate cuts.

**Rings** instead of **Spiral** cuts one full turn at a time at a fixed place along the part, then steps along. The marks are then rings rather than a spiral.

## Axial: passes along the part

The cutter moves along the length of the part in a straight pass, the part turns a small step, and the cutter goes back along the next pass. Each step of A is the stepover at the surface.

Good for
: Parts whose marks should run along their length, such as flutes, and shapes that change a lot along X.

Watch for
: The ends of each pass: the cutter turns round there, so there are more links than with a helix. **Direction** `zigzag` cuts in both directions and is quicker; `climb` cuts every pass the same way.

## Indexed: from a few fixed angles

The part is turned to an angle and held still, and the cutter follows the surface along the part from that side, as a 3-axis machine would. Then the part turns to the next angle. This is sometimes called 3+1 machining.

Good for
: Flat faces, pockets and features seen from a few directions, such as the four sides of a square section.

Watch for
: Surfaces between the angles: a surface the tool cannot see from any of the angles is not cut. Use enough angles, or a helical or simultaneous operation for the round parts.

## Simultaneous 4-axis: the part turns while the tool cuts

The part turns while the cutter moves, so the tool meets each point of the surface close to straight on, along the surface normal. This is true 4-axis machining.

Good for
: Curved shapes where the cutting angle has to change along a pass, undercuts and the walls of slots that a straight approach cannot see, and sections that are off the rotary axis, such as the pin of a crank, which it drives round their own centre.

Watch for
: Four axes cannot follow every surface direction: the slope of a surface along the axis cannot be matched by turning A. Where the program cannot follow the part, it says so and leaves that area uncut instead of cutting it badly; the warning lists the areas. Check the path and the simulation.

## Choosing

| If the part is | Start with |
| --- | --- |
| Round and turned, like a spindle or a baluster | Helical |
| Long, with features along it, such as flutes | Axial |
| Square or flat-sided, seen from a few directions | Indexed |
| Curved in ways the others cannot follow, or with off-axis sections | Simultaneous 4-axis |

Many parts use more than one: for example helical finishing for the round body and indexed finishing for a flat. After finishing, [rest machining and pencil clean-up](roughing-and-finishing.md#cleaning-up) take out what a larger tool could not reach.

Whatever you choose, run the [simulation](simulation.md) and look at the deviation colours before you post the program, and check the program before you run it on a machine.

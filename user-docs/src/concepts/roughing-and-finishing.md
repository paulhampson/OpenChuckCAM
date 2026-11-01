# Roughing and finishing

Most jobs are cut in two stages, with different tools and different aims.

## Roughing

[Roughing](../glossary.md#roughing) removes most of the material quickly. It uses a larger, stronger tool and cuts in layers: each layer goes one [stepdown](../glossary.md#stepdown) deeper into the stock. Roughing leaves a little material on the part on purpose, the [stock to leave](../glossary.md#stock-to-leave), so the cutter never touches the final surface while it is cutting hard.

OpenChuckCAM's rotary roughing turns the work as it cuts each layer, so the layers wrap round the part instead of being flat.

## Finishing

[Finishing](../glossary.md#finishing) cuts the final surface. It uses a smaller tool, often with a round end (a ball end mill), and follows the surface of the model closely, taking off only what roughing left. The distance between neighbouring passes, the [stepover](../glossary.md#stepover), decides how smooth the surface is and how long the cut takes.

OpenChuckCAM has several finishing strategies, which move the cutter round the part in different patterns: see [Helical, axial, indexed and simultaneous finishing](finishing-strategies.md).

## Cleaning up

Some places are too tight for the finishing tool, such as inside corners. Two kinds of operation deal with them:

- **Rest machining** cuts only where an earlier, larger tool could not reach, with a smaller tool.
- **Pencil clean-up** runs a small tool along each crease, where two surfaces meet.

## Operations and their order

Each stage is an [operation](../glossary.md#operation) in the program: one tool, one strategy and its settings. Operations run in the order they are listed. The simulation cuts them in that order too, so you can see what each one leaves for the next.

| Stage | Typical tool | Leaves |
| --- | --- | --- |
| Roughing | A larger flat or bull-nose end mill | The stock to leave, as steps |
| Finishing | A smaller ball end mill | The final surface, with small ridges between passes |
| Rest or pencil | A smaller tool still | Corners and creases the others missed |

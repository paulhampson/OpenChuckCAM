# Your first project

This tutorial takes you through one complete job in OpenChuckCAM, from an empty window to G-code and a setup sheet. Every step has a screenshot of the real program. The job is a small turned and fluted piece (a baluster with a ball finial), rough-cut with one tool and finished with another, for a Makera Z1 with a chuck. You work through the four steps along the top of the window: Model, Setup, Operations and Simulate. Writing the G-code is an action on the Simulate step.

> [!WARNING]
> The piece, the tools and every speed and feed in this tutorial are **an example for learning the program, not a tested cutting recipe**. Nothing here has been cut on a machine. Choose your own tools, speeds and feeds for your material and machine.
>
> Generated G-code **must be verified by you before it is run on a machine**. OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup. The last page of this tutorial, [Before you run anything](11-verify.md), says what to check.

OpenChuckCAM is in early development. See the [known limitations](../start/limitations.md).

## What you need

- The program. See [Install and first run](../start/install.md).
- One file from the OpenChuckCAM repository: [`testdata/examples/baluster/baluster.stl`](https://github.com/paulhampson/OpenChuckCAM/tree/main/testdata/examples/baluster) (269 KB). Save it somewhere you will find it again. A finished copy of the project is in `testdata/examples/baluster/baluster-project` in the same folder, if you want to compare or skip ahead. `generate.py` beside the model writes it again.

The screenshots were taken at 1366 x 768 with a software renderer, so colours and line widths on your screen may differ a little. File paths in them are from the machine that took them; yours will be your own. The program is changing, so a label may differ from the one shown here; the steps stay the same.

## Words used in this tutorial

Rotary axis, A axis
: On a 4-axis machine the work is held at one end in a [chuck](../glossary.md#chuck) and turned about a line along its length. That line is the [rotary axis](../glossary.md#rotary-axis); on the Makera Z1 it lies along X, and the turning is called [A](../glossary.md#a-axis). The cutter moves along X and across the work (Y and Z) while A turns the part, so the cutter can reach all the way round.

Stock
: The blank you start with: here a round bar. The program cuts the part out of the [stock](../glossary.md#stock).

Held region
: The length of the stock that the chuck grips. It is never cut, and the toolpaths keep clear of it. (You can also hold the far end, for example with a tailstock; the program only knows that end as a held length, not as a body.) See [held region](../glossary.md#held-region).

Tool library
: The list of cutters. Each tool has a diameter, a shape and cutting data for a material. See [tool library](../glossary.md#tool-library).

Stepover
: How far the cutter moves sideways between neighbouring passes. See [stepover](../glossary.md#stepover).

Stepdown
: How deep each layer of roughing goes. See [stepdown](../glossary.md#stepdown).

Stock to leave
: Material deliberately left on the part by an operation so that a later operation can finish it. See [stock to leave](../glossary.md#stock-to-leave).

Adaptive spacing
: A finishing setting that adds passes where the material left between two neighbouring passes stands higher than you allow. See [adaptive spacing](../glossary.md#adaptive-spacing).

Tolerance
: How closely the toolpath may differ from the model. See [tolerance](../glossary.md#tolerance).

Post (post-processor)
: The part of the program that turns the toolpaths into the G-code dialect a given machine controller reads. The Makera Z1 has its own post. See [post-processor](../glossary.md#post-processor).

Work offset (G54)
: The register in the controller that stores where the part's zero is. Here it is on the rotary axis at the model's X = 0. See [work offset](../glossary.md#work-offset).

## The steps

1. [Create a project](01-create-project.md)
2. [Import the model](02-import-model.md)
3. [Choose the machine](03-machine.md)
4. [Set up the stock](04-stock.md)
5. [Hold the work](05-hold.md)
6. [Choose the tools](06-tools.md)
7. [Add the operations](07-operations.md)
8. [Generate the toolpaths](08-generate.md)
9. [Simulate and read the results](09-simulate.md)
10. [Post and export the G-code and the setup sheet](10-export.md)
11. [Before you run anything: verify the G-code](11-verify.md)

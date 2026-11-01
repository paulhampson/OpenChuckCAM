# Glossary

<div class="glossary">

The words this guide uses for machining and for the program. A page links a term here the first time it uses it.

<p class="letter-index">A · C · F · G · H · M · O · P · R · S · T</p>

## A axis {#a-axis}

The fourth axis of a rotary machine: the turning of the work about the [rotary axis](#rotary-axis). On the Makera Z1 it turns about X, and a positive A move turns the top of the chuck towards the front of the machine.

## Air cut {#air-cut}

A run of a program with no tool fitted and no material in the chuck, at reduced feed, to watch every move before anything can be damaged. See [Air cut on the Makera Z1](safety/air-cut.md).

## CAM {#cam}

Computer-aided manufacturing: software that works out the moves a machine tool must make to cut a part from a model, and writes them as [G-code](#g-code).

## Chuck {#chuck}

The clamp on a rotary machine that holds one end of the [stock](#stock) with its jaws and turns with the [A axis](#a-axis).

## Feed {#feed}

The speed at which the cutter moves through material, in millimetres per minute. A rapid move travels through air at the machine's fastest speed instead.

## Feed override {#feed-override}

A control on the machine that scales every programmed feed, for example to 10 % for a first run.

## Finishing {#finishing}

Cutting the final surface of the part, usually with a smaller tool and close passes. See [Roughing and finishing](concepts/roughing-and-finishing.md).

## G-code {#g-code}

The text program a CNC machine's controller reads: one move or setting per line, such as `G1 X10 A90 F600`. Each controller reads its own variant, which is why the program needs a [post-processor](#post-processor).

## Held region {#held-region}

A length at one end of the [stock](#stock), measured along the axis, that the [chuck](#chuck) or a [tailstock](#tailstock) grips. It is never cut, and toolpaths keep clear of it. The program knows it only as a length, not as a solid body, so it cannot check the chuck itself against the tool. See [Stock and the held region](concepts/stock-and-held-region.md).

## Machine profile {#machine-profile}

The description of a machine the program uses: its axes, travel, the height and direction of the rotary axis, its feed limits and spindle speeds, and its post. Values nobody has measured on your machine are marked unconfirmed.

## Material {#material}

What the stock is made of, such as hardwood or tooling board. The tool library holds cutting data for each material.

## Operation {#operation}

One step of the cutting: one tool, one [strategy](#strategy) and its settings. A job is a list of operations, cut in order.

## Post-processor {#post-processor}

The part of the program that turns toolpaths into the G-code variant a particular controller reads. Often shortened to *post*. OpenChuckCAM includes a post for the Makera Z1 and one for LinuxCNC.

## Project {#project}

A folder of plain text files that holds one job: the model, the setup, the tools and the operations.

## Rotary axis {#rotary-axis}

The line the work turns about. On the Makera Z1 it lies along X. <span class="see">See also [A axis](#a-axis).</span>

## Roughing {#roughing}

Removing most of the material quickly, in layers, leaving a little for [finishing](#finishing). See [Roughing and finishing](concepts/roughing-and-finishing.md).

## Scallop {#scallop}

The small ridge a round-ended cutter leaves between neighbouring passes. See [Stepover, stepdown, scallop and stock to leave](concepts/stepover-and-stock-to-leave.md).

## Setup sheet {#setup-sheet}

A page the program writes beside the G-code, listing the tools, the operations and a checksum of each program file, for the person at the machine.

## Simulation {#simulation}

The program's model of the material each operation removes, shown in 3D and compared with the part. See [Simulation and deviation colours](concepts/simulation.md).

## Stepdown {#stepdown}

How deep each roughing layer goes.

## Stepover {#stepover}

How far the cutter moves sideways between neighbouring passes. A smaller stepover leaves a smoother surface and takes longer.

## Stock {#stock}

The material you start with, such as a round bar or a block. The program cuts the part out of the stock, and needs its shape and size to know where material is.

## Stock to leave {#stock-to-leave}

Material an operation leaves on the part on purpose, so that a later operation can finish it.

## Strategy {#strategy}

The pattern in which an operation moves the cutter, such as helical finishing (a spiral round the part) or axial finishing (passes along the axis).

## Tab {#tab}

A small bridge of material left uncut so that the part stays attached to the stock until you separate it yourself. See [Tabs](concepts/tabs.md).

## Tailstock {#tailstock}

A support that holds the far end of long work on a rotary machine. The program knows it only as a [held region](#held-region) at that end.

## Tolerance {#tolerance}

How far a toolpath may stray from the model. The program's default is 0.01 mm.

## Toolpath {#toolpath}

The path the cutter follows, with the angle of A at every point, worked out by an operation.

</div>

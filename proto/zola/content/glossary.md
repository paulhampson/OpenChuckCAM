+++
title = "Glossary"
weight = 90
template = "glossary.html"
[extra]
part = "Reference"
+++

The words this guide uses for machining and for the program. Each page links a term here the first time it uses it.

<p class="letter-index">A · C · G · H · P · R · S · T</p>

## A axis {#a-axis}

The rotary axis that turns the work. On the Makera Z1 it turns about the X axis, the line along the length of the work. Positive A is a turn in one fixed direction; the machine profile records which.
<span class="see">See also [rotary axis](#rotary-axis).</span>

## CAM {#cam}

Computer-aided manufacturing: software that works out the moves a machine tool must make to cut a part from a model, and writes them as [G-code](#g-code).

## G-code {#g-code}

The text program a CNC machine's controller reads: one move or setting per line, such as `G1 X10 A90 F600`. Each controller reads its own variant, which is why the program needs a [post-processor](#post-processor).

## Held region {#held-region}

A length at one end of the [stock](#stock), measured along the axis, that the chuck (or a tailstock) grips. It is never cut, and toolpaths keep clear of it. The program knows it only as a length, not as a solid body, so it cannot check the chuck itself against the tool.

## Post-processor {#post-processor}

The part of the program that turns toolpaths into the G-code variant a particular controller reads. Often shortened to *post*. OpenChuckCAM includes a post for the Makera Z1 and one for LinuxCNC.

## Rotary axis {#rotary-axis}

An axis that turns instead of moving in a straight line. In 4-axis rotary work the rotary axis holds the work and turns it, so the cutter can reach all the way round. <span class="see">See also [A axis](#a-axis).</span>

## Stepover {#stepover}

How far the cutter moves sideways between neighbouring passes. A smaller stepover leaves a smoother surface and takes longer.

## Stock {#stock}

The material you start with, such as a round bar or a block. The program cuts the part out of the stock, and needs its shape and size to know where material is.

## Tab {#tab}

A small bridge of material left uncut so that the part stays attached to the stock until you separate it yourself.

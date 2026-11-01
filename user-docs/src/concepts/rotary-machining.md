# How 4-axis rotary machining works

A CNC milling machine moves a spinning cutter along straight axes: X (left and right), Y (front and back) and Z (up and down). A 4-axis rotary machine adds a fourth axis that turns the work. This page explains what that changes, and the words OpenChuckCAM uses for it.

## The rotary axis

On a rotary machine the work is held at one end in a [chuck](../glossary.md#chuck), a clamp with jaws that closes around the stock, and the chuck can turn. The line it turns about is the [rotary axis](../glossary.md#rotary-axis). Turning it is a move of the fourth axis, called **A** when it turns about X.

On the Makera Z1 the rotary axis lies along X. The cutter moves along the length of the work (X), across it (Y) and towards or away from it (Z), while A turns the work. Because the work turns, the cutter can reach all the way round it from above. That is what makes round and carved pieces possible: legs, finials, handles, blades and bladed disks.

<figure>
<svg viewBox="0 0 640 220" width="640" height="220" role="img" aria-labelledby="rot-t rot-d" style="max-width:100%;height:auto">
<title id="rot-t">The axes of a rotary machine</title>
<desc id="rot-d">A side view. The chuck is at the left. The stock runs from it to the right along the X axis, which is also the rotary axis. A curved arrow around the axis shows A turning the work. The cutter comes down from above, along Z.</desc>
<g fill="none" stroke="currentColor" stroke-width="1.5">
<rect x="40" y="60" width="60" height="100"/>
<line x1="100" y1="80" x2="120" y2="80"/><line x1="100" y1="140" x2="120" y2="140"/>
<rect x="110" y="85" width="380" height="50"/>
<line x1="20" y1="110" x2="600" y2="110" stroke-dasharray="10 4 2 4"/>
<path d="M545 80 a 22 30 0 1 1 0 60"/><path d="M545 140 l-8 -2 l4 -7"/>
<rect x="300" y="10" width="16" height="50"/><line x1="308" y1="60" x2="308" y2="80"/>
<path d="M 360 30 v -20 m -5 6 l5 -6 l5 6"/>
</g>
<g fill="currentColor" font-size="14" font-family="inherit">
<text x="44" y="180">Chuck</text><text x="230" y="125">Stock</text>
<text x="575" y="104">X</text><text x="575" y="74">A</text>
<text x="322" y="40">Cutter</text><text x="368" y="22">Z</text>
</g>
</svg>
<figcaption><b>Figure 1.</b> A side view of a rotary machine. The rotary axis runs along X, through the chuck and the stock; A turns the work about it, and the cutter comes down along Z.</figcaption>
</figure>

## Which way A turns

Every rotary machine has a positive direction for A, and a program cut with the wrong direction carves a mirror image. The [machine profile](../glossary.md#machine-profile) records it. On the Makera Z1, a positive A move turns the top of the chuck towards the front of the machine. Check it on your own machine during the [air cut](../safety/air-cut.md).

## Moving the axes together

The program can move the axes in two ways:

- **Indexed**: A turns to an angle and stops, then the cutter works on that side with X, Y and Z alone, like a 3-axis machine looking at one face.
- **Simultaneous**: A turns while the cutter moves, so the cutter follows the surface as the work goes round. This is true 4-axis machining, and it is what OpenChuckCAM is built for.

Different [strategies](../glossary.md#strategy) use one or the other; [Helical, axial, indexed and simultaneous finishing](finishing-strategies.md) compares them.

## What the program works out

From your model, the stock, the machine and the tools, OpenChuckCAM works out the [toolpath](../glossary.md#toolpath): every position of the cutter and every angle of A. A [post-processor](../glossary.md#post-processor) then writes the toolpath as [G-code](../glossary.md#g-code), the text program your machine's controller reads. You check that program before you run it: see [Checking G-code before you run it](../safety/checking-gcode.md).

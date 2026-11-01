# Stepover, stepdown, scallop and stock to leave

Four settings decide how much each pass cuts and how close the result comes to the model. They appear in almost every operation.

Stepover
: How far the cutter moves sideways between neighbouring passes. A smaller stepover takes more passes, takes longer, and leaves a smoother surface.

Stepdown
: How deep each roughing layer goes. A larger stepdown removes material faster and loads the tool more. It must stay within what the tool can cut: its flute length, its strength and the material.

Scallop
: The small ridge a round-ended cutter leaves between two neighbouring passes. Its height depends on the tool's radius and the stepover. Some finishing operations let you set the scallop height you will accept and work out the stepover from it.

Stock to leave
: Material an operation leaves on the part on purpose, so that a later operation can finish it. Roughing leaves some; finishing leaves none.

## How they work together

<figure>
<svg viewBox="0 0 600 200" width="600" height="200" role="img" aria-labelledby="sc-t sc-d" style="max-width:100%;height:auto">
<title id="sc-t">Stepover and scallop</title>
<desc id="sc-d">A cross-section of a surface cut by three passes of a ball end mill. Each pass leaves a curved groove; between grooves stands a small ridge, the scallop. The distance between the centres of two passes is marked as the stepover, and the height of a ridge as the scallop height.</desc>
<g fill="none" stroke="currentColor" stroke-width="1.5">
<path d="M60 120 A 70 70 0 0 0 200 120 A 70 70 0 0 0 340 120 A 70 70 0 0 0 480 120"/>
<line x1="40" y1="120" x2="560" y2="120" stroke-dasharray="4 4"/>
<line x1="130" y1="40" x2="130" y2="180" stroke-dasharray="2 4"/><line x1="270" y1="40" x2="270" y2="180" stroke-dasharray="2 4"/>
<path d="M130 50 H270 M136 44 l-6 6 l6 6 M264 44 l6 6 l-6 6"/>
<path d="M355 120 V 101 M349 114 l6 6 l6 -6"/>
</g>
<g fill="currentColor" font-size="14" font-family="inherit">
<text x="170" y="40">Stepover</text><text x="362" y="100">Scallop height</text>
</g>
</svg>
<figcaption><b>Figure 1.</b> Three passes of a ball end mill, seen in section. The stepover is the distance between neighbouring passes; the ridges left between them are the scallops.</figcaption>
</figure>

- Halving the stepover roughly doubles the number of finishing passes and the time, and makes the scallops much smaller.
- A larger ball (a larger tool radius) leaves smaller scallops at the same stepover, but cannot reach into tight curves.
- Roughing with stock to leave of a few tenths of a millimetre gives the finishing tool an even, light cut.

The program's default [tolerance](../glossary.md#tolerance), how far a toolpath may stray from the model, is 0.01 mm. You can change it in each operation.

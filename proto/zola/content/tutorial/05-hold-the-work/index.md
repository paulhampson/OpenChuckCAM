+++
title = "5. Hold the work"
weight = 5
+++

The chuck grips one end of the [stock](@/glossary.md#stock). In this step you tell the program how much of that end is gripped, so that no toolpath goes there.

> [!NOTE]
> The screenshots in this tutorial were taken from an earlier build, before the workflow changed from five steps to four. The settings shown are the same; the step names along the top of the window differ.

## Set the held length

1. In the Inspector, open **Workholding and tabs**.
2. Tick **Held at the start**.
3. Set **Grip length** to `8`. Lengths are in millimetres.
4. Press **Apply** at the bottom of the section.

{{ <figure src="06-workholding.png" number={5} width={1366} height={768} alt="The Workholding and tabs section of the Inspector with Held at the start ticked and a grip length of 8 mm. In the view, the first 8 mm of the stock is drawn hatched." caption="An 8 mm [held region](@/glossary.md#held-region) at the start of the stock, drawn hatched in the view." /> }}

The first 8 mm of the stock, from X = −1 to X = 7, is now a [held region](@/glossary.md#held-region). It is never cut, and the toolpaths keep clear of it.

The program warns that this length covers 7 mm of the part, the plain end, which therefore stays as bar. That is what you want here. The warning matters when the held length covers part of the shape you meant to cut.

## Tabs

The program also reminds you that, without [tabs](@/glossary.md#tab), the part is not joined to the stock once its last support is cut. This piece stays joined to the held bar, so the tutorial uses no tab. If your own job cuts the part free, add tabs.

## Holding the far end

If you also support the far end of the stock, for example with a tailstock, tick **Held at the end** in the same way.

> [!WARNING]
> A held region is a length along the axis, not a 3D body. OpenChuckCAM does not model the chuck, its jaws or a tailstock, and does not check them against the tool or the holder. Check these clearances yourself, on the machine, before you run a program.

When the held length is set, press **Continue to operations**.

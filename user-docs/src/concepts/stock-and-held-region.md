# Stock and the held region

## Stock

The [stock](../glossary.md#stock) is the material you start with: on a rotary machine, usually a round bar or a square block. The program cuts the part out of the stock, so it needs to know the stock's shape, size and position along the axis. With that it knows where material is, where the cutter can move fast through air, and how much each pass removes.

In the program's Setup step you choose the stock's shape:

| Shape | Use it for |
| --- | --- |
| Cylinder | A round bar. You give its diameter and length. |
| Box | A square or rectangular block. |
| Bounding box | A box that just surrounds the part, with an allowance you set. |

The program can fill in a size that surrounds the part with a small allowance. Measure your real stock and enter its true size: if the real stock is larger than the project's, the first passes cut more than the program planned.

You also choose the stock's [material](../glossary.md#material). The tool library holds cutting data for each material, which the operations use for their speeds and feeds.

## The held region

The chuck grips a length at one end of the stock. That length must never be cut, and no toolpath may go there. In OpenChuckCAM it is the [held region](../glossary.md#held-region): you tick **Held at the start** and set the **Grip length**, the length the jaws cover.

If you also support the far end, for example with a [tailstock](../glossary.md#tailstock), tick **Held at the end** in the same way.

> [!WARNING]
> A held region is a length along the axis, not a solid body. OpenChuckCAM does not model the chuck, its jaws or a tailstock, and does not check them against the tool or the holder. Check these clearances yourself, on the machine, before you run a program.

## When the part comes free

When the last material joining the part to the held stock is cut, the part comes free and can fall or be caught by the cutter. If your job cuts the part free, leave [tabs](tabs.md): small bridges of material, cut through by hand afterwards. Many turned parts never come free, because one end stays joined to the held bar; the program warns you when it cannot tell.

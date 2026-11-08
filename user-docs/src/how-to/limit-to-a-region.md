# Limit an operation to a region

An operation cuts the whole part unless you tell it otherwise. To limit it to one area, make a selection set of that area and put it in the operation's **Contain within** role: the tool then cuts only inside the window of X and angle the set spans.

## Make a selection set

1. In the project panel, click **+** on the **SELECTION SETS** heading (or choose **Tools > Select on the part...**). The Operations step opens with the **New selection** inspector.
2. Choose a picking tool in the box on the view (or press the key): **Select one** (<kbd>Q</kbd>), **Flood fill** (<kbd>W</kbd>), **By angle** (<kbd>E</kbd>) or **Same direction** (<kbd>R</kbd>). A mesh is picked by triangle or edge, a STEP model by face or edge.
3. Click the part. <kbd>Shift</kbd>-click adds to the selection, <kbd>Ctrl</kbd>-click takes away. <kbd>Esc</kbd> stops picking.
4. Type a name, and click **Save as new set**.

## Use it in an operation

1. Select the operation.
2. In its **Geometry** section, in the **Contain within** row, choose the set from **add...**.
3. Generate the operation again.

To take a set out, click it in the row. Editing a set makes every operation that uses it out of date.

The other roles do different things: **Cut on** cuts only those surfaces, **Avoid** never cuts them but lets the tool pass over, and **Check** keeps the tool off them. [Operations and strategies](../reference/operations.md) describes them and their **Cut short**, **Warn only** and **Ignore** choices. Facing, rest machining, pencil clean-up, edge following and radial drilling work on the whole part and are not limited by **Contain within**.

Check the result in the simulation: the window a set spans is a box of X and angle, so it can include more of the part than the faces you picked.

# Add a tool to the library

Every operation needs a tool from the project's tools, which come from your [tool library](../reference/tool-library.md). The library already holds the ready-made tools; this page adds one of your own and puts it in a project.

## Make a new tool

1. Choose **Tools > Tool library...** (or **Tool library...** on the Operations step).
2. Under the list, open **New tool** and choose the type, for example **Ball end mill**.
3. Fill in **Name**, the sizes (**Diameter**, **Flute length**, **Overall length**, **Shank diameter**, **Flutes**) and **Material**. Measure the tool rather than trusting a catalogue: the flute length decides how deep a layer can be.
4. In the right column, under **Cutting data by material**, click **Add material**, type the stock material (for example `Hardwood`, as the setup names it), fill in the spindle speed, feed per tooth, plunge feed, stepover and stepdown, and click **Apply**.
5. Click **Save**.

To start from a similar tool, select it and click **Duplicate**. A ready-made tool cannot be edited until you copy it.

The cutting data you enter is yours to check against the tool maker's figures, your material and your machine.

## Put it in the project

1. With a project open, select the tool in the library list and click **Add to project**. The status line says which number it got, for example "Added to the project as T204 (from the machine's tool set)".
2. Or, from an operation: in the operation's **Tool** section click **From the library...**, choose the tool, and click **Use for the operation**.

The project keeps its own copy of the tool. If you change the tool in the library later, **Update from library** in the operation's tool section brings the change into the project.

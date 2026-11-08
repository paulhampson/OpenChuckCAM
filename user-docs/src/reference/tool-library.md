# Tool library

The tool library holds your cutting tools: their shape and sizes, and the cutting data for each stock material. A project keeps its own copy of each tool it uses, with the tool's number, so a change in the library never changes a project until you ask for it.

> [!WARNING]
> The cutting data of the ready-made tools are their sources' values, not cutting recipes tested by this project. Check the speeds and feeds against your tool, material and machine, and check every program before you run it (see [Checking G-code before you run it](../safety/checking-gcode.md)).

## The tool library window

Open it with **Tools > Tool library...**, or **Tool library...** on the Operations step. It works with or without a project open.

- The left column lists the tools by group, with **Search tools**, **New tool**, **Duplicate** and **Delete**. With a project open, a **This project** group lists the project's tools, with **Remove unused tools...**.
- The centre shows the selected tool's fields. A library tool is edited as a draft: **Save** writes it, **Cancel** drops the changes. A project tool's changes take effect at once, and each can be undone.
- The right column shows **Cutting data by material**, then either **Machine tool sets** (a library tool) or **In this project** (a project tool).
- At the bottom: **Import library...**, **Export library...**, **Close**, and **Add to project** (or **Use for the operation** when the window was opened to choose an operation's tool).

Deleting a library tool deletes its file. Projects that use it keep their copies, which then show "Not in the library".

## Tool types

| Type | Extra sizes |
| --- | --- |
| Flat end mill | |
| Ball end mill | |
| Bull-nose end mill | Corner radius |
| Tapered ball nose | Tip radius, Taper half angle |
| V-bit | Included angle, Tip diameter |
| Chamfer mill | Included angle, Tip diameter |
| Drill | Point angle |
| Custom profile | The profile, drawn in the profile editor |

Changing the type keeps the sizes. A standard type's shape follows its sizes; choose **Custom profile** to draw a shape of your own with **Edit the profile...**, from lines and quarter-turn arcs, with a preview that turns red and says why while the profile is not valid.

## The fields of a tool

All lengths are in millimetres and angles in degrees.

| Field | What it is | Allowed |
| --- | --- | --- |
| Name | The name in lists and on the setup sheet. | |
| Diameter | The width of the cutting part. For a custom profile it is where the profile ends, doubled. | above 0 |
| Flute length | The length of the cutting part from the tip. A layer cannot be deeper than this. | above 0, at most the overall length |
| Overall length | From the tip to the end of the shank; shown on the setup sheet. | above 0 |
| Shank diameter | The plain part the machine holds. | above 0 |
| Flutes | The number of cutting edges, used to work out the cutting feed. | 1 to 99 |
| Corner radius | Bull-nose: the roundness of the tip's corners. | above 0, at most the tool's radius |
| Tip radius, Taper half angle | Tapered ball nose: the ball at the tip, and the taper measured from the centre line. | the angle between 0° and 90° |
| Included angle, Tip diameter | V-bit and chamfer mill: the angle between the two cutting sides, and the width of the flat at the tip (0 is a sharp point). | the angle between 0° and 180° |
| Point angle | Drill: the angle across the point, often 118° or 135°. | between 0° and 180° |
| Material | What the tool is made of, such as carbide or HSS. Not the stock material. | |

Every standard type is taken to be able to plunge straight down; a custom profile is not. An operation's **Entry** setting depends on it (see [Operations and strategies](operations.md#settings-every-operation-has)).

## Cutting data by material

Each tool has a row of cutting data for each stock material it has values for:

| Value | Unit | What it sets |
| --- | --- | --- |
| Spindle speed | rpm | The S value. |
| Feed per tooth | mm | The cutting feed: spindle speed × flutes × feed per tooth. |
| Plunge feed | mm/min | The feed of plunging moves. |
| Stepover | mm | At most the diameter. Smaller leaves a smoother surface and takes longer. |
| Stepdown | mm | At most the flute length. Smaller loads the tool less and takes longer. |

**Add material** adds a row. A setup's stock material is matched to these rows by name, ignoring capital letters and extra spaces, so use the same name, such as `Hardwood`, in both places.

An operation takes its tool's values for its **Material preset**, which is the setup's stock material unless the operation chooses another one the tool has data for. A material that is not the setup's is marked with a warning. Changing a cutting value on the operation drops the preset, and the operation shows **No preset**; choose a material to apply the preset again. If the tool has no data for the material, no preset applies and the operation says so.

## Tool numbers and tool sets

The T number belongs to the project, not to the library: a library tool has no number. In a project, each tool has a **Tool number** (the `T` word), and an H and a D register that follow the tool number unless set. All are 1 or more.

A **machine tool set** gives the number a library tool gets when it is added to a project for that machine. If that number is taken in the project, the tool gets the next free number. Set it in the right column, under **Machine tool sets**. Before writing G-code, the program shows the numbers it will use and lets you keep them or renumber the tools in order of use.

## The ready-made tools

The first time the program window starts, it puts 77 ready-made tools into your library: 23 standard tools (flat, ball, bull-nose, tapered ball, V-bit, chamfer and drill) and 54 tools for the Makera Z1 (21 flat end mills, 14 ball end mills, 14 V-bits, 1 chamfer mill and 4 drills), with a Makera Z1 tool set. The Z1 set numbers them by kind: 1xx flat end, 2xx ball, 3xx V-bit, 4xx chamfer, 5xx drill. These numbers were chosen by this project, not taken from Makera.

A ready-made tool cannot be edited until you copy it with **Duplicate**. A ready-made tool you delete is not put back, and one you have changed is never overwritten. The program reads them from itself, never from a network.

## Where the library is kept

| System | Folder |
| --- | --- |
| Linux | `~/.config/OpenChuckCAM/tools` (or under `$XDG_CONFIG_HOME`) |
| Windows | `%APPDATA%\OpenChuckCAM\tools` |
| macOS | `~/Library/Application Support/OpenChuckCAM/tools` |

Each tool is one text file; the tool sets are in its `tool-sets` folder. **Import library...** reads tools from another folder and, for a tool you already have that differs, offers **Keep mine**, **Replace** or **Keep both**. **Export library...** writes them to a folder. The [command line](command-line.md#tools) has the same operations.

## Where the Makera Z1 tool data comes from

The ready-made Makera Z1 tools and the Z1 default tool set are written from facts that others published, and we credit them:

- **Makera**: the tool pages of its [store](https://global.makera.com/en-gb/products/) and the repository [MakeraInc/CarveraProfiles](https://github.com/MakeraInc/CarveraProfiles), the main source of the tools' dimensions and cutting parameters.
- **The Carvera community**: the community profiles in [Carvera-Community/Carvera_Community_Profiles](https://github.com/Carvera-Community/Carvera_Community_Profiles), used for what Makera's pages do not state, such as stepover and flute counts.

This is a credit, not a licence: neither repository has a licence file. OpenChuckCAM uses only facts from these sources (dimensions, parameters, tool numbers and cutting parameters), each recorded with its source in its tool file, and copies no descriptions, images or file structures. OpenChuckCAM is an independent project, not made or endorsed by Makera. The same credit is in `TOOL-DATA-CREDITS.txt` in each download.

The cutting data are the sources' values. Check every program before you run it on a machine (see [Checking G-code before you run it](../safety/checking-gcode.md)).

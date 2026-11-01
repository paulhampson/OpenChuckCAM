# Before you run anything: verify the G-code

This is the part that matters most. **The example job is not a tested machining recipe.** The tools, speeds, feeds, stock and the hold in this tutorial have never been cut on a machine. Even for your own jobs, the program cannot know your machine, your fixture or your tool.

Before any program runs on a machine:

1. **Read the program** from the first line to the last, or at least up to the first cut and the end. The header names the project, the setup and the software version. Check the units, the tool numbers and the safe Z.
2. **Check the machine values.** The axis height, travel and clearance in the profile must be what you measured on your machine, not the built-in assumptions. Set the [work offset](../glossary.md#work-offset) (G54) on the A axis centre line, at the X your setup says.
3. **Check the direction of A** and what the controller does at the unwinding between operations.
4. **Run an [air cut](../glossary.md#air-cut) first**: no tool and no material, at reduced feed, attended, with a hand on the stop, watching the first moves. Stop at anything unexpected.
5. Only then consider cutting, with a tool and material you have chosen for your machine, attended, starting with a trial in cheap material.

For the Makera Z1 the project keeps a [safe-practice checklist](../safety/checklist.md) and an [air-cut procedure](../safety/air-cut.md). They are the project's own starting points, written for its owner's machine. [Checking G-code before you run it](../safety/checking-gcode.md) says more.

> [!WARNING]
> **The generated G-code is yours to verify.** OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup.

## What next

- Open the finished project (**File > Open project**) in `testdata/examples/baluster/baluster-project` and change something: try **Indexed finishing** or a smaller stepover and see the result in the simulation.
- Read the [known limitations](../start/limitations.md) before relying on any strategy.
- If something is wrong or unclear, please [open an issue](https://github.com/paulhampson/OpenChuckCAM/issues).

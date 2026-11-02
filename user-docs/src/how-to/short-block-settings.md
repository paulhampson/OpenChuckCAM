# Machine setup: short moves

Some controllers cannot read G-code lines as fast as a program supplies them. A program with many very short moves, which a rotary program often has, can then run slower than its programmed feed, or run unevenly. The program's **short move** settings let it estimate that demand when it writes G-code, and optionally lower the feed where the demand is too high.

These settings are part of the [machine profile](../glossary.md#machine-profile). They are a way to find out whether a program is likely to hit this limit. They do not make a program safe or correct, and they do not replace checking it: generated G-code must be verified by you before it is run on a machine, and OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup. See [Checking G-code before you run it](../safety/checking-gcode.md).

## The three settings

Minimum block time
: The shortest average time per [block](../glossary.md#block) your controller handles smoothly, in milliseconds (ms). A block is one line of G-code that makes one move. The Makera Z1 profile starts at 7 ms.

Block window
: How many blocks in a row the program averages when it compares them with the minimum block time. The Z1 profile starts at 32. It is a whole number from 1 to 4096. Set it from the depth of your controller's queue of waiting moves.

Mode
: What the program does when the average falls below the minimum:

  - **Off**: no analysis. A minimum block time of 0 also turns the analysis off.
  - **Report**: the G-code does not change. The program estimates the demand and the time a slowdown would cost, and shows it.
  - **Slow down**: the program lowers the feed of the affected moves so each window meets the minimum. Coordinates and geometry stay the same. A larger target costs time and cutting feed, and gives the controller more margin; it does not change the geometric [tolerance](../glossary.md#tolerance).

There is no mode that merges short moves into longer ones. It is left out because merging would use up some of the geometric accuracy the toolpath is allowed.

## Where to set them

In the Inspector on the Setup step, open the machine section and look under **Short moves**: **Minimum block time**, **Block window** and **Mode**. The values apply to the copy of the machine profile in this project.

The values are stored in the project's `machine.toml` as `min_block_ms`, `block_window` and `short_block_mode`.

A [post-processor](../glossary.md#post-processor) definition can override any of the three fields on its own, under `[options]` in its `post.toml`, using the same names. A field the post leaves out keeps the project's machine value. See [Writing a post](../reference/writing-a-post.md).

The **Post and export** dialog and the [setup sheet](../glossary.md#setup-sheet) show the settings that were used and where each came from: the machine profile or a post override. That includes an override that turns the analysis off.

## What the Makera Z1 profile starts with

The built-in Makera Z1 profile starts at **Report**, 7 ms and 32 blocks. These numbers come from tests on firmware 1.1.2 with the program sent over WiFi. Treat them as a starting point for that setup only.

- A project copy of the profile that was made before these fields existed starts with the mode **Off**, until you edit it or update it from the library. A saved **Off** or **Report** keeps its mode.
- For any other machine, measure your own values. Do not copy the Z1's threshold to a controller you have not measured.

## Calibrate the values for your machine

Calibration finds the shortest setting your controller handles smoothly. Do it with the machine in the state you will use it, and check each program as the rest of this guide describes.

1. Prepare test programs with long, sustained sections of short moves, longer than your controller's buffering. Use line lengths and rotary commands like the ones your real programs have.
2. Write down the controller's firmware version and how the program reaches it (for example a card, USB or WiFi).
3. Run the test programs as an [air cut](../glossary.md#air-cut): no tool and no material. Watch whether the motion stays smooth.
4. Change the minimum block time and the block window, and run again. Keep the shortest setting that passes your agreed smoothness criteria on every repeat.
5. Repeat the calibration when the firmware, the way you deliver programs, or the controller's configuration changes.

> [!WARNING]
> A feed override above the setting you calibrated at can invalidate the target. A program that ran smoothly at 100 % can run unevenly at a higher override.

[Confirm machine profile values](confirm-machine-values.md) covers the other values the program marks as unconfirmed.

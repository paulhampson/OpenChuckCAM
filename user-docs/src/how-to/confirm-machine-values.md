# Confirm machine profile values

Some values in the built-in Makera Z1 profile have not been confirmed on a machine, and every value you change in a profile of your own starts unconfirmed. The program marks each of them, counts them in the status bar ("6 machine values unconfirmed"), and lists them as a warning when you export. Measure each one on your machine before you rely on it.

> [!WARNING]
> Measure with the machine at a safe state: spindle stopped, no tool where it can hit the chuck, and your hand on the emergency stop when anything moves. A wrong axis height, travel or safe Z makes every program the program writes wrong in the same way, and its checks cannot see it.

## See which values are unconfirmed

Click the "machine values unconfirmed" link in the status bar, or open the setup's machine section on the Setup step. An unconfirmed value has an amber triangle; hold the mouse over it to read where the value came from and how to confirm it.

## How to check each value

These are the program's own instructions, from the triangle's tooltip:

| Value | How to confirm it |
| --- | --- |
| Axis height | Run the controller's 4th-axis Z probing routine and read G54 Z in machine coordinates (G53). The value holds for the tool fitted then, the reference tool: a longer tool raises the spindle by the difference. |
| A+ direction | Send a small A+ move with a mark on the chuck and see which way its top turns. |
| A range | Turn A past a full turn each way and see whether the controller stops it. |
| Travel | Jog each axis to its limits and read the machine coordinates (G53). |
| Maximum feeds | Take them from the controller's settings or the machine's documented limits. A setting can be capped by the firmware: on a Makera Z1, send `M203.1` in the console, which prints each axis's limit as the controller holds it (A in degrees per second, so `A: 30` is 1800 degrees/min). |
| Spindle speed range | Check the spindle's lowest and highest speed in the machine's documents or controller settings. |
| Clearance | Set a clearance on the setup to suit its stock. |
| Minimum clearance radius | Measure the largest radius about the rotary axis of the chuck body, the jaws at the opening you use, the headstock and the tailstock, and add a margin. |
| Spindle to axis | With the spindle at home, measure from the collet nose down to the chuck centre; or add the probe's stick-out to the distance from home down to the axis measured with the probe. |
| Recommended minimum safe Z | Check that the safe Z clears the chuck, the jaws and the part on your machine. |
| Tool length measurement | Check in the controller's documents whether it measures each tool on a tool setter and offsets it. |
| Minimum block time | Run the block-rate air cut and record the shortest smooth setting, the firmware and how the program is sent. |
| Block window | Take the planner queue depth from the controller's documents; 1 checks each block. |

**Short moves**, the mode, is a choice of how the program writes, not a machine value, so it cannot be confirmed.

## Record what you found

1. Open the machine library with **Manage machines...** (or **Tools > Machine library...**). The built-in profile cannot be changed: work on the project's copy, or click **Copy to edit** to make your own profile (see [Machine profiles](../reference/machine-profiles.md)).
2. Type the value you measured, if it differs, and press <kbd>Enter</kbd>.
3. Click **Mark confirmed...** next to it, say how you confirmed it, for example "Measured with the probe on the chuck face, 7 Nov 2026", and click **Mark confirmed**.
4. Save the profile, and choose it for the project with **Use this machine**.

Changing a value later marks it unconfirmed again.

## On the Makera Z1

The project's own [air cut on the Makera Z1](../safety/air-cut.md) measures the axis height and checks the travel before the first run, and the [safe-practice checklist](../safety/checklist.md) covers the rest. Do the air cut after you confirm the values, and again whenever you change one.

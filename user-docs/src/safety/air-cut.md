# Air cut on the Makera Z1

An [air cut](../glossary.md#air-cut) runs a program on the machine with **no tool fitted and no material in the chuck**, at reduced feed. Every move happens in the air, so you can watch the program and stop it before anything can be damaged. Run one for every new program, and again whenever you change the machine profile, the post or the setup.

> [!NOTE]
> This page adapts the project's own air-cut procedure, [`docs/z1-air-cut-procedure.md`](https://github.com/paulhampson/OpenChuckCAM/blob/main/docs/z1-air-cut-procedure.md), for any program you make. That document is the authority: where the two differ, follow it, and tell the project.

> [!WARNING]
> Stay at the machine for the whole run, with the emergency stop within reach of your hand. Any move you did not expect, any alarm or any line the controller rejects ends the air cut: stop, and do not go on to a cut.

## Before you start

1. Complete sections 1 and 2 of the [safe-practice checklist](checklist.md).
2. Note the controller's firmware version. The project's checks of the Makera Z1 were made on firmware 1.1.2; on another version, do not assume the machine behaves the same way.
3. Measure the height of the rotary axis on your machine with Makera's 4th-axis Z probing routine, and compare it with the value in the machine profile. If it differs, correct the profile and generate the program again: a wrong axis height moves every Z in the program.
4. Set the work offset (G54) with Y0 and Z0 on the centre line of the rotary axis, and X0 at the part's X origin, at least 10 mm clear of the jaws.
5. Remove the tool from the spindle. Leave the chuck empty, or close the jaws on nothing.
6. Clear the bed and close the enclosure.
7. Put a piece of tape on one jaw, so you can see which way A turns.

## Run it

1. Open the program and read it: the header names the project, the setup and the software version; the units are millimetres; the safe Z suits the measured axis height.
2. Set the feed override to 10 %.
3. Start the program and watch the first 20 lines with your hand near the stop. A tool change line (`T1 M6`) and a spindle start run as written, with no tool fitted.
4. Watch A. A positive A move turns the top of the chuck towards the front of the machine.
5. Watch the end of the program. After the last cut the program unwinds A with `G92.4 A0 S0`: the A reading changes, but **A does not move**.
6. If everything matched what you expected, you may run it again at a higher override, in steps (25 %, 50 %, 100 %).

## Record it

Write down, for each run: the date, the firmware, the program file, the software version, the override, the axis height you measured, any alarm or rejected line, and whether the motion was what you expected. A photo of a filled-in sheet is enough. Keep the record with the program.

## After the air cut

Make the first cut only when the air cut passed. Use soft, low-cost material such as wax, foam or tooling board, and follow section 3 of the [safe-practice checklist](checklist.md).

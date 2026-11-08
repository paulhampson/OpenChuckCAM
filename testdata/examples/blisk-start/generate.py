# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the starting project of the user guide's worked examples on the public blisk.

    python3 testdata/examples/blisk-start/generate.py

It writes blisk-project/ beside this script: the project file, the built-in Makera Z1 machine
profile, one setup with its stock, a 3 mm flat end mill and a 3 mm ball end mill, and a copy of
testdata/step/valid/blisk.step in models/. It has no selection sets and no operations: the reader
makes them. The ids are fixed, so the same script always writes the same bytes. Written for this
repository (lane EXAMPLES-B); AGPL-3.0-or-later, as the rest of the repository, and the blisk is
the project owner's (testdata/README.md).
"""
import hashlib
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SOURCE = REPO / "testdata/step/valid/blisk.step"
SOURCE_SHA256 = "0e472ff5374c6bc40825a4f7e8885d69f6ac99e53fbb6dbc412104fadb55b646"
# The project file records the model's BLAKE3 hash (testdata/projects/face_flow holds the same).
MODEL_HASH = "blake3:883a66d9e35b50c8b8ff2e5380d698a14566c0e7d14eb92efc8dfb225de1c616"
SCHEMA = 27
MODEL_ID = "b1150000-0000-4000-8000-000000000001"
SETUP_ID = "b1150000-0000-4000-8000-000000000002"

TOOLS = {
    # id: (name, diameter, shape, flute length, number)
    "b1150000-0000-4000-8000-000000000101": ("3 mm flat end mill", 3.0, "flat_end", 16.0, 1),
    "b1150000-0000-4000-8000-000000000102": ("3 mm ball", 3.0, "ball_end", 16.0, 2),
}

# The built-in Makera Z1 profile as a project holds it (the same as testdata/projects/face_flow).
MACHINE = """schema = 27

[machine]
name = "Makera Z1"
rotary_axis = "x"
a_sense = "positive"
max_tool_number = 1000
max_spindle_rpm = 13000.0
min_block_ms = 7.0
block_window = 32
short_block_mode = "report"
automatic_tool_length = true
spindle_to_axis = 73.1
recommended_min_safe_z = 30.0

[machine.rotary_origin]
x = -180.4
y = -108.8
z = -48.6

[machine.a_range]
type = "continuous"

[machine.travel.min]
x = -200.0
y = -200.0
z = -100.0

[machine.travel.max]
x = 0.0
y = 0.0
z = 0.0

[machine.max_feeds]
linear = 1200.0
z = 600.0
rotary = 3600.0
rapid_linear = 1200.0
rapid_z = 600.0
rapid_rotary = 3600.0
rotary_in_feed = "makera"

[machine.clearance]
radius = 40.0
safe_z = 40.0

[machine.provenance.rotary_axis]
state = "confirmed"
source = "confirmed by the owner from use of the machine (11 Oct 2026): the rotary axis is along X. Consistent with 7 of 7 Makera 4-axis profiles (Fusion Z1, Carvera, Carvera Air and Carvera Harmonic; FreeCAD Z1, C1 and Air) and the Makera Mastercam and Carveco posts (decision 0013)"

[machine.provenance.a_sense]
state = "confirmed"
source = "confirmed by the owner from use of the machine (11 Oct 2026): positive A is right-handed about +X, so the side facing up at A 0 faces machine -Y at A +90. Checked again on the machine on 13 Oct 2026 by sending an A+ command: a camera behind the chuck, looking toward home, sees the top of the chuck turn toward the front of the machine (away from home, -Y): clockwise from that view, counter-clockwise looking at the chuck from +X. Consistent with the Fusion (4 four-axis machine files), Mastercam and SolidCAM profiles. The SP-12 air cut (14 Oct 2026, firmware 1.1.2, decision 0007) confirmed it a third time: at A +90 the top moved toward the front"

[machine.provenance.rotary_origin]
state = "confirmed"
source = "confirmed by the owner (30 Oct 2026, decision 0317): Z -48.6 is the machine Z (G53) of the axis with the probe as the reference tool, from Makera's 4th-axis Z probing routine on the owner's Z1 (gate 4, PR 284 comment 6399134358, firmware 1.1.2). The routine touches a point on the headstock 23 mm above the chuck centre and sets G54 Z0 on the axis; it makes the probe the reference tool, so another tool's tool length offset raises or lowers the spindle by its length difference from the probe. Not recorded: G54 X, Y and Z in machine coordinates, the tool length offset state, and gate steps P8 and M4 (OI-471). Y is the firmware's probing position (anchor1 plus rotation offset), not measured separately. X is from the firmware config (anchor1 plus rotation offset); its datum, chuck face or module reference, is not stated (OI-60)"

[machine.provenance.a_range]
state = "confirmed"
source = "confirmed by the owner from use of the machine (11 Oct 2026): the A axis is continuous. Consistent with Fusion (no limits, never reset), FreeCAD (+/-1e7, unwound), Mastercam (signed continuous) and the Z1 firmware (no A soft limit). The air cut turned A to 570 and -370 and unwound by `G92.4 A0 S0` with no limit stop"

[machine.provenance.travel]
state = "unconfirmed"
source = "spans 200 x 200 x 100 mm: FreeCAD Z1 exactly, Fusion Z1 with 10 mm extra on X and Y, firmware soft limits -206/-206/-102 (disabled by default). Frame is the firmware's G53 with home at 0; with the rotary module and soft limits on, the firmware holds Y above -160. Stock defaults: a user who changes them makes their own profile (decision 0013)"

[machine.provenance.max_feeds]
state = "confirmed"
source = "confirmed by the owner from use of the machine (11 Oct 2026): X and Y 1200 mm/min, Z 600 mm/min, A 3600 deg/min (10 RPM). Consistent with Makera's published A rate, the Z1 and Z1 Pro firmware default config (Carvera and Air differ) and the 1200 reported from Makera Studio; the CAM profiles' figures are template values and were not used (decision 0013). How the firmware counts A in the F word was measured by the SP-12 air cut on firmware 1.1.2 (decision 0007, tests T3 and T4); the feed limits are applied to that length (decision 0076)"

[machine.provenance.clearance]
state = "unconfirmed"
source = "OpenChuckCAM default chosen by the owner (30 Oct 2026, decision 0317), not a machine value: a safe Z of 40 mm puts the spindle at machine Z -8.6 with the probe fitted, and a longer tool raises it. A stock wider than 80 mm in diameter is inside the safe cylinder and warns; set a clearance on each setup to suit its stock"

[machine.provenance.min_block_ms]
state = "confirmed"
source = "Owner gate 4, PR 284 comment 6399134358, submitted 25 Oct 2026: firmware 1.1.2, ESP32 (version not supplied), WiFi upload; 7, 10 and 20 ms passed in both parts, 5/3/2/1 ms stuttered. K4 retained by the owner as a release waiver; subsequent LinuxCNC replay passed. This measured program/transport only (decision 0222)."

[machine.provenance.block_window]
state = "unconfirmed"
source = "32-block planner queue in firmware source; accepted averaging rule, decision 0222 item 3; not a separately measured queue depth."

[machine.provenance.short_block_mode]
state = "unconfirmed"
source = "Report first, accepted decision 0222; software policy, not a controller measurement."

[machine.provenance.spindle_range]
state = "unconfirmed"
source = "Makera's published specification: the Makera Z1 Safety Guidelines, V1.3 (makera.com, file MakeraZ1-Safety_Guidelines-V1.3-multi-lan-260709-1.pdf), give the spindle as 0 to 13 000 rpm with closed-loop control; reviews (CNX Software, 1 Sep 2026) give the same. Not yet checked on the machine (decision 0231)"

[machine.provenance.tool_length_measurement]
state = "confirmed"
source = "confirmed by the owner (30 Oct 2026, decision 0317): the Z1 measures each tool on its tool setter and offsets it from the probe as the reference tool (firmware 1.1.2, ATCHandler.cpp:1909-1919; Robot.cpp:498-506)"

[machine.provenance.spindle_to_axis]
state = "unconfirmed"
source = "derived (30 Oct 2026, decision 0318, issue #395): the owner measured the probe's stick-out from the collet as 24.5 mm, from the same reference as a tool's stick-out; D = 48.6 + 24.5 = 73.1 mm, the axis height measured with the probe plus the probe's stick-out. Not measured directly"

[machine.provenance.recommended_min_safe_z]
state = "unconfirmed"
source = "the owner's recommendation from measurements and experience (30 Oct 2026, decision 0317): The recommended minimum is 30 mm. This must be checked before machining."
"""


def project():
    return f"""schema = {SCHEMA}
name = "Blisk"

[[models]]
id = "{MODEL_ID}"
path = "models/blisk.step"
hash = "{MODEL_HASH}"

[[models.transform]]
kind = "centre_on_rotary_axis"

[[models.transform]]
kind = "translate"
x = 10.0
y = 0.0
z = 0.0
"""


def setup():
    return f"""schema = {SCHEMA}
id = "{SETUP_ID}"
order = "V"
name = "Setup 1"
part = "{MODEL_ID}"
material = "oak"
part_offset = 0.0

[work_offset]
register = "g54"
x = 0.0

[stock]
type = "cylinder"
diameter = 42.0
length = 52.0
x_start = 9.0
"""


def tool(tid, name, diameter, shape, flutes, number):
    return f"""schema = {SCHEMA}
id = "{tid}"
order = "V"
name = "{name}"

[tool]
diameter = {diameter}
flute_length = {flutes}
overall_length = 45.0
shank_diameter = {diameter}
flute_count = 2

[tool.shape]
type = "{shape}"

[controller]
number = {number}

[cutting_data.oak]
spindle_speed = 12000.0
feed_per_tooth = 0.02
plunge_feed = 150.0
stepover = 1.0
stepdown = 1.0
"""


def main():
    data = SOURCE.read_bytes()
    if hashlib.sha256(data).hexdigest() != SOURCE_SHA256:
        sys.exit(f"error: {SOURCE} is not the blisk this example was written for")
    out = HERE / "blisk-project"
    if out.exists():
        shutil.rmtree(out)
    for sub in ("models", "setups", "tools"):
        (out / sub).mkdir(parents=True)
    (out / "models/blisk.step").write_bytes(data)
    (out / "project.toml").write_text(project())
    (out / "machine.toml").write_text(MACHINE)
    (out / "setups" / f"{SETUP_ID}.toml").write_text(setup())
    for tid, spec in TOOLS.items():
        (out / "tools" / f"{tid}.toml").write_text(tool(tid, *spec))
    print("wrote", out)


if __name__ == "__main__":
    main()

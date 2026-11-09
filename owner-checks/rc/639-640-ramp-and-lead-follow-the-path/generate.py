# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issues #639 and #640 (decisions 0543 and 0544: a ramp
follows the pass's own toolpath; a lead on a pass that turns A stays in the plane of the cut):

    python3 owner-checks/rc/639-640-ramp-and-lead-follow-the-path/generate.py

It writes project/ beside this script: the user guide's tutorial project
(testdata/examples/baluster/baluster-project: the public baluster, 63 mm of 24 mm stock from
X -1, the chuck holding 8 mm) with its two operations replaced by five, each showing one
behaviour of the records with its tools (the 3.175 mm flat end mill for roughing, the 3.175 mm
ball for finishing), all with a 3 degree ramp unless named otherwise:

1. "Rough rings, ramp": rotary roughing in Rings, Entry Ramp, Allow single-point plunge on;
2. "Rough spiral, ramp": rotary roughing (spiral), Entry Ramp, Allow single-point plunge on:
   its last layers near the axis need more than 10 turns, so it is refused naming the bound;
3. "Rough spiral, steeper": as 2 with Steeper ramp near the axis on (Maximum ramp angle 15);
4. "Finish rings, ramp": helical finishing in Rings, Entry Ramp, Allow single-point plunge on
   (a ring on the rotary axis at the end face: the tool descends there while the part turns);
5. "Finish rings, lead": helical finishing in Rings, Entry Lead, on "Setup 2, no grip" (the
   same stock with no held region).

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import shutil
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
START = oc.REPO / "testdata/examples/baluster/baluster-project"
SETUP_ID = "57ad6798-2544-4dfb-90a0-caf67ec69484"
FLAT = "c0de0001-0000-4000-8000-000000000104"
BALL = "c0de0001-0000-4000-8000-000000000204"
ROUGH = {"feed": "1000.0", "plunge_feed": "300.0", "stepdown": "1.0", "stepover": "2.2225",
         "stock_to_leave": "0.3", "entry": '"ramp"', "ramp_angle": "3.0",
         "allow_single_point_plunge": "true"}
RINGS = {"pass_shape": '"rings"'}
FINISH = {"adaptive_spacing": "false", "pass_shape": '"rings"'}
OPS = [
    ("V", "Rough rings, ramp", "rotary_roughing", FLAT, {**ROUGH, **RINGS}),
    ("W", "Rough spiral, ramp", "rotary_roughing", FLAT, ROUGH),
    ("X", "Rough spiral, steeper", "rotary_roughing", FLAT,
     {**ROUGH, "ramp_steeper_near_axis": "true", "ramp_angle_max": "15.0"}),
    ("Z", "Finish rings, ramp", "helical_finishing", BALL,
     {**FINISH, "entry": '"ramp"', "ramp_angle": "3.0", "allow_single_point_plunge": "true"}),
    ("ZA", "Finish rings, lead", "helical_finishing", BALL, {**FINISH, "entry": '"lead"'}),
]
# The lead check runs on a second setup with the same stock and no grip (the coordinator's
# instruction of 9 Nov 2026: with the grip, the pass starts with a short cut along X, a case
# left to the owner's decision under decision 0544 L2).
SETUP2_ID = "0c0c0639-0000-4000-8000-000000000002"


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(START, out)
    shutil.rmtree(out / "operations")
    (out / "operations").mkdir()
    setup = (out / f"setups/{SETUP_ID}.toml").read_text(encoding="utf-8")
    grip = "\n[workholding.start]\nlength = 8.0\n"
    if grip not in setup:
        raise SystemExit("the tutorial's setup is not as expected")
    setup2 = (setup.replace(grip, "").replace(SETUP_ID, SETUP2_ID)
              .replace('order = "V"', 'order = "W"').replace('name = "Setup 1"', 'name = "Setup 2, no grip"'))
    oc.write(out / f"setups/{SETUP2_ID}.toml", setup2)
    for i, (order, name, strategy, tool, params) in enumerate(OPS, start=1):
        op = f"0c0c0639-0000-4000-8000-0000000003{i:02d}"
        setup_id = SETUP2_ID if name == "Finish rings, lead" else SETUP_ID
        oc.write(out / f"operations/{op}.toml",
                 oc.operation_toml(op, order, name, setup_id, strategy, tool, params))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()

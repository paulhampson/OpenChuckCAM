# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issues #351 and #257 (decision 0526, OP-27):

    python3 testdata/owner-checks/351-257-exact-step-contact/generate.py

It writes project/ beside this script: the public STEP fixture testdata/step/valid/
shouldered_shaft.step (40 mm long; radius 8, a sharp shoulder out to radius 12, radius 12, a
shoulder in to radius 10 with a 2 mm fillet down to radius 8), turned to lie along X from 0 to
40, in 26 mm stock from X -10 with the chuck holding 6 mm, and two operations on the whole part:

- "Axial, tapered ball": axial finishing with the library's tapered ball (tip radius 0.5,
  5 degrees, 3.175 mm);
- "Pencil, flat": pencil clean-up with a 2 mm flat end mill, which finds the sharp inside corner.

The project is written at schema 25 without the "Exact STEP surface contact" setting, as a
project saved before the setting existed; a build with it gives both operations the setting off
(their default, decision 0526 item 5), so the check turns it on by hand.

Written for this repository (lane OWNERCHECKS); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
MODEL = "0c0c0351-0000-4000-8000-000000000001"
SETUP_ID = "0c0c0351-0000-4000-8000-000000000002"
TAPERED = "0c0c0351-0000-4000-8000-000000000101"
FLAT = "0c0c0351-0000-4000-8000-000000000102"
OP_AXIAL = "0c0c0351-0000-4000-8000-000000000301"
OP_PENCIL = "0c0c0351-0000-4000-8000-000000000302"

TRANSFORMS = """
[[models.transform]]
kind = "rotate"
axis = "y"
angle = 90.0

[[models.transform]]
kind = "centre_on_rotary_axis"
"""

SETUP = """[stock]
type = "cylinder"
diameter = 26.0
length = 52.0
x_start = -10.0

[workholding.start]
length = 6.0
"""

TAPERED_SHAPE = "tip_radius = 0.5\ntaper_half_angle = 5.0\n"


def main():
    oc.step_project(
        HERE / "project", "Owner check 351 and 257", "testdata/step/valid/shouldered_shaft.step",
        "54362c47d585199c0f3f191c1155f931e8473876f55fc82c11746bf837b02dfe",
        "c7ac2c385f28ab810ba098b2438b7ca1fe843c5f0babc939d2fdf3edc6756726",
        MODEL, SETUP_ID, TRANSFORMS, SETUP,
        tools=[
            (TAPERED, oc.tool_toml(TAPERED, "V", "Tapered ball r0.5 5 deg (check 351)", 3.175,
                                   20.0, "tapered_ball", 351, overall=50.0,
                                   extra_shape=TAPERED_SHAPE)),
            (FLAT, oc.tool_toml(FLAT, "W", "2 mm flat (check 351)", 2.0, 10.0, "flat_end", 352)),
        ],
        operations=[
            (OP_AXIAL, oc.operation_toml(OP_AXIAL, "V", "Axial, tapered ball", SETUP_ID,
                                         "axial_finishing", TAPERED,
                                         {"adaptive_spacing": "false", "stepover": "0.5",
                                          "face_free_ends": "false"})),
            (OP_PENCIL, oc.operation_toml(OP_PENCIL, "W", "Pencil, flat", SETUP_ID,
                                          "pencil_finishing", FLAT, {})),
        ],
    )
    print(f"wrote {HERE / 'project'}")


if __name__ == "__main__":
    main()

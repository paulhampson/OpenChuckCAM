# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #640 (decision 0544: a lead on a pass that turns A
stays in the plane of the cut):

    python3 owner-checks/640-lead-follows-the-pass/generate.py

It writes project/ beside this script: the user guide's tutorial project
(testdata/examples/baluster/baluster-project: the public baluster) with the public STEP fixture
testdata/step/valid/cylinder.step added as a second model, and four operations, all with
Entry Lead:

1. "Finish rings, lead": helical finishing in Rings with the 3.175 mm ball, on "Setup 1, no
   grip" (the tutorial's 63 mm of 24 mm stock from X -1, without its 8 mm grip);
2. "Axial lead, through stock": axial finishing of the cylinder (radius 10, 50 mm long, turned
   to lie along X from 0 to 50) with "O-flute, not centre cutting" (the tutorial's 3.175 mm
   flat end mill, marked as not centre cutting), Extend past the stock at the far end, every
   pass the same way, on "Setup 2, cylinder in long stock" (24 mm stock from X -10): each line
   lead's arc starts in the stock beyond the cylinder's end, so the descent to it would cut
   straight down, and the operation is refused naming the passes;
3. "Axial lead, through air": as 2, on "Setup 3, cylinder in flush stock" (the stock starts at
   the cylinder's end, X 0): the descent is through air, and it generates;
4. "Finish rings, lead, with grip": as 1, on "Setup 4, with grip" (the tutorial's setup as it
   is, the chuck holding 8 mm): with decision 0550 (#664) the rings start on a whole ring at the
   held region's edge, so the lead turns into that ring.

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import shutil
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
START = oc.REPO / "testdata/examples/baluster/baluster-project"
CYLINDER = oc.REPO / "testdata/step/valid/cylinder.step"
CYLINDER_SHA256 = "93f461c451602880c347738adccbaa75e8ead937888db3883a14c7794394e7d7"
CYLINDER_BLAKE3 = "d6c36647a154b1b5b05017988f6dbeb7e302749b201c0c30fe27671af0602027"
CYLINDER_ID = "0c0c0640-0000-4000-8000-000000000011"
SETUP1 = "57ad6798-2544-4dfb-90a0-caf67ec69484"
SETUP2 = "0c0c0640-0000-4000-8000-000000000002"
SETUP3 = "0c0c0640-0000-4000-8000-000000000003"
SETUP4 = "0c0c0640-0000-4000-8000-000000000004"
FLAT = "c0de0001-0000-4000-8000-000000000104"
NOCENTRE = "0c0c0640-0000-4000-8000-000000000105"
BALL = "c0de0001-0000-4000-8000-000000000204"
RINGS = {"entry": '"lead"', "adaptive_spacing": "false", "pass_shape": '"rings"'}
LINE = {"entry": '"lead"', "direction": '"climb"', "extend": '"far end"',
        "face_free_ends": "false"}
OPS = [
    ("V", "Finish rings, lead", SETUP1, "helical_finishing", BALL, RINGS),
    ("W", "Axial lead, through stock", SETUP2, "axial_finishing", NOCENTRE, LINE),
    ("X", "Axial lead, through air", SETUP3, "axial_finishing", NOCENTRE, LINE),
    ("Y", "Finish rings, lead, with grip", SETUP4, "helical_finishing", BALL, RINGS),
]
CYLINDER_MODEL = f"""
[[models]]
id = "{CYLINDER_ID}"
path = "models/cylinder.step"
hash = "blake3:{CYLINDER_BLAKE3}"
name = "cylinder.step"

[[models.transform]]
kind = "rotate"
axis = "y"
angle = 90.0

[[models.transform]]
kind = "centre_on_rotary_axis"
"""


def cylinder_setup(setup_id, order, name, x_start, length):
    return f"""schema = {oc.SCHEMA}
id = "{setup_id}"
order = "{order}"
name = "{name}"
part = "{CYLINDER_ID}"
material = "Hardwood"
part_offset = 0.0

[work_offset]
register = "g54"
x = 0.0

[stock]
type = "cylinder"
diameter = 24.0
length = {length}
x_start = {x_start}
"""


def main():
    if oc.sha256(CYLINDER) != CYLINDER_SHA256:
        raise SystemExit(f"{CYLINDER} is not the expected model")
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(START, out)
    shutil.rmtree(out / "operations")
    (out / "operations").mkdir()
    shutil.copyfile(CYLINDER, out / "models/cylinder.step")
    project = (out / "project.toml").read_text(encoding="utf-8")
    oc.write(out / "project.toml", project.replace('name = "Baluster"',
                                                   'name = "Owner check 640"') + CYLINDER_MODEL)
    path = out / f"setups/{SETUP1}.toml"
    setup = path.read_text(encoding="utf-8")
    grip = "\n[workholding.start]\nlength = 8.0\n"
    if grip not in setup or 'name = "Setup 1"' not in setup:
        raise SystemExit("the tutorial's setup is not as expected")
    oc.write(path, setup.replace(grip, "").replace('name = "Setup 1"', 'name = "Setup 1, no grip"'))
    oc.write(out / f"setups/{SETUP4}.toml",
             setup.replace(SETUP1, SETUP4).replace('order = "V"', 'order = "Y"')
             .replace('name = "Setup 1"', 'name = "Setup 4, with grip"'))
    oc.write(out / f"setups/{SETUP2}.toml",
             cylinder_setup(SETUP2, "W", "Setup 2, cylinder in long stock", "-10.0", "62.0"))
    oc.write(out / f"setups/{SETUP3}.toml",
             cylinder_setup(SETUP3, "X", "Setup 3, cylinder in flush stock", "0.0", "52.0"))
    tool = (out / f"tools/{FLAT}.toml").read_text(encoding="utf-8")
    for old, new in [(FLAT, NOCENTRE), ('order = "V"', 'order = "X"'),
                     ('name = "Z1 O-flute 3.175x25"', 'name = "O-flute, not centre cutting"'),
                     ('material = "carbide"\n', 'material = "carbide"\ncentre_cutting = false\n'),
                     ("number = 104", "number = 105")]:
        if old not in tool:
            raise SystemExit(f"the tutorial's tool is not as expected: {old}")
        tool = tool.replace(old, new)
    oc.write(out / f"tools/{NOCENTRE}.toml", tool)
    for i, (order, name, setup_id, strategy, tool_id, params) in enumerate(OPS, start=1):
        op = f"0c0c0640-0000-4000-8000-0000000003{i:02d}"
        oc.write(out / f"operations/{op}.toml",
                 oc.operation_toml(op, order, name, setup_id, strategy, tool_id, params))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()

# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #640 (decision 0544: a lead on a pass that turns A
stays in the plane of the cut):

    python3 owner-checks/640-lead-follows-the-pass/generate.py

It writes project/ beside this script: the user guide's tutorial project
(testdata/examples/baluster/baluster-project: the public baluster) with no grip, and three
operations, all with Entry Lead:

1. "Finish rings, lead": helical finishing in Rings with the 3.175 mm ball, on "Setup 1, no
   grip" (the tutorial's 63 mm of 24 mm stock from X -1, without the 8 mm grip);
2. "Axial lead, through stock": axial finishing with "O-flute, not centre cutting" (the
   tutorial's 3.175 mm flat end mill, marked as not centre cutting), on "Setup 2, long stock"
   (stock from X -13, no grip): the line lead's arc starts inside the stock, so the descent to
   it would cut straight down, and the operation is refused naming the pass;
3. "Axial lead, through air": as 2 on "Setup 1, no grip": the arc starts past the end of the
   stock, the descent is through air, and it generates.

With the tutorial's grip, the rings pass starts with a short cut along X before its first ring
and keeps the line lead; that case waits for pull request #664 and is not part of this check.

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
SETUP1 = "57ad6798-2544-4dfb-90a0-caf67ec69484"
SETUP2 = "0c0c0640-0000-4000-8000-000000000002"
FLAT = "c0de0001-0000-4000-8000-000000000104"
NOCENTRE = "0c0c0640-0000-4000-8000-000000000105"
BALL = "c0de0001-0000-4000-8000-000000000204"
FINISH = {"entry": '"lead"'}
OPS = [
    ("V", "Finish rings, lead", SETUP1, "helical_finishing", BALL,
     {**FINISH, "adaptive_spacing": "false", "pass_shape": '"rings"'}),
    ("W", "Axial lead, through stock", SETUP2, "axial_finishing", NOCENTRE, FINISH),
    ("X", "Axial lead, through air", SETUP1, "axial_finishing", NOCENTRE, FINISH),
]


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(START, out)
    shutil.rmtree(out / "operations")
    (out / "operations").mkdir()
    path = out / f"setups/{SETUP1}.toml"
    setup = path.read_text(encoding="utf-8")
    grip = "\n[workholding.start]\nlength = 8.0\n"
    stock = "length = 63.0\nx_start = -1.0\n"
    if grip not in setup or stock not in setup or 'name = "Setup 1"' not in setup:
        raise SystemExit("the tutorial's setup is not as expected")
    setup = setup.replace(grip, "")
    oc.write(path, setup.replace('name = "Setup 1"', 'name = "Setup 1, no grip"'))
    oc.write(out / f"setups/{SETUP2}.toml",
             setup.replace(SETUP1, SETUP2).replace('order = "V"', 'order = "W"')
             .replace('name = "Setup 1"', 'name = "Setup 2, long stock"')
             .replace(stock, "length = 75.0\nx_start = -13.0\n"))
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

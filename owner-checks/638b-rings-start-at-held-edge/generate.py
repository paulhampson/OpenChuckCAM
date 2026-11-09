# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for decision 0550 (issues #638 and #640: rings start on a whole
ring at a held region's edge):

    python3 owner-checks/638b-rings-start-at-held-edge/generate.py

It writes project/ beside this script: the user guide's tutorial project
(testdata/examples/baluster/baluster-project: the public baluster in 63 mm of 24 mm stock from
X -1, the chuck holding 8 mm) with its two operations replaced by the tutorial's Finish in Rings
twice:

1. "Finish rings, sharp": Steps between rings Sharp;
2. "Finish rings, blended": Steps between rings Blended, Blend over 45 degrees.

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
SETUP = "57ad6798-2544-4dfb-90a0-caf67ec69484"
BALL = "c0de0001-0000-4000-8000-000000000204"
RINGS = {"adaptive_spacing": "false", "pass_shape": '"rings"'}
OPS = [
    ("V", "Finish rings, sharp", {**RINGS, "ring_step": '"sharp"'}),
    ("W", "Finish rings, blended", {**RINGS, "ring_step": '"blended"', "ring_blend_angle": "45.0"}),
]


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(START, out)
    if "\n[workholding.start]\nlength = 8.0\n" not in (out / f"setups/{SETUP}.toml").read_text(
            encoding="utf-8"):
        raise SystemExit("the tutorial's setup is not as expected")
    shutil.rmtree(out / "operations")
    (out / "operations").mkdir()
    project = out / "project.toml"
    project.write_text(project.read_text(encoding="utf-8").replace(
        'name = "Baluster"', 'name = "Owner check 638b"'), encoding="utf-8")
    for i, (order, name, params) in enumerate(OPS, start=1):
        op = f"0c0c0550-0000-4000-8000-0000000003{i:02d}"
        oc.write(out / f"operations/{op}.toml",
                 oc.operation_toml(op, order, name, SETUP, "helical_finishing", BALL, params))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()

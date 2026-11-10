# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #644 (decision 0546: run-outs on ring and spiral
passes follow the pass):

    python3 owner-checks/644-run-outs-follow-the-pass/generate.py

It writes project/ beside this script: the public baluster of the user guide's tutorial in 78 mm
of 24 mm stock from X -13 to 65 (4 mm of stock past the part's far end at X 61), the chuck holding
8 mm (X -13 to -5), and a 3.175 mm ball. Two setups on that same stock, each with two helical
finishing operations with Extend past the stock at the far end (the end the chuck does not hold):

- "Setup 1, Face free ends on" (the default): "Spiral, extend" (the helix, Passes Helix, 1 mm
  pitch) and "Rings, extend" (Passes Rings, 1 mm apart);
- "Setup 2, Face free ends off": the same two, "Spiral, extend, ends off" and "Rings, extend,
  ends off", as the first issue of this check had them.

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
BALL = "0c0c0644-0000-4000-8000-000000000102"
OP_SPIRAL = "0c0c0644-0000-4000-8000-000000000301"
OP_RINGS = "0c0c0644-0000-4000-8000-000000000302"
OP_SPIRAL_OFF = "0c0c0644-0000-4000-8000-000000000303"
OP_RINGS_OFF = "0c0c0644-0000-4000-8000-000000000304"
SETUP2 = "0c0c0644-0000-4000-8000-000000000012"

SETUP = """[stock]
type = "cylinder"
diameter = 24.0
length = 78.0
x_start = -13.0

[workholding.start]
length = 8.0
"""

# Setup 1 keeps Face free ends at its default (on); Setup 2 is the same stock with it off,
# as the first issue of this check had it (owner FAIL on rc1, 10 Nov 2026: the run-out keeps
# the height of the pass's end point, so with it off a core stays past the part's end).
ON = {"adaptive_spacing": "false", "stepover": "1.0", "extend": '"far end"'}
OFF = {**ON, "face_free_ends": "false"}
RINGS = {"pass_shape": '"rings"'}


def main():
    out = HERE / "project"
    oc.baluster_project(
        out,
        "Owner check 644",
        SETUP,
        tools=[(BALL, oc.tool_toml(BALL, "V", "3.175 mm ball (check 644)", 3.175, 22.0,
                                   "ball_end", 204, shank=3.175))],
        operations=[
            (OP_SPIRAL, oc.operation_toml(OP_SPIRAL, "V", "Spiral, extend", oc.BALUSTER_SETUP,
                                          "helical_finishing", BALL, ON)),
            (OP_RINGS, oc.operation_toml(OP_RINGS, "W", "Rings, extend", oc.BALUSTER_SETUP,
                                         "helical_finishing", BALL, {**ON, **RINGS})),
            (OP_SPIRAL_OFF, oc.operation_toml(OP_SPIRAL_OFF, "X", "Spiral, extend, ends off",
                                              SETUP2, "helical_finishing", BALL, OFF)),
            (OP_RINGS_OFF, oc.operation_toml(OP_RINGS_OFF, "Y", "Rings, extend, ends off",
                                             SETUP2, "helical_finishing", BALL,
                                             {**OFF, **RINGS})),
        ],
    )
    setup = (out / f"setups/{oc.BALUSTER_SETUP}.toml").read_text(encoding="utf-8")
    for a, b in [(oc.BALUSTER_SETUP, SETUP2), ('order = "V"', 'order = "W"'),
                 ('name = "Setup 1"', 'name = "Setup 2, Face free ends off"')]:
        if a not in setup:
            raise SystemExit(f"setup not as expected: {a}")
        setup = setup.replace(a, b, 1)
    oc.write(out / f"setups/{SETUP2}.toml", setup)
    setup1 = out / f"setups/{oc.BALUSTER_SETUP}.toml"
    oc.write(setup1, setup1.read_text(encoding="utf-8").replace(
        'name = "Setup 1"', 'name = "Setup 1, Face free ends on"', 1))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()

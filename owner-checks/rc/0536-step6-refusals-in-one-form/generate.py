# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for decision 0536 build step 6 (decision 0644: every refusal to
generate, the Facing notes and the setup checks take the one form):

    python3 owner-checks/rc/0536-step6-refusals-in-one-form/generate.py

It writes project/ beside this script: the user guide's tutorial project
(testdata/examples/baluster/baluster-project: the public baluster in 63 mm of 24 mm stock from
X -1, the chuck holding 8 mm) with the public STEP fixture testdata/step/valid/cylinder.step
added as a second model, three setups and seven operations, each showing one kind of message:

Setup 1 (the tutorial's):
  1. "Face end": Facing of the free end (the Facing note);
  2. "Face end, wide stepover": the same with a Stepover (slice thickness along X) of 5 mm,
     wider than the 3.175 mm tool (End facing);
  3. "Rough, deep stepdown": rotary roughing with a Stepdown deeper than the flutes (Stepdown);
  4. "Rough rings, ramp": rotary roughing in Rings with Entry Ramp and Allow single-point plunge
     off (Ramp entry);
  5. "Finish": the tutorial's Finish, which generates.
Setup 2, low clearance (the same stock, the safe cylinder's radius 10 mm, inside the stock):
  6. "Finish, low clearance": the tutorial's Finish (Links; and a Clearance check of the setup).
Setup 3, cylinder in long stock (cylinder.step, radius 10, X 0 to 50, in 24 mm stock from X -10):
  7. "Axial lead, through stock": axial finishing with Entry Lead and a 3.175 mm flat end mill
     marked as not centre cutting (Lead entry, the passes named).

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
CYLINDER = oc.REPO / "testdata/step/valid/cylinder.step"
CYLINDER_SHA256 = "93f461c451602880c347738adccbaa75e8ead937888db3883a14c7794394e7d7"
CYLINDER_BLAKE3 = "d6c36647a154b1b5b05017988f6dbeb7e302749b201c0c30fe27671af0602027"
CYLINDER_ID = "0c0c0536-0000-4000-8000-000000000611"
SETUP1 = "57ad6798-2544-4dfb-90a0-caf67ec69484"
SETUP2 = "0c0c0536-0000-4000-8000-000000000602"
SETUP3 = "0c0c0536-0000-4000-8000-000000000603"
FLAT = "c0de0001-0000-4000-8000-000000000104"
BALL = "c0de0001-0000-4000-8000-000000000204"
NOCENTRE = "0c0c0536-0000-4000-8000-000000000605"
ROUGH = {"feed": "1000.0", "plunge_feed": "300.0", "stepdown": "1.0", "stepover": "2.2225",
         "stock_to_leave": "0.3"}
FINISH = {"adaptive_spacing": "false"}
FACE = {"shape": '"end"', "stepdown": "0.5", "stepover": "1.0"}
OPS = [
    ("U", "Face end", SETUP1, "facing", FLAT, FACE),
    # Written at schema 25, where End facing stored the slice along X as `stepdown`; a build at
    # schema 28 or later reads it as Stepover (slice thickness along X) (decision 0547 item 3).
    ("V", "Face end, wide stepover", SETUP1, "facing", FLAT, {**FACE, "stepdown": "5.0"}),
    ("W", "Rough, deep stepdown", SETUP1, "rotary_roughing", FLAT, {**ROUGH, "stepdown": "30.0"}),
    ("X", "Rough rings, ramp", SETUP1, "rotary_roughing", FLAT,
     {**ROUGH, "pass_shape": '"rings"', "entry": '"ramp"', "ramp_angle": "3.0"}),
    ("Y", "Finish", SETUP1, "helical_finishing", BALL, FINISH),
    ("Z", "Finish, low clearance", SETUP2, "helical_finishing", BALL, FINISH),
    ("ZA", "Axial lead, through stock", SETUP3, "axial_finishing", NOCENTRE,
     {"entry": '"lead"', "direction": '"climb"', "extend": '"far end"',
      "face_free_ends": "false"}),
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
SETUP3_TOML = f"""schema = {oc.SCHEMA}
id = "{SETUP3}"
order = "X"
name = "Setup 3, cylinder in long stock"
part = "{CYLINDER_ID}"
material = "Hardwood"
part_offset = 0.0

[work_offset]
register = "g54"
x = 0.0

[stock]
type = "cylinder"
diameter = 24.0
length = 62.0
x_start = -10.0
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
    oc.write(out / "project.toml", project.replace(
        'name = "Baluster"', 'name = "Owner check 0536 step 6"') + CYLINDER_MODEL)
    setup = (out / f"setups/{SETUP1}.toml").read_text(encoding="utf-8")
    grip = "\n[workholding.start]\nlength = 8.0\n"
    if grip not in setup or 'name = "Setup 1"' not in setup:
        raise SystemExit("the tutorial's setup is not as expected")
    low = (setup.replace(SETUP1, SETUP2).replace('order = "V"', 'order = "W"')
           .replace('name = "Setup 1"', 'name = "Setup 2, low clearance"')
           .replace(grip, "\n[clearance]\nradius = 10.0\nsafe_z = 40.0\n" + grip))
    oc.write(out / f"setups/{SETUP2}.toml", low)
    oc.write(out / f"setups/{SETUP3}.toml", SETUP3_TOML)
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
        op = f"0c0c0536-0000-4000-8000-0000000006{i + 20:02d}"
        oc.write(out / f"operations/{op}.toml",
                 oc.operation_toml(op, order, name, setup_id, strategy, tool_id, params))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()

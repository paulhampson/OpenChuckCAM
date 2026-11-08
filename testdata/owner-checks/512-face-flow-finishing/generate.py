# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #512 (face-flow finishing, OP-28, decision 0525)
into a folder you name:

    python3 testdata/owner-checks/512-face-flow-finishing/generate.py OUT_FOLDER

It copies the starting project of the user guide's blisk examples
(testdata/examples/blisk-start/blisk-project) into OUT_FOLDER, which must not exist yet or must
be empty, and adds the sets and the operation of step 4 of the user guide's worked example
"Selection sets on a blisk":

- "Hub floor": face 20, the floor between two blades in the middle of the front view;
- "Floor guide": edge 65, where the blade above that floor meets it;
- "Blade wall": face 19, that blade's side;
- the operation "Floor finish": face-flow finishing with the 3 mm ball, cutting on Hub floor
  with Floor guide as its guide edge, Blade wall as a check set (Cut short), and 0.2 mm of
  stock to leave.

The project is not stored in the repository so as not to keep a second copy of the blisk.
Written for this repository (lane OWNERCHECKS); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import shutil
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

START = oc.REPO / "testdata/examples/blisk-start/blisk-project"
MODEL = "b1150000-0000-4000-8000-000000000001"
MODEL_HASH = "blake3:883a66d9e35b50c8b8ff2e5380d698a14566c0e7d14eb92efc8dfb225de1c616"
SETUP_ID = "b1150000-0000-4000-8000-000000000002"
BALL = "b1150000-0000-4000-8000-000000000102"
FLOOR = "0c0c0512-0000-4000-8000-000000000201"
GUIDE = "0c0c0512-0000-4000-8000-000000000202"
WALL = "0c0c0512-0000-4000-8000-000000000203"
OP = "0c0c0512-0000-4000-8000-000000000301"
SCHEMA = 27  # the starting project's schema


def at_schema(text):
    return text.replace(f"schema = {oc.SCHEMA}", f"schema = {SCHEMA}", 1)


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    out = Path(sys.argv[1]).resolve()
    if out.exists() and any(out.iterdir()):
        raise SystemExit(f"{out} is not empty")
    shutil.copytree(START, out, dirs_exist_ok=True)
    (out / "selections").mkdir()
    (out / "operations").mkdir()
    for set_id, order, name, kind, ids in (
        (FLOOR, "V", "Hub floor", "faces", [20]),
        (GUIDE, "W", "Floor guide", "edges", [65]),
        (WALL, "X", "Blade wall", "faces", [19]),
    ):
        oc.write(out / f"selections/{set_id}.toml",
                 at_schema(oc.selection_toml(set_id, order, name, MODEL, MODEL_HASH, kind, ids)))
    oc.write(out / f"operations/{OP}.toml",
             at_schema(oc.operation_toml(OP, "V", "Floor finish", SETUP_ID, "face_flow", BALL,
                                         {"stock_to_leave": "0.2"}, selections=[FLOOR, GUIDE],
                                         check=[WALL])))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()

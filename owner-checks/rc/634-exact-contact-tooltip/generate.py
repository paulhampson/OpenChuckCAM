# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #634 (the tooltip of "Exact STEP surface contact"):

    python3 owner-checks/rc/634-exact-contact-tooltip/generate.py

It writes project/ beside this script: the project of the #351/#257 check (the public STEP
fixture testdata/step/valid/shouldered_shaft.step, with "Axial, tapered ball" and "Pencil,
flat"), written by that check's generate.py, plus a third operation, "Helical, ball": helical
finishing with a 3.175 mm ball end mill. The setting has two tooltips, one for axial finishing and
pencil clean-up, one for helical, rest and simultaneous finishing; the project shows both.

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import importlib.util
import shutil
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent.parent / "351-257-exact-step-contact"
SETUP_ID = "0c0c0351-0000-4000-8000-000000000002"
TOOL = "0c0c0634-0000-4000-8000-000000000101"
OP = "0c0c0634-0000-4000-8000-000000000301"


def main():
    spec = importlib.util.spec_from_file_location("check351", SOURCE / "generate.py")
    check351 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(check351)
    check351.main()  # writes the #351/#257 project in its own folder, as committed
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(SOURCE / "project", out)
    text = (out / "project.toml").read_text(encoding="utf-8")
    oc.write(out / "project.toml", text.replace('name = "Owner check 351 and 257"',
                                                'name = "Owner check 634"'))
    oc.write(out / f"tools/{TOOL}.toml",
             oc.tool_toml(TOOL, "X", "3.175 mm ball (check 634)", 3.175, 12.0, "ball_end", 3))
    oc.write(out / f"operations/{OP}.toml",
             oc.operation_toml(OP, "X", "Helical, ball", SETUP_ID, "helical_finishing", TOOL,
                               {"adaptive_spacing": "false"}))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()

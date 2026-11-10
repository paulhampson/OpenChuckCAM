# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #633 (fixed by #769, decision 0551: the held region's
own end is not named as a shoulder):

    python3 owner-checks/633-no-step-note-outside-selection/generate.py

It writes project/ beside this script: the three owner-check projects issue #633 cites, put
together as three setups of one project on the public baluster of the user guide's tutorial
(each in 78 mm of 24 mm stock from X -13, the chuck holding 8 mm, X -13 to -5):

- "Setup 1, tenon side (check 555)": owner-checks/555-axial-into-tab, axial finishing
  "Tenon side" with a 60 degree V-bit, limited to the tenon's side, beside an 11 mm tab;
- "Setup 2, vase and whole part (check 548)": owner-checks/548-tab-length-limited-op, axial
  finishing "Vase only" (limited to the vase) and "Whole part" with a 6 mm ball, beside a 14 mm
  tab;
- "Setup 3, finish vase (check 547)": owner-checks/547-flute-check-outside-selection, "Rough"
  and helical finishing "Finish vase" (limited to the vase) with a short-flute ball, with tabs at
  both ends.

All three are generated from the public baluster only (their own generate.py say so); none comes
from a private project. The files are copied from those folders with new setup ids and orders,
so the operations are the same as in the original checks.

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
OLD_SETUP = "0c0c0000-0000-4000-8000-000000000002"
# (source folder, new setup id, setup order, setup name, {old op order: new op order},
#  {old selection name: new name}, {old tool order: new tool order})
SOURCES = [
    ("555-axial-into-tab", "0c0c0633-0000-4000-8000-000000000011", "V",
     "Setup 1, tenon side (check 555)", {"V": "V"}, {"Tenon side": "Tenon side"}, {"V": "V"}),
    ("548-tab-length-limited-op", "0c0c0633-0000-4000-8000-000000000012", "W",
     "Setup 2, vase and whole part (check 548)", {"V": "W", "W": "X"},
     {"Vase": "Vase (setup 2)"}, {"V": "W"}),
    ("547-flute-check-outside-selection", "0c0c0633-0000-4000-8000-000000000013", "X",
     "Setup 3, finish vase (check 547)", {"V": "Y", "W": "Z"},
     {"Vase": "Vase (setup 3)"}, {"V": "X", "W": "Y"}),
]


def renamed(text, key, mapping):
    for old, new in mapping.items():
        a = f'{key} = "{old}"\n'
        if a in text:
            return text.replace(a, f'{key} = "{new}"\n', 1)
    raise SystemExit(f"{key} not as expected in {text[:80]!r}")


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    first = oc.HERE / SOURCES[0][0] / "project"
    shutil.copytree(first, out, ignore=shutil.ignore_patterns("operations", "selections",
                                                              "setups", "tools"))
    project = (out / "project.toml").read_text(encoding="utf-8")
    name = project.splitlines()[1]
    oc.write(out / "project.toml", project.replace(name, 'name = "Owner check 633"', 1))
    for sub in ("operations", "selections", "setups", "tools"):
        (out / sub).mkdir()
    for folder, setup_id, order, setup_name, ops, sels, tools in SOURCES:
        src = oc.HERE / folder / "project"
        if (src / "project.toml").read_text(encoding="utf-8").split("\n", 2)[2] != \
                project.split("\n", 2)[2]:
            raise SystemExit(f"{folder}: not the same model as the others")
        setup = (src / f"setups/{OLD_SETUP}.toml").read_text(encoding="utf-8")
        setup = setup.replace(OLD_SETUP, setup_id).replace('order = "V"', f'order = "{order}"', 1)
        oc.write(out / f"setups/{setup_id}.toml",
                 setup.replace('name = "Setup 1"', f'name = "{setup_name}"', 1))
        for f in sorted((src / "operations").glob("*.toml")):
            text = renamed(f.read_text(encoding="utf-8").replace(OLD_SETUP, setup_id),
                           "order", ops)
            oc.write(out / f"operations/{f.name}", text)
        for f in sorted((src / "selections").glob("*.toml")):
            oc.write(out / f"selections/{f.name}",
                     renamed(f.read_text(encoding="utf-8"), "name", sels))
        for f in sorted((src / "tools").glob("*.toml")):
            oc.write(out / f"tools/{f.name}", renamed(f.read_text(encoding="utf-8"), "order",
                                                      tools))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()

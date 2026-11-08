# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #643 (decision 0548: a steep descent inside a
roughing pass is fed no faster than the plunge feed):

    python3 owner-checks/rc/643-roughing-descent-feed/generate.py

It writes project/ beside this script: the public test blisk testdata/stl/blisk.stl (X 0 to 50,
radius 20) centred on the rotary axis in a 42 mm cylinder of stock 52 mm long from X -1, no
workholding, with the user guide's tutorial machine, its 3.175 mm flat end mill and its Rough
operation (rotary roughing, plunge feed 300 mm/min). Decision 0548 measured this set-up: 193
roughing moves going down the tool axis faster than the plunge feed, up to 600 mm/min.

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
BLISK = oc.REPO / "testdata/stl/blisk.stl"
SETUP = "setups/57ad6798-2544-4dfb-90a0-caf67ec69484.toml"
FINISH = "operations/34e3aa5e-36cc-4614-be6d-ec39cf22f5f8.toml"


def blake3_of(data):
    try:
        import blake3
        return blake3.blake3(data).hexdigest()
    except ImportError:
        import blake3_pure
        return blake3_pure.hexdigest(data)


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(START, out)
    (out / FINISH).unlink()  # the check is about roughing only
    for old in (out / "models").iterdir():
        old.unlink()
    data = BLISK.read_bytes()
    digest = blake3_of(data)
    shutil.copyfile(BLISK, out / "models" / f"{digest[:32]}.stl")
    project = (out / "project.toml").read_text(encoding="utf-8")
    lines = []
    for line in project.splitlines():
        if line.startswith("path = "):
            line = f'path = "models/{digest[:32]}.stl"'
        elif line.startswith("hash = "):
            line = f'hash = "blake3:{digest}"'
        elif line.startswith('name = "baluster.stl"'):
            line = 'name = "blisk.stl"'
        elif line.startswith('name = "'):
            line = 'name = "Owner check 643"'
        lines.append(line)
    oc.write(out / "project.toml", "\n".join(lines) + "\n")
    setup = (out / SETUP).read_text(encoding="utf-8")
    head = setup[: setup.index("[stock]")]
    oc.write(out / SETUP, head + """[stock]
type = "cylinder"
diameter = 42.0
length = 52.0
x_start = -1.0
""")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()

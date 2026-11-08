# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #556 (pull request #560) into a folder you name:

    python3 testdata/owner-checks/556-zoom-large-selection/generate.py OUT_FOLDER

The project is too large to store in the repository (its model is about 25 MB), so it is written
when you run this script; the folder must not exist yet or must be empty, and it should be
outside the repository. It holds:

- a dense copy of the public baluster of the user guide's tutorial: the same turned profile as
  testdata/examples/baluster/generate.py, with 4,500 points round each ring instead of 48, so
  about 500,000 triangles (the size of the selection set in the report on #556);
- one setup (24 mm stock, the chuck holding 8 mm) and no operations;
- one selection set, "Everything but the ends", holding every triangle from X 0.5 to X 60.5:
  nearly the whole model.

Written for this repository (lane OWNERCHECKS); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used. Needs Python 3.8 or later and nothing else; it takes
about a minute.
"""
import importlib.util
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(HERE.parent))
import ownercheck as oc  # noqa: E402

N_THETA = 4500
MODEL = "0c0c0556-0000-4000-8000-000000000001"
SETUP = "0c0c0556-0000-4000-8000-000000000002"
SET = "0c0c0556-0000-4000-8000-000000000201"


def baluster_generator():
    spec = importlib.util.spec_from_file_location(
        "baluster_generate", oc.REPO / "testdata/examples/baluster/generate.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.N_THETA = N_THETA
    return module


def dense_stl(path):
    """Writes the dense baluster; returns the indices of the triangles from X 0.5 to 60.5."""
    gen = baluster_generator()
    tris = gen.main(str(path))
    tris = [t for t in tris if all(sum((t[a][k] - t[b][k]) ** 2 for k in range(3)) > 1e-12
                                   for a, b in ((0, 1), (1, 2), (0, 2)))]
    chunks = [b"OpenChuckCAM owner check 556: dense baluster".ljust(80, b"\0"),
              struct.pack("<I", len(tris))]
    side = []
    for i, t in enumerate(tris):
        n = gen.normal(t)
        chunks.append(struct.pack("<12fH", *n, *t[0], *t[1], *t[2], 0))
        if all(0.5 <= p[0] <= 60.5 for p in t):
            side.append(i)
    Path(path).write_bytes(b"".join(chunks))
    return len(tris), side


def blake3_of(path):
    """The model's hash as the project records it: with the `blake3` package when it is
    installed, otherwise in plain Python (about a minute for this model)."""
    data = Path(path).read_bytes()
    try:
        import blake3
    except ImportError:
        import blake3_pure
        print("working out the model's hash (about a minute) ...")
        return "blake3:" + blake3_pure.hexdigest(data)
    return "blake3:" + blake3.blake3(data).hexdigest()


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    out = Path(sys.argv[1]).resolve()
    if out.exists() and any(out.iterdir()):
        raise SystemExit(f"{out} is not empty")
    for sub in ("models", "setups", "selections"):
        (out / sub).mkdir(parents=True, exist_ok=True)
    stl = out / "models/dense-baluster.stl"
    count, side = dense_stl(stl)
    digest = blake3_of(stl)
    oc.write(out / "project.toml", f"""schema = {oc.SCHEMA}
name = "Owner check 556"

[[models]]
id = "{MODEL}"
path = "models/dense-baluster.stl"
hash = "{digest}"
name = "dense-baluster.stl"

[[models.transform]]
kind = "centre_on_rotary_axis"
""")
    (out / "machine.toml").write_bytes(oc.BALUSTER_MACHINE.read_bytes())
    oc.write(out / f"setups/{SETUP}.toml", f"""schema = {oc.SCHEMA}
id = "{SETUP}"
order = "V"
name = "Setup 1"
part = "{MODEL}"
material = "Hardwood"
part_offset = 0.0

[work_offset]
register = "g54"
x = 0.0

[stock]
type = "cylinder"
diameter = 24.0
length = 63.0
x_start = -1.0

[workholding.start]
length = 8.0
""")
    oc.write(out / f"selections/{SET}.toml",
             oc.selection_toml(SET, "V", "Everything but the ends", MODEL, digest, "triangles",
                               side))
    print(f"wrote {out}: {count} triangles, {len(side)} in the set 'Everything but the ends'")


if __name__ == "__main__":
    main()

# SPDX-License-Identifier: AGPL-3.0-or-later
"""Shared helpers for the owner-check project generators in testdata/owner-checks/.

Each <ticket>/generate.py imports this module and writes a small project folder from public
models of this repository only (the baluster of testdata/examples/baluster, and the STEP files of
testdata/step/valid). The project files are written as text with fixed ids, so the same script
always writes the same bytes. Written for this repository (lane OWNERCHECKS); AGPL-3.0-or-later,
as the rest of the repository.

The files are written at project schema 25 where the check allows it, so that the build from
before each fix can open the same project: a newer build reads an older schema and upgrades it
when it saves.
"""
import hashlib
import shutil
import struct
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
BALUSTER = REPO / "testdata/examples/baluster/baluster.stl"
BALUSTER_MACHINE = REPO / "testdata/examples/baluster/baluster-project/machine.toml"


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def stl_triangles(path):
    """The triangles of a binary STL file, in file order, as three (x, y, z) corners each."""
    data = Path(path).read_bytes()
    (count,) = struct.unpack_from("<I", data, 80)
    tris = []
    for i in range(count):
        values = struct.unpack_from("<12f", data, 84 + 50 * i)
        tris.append((values[3:6], values[6:9], values[9:12]))
    return tris


def triangles_in_x(path, x_from, x_to):
    """Indices of the triangles whose three corners all lie within X x_from to x_to (model frame)."""
    return [
        i
        for i, t in enumerate(stl_triangles(path))
        if all(x_from <= c[0] <= x_to for c in t)
    ]


def fresh(folder):
    folder = Path(folder)
    if folder.exists():
        shutil.rmtree(folder)
    for sub in ("models", "setups", "tools", "operations", "selections"):
        (folder / sub).mkdir(parents=True)
    return folder


def write(path, text):
    Path(path).write_text(text, encoding="utf-8", newline="\n")


def toml_list(values, per_line=16):
    """A TOML array of integers, wrapped so the file stays readable in a diff."""
    values = list(values)
    if len(values) <= per_line:
        return "[" + ", ".join(str(v) for v in values) + "]"
    lines = []
    for i in range(0, len(values), per_line):
        lines.append("    " + ", ".join(str(v) for v in values[i : i + per_line]) + ",")
    return "[\n" + "\n".join(lines) + "\n]"


SCHEMA = 25
BALUSTER_SHA256 = "5a83e91be1e7f5afb487ad77a5fcf98605455285ecabd2ed2076917febf85a3f"
BALUSTER_BLAKE3 = "419712af3f46aa8fa43fea3f17e544c681583a620f009f62466e13887d9304ff"


def tool_toml(tool_id, order, name, diameter, flute_length, shape, number, overall=40.0,
              shank=None, extra_shape="", spindle=10000.0, feed_per_tooth=0.01, stepover=0.5):
    """One tool snapshot with cutting data for Hardwood, the stock material of these projects."""
    shank = diameter if shank is None else shank
    return f"""schema = {SCHEMA}
id = "{tool_id}"
order = "{order}"
name = "{name}"

[tool]
diameter = {diameter}
flute_length = {flute_length}
overall_length = {overall}
shank_diameter = {shank}
flute_count = 2
material = "carbide"

[tool.shape]
type = "{shape}"
{extra_shape}
[controller]
number = {number}

[cutting_data.Hardwood]
spindle_speed = {spindle}
feed_per_tooth = {feed_per_tooth}
plunge_feed = 200.0
stepover = {stepover}
stepdown = 1.0
"""


def operation_toml(op_id, order, name, setup_id, strategy, tool_id, parameters,
                   selections=(), avoid=(), check=()):
    """One operation. `parameters` maps a parameter name to its TOML value text."""
    sel = ", ".join(f'"{s}"' for s in selections)
    text = f"""schema = {SCHEMA}
id = "{op_id}"
order = "{order}"
name = "{name}"
setup = "{setup_id}"
strategy = "{strategy}"
tool = "{tool_id}"
selections = [{sel}]
"""
    if avoid:
        text += "avoid = [" + ", ".join(f'"{s}"' for s in avoid) + "]\n"
    if check:
        text += "check = [" + ", ".join(f'"{s}"' for s in check) + "]\n"
    text += "\n[parameters]\n"
    for key in sorted(parameters):
        text += f"{key} = {parameters[key]}\n"
    return text


def selection_toml(set_id, order, name, model_id, model_hash, kind, ids):
    """A selection set of mesh triangles (kind "triangles") or STEP faces (kind "faces")."""
    return f"""schema = {SCHEMA}
id = "{set_id}"
order = "{order}"
name = "{name}"
model = "{model_id}"
model_hash = "{model_hash}"
descriptors = []

[target]
{kind} = {toml_list(sorted(ids))}
"""


def baluster_project(folder, name, setup_toml, tools, operations, selections=()):
    """Writes a project on the public baluster of the user guide's tutorial.

    `setup_toml` is the text of the setup file after its header (stock and workholding);
    `tools`, `operations` and `selections` are lists of (id, text)."""
    if sha256(BALUSTER) != BALUSTER_SHA256:
        raise SystemExit(f"{BALUSTER} is not the expected baluster model")
    folder = fresh(folder)
    model_file = f"models/{BALUSTER_BLAKE3[:32]}.stl"
    shutil.copyfile(BALUSTER, folder / model_file)
    write(folder / "project.toml", f"""schema = {SCHEMA}
name = "{name}"

[[models]]
id = "{BALUSTER_MODEL}"
path = "{model_file}"
hash = "blake3:{BALUSTER_BLAKE3}"
name = "baluster.stl"

[[models.transform]]
kind = "centre_on_rotary_axis"
""")
    shutil.copyfile(BALUSTER_MACHINE, folder / "machine.toml")
    write(folder / f"setups/{BALUSTER_SETUP}.toml", f"""schema = {SCHEMA}
id = "{BALUSTER_SETUP}"
order = "V"
name = "Setup 1"
part = "{BALUSTER_MODEL}"
material = "Hardwood"
part_offset = 0.0

[work_offset]
register = "g54"
x = 0.0

{setup_toml}""")
    for tool_id, text in tools:
        write(folder / f"tools/{tool_id}.toml", text)
    for op_id, text in operations:
        write(folder / f"operations/{op_id}.toml", text)
    for set_id, text in selections:
        write(folder / f"selections/{set_id}.toml", text)
    for sub in ("operations", "selections"):
        if not any((folder / sub).iterdir()):
            (folder / sub).rmdir()
    return folder


BALUSTER_MODEL = "0c0c0000-0000-4000-8000-000000000001"
BALUSTER_SETUP = "0c0c0000-0000-4000-8000-000000000002"
BALUSTER_MODEL_HASH = f"blake3:{BALUSTER_BLAKE3}"


def side_triangles_in_x(path, x_from, x_to):
    """As triangles_in_x, but only the triangles of the turned side: those whose normal is
    within 60 degrees of square to the rotary axis (X), leaving out end faces and shoulders."""
    picked = []
    for i, (a, b, c) in enumerate(stl_triangles(path)):
        if not all(x_from <= p[0] <= x_to for p in (a, b, c)):
            continue
        u = [b[k] - a[k] for k in range(3)]
        v = [c[k] - a[k] for k in range(3)]
        n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
        length = sum(x * x for x in n) ** 0.5
        if length > 0 and abs(n[0]) / length < 0.5:
            picked.append(i)
    return picked


def step_project(folder, name, source, source_sha256, model_blake3, model_id, setup_id,
                 transforms, setup_toml, tools, operations, selections=(), schema=SCHEMA):
    """Writes a project on one of the public STEP files of testdata/step/valid, copied into the
    project's models folder. `transforms` is the text of the [[models.transform]] tables;
    the rest as for baluster_project."""
    source = REPO / source
    if sha256(source) != source_sha256:
        raise SystemExit(f"{source} is not the expected model")
    folder = fresh(folder)
    model_file = f"models/{source.name}"
    shutil.copyfile(source, folder / model_file)
    write(folder / "project.toml", f"""schema = {schema}
name = "{name}"

[[models]]
id = "{model_id}"
path = "{model_file}"
hash = "blake3:{model_blake3}"
name = "{source.name}"
{transforms}""")
    shutil.copyfile(BALUSTER_MACHINE, folder / "machine.toml")
    write(folder / f"setups/{setup_id}.toml", f"""schema = {schema}
id = "{setup_id}"
order = "V"
name = "Setup 1"
part = "{model_id}"
material = "Hardwood"
part_offset = 0.0

[work_offset]
register = "g54"
x = 0.0

{setup_toml}""")
    for tool_id, text in tools:
        write(folder / f"tools/{tool_id}.toml", text)
    for op_id, text in operations:
        write(folder / f"operations/{op_id}.toml", text)
    for set_id, text in selections:
        write(folder / f"selections/{set_id}.toml", text)
    for sub in ("operations", "selections"):
        if not any((folder / sub).iterdir()):
            (folder / sub).rmdir()
    return folder

# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the zip of each owner check: <folder>/<folder>.zip, holding <folder>/README.txt and
<folder>/project/ (the finished project, so nothing needs to be run to open it):

    python3 owner-checks/make_zips.py [FOLDER ...]

A FOLDER is relative to owner-checks/, such as 600-feed-limit-summary or rc/636-open-no-unconfirmed-dialog
(the release-candidate drafts).

Without arguments it writes every folder's zip. A folder whose project/ is not stored (the
model is too large) has it written first by its generate.py. The zips are deterministic: entries
sorted, every time stamp 1980-01-01 00:00, fixed permissions, deflate at level 9, no folder
entries, no hidden files. Written for this repository (lane OWNERCHECKS); AGPL-3.0-or-later.
"""
import subprocess
import sys
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ALLOWED = {".toml", ".step", ".stl", ".md", ".py", ".txt"}


def files_of(folder):
    out = [folder / "README.txt"]
    out += sorted(p for p in (folder / "project").rglob("*") if p.is_file())
    for p in out:
        if p.suffix.lower() not in ALLOWED or any(part.startswith(".") for part in p.parts):
            raise SystemExit(f"{p}: not allowed in an owner-check zip")
        if p.is_symlink():
            raise SystemExit(f"{p}: a link")
    return out


def make(folder):
    if not (folder / "project").is_dir():
        subprocess.run([sys.executable, str(folder / "generate.py")], check=True)
    target = folder / f"{folder.name}.zip"
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for path in files_of(folder):
            name = f"{folder.name}/{path.relative_to(folder).as_posix()}"
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            info.create_system = 3
            z.writestr(info, path.read_bytes(), compresslevel=9)
    print(f"{target.relative_to(HERE)}: {target.stat().st_size} bytes")


def main():
    names = sys.argv[1:] or sorted(
        p.relative_to(HERE).as_posix() for p in [*HERE.iterdir(), *(HERE / "rc").glob("*")]
        if p.is_dir() and (p / "generate.py").exists())
    for name in names:
        make(HERE / name)


if __name__ == "__main__":
    main()

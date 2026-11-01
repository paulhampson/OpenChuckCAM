# SPDX-License-Identifier: AGPL-3.0-or-later
"""The SHA-256 manifest of a released documentation folder (decision 0400 section V).

    python3 user-docs/tools/manifest.py write DIR
    python3 user-docs/tools/manifest.py verify DIR

The manifest lists every file of the folder except itself, one per line as `sha256sum` writes it
(`<hex>  <path>`), sorted by path, so the same folder always gives the same manifest and
`sha256sum -c MANIFEST.sha256` also checks it.
"""
import hashlib
import os
import sys

NAME = "MANIFEST.sha256"


def _files(folder):
    out = []
    for root, dirs, files in os.walk(folder):
        dirs.sort()
        for name in files:
            rel = os.path.relpath(os.path.join(root, name), folder).replace(os.sep, "/")
            if rel != NAME:
                out.append(rel)
    return sorted(out)


def _digest(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 16), b""):
            h.update(block)
    return h.hexdigest()


def text(folder):
    return "".join(f"{_digest(os.path.join(folder, rel))}  {rel}\n" for rel in _files(folder))


def write(folder):
    body = text(folder)
    with open(os.path.join(folder, NAME), "w", encoding="utf-8", newline="\n") as f:
        f.write(body)
    return hashlib.sha256(body.encode()).hexdigest()


def verify(folder):
    """Problems with the folder against its manifest; empty when it matches exactly."""
    path = os.path.join(folder, NAME)
    if not os.path.exists(path):
        return [f"{folder}: no {NAME}"]
    expected = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            digest, _, rel = line.rstrip("\n").partition("  ")
            expected[rel] = digest
    actual = _files(folder)
    problems = [f"{rel}: listed in {NAME} but missing" for rel in sorted(set(expected) - set(actual))]
    problems += [f"{rel}: not listed in {NAME}" for rel in actual if rel not in expected]
    problems += [f"{rel}: changed since it was released" for rel in actual
                 if rel in expected and _digest(os.path.join(folder, rel)) != expected[rel]]
    return problems


def main(argv):
    if len(argv) != 3 or argv[1] not in ("write", "verify"):
        print(__doc__)
        return 2
    if argv[1] == "write":
        print(f"{NAME} written; its SHA-256 is {write(argv[2])}")
        return 0
    problems = verify(argv[2])
    for p in problems:
        print(f"error: {p}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

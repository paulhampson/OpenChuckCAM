#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""The F-word diff check of decision record 0647.

For every golden program changed between two commits, strips each F word and
compares the rest byte for byte: only F words may change. Prints, per file,
the fed blocks (lines with an F word or in G1 mode that move) and how many of
them changed their F, and exits non-zero if anything else changed.

Usage: python3 fdiff.py <old-commit> [<new-commit>]   (new defaults to the
working tree). Run from the repository root.
"""
import re
import subprocess
import sys

F = re.compile(r" F[0-9.]+")


def show(commit, path):
    if commit is None:
        with open(path, encoding="utf-8") as f:
            return f.read()
    return subprocess.run(
        ["git", "show", f"{commit}:{path}"], check=True, capture_output=True, text=True
    ).stdout


def main():
    old = sys.argv[1]
    new = sys.argv[2] if len(sys.argv) > 2 else None
    args = ["git", "diff", "--name-only", old] + ([new] if new else []) + ["--", "testdata/golden"]
    files = subprocess.run(args, check=True, capture_output=True, text=True).stdout.split()
    bad = 0
    print(f"{'file':55} {'fed':>7} {'F changed':>10}  other changes")
    for path in sorted(files):
        a, b = show(old, path).splitlines(), show(new, path).splitlines()
        if len(a) != len(b):
            print(f"{path:55} line count {len(a)} -> {len(b)}")
            bad += 1
            continue
        fed = sum(1 for line in b if F.search(line))
        changed = sum(1 for x, y in zip(a, b) if x != y and F.sub("", x) == F.sub("", y))
        other = [i + 1 for i, (x, y) in enumerate(zip(a, b)) if F.sub("", x) != F.sub("", y)]
        bad += bool(other)
        print(f"{path:55} {fed:>7} {changed:>10}  {other[:5] if other else 'none'}")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()

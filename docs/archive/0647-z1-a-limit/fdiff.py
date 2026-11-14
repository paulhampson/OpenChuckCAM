#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""The F-word diff check of decision record 0647.

For every golden program changed between two commits, strips each F word and
compares the rest line by line: only F words, the estimate in an operation's
comment and the merging of a pure-A block into the next (decision 0506) may
change. Prints, per file, the fed blocks (G1 lines that move), the fed blocks
whose F in force changed (F is modal), the estimate comments that changed and the merged pure-A blocks,
and exits non-zero if anything else changed.

Usage: python3 fdiff.py <old-commit> [<new-commit>]   (new defaults to the
working tree). Run from the repository root.
"""
import difflib
import re
import subprocess
import sys

F = re.compile(r" F[0-9.]+")
# The estimate a post writes in an operation's comment, "(operation ..., about 1923 s)".
ESTIMATE = re.compile(r", about [0-9]+ s\)$")
PURE_A = re.compile(r"^A-?[0-9.]+$")


def strip(line):
    return ESTIMATE.sub(", about <t> s)", F.sub("", line))


def compare(a, b):
    """Positions of difference in the F-stripped, estimate-masked texts, after
    allowing one kind of change: a pure-A block (A alone) followed by another
    pure-A block may be merged into it (decision 0506 writes a run of pure-A
    moves with the same F as one block), which reaches the same positions."""
    a, b = [strip(x) for x in a], [strip(x) for x in b]
    merged, other = [], []
    sm = difflib.SequenceMatcher(None, a, b, autojunk=False)
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == "equal":
            continue
        if tag == "delete" and all(PURE_A.match(a[i]) for i in range(i1, i2)) and i2 < len(a) and PURE_A.match(a[i2]):
            merged.extend(range(i1 + 1, i2 + 1))
            continue
        other.append((tag, i1 + 1, j1 + 1))
    return merged, other


def fed_blocks(lines):
    """The lines in G1 mode that move (an X, Y, Z or A word), each with the F
    in force for it (F is modal), as (F-stripped line, F)."""
    mode, f, out = None, None, []
    for line in lines:
        if line.startswith("("):
            continue
        words = line.split()
        f = next((w[1:] for w in words if w.startswith("F")), f)
        if "G0" in words:
            mode = "G0"
        if "G1" in words:
            mode = "G1"
        if mode == "G1" and "G92.4" not in words and any(w[0] in "XYZA" for w in words):
            out.append((strip(line), float(f)))
    return out


def f_changes(a, b):
    """Fed blocks of `b` whose F in force differs from the same block's in `a`."""
    fa, fb = fed_blocks(a), fed_blocks(b)
    sm = difflib.SequenceMatcher(None, [x for x, _ in fa], [x for x, _ in fb], autojunk=False)
    n = 0
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == "equal":
            n += sum(1 for (_, x), (_, y) in zip(fa[i1:i2], fb[j1:j2]) if x != y)
    return n, len(fb)


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
    files = [
        f
        for f in subprocess.run(args, check=True, capture_output=True, text=True).stdout.split()
        if f.endswith((".nc", ".ngc"))
    ]
    bad = 0
    print(f"{'file':52} {'fed blocks':>10} {'F changed':>10} {'estimates':>9} {'merged A':>8}  other")
    for path in sorted(files):
        a, b = show(old, path).splitlines(), show(new, path).splitlines()
        merged, other = compare(a, b)
        estimates = sum(1 for x, y in zip(a, b) if x != y and ESTIMATE.search(x) and F.sub("", x) != F.sub("", y))
        changed, fed = f_changes(a, b)
        bad += bool(other)
        print(f"{path:52} {fed:>10} {changed:>10} {estimates:>9} {len(merged):>8}  {other[:5] if other else 'none'}")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()

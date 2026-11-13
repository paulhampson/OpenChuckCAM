# SPDX-License-Identifier: AGPL-3.0-or-later
"""SP-14 (throwaway): automated aid for the K1 to K3 reading of the split
block-rate files (decision 0232, instructions 4.8). It does not replace the
owner's reading by hand, and it says nothing about safety (NFR-12).

Run from this directory, with the full file written beside it first:
  python3 -I ../gen_block_rate.py > ../block_rate_z1.nc
  python3 -I check_split.py ../block_rate_z1.nc block_rate_z1_s*.nc
"""
import hashlib
import re
import sys

HEADER = ["(SP-14 Z1 block-rate air cut - decision 0222)",
          "(NO TOOL, NO MATERIAL. Attended. E-stop in reach.)",
          "G90 G21 G17", "G54", "G0 X0.000 Y0.000 Z50.000 A0.000"]
FOOTER = ["G0 Z50.000", "G92.4 A0 S0", "G53 G0 Z-3", "G28", "M2"]
FORBIDDEN = re.compile(r"T[0-9]|M6|S[0-9]|M3|G93|G4")
G1 = re.compile(r"^G1 X(-?[0-9]+\.[0-9]{3}) Y0\.000 Z50\.000 A(-?[0-9]+\.[0-9]{3}) F1000\.0$")
SEC = re.compile(r"^\(section (\d+): part ([AB]), (\d+) ms blocks, (\d+) lines\)$")


def sections(lines):
    out, cur = {}, None
    for ln in lines:
        m = SEC.match(ln)
        if m:
            cur = int(m.group(1))
            out[cur] = [ln]
        elif ln == "G0 Z50.000":
            cur = None
        elif cur is not None:
            out[cur].append(ln)
    return out


def check(path, full_secs):
    lines = open(path, encoding="ascii").read().split("\n")
    if lines[-1] == "":
        lines.pop()
    bad = []
    if lines[:5] != HEADER:
        bad.append("header lines 1-5 differ from the full file's")
    if not re.match(r"^G0 X(0|100)\.000 Y0\.000 Z50\.000 A-?[0-9]+\.[0-9]{3}$", lines[5]):
        bad.append(f"line 6 is not a G0 to the section start: {lines[5]}")
    if lines[-5:] != FOOTER:
        bad.append("footer differs")
    hits = [(i + 1, ln) for i, ln in enumerate(lines) if FORBIDDEN.search(ln)]
    if hits != [(len(lines) - 3, "G92.4 A0 S0")]:
        bad.append(f"forbidden-word grep found {hits}")
    secs = sections(lines)
    if len(secs) != 1:
        bad.append(f"{len(secs)} sections, expected 1")
    (n, body), = secs.items()
    m = SEC.match(body[0])
    part, ms, count = m.group(2), int(m.group(3)), int(m.group(4))
    if body != full_secs[n]:
        bad.append("section body differs from the same section of the full file")
    if body[1:3] != ["G1 Z55.000 F1000.0", "G1 Z50.000 F1000.0"]:
        bad.append("marker lines differ")
    moves = body[3:]
    if len(moves) != count:
        bad.append(f"{len(moves)} G1 lines, header says {count}")
    x0 = float(re.search(r"X(-?[0-9.]+)", lines[5]).group(1))
    a0 = float(re.search(r"A(-?[0-9.]+)", lines[5]).group(1))
    xs, as_ = [x0], [a0]
    for ln in moves:
        g = G1.match(ln)
        if not g:
            bad.append(f"not a plain G1 line: {ln}")
            break
        xs.append(float(g.group(1)))
        as_.append(float(g.group(2)))
    steps = [abs(b - a) for a, b in zip(xs, xs[1:])]
    if min(xs) < 0 or max(xs) > 100:
        bad.append("X leaves 0..100")
    if abs(abs(xs[-1] - xs[0]) - 100.0) > 1e-9:
        bad.append("X does not travel 100 mm")
    length = 100.0 / count
    if max(abs(s - length) for s in steps) > 0.0011:
        bad.append("X step differs from the block length by more than rounding")
    da = [b - a for a, b in zip(as_, as_[1:])]
    if part == "A" and any(abs(d) > 0 for d in da):
        bad.append("A changes in part A")
    if part == "B" and abs((as_[-1] - as_[0]) / 100.0 - 0.6283) > 0.001:
        bad.append(f"part B A rate {(as_[-1] - as_[0]) / 100:.4f} deg/mm, expected about 0.628")
    longest = max(len(ln) for ln in lines)
    if longest > 63:
        bad.append(f"line of {longest} characters")
    sha = hashlib.sha256(open(path, "rb").read()).hexdigest()
    status = "PASS" if not bad else "FAIL: " + "; ".join(bad)
    print(f"| {path} | {n} | {part} | {ms} | {count} | {len(lines)} | "
          f"{sum(len(l) + 1 for l in lines)} | {longest} | X {xs[0]:.3f}->{xs[-1]:.3f}, "
          f"A {as_[0]:.3f}->{as_[-1]:.3f} | {length * 60 / 1000 * 1000 / 1000:.4f} s x {count} | "
          f"{status} | {sha[:16]} |")
    return not bad


def main(argv):
    full = open(argv[1], encoding="ascii").read().split("\n")
    full_secs = sections(full)
    print("| File | Section | Part | Block ms | G1 blocks | Lines | Bytes | Longest line | "
          "Travel | Nominal block | Automated K1-K3 | SHA-256 (first 16) |")
    print("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    ok = all([check(p, full_secs) for p in argv[2:]])
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main(sys.argv)

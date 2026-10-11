# SPDX-License-Identifier: AGPL-3.0-or-later
"""Spike (throwaway): compare simulator reports with the SP-3 files.

usage: python3 -I analyse.py out/sim_sp3_g94.json fixtures/sp3_g94.ngc IR_CUT_SECONDS

For one SP-3 file prints (1) how many of the file's nodes fall outside the
window between the last sample of a line and the first sample of the next
(the sampled commanded position lags a node by up to one 1 ms sample, so a few
misses at the sample step are expected), (2) the final position, and (3) the
elapsed time from the start of the first cut line to the end of the last,
against the IR's cut time (SP-3 `cut_time_ir`).
"""
import json
import math
import re
import sys

WORD = re.compile(r"([A-Z])\s*([-+]?[0-9]*\.?[0-9]+)")


def parse(path):
    pos = dict(X=0, Y=0, Z=0, A=0)
    mode = None
    tgt = {}
    for i, raw in enumerate(open(path, encoding="ascii").read().splitlines(), 1):
        line = re.sub(r"\(.*?\)|;.*", "", raw).upper()
        t = dict(pos)
        for a, v in WORD.findall(line):
            v = float(v)
            if a == "G" and v in (0, 1):
                mode = int(v)
            elif a in t:
                t[a] = v
        if t != pos:
            tgt[i] = dict(mode=mode, t=[t[k] for k in "XYZA"])
        pos = t
    return tgt


def main(report, ngc, ir_seconds):
    r = json.load(open(report))
    tgt = parse(ngc)
    rows = sorted((x for x in r["rows"] if x["line"] in tgt), key=lambda x: x["line"])
    miss, worst = 0, 0.0
    for a, b in zip(rows, rows[1:]):
        t = tgt[a["line"]]["t"]
        for k in range(4):
            lo = min(a["end"][k], b["first"][k]) - 0.002
            hi = max(a["end"][k], b["first"][k]) + 0.002
            if not lo <= t[k] <= hi:
                miss += 1
                worst = max(worst, min(abs(t[k] - lo), abs(t[k] - hi)))
    cut = [x for x in rows if tgt[x["line"]]["mode"] == 1 and abs(tgt[x["line"]]["t"][2] - x["first"][2]) <= 1e-3]
    elapsed = cut[-1]["t1"] - cut[0]["t0"]
    print(f"{ngc}: {len(rows)} moving lines, errors {r['errors']}")
    print(f"  node checks outside the sample window: {miss} of {4 * (len(rows) - 1)} (worst {worst:.4f})")
    print("  final position: " + " ".join(f"{v:.4f}" for v in r["final_position"]))
    print(f"  cut time (first to last cut line, {len(cut)} lines): {elapsed:.3f} s against IR {ir_seconds:.3f} s ({(elapsed / ir_seconds - 1) * 100:+.2f} %)")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], float(sys.argv[3]))

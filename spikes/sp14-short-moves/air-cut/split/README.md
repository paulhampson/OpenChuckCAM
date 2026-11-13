# SP-14: the block-rate air cut split into one file per section

Throwaway spike material on `spike/sp-14-split-files`. It is never merged.

**Nothing here is verified safe.** These programs are written by a script, not
by the application's post. Read every file yourself before it goes near the
machine, and run it only as an attended air cut under the gate 4 rules: no
tool, no material, emergency stop in reach (NFR-12). The automated checks below
help with that reading. They do not replace it.

## Why

On firmware 1.1.3 (ESP32 1.1.3, WiFi upload) the Z1 refused to run the full
block-rate program `../block_rate_z1.nc`, which is 1,170,259 bytes and 27,966
lines, in every section. On firmware 1.1.2 the same program ran.

The open-source LPC firmware is the same in 1.1.2 and 1.1.3 for file play
(Player.cpp, SerialConsole.cpp). It has no limit on file size or line count.
The file is stored on the ESP32 board, whose firmware is closed source, so a
size limit there can be neither confirmed nor ruled out.

These files keep each section the same and make each file small, so that a
size limit and a content problem can be told apart.

## What the files are

`block_rate_z1_sN.nc` holds section N of the full program, and nothing else
from it:

- the full program's header (lines 1 to 5);
- one extra line, `G0 X<start> Y0.000 Z50.000 A<start>`, a rapid to the point
  where section N starts in the full program;
- section N exactly as in the full program, byte for byte: its marker
  `G1 Z55.000` / `G1 Z50.000` and its G1 blocks;
- the full program's footer: `G0 Z50.000`, `G92.4 A0 S0`, `G53 G0 Z-3`, `G28`,
  `M2`.

Block times, F1000, block lengths and coordinates are the same as in the full
program, so the results compare directly with the 1.1.2 trial (OI-930).

Two things differ from the full program:

- In s9 to s14 the extra G0 also turns A from 0 to the section's starting A,
  between 62.762 and 376.570 degrees, at Z50.
- Each file ends with the footer: `G92.4 A0 S0` unwinds A, then `G53 G0 Z-3`
  and `G28` home the machine.

| File | Part | Block time | G1 blocks | Lines | Bytes |
| --- | --- | --- | --- | --- | --- |
| block_rate_z1_s1.nc | A (X only) | 20 ms | 300 | 314 | 12,573 |
| block_rate_z1_s2.nc | A | 10 ms | 600 | 614 | 24,843 |
| block_rate_z1_s3.nc | A | 7 ms | 857 | 871 | 35,353 |
| block_rate_z1_s4.nc | A | 5 ms | 1,200 | 1,214 | 49,383 |
| block_rate_z1_s5.nc | A | 3 ms | 2,000 | 2,014 | 82,103 |
| block_rate_z1_s6.nc | A | 2 ms | 3,000 | 3,014 | 123,003 |
| block_rate_z1_s7.nc | A | 1 ms | 6,000 | 6,014 | 245,703 |
| block_rate_z1_s8.nc | B (X and A) | 20 ms | 300 | 314 | 12,826 |
| block_rate_z1_s9.nc | B | 10 ms | 600 | 614 | 25,689 |
| block_rate_z1_s10.nc | B | 7 ms | 857 | 871 | 37,070 |
| block_rate_z1_s11.nc | B | 5 ms | 1,200 | 1,214 | 51,786 |
| block_rate_z1_s12.nc | B | 3 ms | 2,000 | 2,014 | 86,106 |
| block_rate_z1_s13.nc | B | 2 ms | 3,000 | 3,014 | 129,006 |
| block_rate_z1_s14.nc | B | 1 ms | 6,000 | 6,014 | 257,706 |

The longest line in every file is 50 characters, under the firmware's
63-character limit. Each section is 100 mm of X at F1000, 6.0 s nominal.

## How they are made

From `spikes/sp14-short-moves/air-cut/`:

```
python3 -I gen_block_rate.py > block_rate_z1.nc                 # full program, unchanged
for N in 1 2 3 4 5 6 7 8 9 10 11 12 13 14; do
  python3 -I gen_block_rate.py $N > split/block_rate_z1_s$N.nc
done
```

With no argument the generator writes `block_rate_z1.nc` byte for byte as at
commit 69d532f. That was checked with `cmp` against
`git show 69d532f:spikes/sp14-short-moves/air-cut/block_rate_z1.nc`.

## Checks (decision 0232, instructions section 4.8, K1 to K4, adapted per file)

Decision 0232 item 2 says "A change to the program sends it back to step 1",
and item 3 says "No other program gains the exception". These files are a
changed program. Before they run on the Z1, the owner decides whether 0232
covers them once K1 to K4 pass for each file, or whether they need a record of
their own.

- **K1 (fetch):** `git fetch origin spike/sp-14-split-files`, then
  `git show <commit>:spikes/sp14-short-moves/air-cut/split/block_rate_z1_sN.nc > block_rate_z1_sN.nc`.
  Check the line count in the table and the first line,
  `(SP-14 Z1 block-rate air cut - decision 0222)`.
- **K2 (header and footer):** lines 1 to 5 as in the full program; line 6 is
  the G0 to the section start; the last 5 lines are the footer.
  `grep -nE 'T[0-9]|M6|S[0-9]|M3|G93|G4' block_rate_z1_sN.nc` finds only the
  `S0` of `G92.4 A0 S0`.
- **K3 (the section):** it starts `G1 Z55.000 F1000.0`, `G1 Z50.000 F1000.0`.
  Every other line is `G1 X.. Y0.000 Z50.000 A.. F1000.0`. X steps by the
  block length and stays within 0 to 100. In part A, A stays put. In part B,
  A grows by about 0.628 degrees per mm of X.
- **K4 (gate 3):** make the LinuxCNC copy without the Z1 footer and replay it:
  `sed -e '/^G92.4 A0 S0$/,$d' block_rate_z1_sN.nc > block_rate_sN_linuxcnc.ngc && printf 'M5\nM30\n' >> block_rate_sN_linuxcnc.ngc`,
  then `cargo xtask replay --docker openchuck-linuxcnc block_rate_sN_linuxcnc.ngc`.
  Ten of the fourteen copies made this way fail at the corner between the two
  leading G0 lines: LinuxCNC blends it because the copy has no `G61`. Adding
  `G61` after `G54` in the LinuxCNC copy makes all fourteen pass. See
  `K4-RESULTS.md`; whether that copy is accepted for K4 is the owner's call.

`check_split.py` automates the K1 to K3 comparisons. Every file must be one
section, its section body byte-identical to the same section of the full
program, with the header, footer, forbidden-word grep, line form, X range,
X step, A behaviour and line length all as above:

```
python3 -I gen_block_rate.py > block_rate_z1.nc
cd split && python3 -I check_split.py ../block_rate_z1.nc block_rate_z1_s*.nc
```

The K4 results of the session that wrote these files are in
`K4-RESULTS.md`, with what could and could not be run.

## Suggested order on the machine

1. Before anything else, record the exact message the controller or Makera
   Studio shows when the full file is refused.
2. Home the machine.
3. Upload `block_rate_z1_s1.nc` under a short plain file name and play it.
   - If it runs, the refusal depends on size: try s7 (246 KB), then the full
     file.
   - If s1 is also refused, the cause is the content or the machine's state,
     not the size.
4. Then run the sections and time them as in instructions section 4.8.

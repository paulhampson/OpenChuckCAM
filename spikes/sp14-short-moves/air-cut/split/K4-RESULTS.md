# SP-14 split files: results of the automated checks (13 Nov 2026, lane FWDIFF)

These are checks a session ran. They do not make any file safe to run, and
the owner still reads each file by hand (NFR-12, decision 0232 K1 to K3).

## Environment

- Files: `block_rate_z1_s1.nc` to `block_rate_z1_s14.nc` in this directory,
  written by `../gen_block_rate.py N` on this branch.
- xtask from `main` at bc1df28bdcf9b8363789ce43a25ad38e91b7ac2c.
- LinuxCNC image `openchuck-linuxcnc`, built locally from `docker/linuxcnc` at
  that commit: LinuxCNC v2.9.10 on Ubuntu 24.04, image ID
  `sha256:47fc56843a91ee2c1a4e7bc08b7ef41cf15a4546a5cf3d3460d16eed32179b69`.
  The session's network proxy needed three Dockerfile lines added: its CA
  certificate, `GIT_SSL_CAINFO`, and apt over HTTPS. Nothing else in the build
  changed.

## K1 to K3 (automated aid, `check_split.py`): all 14 pass

Each file's header, footer, forbidden-word grep, section body (byte-identical
to the same section of the full program at 69d532f), line form, X range and
step, A behaviour and longest line (50 characters) are as README.md says.
Full SHA-256 values: `sha256sum block_rate_z1_s*.nc`.

## K4 (gate 3, LinuxCNC replay): all 14 pass, with one deviation from the 0232 recipe

**With the 0232 recipe as written** (footer replaced by `M5`, `M30`), s1, s3,
s5 and s7 pass. The other ten fail, each in the same way, at the first G0
lines, before any section block:

- s2, s4 and s6 report "line 6: the simulator never reached [100.0, 0.0, 50.0, 0.0]";
- s8 to s14 report "line 5: ... never reached [0.0, 0.0, 50.0, 0.0]".

The cause is in the LinuxCNC copy, not in the Z1 program. The copy has no
`G61`, so LinuxCNC blends the corner between the header's
`G0 X0 Y0 Z50 A0` and the split file's extra G0 to the section start. In the
kept report for s2, line 5 hands over to line 6 at Z48.09, so the corner point
is never reached. The four files that pass are the ones whose extra G0 is a
zero-length move (section start X0, A0), so they have no corner.

The full program has no such corner either, which is why the recipe works for
it. The application's LinuxCNC post writes `G61`
(`testdata/golden/reference/*_linuxcnc.ngc`, line 6). The Z1 itself blends
G0 corners within its junction deviation of 0.01 mm (configZ1.default).

**With `G61` inserted after `G54`** in the LinuxCNC copy only, all 14 pass on
the first attempt:

```
sed -e '/^G92.4 A0 S0$/,$d' -e 's/^G54$/G54\nG61/' block_rate_z1_sN.nc > block_rate_sN_g61_linuxcnc.ngc
printf 'M5\nM30\n' >> block_rate_sN_g61_linuxcnc.ngc
cargo xtask replay --docker openchuck-linuxcnc block_rate_sN_g61_linuxcnc.ngc
```

| File | Moves reached | Largest difference (mm) | Motion (s) | Wall time (s) | LinuxCNC copy SHA-256 (first 16) |
| --- | --- | --- | --- | --- | --- |
| s1 | 303 | 0.0186 | 6.7 | 9.2 | 0ab089e889a19d31 |
| s2 | 604 | 0.1000 | 7.7 | 10.8 | 00349a9925ed4074 |
| s3 | 860 | 0.0237 | 5.3 | 9.2 | 18d5f92f2efbbe41 |
| s4 | 4 | 0.0005 | 5.9 | 10.7 | dae84c4c8edbd738 |
| s5 | 3 | 0.0000 | 2.8 | 9.3 | 6d46360d58efbafe |
| s6 | 4 | 0.0005 | 2.9 | 10.9 | 1751260120013da4 |
| s7 | 3 | 0.0001 | 1.4 | 9.5 | 0644fa3c6c1e9d8b |
| s8 | 304 | 0.0015 | 10.0 | 12.6 | 58aec1f3f41a9e2f |
| s9 | 604 | 0.0036 | 9.7 | 13.3 | 507da8673bfabc34 |
| s10 | 861 | 0.0102 | 12.1 | 16.1 | 8db45e1994de17a5 |
| s11 | 4 | 0.0000 | 12.6 | 17.1 | 89f6c74cd94b64fc |
| s12 | 4 | 0.0000 | 16.8 | 22.9 | a1ffb38bd5c87ecf |
| s13 | 4 | 0.0015 | 15.3 | 25.4 | 0921c520b54eed41 |
| s14 | 4 | 0.0000 | 16.5 | 35.2 | bf4faa0c2a8e31ac |

Notes:

- **s2 is at the comparison tolerance.** Its largest difference is 0.1000 mm,
  which is exactly the limit. It passed, but with no margin.
- **Few "moves reached" in the 5 ms and shorter sections.** The replay
  reports "moves reached" after grouping blocks shorter than its sampling. So
  the 1 to 5 ms sections show 3 or 4 moves; their blocks are collinear and
  the path is compared by position, not block by block.
- **Times are not comparable with the Z1.** The LinuxCNC motion times say
  nothing about the Z1's block rate; that is what the air cut measures.

## Owner decisions needed before the machine run

1. **Do the split files fall under decision 0232?** Item 2 says "A change to
   the program sends it back to step 1", and item 3 says "No other program
   gains the exception". The owner decides whether these files are covered
   once K1 to K4 pass, or need a record of their own.
2. **Is the K4 copy with `G61` accepted?** The recipe in instructions 4.8
   does not add it, and without it ten files fail for the reason above.

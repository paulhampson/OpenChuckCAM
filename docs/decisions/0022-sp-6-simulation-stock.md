# 0022: SP-6 result, simulation stock representation

- **Status:** accepted by the project owner (14 Oct 2026): dexels (item 1), voxel as the fallback (item 2), the budget (item 4). Item 3 was amended by the owner: the default stays 0.1 mm and the user may configure a finer resolution at the cost of speed, never as the default. The spike met its pass criteria for dexel and voxel.
- **Date:** 14 Oct 2026.
- **Spike branch:** `spike/sp-6-simulation-stock` (throwaway, never merged). The code is in `spikes/sp-6-simulation-stock/`, a separate Cargo project with no third-party dependencies, never a workspace member; the write-up is `spikes/sp-6-simulation-stock/RESULT.md` and the full tables `spikes/sp-6-simulation-stock/out/report.md`, both on that branch.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-56 and OI-99 closed; OI-100 to OI-102 are new and open for slice 7.

Numbering: record 0021 and OI-94 to OI-98 are on `main` (SP-11), and record 0020 is claimed on the unmerged slice 6 branch (`claude/nice-fermi-lyjx5n`, OI-88 to OI-93), so this record takes 0022 and OI-99 to OI-102.

## Context

SP-6 (work breakdown, Spikes, 5 days) asks which of rotary grid, dexel or voxel gives the best accuracy, speed and memory on a rotary part, behind one `StockModel` interface, with a sketch of the COL-05 collision query and a V-bit and a custom-profile tool in the sweep (SP-13). Pass criteria: removed volume on cylinder, cone and sphere within the resolution error; time and memory within budget at default resolution; same results at any thread count; ray and region queries supported. Fallback: pick the next best.

Requirements: SIM-03, SIM-05, SIM-06, SIM-07, COL-05 (P2), OP-03 and OP-09 (rest machining read the stock), NFR-04, NFR-06. Architecture section 5.2 (the `StockModel` port), section 9 and section 13 ("Simulation representation" is undecided). Verification plan section 4.1 (simulation within one cell, no worse than 0.02 mm at the finest setting). Decision 0012 item 1 and OI-56 (the budget SP-13 criterion 3 waits on).

## Method

Three small implementations behind one trait, in the part frame, with one shared line-versus-tool span solver (from the SP-13 profile, `h(r)`); stock Ø50 x 40 mm; cylinder, cone and sphere zones turned out with a Ø4 ball (52,200 and 23,400 poses); a plan-view quadrature as the oracle for straight passes with a flat, a ball, a V-bit and a custom profile; cells 0.2, 0.1 and 0.05 mm; threads 1, 2 and 4. Five tests named after SIM-03, SIM-05, SIM-07 and NFR-06. Details and numbers: `RESULT.md`.

## Results

| Criterion | Rotary grid | Dexel | Voxel |
| --- | --- | --- | --- |
| Volume within the resolution error (turned parts) | Pass | Pass | Pass |
| Worst radial error at 0.1 / 0.05 mm | 0.023 / 0.017 | 0.071 / 0.036 | 0.084 / 0.043 |
| Straight pass (flat, ball, V-bit, custom) | **Flat overcut 6 % (+27.8 mm3), not shrinking with the cell** | Within 0.2 mm3 | Within 0.6 mm3 (V-bit -3.7) |
| Reference job (52,200 poses, 0.1 mm), 1 / 4 threads | 83 s / 24 s | 46 s / 13 s | 51 s / 14 s |
| Memory at 0.1 / 0.05 mm | 5 / 20 MB | 8 to 12 / 32 to 48 MB | 12.5 / 100 MB |
| Same state at 1, 2, 4 threads | Yes | Yes | Yes |
| Ray and region queries | Yes (region by sampling) | Yes (exact on intervals) | Yes |

The rotary grid is the most accurate and smallest on turned surfaces, but a ray from the axis that leaves the tool through a vertical wall removes the material beyond it, so any slot, pocket or island is wrong. That rules it out as the stock for roughing, rest machining and carry-over (OP-03, OP-09, SIM-06). Voxel memory is cubic in the resolution.

## Decision (accepted)

1. **Dexels in the part frame (Z dexels on an (x, y) grid), with the tool span solved exactly per column,** are the `StockModel` implementation for slice 7 (OI-99).
2. **Voxel is the fallback** behind the same interface; **the rotary grid is not used as the main stock.**
3. **Default cell 0.1 mm; a finer resolution is a user setting, never the default** (SIM-07, owner's amendment). The user may choose a finer cell at the cost of speed and memory; slice 7 builds the setting, shows its cost and fixes the finest supported cell. At 0.05 mm dexel and voxel show 0.036 to 0.043 mm worst radial error, which meets "within one cell" but not "no worse than 0.02 mm at the finest setting"; the error scales with the cell, so about 0.025 mm is expected to meet it (an estimate, to be measured; OI-100).
4. **Budget for SP-13 criterion 3 (OI-56):** the reference job (52,200 poses, Ø4 ball, 0.1 mm) finishes in no more than 30 s on 4 threads, 120 s on one thread, and 256 MB. Dexel measures 13 s, 46 s and 12 MB, so criterion 3 is met against it.
5. **Collision queries (COL-05)** run as the same span test without the subtraction, on dexel columns, with the holder as a second revolved profile.

## Alternatives considered

- **Rotary grid only**: the best fit to a turned part but wrong for walls (see above). Could return as a fast path for wrapped work where walls do not occur.
- **Rotary grid with several intervals per ray**: that is radial dexels; not built, a possible variant if Z dexels prove anisotropic (OI-102).
- **Per-voxel tool tests**: 10 to 100 times slower than the column-span form and the same bits; not used.

## Consequences

- **Slice 7** implements dexels in `openchuck-sim` behind `StockModel`; items 1 to 3 take decision items 1 and 3; its review checklist limits follow OI-100.
- Documents updated with this record (owner agreed): `requirements.md` SIM-07 (default 0.1 mm, finer opt-in), `architecture.md` section 5 (stock representation), section 11 spike note, section 13 (decided) and the revision note, `verification-plan.md` (finest setting, risk and open-decision lines), `crate-selection.md` (in-house dexels) and `work-breakdown.md` (SP-6 status, slice 7 items and checklist).
- No dependency enters the product; the spike uses only the standard library.

## Open items for later decisions

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | Agree the representation (OI-99). Closed 14 Oct 2026 | Closed | |
| 2 | Finest user-selectable cell and the 0.02 mm limit there (OI-100) | Slice 7 | Measurement at about 0.025 mm |
| 3 | Agree the budget; SP-13 criterion 3 (OI-56). Closed 14 Oct 2026 | Closed | |
| 4 | Span solver speed (OI-101) | Slice 7 | Profiling |
| 5 | Sampling rules, shank, platforms (OI-102) | Slice 7 | Tests on all three platforms |

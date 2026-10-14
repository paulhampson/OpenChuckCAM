# 0022: SP-6 result, simulation stock representation

- **Status:** proposed. The spike met its pass criteria for two of the three candidates; the recommendation, the default cell and the budget need the owner's agreement (OI-99, OI-100, OI-56).
- **Date:** 14 Oct 2026.
- **Spike branch:** `spike/sp-6-simulation-stock` (throwaway, never merged). The code is in `spikes/sp-6-simulation-stock/`, a separate Cargo project with no third-party dependencies, never a workspace member; the write-up is `spikes/sp-6-simulation-stock/RESULT.md` and the full tables `spikes/sp-6-simulation-stock/out/report.md`, both on that branch.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-56 moves to proposed; OI-99 to OI-102 are new.

Numbering: record 0020 and 0021 are claimed on unmerged branches (slice 6, SP-11), and OI-96 to OI-98 on unmerged branches, so this record takes 0022 and OI-99 to OI-102.

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

## Decision (proposed)

1. **Dexels in the part frame (Z dexels on an (x, y) grid), with the tool span solved exactly per column,** are the `StockModel` implementation for slice 7 (OI-99).
2. **Voxel is the fallback** behind the same interface; **the rotary grid is not used as the main stock.**
3. **Default cell 0.1 mm, finest 0.05 mm** (SIM-07); the verification limit "no worse than 0.02 mm at the finest setting" is not met by dexel or voxel at 0.05 mm (0.036 to 0.043 mm), so the owner chooses between a finer finest setting and a restated limit (OI-100).
4. **Budget for SP-13 criterion 3 (OI-56):** the reference job (52,200 poses, Ø4 ball, 0.1 mm) finishes in no more than 30 s on 4 threads, 120 s on one thread, and 256 MB. Dexel measures 13 s, 46 s and 12 MB, so criterion 3 is met against it.
5. **Collision queries (COL-05)** run as the same span test without the subtraction, on dexel columns, with the holder as a second revolved profile.

## Alternatives considered

- **Rotary grid only**: the best fit to a turned part but wrong for walls (see above). Could return as a fast path for wrapped work where walls do not occur.
- **Rotary grid with several intervals per ray**: that is radial dexels; not built, a possible variant if Z dexels prove anisotropic (OI-102).
- **Per-voxel tool tests**: 10 to 100 times slower than the column-span form and the same bits; not used.

## Consequences

- **Slice 7** implements dexels in `openchuck-sim` behind `StockModel`; items 1 to 3 take decision items 1 and 3; its review checklist limits follow OI-100.
- No change to `requirements.md` or `architecture.md` yet. When the owner agrees, architecture sections 5.2, 9 and 13 and the verification plan's open-decision line are edited in the same change that records the agreement.
- No dependency enters the product; the spike uses only the standard library.

## Open items for later decisions

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | Agree the representation (OI-99) | Owner | This record |
| 2 | Default and finest cell, and the 0.02 mm limit (OI-100) | Slice 7 | Owner's choice |
| 3 | Agree the budget; then SP-13 criterion 3 closes (OI-56) | Owner | This record |
| 4 | Span solver speed (OI-101) | Slice 7 | Profiling |
| 5 | Sampling rules, shank, platforms (OI-102) | Slice 7 | Tests on all three platforms |

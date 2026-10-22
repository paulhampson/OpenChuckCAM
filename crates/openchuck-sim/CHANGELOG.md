# Changelog

All notable changes to `openchuck-sim` are recorded here.

## Unreleased

- Version 0.17.0 with the library family (decisions 0201 and 0202); no change to this crate's items.
- Version 0.16.0 with the library family (decision 0204); no change to this crate's items.
- Version 0.15.0 with the library family.
- Version 0.13.0 with the library family (`StrategyOutput` and `GeneratedPath` in `openchuck-path` gain `warnings`, a breaking change; decision 0175).
- Version 0.10.0 with the library family (`assemble::smoothed_contacts` in `openchuck-path` takes the rotary mapping, a breaking change; decision 0115).
- Version 0.9.0 with the library family (decision 0076).
- Version 0.7.0 with the library family (decision 0033). New: `Leftover` and `Deviation::leftover` (roughing leftover against a stock to leave).
- Slice 7 (decision 0025): `DexelStock` (implements `StockModel`), `ToolSolid` and `cut_spans` for every revolved tool, `sample_move` through the shared kinematics, `Simulator` with checkpoints, progress and cancellation, `compare` (deviation as the closest distance) and `stock_mesh`.
- Crate created as a shell (work breakdown slice 0).

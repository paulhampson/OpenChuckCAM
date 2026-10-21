# Changelog

All notable changes to `openchuck-sim` are recorded here.

## Unreleased

- Each deviation sample says what it lies on (decision 0194, issue #248): new `SampleOn` (`Part`, `Workholding`, `Inside`), `DeviationPoint::on` and `DeviationPoint::counts`, and `compare_classified`, which takes a `Classify` function; `compare` and `compare_cancellable` mark every sample `Part`. The worst values, overcut count and places, `Deviation::leftover` and `triangle_values` use only the samples that count. Breaking for a host that builds `DeviationPoint` by its fields: add `on`. `DexelStock::bounds` keeps the material's extent until the next cut (decision 0195, issue #252), so a slanted `first_material` ray no longer reads every column.
- Version 0.13.0 with the library family (`StrategyOutput` and `GeneratedPath` in `openchuck-path` gain `warnings`, a breaking change; decision 0175).
- Version 0.10.0 with the library family (`assemble::smoothed_contacts` in `openchuck-path` takes the rotary mapping, a breaking change; decision 0115).
- Version 0.9.0 with the library family (decision 0076).
- Version 0.7.0 with the library family (decision 0033). New: `Leftover` and `Deviation::leftover` (roughing leftover against a stock to leave).
- Slice 7 (decision 0025): `DexelStock` (implements `StockModel`), `ToolSolid` and `cut_spans` for every revolved tool, `sample_move` through the shared kinematics, `Simulator` with checkpoints, progress and cancellation, `compare` (deviation as the closest distance) and `stock_mesh`.
- Crate created as a shell (work breakdown slice 0).

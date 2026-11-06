# Changelog

## 0.47.0

- **Family release (0.47.0, TABCUT; issue #555; decision 0527, proposed):** `openchuck-path`'s `RegionConstraint` gains the public field `outside`, a breaking change; no change to this crate.

## Unreleased (FLUTE-TABS PR3, issue #547)

- `DexelStock` gives its reach profile (`StockModel::reach_profile`, decision 0524): each column's spans as boxes, a slice a cell along X, for the flute check's bound from the material within the tool's reach. No API change of its own.

## 0.34.0

- **Breaking family release (0.34.0, lane CLEARANCE, issue #336; decision 0495, accepted):** no change in this crate; all family crates advance together under 0048.

All notable changes to `openchuck-sim` are recorded here.

## 0.41.0

- **Breaking (0.41.0, lane SIMVIS #492, CodeRabbit on #486; decision 0514, accepted):** `Reach` gains `classified` (a new public field, so code that builds a `Reach` must set it). It is false when the enclosure test was skipped (a tool narrower than two grid pitches, or nothing to cast against), so a run that did not classify says so and claims no share as unreachable (NFR-12; decision 0513 item 7). The workspace minor version moves to 0.41.0 (decision 0048).

## Unreleased

- SIMVIS (#488, accepted decision 0514): `triangle_samples` gives the samples each target triangle holds, in the comparison's order. Additive only.

- SIMVIS (#478, accepted decision 0513): `classify_reach` with `ReachOptions`, `ReachClass` and `Reach` tells a counted part sample's material left as enclosed (no tool across the rotary axis reaches it), open or undecided; `triangle_worst` (the sample each triangle is coloured by) and `sample_areas` (the surface each sample stands for). Additive only; `triangle_values` now uses `triangle_worst`, with the same results.

- SIMPERF (#382, accepted decision 0322): `Simulator::stock_at_from` replays from a kept stock, `with_checkpoint_budget` and `checkpoint_bytes` bound the checkpoints' memory, checkpoints hold a `PackedStock` (`DexelStock::pack`, the same stock bit for bit), and a jump reports its replay against one total.

- Version 0.25.0 with the library family for FACING (decision0252). Hosts constructing `StrategyInput` supply the new optional `facing` setup context; use `None` to retain legacy whole-stock inputs. No project-schema migration.

- Version 0.21.0 with the library family (decision 0241): `PostJob` and `ConvertOptions` require `a_range` and work-frame `safe_z`; `Step::Unwind` is replaced by the non-moving `RotaryBoundary`. `Regeneration` keeps `canonical_toolpath`, and `Application::planned_files` takes the regeneration. Hosts connect canonical groups through `openchuck_path::program::connect_group` in file order before posting.

- Version 0.19.0 with the library family (`BrepEdge` in `openchuck-types` gains `curve`, a breaking change; decision 0227); no change to this crate's items.
- Version 0.18.0 with the library family (`PencilFinishing` in `openchuck-path` becomes a struct with a private field, no longer `Copy` or `Eq`, a breaking change; decision 0226); no change to this crate's items.
- `DexelStock` answers `StockModel::x_extent`: the X range of the columns that hold material (OP-24; decision 0212 part B).
- Each deviation sample says what it lies on (decision 0194, issue #248): new `SampleOn` (`Part`, `Workholding`, `Inside`), `DeviationPoint::on` and `DeviationPoint::counts`, and `compare_classified`, which takes a `Classify` function; `compare` and `compare_cancellable` mark every sample `Part`. The worst values, overcut count and places, `Deviation::leftover` and `triangle_values` use only the samples that count. Breaking for a host that builds `DeviationPoint` by its fields: add `on`. `DexelStock::bounds` keeps the material's extent until the next cut (decision 0195, issue #252), so a slanted `first_material` ray no longer reads every column.
- Version 0.17.0 with the library family (decisions 0201 and 0202); no change to this crate's items.
- Version 0.16.0 with the library family (decision 0204); no change to this crate's items.
- Version 0.15.0 with the library family.
- Version 0.13.0 with the library family (`StrategyOutput` and `GeneratedPath` in `openchuck-path` gain `warnings`, a breaking change; decision 0175).
- Version 0.10.0 with the library family (`assemble::smoothed_contacts` in `openchuck-path` takes the rotary mapping, a breaking change; decision 0115).
- Version 0.9.0 with the library family (decision 0076).
- Version 0.7.0 with the library family (decision 0033). New: `Leftover` and `Deviation::leftover` (roughing leftover against a stock to leave).
- Slice 7 (decision 0025): `DexelStock` (implements `StockModel`), `ToolSolid` and `cut_spans` for every revolved tool, `sample_move` through the shared kinematics, `Simulator` with checkpoints, progress and cancellation, `compare` (deviation as the closest distance) and `stock_mesh`.
- Crate created as a shell (work breakdown slice 0).

# Changelog

All notable changes to `openchuck-app` are recorded here.

## Unreleased

- Slice 8, first step (decision 0026): the `scene` module and `Application::start_scene` (the viewport's view model: part with feature edges, stock outline, rotary axis, A+ arrow from the shared kinematics, work coordinate system, labels and notes); `JobHandle::try_result`; `Application::is_modified` and `location`.
- Slice 7 (decision 0025): `Command::SetSimulation`; `Application::simulate` and `start_simulate`; `Regeneration::simulation` with a `SimulationReport`; operations after the first in a setup are generated against the simulated stock (`generate_operation_on`, SIM-06); `OperationPath` gains the tool profile and part bounds; `openchuck_sim` re-exported as `sim`. `openchuck-cli simulate`.
- Feed and material (decision 0018 item 7): `parameter_values` returns `ResolvedValues` with notes where an operation's value overrides the material's cutting data; a required feed, plunge feed or stepover with no source is refused naming the operation, the material and the tool; `generate_operation` returns `OperationPath` (group and notes). `openchuck-cli` prints regeneration notes as warnings in `validate` and `regenerate`.
- Slice 5, first step (decision 0018): the `operation` module maps an operation to the pathing library (`generate_operation`, `parameter_values` with feed, plunge feed and stepover from the tool's cutting data); regeneration runs every operation in machining order and `Regeneration` gains `notes`; `Application::strategies` and `register_strategy`; errors `UnknownOperation`, `Operation` and `Path`; `Registry` and `Strategy` re-exported.
- Slice 4 (decision 0014): the `tools` module: `check_tools` (shared tool numbers, missing cutting data, library and tool set differences), `default_controller` and `NumberSource` (TL-11), `find_tool`, `describe`, `describe_controller` and `with_custom_profile`, and re-exports of the tool and library types a front end needs.
- Slice 3 (decision 0011): `Command::SetMachine` takes a machine profile; `Application::check_setups` and `find_setup`; `ModelInspection::extent`; the `setup` module re-exports what a front end needs and states the A direction in words.
- `Application::add_model`, `find_model`, `inspect_model` and `inspect_file` as jobs; commands `SetModelUnits` and `SetModelTransform`; the `model` module re-exports the model types a front end needs (slice 2, decision 0010).
- Commands with undo and redo for every project edit (slice 1, UI-08). `openchuck-project` is re-exported as `project`.
- `Application::create`, `open` (loads `project.toml`), `save` and `check_models`. A folder without a project gives `AppError::NoProject`, which a front end answers by offering `create`.
- Skeleton: commands, queries, event stream, job manager and an empty pipeline (work breakdown slice 0).

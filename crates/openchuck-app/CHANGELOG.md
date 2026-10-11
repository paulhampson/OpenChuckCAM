# Changelog

All notable changes to `openchuck-app` are recorded here.

## Unreleased

- Slice 3 (decision 0011): `Command::SetMachine` takes a machine profile; `Application::check_setups` and `find_setup`; `ModelInspection::extent`; the `setup` module re-exports what a front end needs and states the A direction in words.
- `Application::add_model`, `find_model`, `inspect_model` and `inspect_file` as jobs; commands `SetModelUnits` and `SetModelTransform`; the `model` module re-exports the model types a front end needs (slice 2, decision 0010).
- Commands with undo and redo for every project edit (slice 1, UI-08). `openchuck-project` is re-exported as `project`.
- `Application::create`, `open` (loads `project.toml`), `save` and `check_models`. A folder without a project gives `AppError::NoProject`, which a front end answers by offering `create`.
- Skeleton: commands, queries, event stream, job manager and an empty pipeline (work breakdown slice 0).

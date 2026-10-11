# Changelog

All notable changes to `openchuck-app` are recorded here.

## Unreleased

- Commands with undo and redo for every project edit (slice 1, UI-08). `openchuck-project` is re-exported as `project`.
- `Application::create`, `open` (loads `project.toml`), `save` and `check_models`. A folder without a project gives `AppError::NoProject`, which a front end answers by offering `create`.
- Skeleton: commands, queries, event stream, job manager and an empty pipeline (work breakdown slice 0).

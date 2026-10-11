# openchuck-app

The OpenChuckCAM application layer: commands, queries, the event stream, the job manager and the operation pipeline (architecture section 5.7). UI-agnostic; the UI and the CLI are two equal front ends over this API.

**Status:** slice 1. Projects are created, opened and saved as folders of TOML files (a folder with none gives `NoProject`, to which a front end offers `create`), and every project edit is a command with undo and redo (decision record 0008). Slice 2: models are added, inspected as jobs (import, placement and mesh checks), and given units and placement by command (decision record 0010).

Licence: AGPL-3.0-or-later.

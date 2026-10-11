# openchuck-app

The OpenChuckCAM application layer: commands, queries, the event stream, the job manager and the operation pipeline (architecture section 5.7). UI-agnostic; the UI and the CLI are two equal front ends over this API.

**Status:** slice 1 in progress. Every project edit is a command with undo and redo (decision record 0005). Until the project file format exists (spike SP-8) an existing folder opens as an empty project.

Licence: AGPL-3.0-or-later.

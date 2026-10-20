# 0144: The Setups "+" creates the project's one setup (lane SU)

- **Status:** accepted by the project owner on 19 Oct 2026, given directly in the lane SU session. The owner chose the behaviour (option A) and decided OI-425 (no setup without a model) the same day, both relayed by the coordinator session before the acceptance. This record amends specification section 6.
- **Date:** 19 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-425 and OI-426 are new and closed (OI-426 on 20 Oct 2026).

Numbering: the coordinator assigned this lane records 0144 and 0145 and OI-425 to OI-429. This lane uses 0144, OI-425 and OI-426. Record 0145 and OI-427 to OI-429 are not used.

## Context

File, New project makes a project with no setup, and the Setups "+" was hidden (specification section 6: "Only once multiple setups (SET-07, P2) are built; until then it is hidden"). The only code that created a setup was the command line (`setup --new`) and tests. A user of the interface alone could therefore not set stock, part position, work offset or clearance (SET-01 to SET-06, P1), and the add-operation dialog said "none: add a setup first".

Requirements: SET-01 to SET-06 (P1) need a setup; UI-10, UI-11 and UI-14 for the steps, the tree and the keys. SET-07 (several setups, P2) stays unbuilt. No requirement, architecture or project format change.

## Decision

1. **The "+" is shown and creates exactly one setup.** One click adds "Setup 1" as a single undoable command (`openchuck_app::new_setup`, a `Command::Add` of the setup at the end of the list), selects it and opens the Setup step. Every other field is what `Setup::new` already gives and the command line already uses: no stock or material, the part origin at the rotary reference point, G54 at the model origin, the machine's clearance. No new default is invented. Creating the setup does not happen on import (not option B).
2. **A setup needs a model (OI-425, decided by the owner on 19 Oct 2026: "Don't allow creation of a setup without a part imported").** Creation always sets the part to the project's first model, and the "+" and the workflow bar's button are disabled until the project has one, with the tooltip "Import a model first". `new_setup` refuses with `AppError::NoModelForSetup` otherwise. A setup with no part loaded from a file is unaffected, and the import path is unchanged.
3. **Once a setup exists the "+" stays visible but disabled** (`openchuck_app::can_add_setup`), with the tooltip "More than one setup per project is not supported yet (SET-07)". A second request does nothing. Undo removes the setup and enables the "+" again. When both reasons could apply the control shows one: the SET-07 limit if a setup exists, else the missing model (`add_setup_blocked`).
4. **The workflow bar's "Add setup"** (specification sections 3 and 5 name it) is a button on the Setup step, with the same enabled state and the same tooltip. It does the same as the tree's "+".
5. **Specification section 6** is edited in the same change: the Setups row of the "+" table now reads as items 1 to 3 say, replacing "Only once multiple setups ... are built; until then it is hidden". The doc comment on `Group::add` in `shell/tree.rs` says the same.
6. **Provenance of the defaults (OI-426, closed).** The new setup's defaults appear in the setup inspector as the existing form shows an unset field: the value that applies with its source (decision 0123, lane X4). The new setup holds the same data as one made by `setup --new`, so no mark is added here.

## Alternatives considered

- **Option B, a setup created on model import.** Rejected by the owner: it hides a choice the user should see.
- **A setup with no part, made before any model** (this record's first design). Replaced by the owner's decision on OI-425: the Setup step is closed without a model, so such a setup could not be edited there.
- **Hide the "+" once a setup exists.** Rejected by the owner: a disabled control with a reason is clearer than one that vanishes.

## Consequences

- `openchuck-app` gains `new_setup`, `can_add_setup`, `add_setup_blocked`, the two reason strings, and `AppError::OneSetupOnly` and `NoModelForSetup`. The interface changes are `Group::add`, a `header_with` that gives a disabled "+" its own tooltip, the Add handler's Setups arm and `Shell::add_setup` in `shell.rs`, and the Setup step's arm of `workflow_bar` in `shell/frame.rs`.
- Tests: `crates/openchuck-app/tests/new_setup.rs` and four `req_set_01_*` tests in `shell/frame_tests.rs` (the "+" creates the setup and opens the step, one undo, a second click does nothing, an existing setup shows it disabled, undo enables it, no model leaves it disabled).
- SET-07 will lift item 3's limit and item 1's "exactly one".

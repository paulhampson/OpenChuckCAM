# 0172: What makes an operation stale, and a toolpath whose operation is gone (lane BUG-STALE, part 1)

- **Status:** proposed (20 Oct 2026). Only the project owner accepts a record.
- **Date:** 20 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-620 is new. Record 0173 and OI-621 to OI-629 are for part 2 (issues 210 and 224).

## Context

Trial findings F-40 (issue 196), F-53 (issue 209) and F-33 (issue 189). Requirements: UI-06 (operations), UI-11 (item states follow the inputs of the cache key), GC-07 and NFR-06 (determinism), GC-12 (comments in the G-code), NFR-12 (the user verifies before running). Architecture section 6 (the cache key). Decision 0030 item 1 (the pipeline cache key) and decision 0117 (item states).

What the code did:

- `operation_key` hashed the whole `Operation` and the whole `Setup`, so the **name** and the **order key** (and the `enabled` flag) were inputs. A rename made the operation and every later one stale and made the next run regenerate them. A setup rename did the same for all its operations.
- The name is **not** in the toolpath: the path's tags carry the operation's ID, and the post reads the name from the project when it writes the comment (`operation_names`, GC-12). The simulation report does hold the name as it was when the run was made.
- A run that holds the path of an operation which was **removed or turned off** after it was made was not reported by `export_checks` (it compared only the operations that exist and are on now). The export dialog therefore allowed writing, and the posted file included the moves of the operation that was gone. The viewport kept drawing the same run, and the playback and the comparison with the model still used it.
- Disabling an operation and changing its strategy: a strategy change was already stale (the strategy is in the key); a disabled operation was the gap above.

## Decision

1. **The stale state follows the inputs that make the toolpath.** The operation's name, its position key and its on/off flag are not part of `operation_key`. The position reaches the key through `previous` (the key of the enabled operation before), so a move that changes the order is still stale and a move back is up to date. A disabled operation has no key. The setup's name and position key are left out the same way. Everything else stays in the key (parameters, strategy, tool snapshot without its controller number, selections, setup, machine, simulation settings, model and file hash, engine version), so determinism is untouched: the same inputs give the same key and the same bytes.
2. **A rename changes the posted output but not the toolpath.** Because the name is read when posting, a rename needs a new run that **reuses every cached path** and posts again: no toolpath is regenerated. The export check does not call a rename out of date. The simulation report keeps the names of the run; `export_checks` looks the name up in the run's own project copy, so a rename is not reported as "not simulated". The Results window and the playback show the old name until the next simulation (OI-620).
3. **A path whose operation is gone is never used.** `export_checks` reports every operation the run holds that the project has removed or turned off as blocking ("it was removed after the toolpath was generated, but the toolpath still holds it. Generate again."). The playback panel drops a run that holds the path of an operation that is removed or turned off, which empties the path, tool and stock layers and the deviation bands at once; the stale mark on the remaining operations says to generate again. An ordinary edit (a parameter) keeps the old path drawn and marks it stale, as before.
4. **"Machine setting" (issue 189).** A value that falls back to the machine is called "Machine setting" in the Setup inspector's hints (`12.000 · Machine setting`), their tooltips ("Empty uses the Machine setting.") and in the command line's `setup` summary. One constant, `setup_form::MACHINE_SETTING`. The hint is one character longer than before and is shown in the field when it is empty; a field too narrow to hold it clips it, as before (the field width is lane BUG-INSP-FIELD-WIDTH).

## Alternatives considered

- Keep the name in the key and re-label on a hit: rejected, a rename would still look stale and the key would no longer say what the path depends on.
- Filter the removed operation's group out of the run instead of dropping it: rejected, the stock the later operations cut and the simulation depend on it, so a filtered run would be wrong where it looks right.
- Regenerate on every change to the project: rejected, wasteful and not what UI-11 asks.

## Consequences

- `cache.rs` `operation_key`, `export.rs` `stale_operations` and `export_checks`, `playback_panel.rs` `forget_removed`, `setup_form.rs`, the command line's `setup` summary.
- No project format change. A project saved before this change keeps working; its cached results are keyed again by the new key (a one-time regeneration of cached paths).
- Tests: `req_ui_11_req_gc_07_a_rename_keeps_every_operation_up_to_date`, `req_ui_11_renaming_a_setup_keeps_its_operations_up_to_date`, `req_ui_11_moving_and_turning_off_then_on_are_exact_inverses`, `req_nfr_12_req_gc_15_a_removed_or_disabled_operation_blocks_a_run_that_still_holds_it` (`tests/overview.rs`), `req_gc_12_req_gc_07_a_rename_is_posted_without_regenerating_and_is_deterministic` (`tests/export_dialog.rs`), `req_nfr_12_req_ui_06_removing_or_disabling_an_operation_clears_its_path_from_the_view` (`playback_panel.rs`), the `setup_form` clearance test.

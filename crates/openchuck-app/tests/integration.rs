// SPDX-License-Identifier: AGPL-3.0-or-later
//! Every integration test file of this crate, built as one test binary (decision 0516, option C):
//! one binary links the dependencies once instead of once per file, which cuts the build's disk
//! space and link time. Each file stays where it was and is a module here, so its tests are named
//! `<file>::<test>`. A new file under `tests/` must be added below (or declared as its own
//! `[[test]]` in Cargo.toml); `cargo xtask trace` fails on a file that is neither.

// Each file keeps its own `mod support;` (and the step accuracy files their own copies of the
// example's modules), so the files are unchanged from when each was its own binary and still read
// on their own; the shared helpers are therefore compiled once per file that uses them, as before.
#![allow(clippy::duplicate_mod)]

#[path = "adaptive_example.rs"]
mod adaptive_example;
#[path = "app.rs"]
mod app;
#[path = "checkpoint_schedule.rs"]
mod checkpoint_schedule;
#[path = "checkpoints.rs"]
mod checkpoints;
#[path = "drilling.rs"]
mod drilling;
#[path = "export_dialog.rs"]
mod export_dialog;
#[path = "facing_approach.rs"]
mod facing_approach;
#[path = "feed_limits.rs"]
mod feed_limits;
#[path = "flute_check.rs"]
mod flute_check;
#[path = "golden_audit.rs"]
mod golden_audit;
#[path = "held_region.rs"]
mod held_region;
#[path = "import_preview.rs"]
mod import_preview;
#[path = "library.rs"]
mod library;
#[path = "material_preset.rs"]
mod material_preset;
#[path = "model.rs"]
mod model;
#[path = "model_copy.rs"]
mod model_copy;
#[path = "new_setup.rs"]
mod new_setup;
#[path = "operation.rs"]
mod operation;
#[path = "output_options.rs"]
mod output_options;
#[path = "overview.rs"]
mod overview;
#[path = "part_offset.rs"]
mod part_offset;
#[path = "picking.rs"]
mod picking;
#[path = "playback.rs"]
mod playback;
#[path = "posting.rs"]
mod posting;
#[path = "reach.rs"]
mod reach;
#[path = "reference_jobs.rs"]
mod reference_jobs;
#[path = "rotary_pass_resets.rs"]
mod rotary_pass_resets;
#[path = "round_trip.rs"]
mod round_trip;
#[path = "scene.rs"]
mod scene;
#[path = "selection.rs"]
mod selection;
#[path = "settings.rs"]
mod settings;
#[path = "setup.rs"]
mod setup;
#[path = "setup_panes.rs"]
mod setup_panes;
#[path = "simulation.rs"]
mod simulation;
#[path = "step_accuracy_bounded.rs"]
mod step_accuracy_bounded;
#[path = "step_accuracy_checks.rs"]
mod step_accuracy_checks;
#[path = "step_accuracy_posting.rs"]
mod step_accuracy_posting;
#[path = "step_accuracy_timing.rs"]
mod step_accuracy_timing;
#[path = "step_accuracy_workspace.rs"]
mod step_accuracy_workspace;
#[path = "swarf.rs"]
mod swarf;
#[path = "tab_scope.rs"]
mod tab_scope;
#[path = "tab_zone_clearance.rs"]
mod tab_zone_clearance;
#[path = "tool_edit.rs"]
mod tool_edit;
#[path = "tool_library.rs"]
mod tool_library;
#[path = "tool_library_sections.rs"]
mod tool_library_sections;
#[path = "undo.rs"]
mod undo;
#[path = "unreached.rs"]
mod unreached;
#[path = "walls.rs"]
mod walls;
#[path = "workholding.rs"]
mod workholding;
#[path = "workholding_view.rs"]
mod workholding_view;

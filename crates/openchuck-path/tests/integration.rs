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

#[path = "adaptive.rs"]
mod adaptive;
#[path = "axial.rs"]
mod axial;
#[path = "axial_links.rs"]
mod axial_links;
#[path = "brep_drop.rs"]
mod brep_drop;
#[path = "brep_edge_following.rs"]
mod brep_edge_following;
#[path = "brep_faces.rs"]
mod brep_faces;
#[path = "brep_outline_drop.rs"]
mod brep_outline_drop;
#[path = "brep_pencil.rs"]
mod brep_pencil;
#[path = "brep_queries.rs"]
mod brep_queries;
#[path = "carving.rs"]
mod carving;
#[path = "cutter.rs"]
mod cutter;
#[path = "drill.rs"]
mod drill;
#[path = "drop_threads.rs"]
mod drop_threads;
#[path = "edge_curve.rs"]
mod edge_curve;
#[path = "edge_following.rs"]
mod edge_following;
#[path = "entry.rs"]
mod entry;
#[path = "entry_safety.rs"]
mod entry_safety;
#[path = "exact_faces.rs"]
mod exact_faces;
#[path = "extend.rs"]
mod extend;
#[path = "faces_cancel.rs"]
mod faces_cancel;
#[path = "facing_box_plan.rs"]
mod facing_box_plan;
#[path = "facing_box_validation.rs"]
mod facing_box_validation;
#[path = "facing_coverage.rs"]
mod facing_coverage;
#[path = "facing_end_plan.rs"]
mod facing_end_plan;
#[path = "facing_extents.rs"]
mod facing_extents;
#[path = "facing_geometry.rs"]
mod facing_geometry;
#[path = "facing_legacy_scope.rs"]
mod facing_legacy_scope;
#[path = "facing_motion.rs"]
mod facing_motion;
#[path = "facing_parameters.rs"]
mod facing_parameters;
#[path = "facing_selection.rs"]
mod facing_selection;
#[path = "facing_validation.rs"]
mod facing_validation;
#[path = "field.rs"]
mod field;
#[path = "finishing.rs"]
mod finishing;
#[path = "fit_interior.rs"]
mod fit_interior;
#[path = "flutes.rs"]
mod flutes;
#[path = "helical.rs"]
mod helical;
#[path = "helical_faces.rs"]
mod helical_faces;
#[path = "helical_rings.rs"]
mod helical_rings;
#[path = "indexed.rs"]
mod indexed;
#[path = "pathing.rs"]
mod pathing;
#[path = "program.rs"]
mod program;
#[path = "region.rs"]
mod region;
#[path = "region_offsets.rs"]
mod region_offsets;
#[path = "rest.rs"]
mod rest;
#[path = "roughing.rs"]
mod roughing;
#[path = "roughing_region.rs"]
mod roughing_region;
#[path = "rounding_noise.rs"]
mod rounding_noise;
#[path = "simultaneous.rs"]
mod simultaneous;
#[path = "simultaneous_hidden.rs"]
mod simultaneous_hidden;
#[path = "simultaneous_off_axis.rs"]
mod simultaneous_off_axis;
#[path = "simultaneous_rings.rs"]
mod simultaneous_rings;
#[path = "solids.rs"]
mod solids;
#[path = "spiral_rings.rs"]
mod spiral_rings;
#[path = "strategy_menu.rs"]
mod strategy_menu;
#[path = "swarf.rs"]
mod swarf;
#[path = "unroll.rs"]
mod unroll;
#[path = "walls.rs"]
mod walls;
#[path = "zones.rs"]
mod zones;

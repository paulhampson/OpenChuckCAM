// SPDX-License-Identifier: AGPL-3.0-or-later
//! Material removal simulation, deviation measurement and the implementations of the pathing ports.
//!
//! Architecture section 5.5; requirements SIM-01 to SIM-10, COL-01 to COL-08.
//! Work breakdown slice 7 builds the headless simulation (SIM-03, SIM-05,
//! SIM-06, SIM-07) on the representation spike SP-6 chose (decision record
//! 0022; the slice's design is in decision record 0025):
//!
//! - [`DexelStock`]: material intervals along model Z on an (x, y) grid of
//!   columns in the part frame, with the tool span solved exactly per column.
//!   It implements the pathing library's [`StockModel`](openchuck_path::StockModel)
//!   port, so later operations generate against the material actually left.
//! - [`Simulator`]: runs toolpath groups in machining order through the shared
//!   kinematics, sampled no coarser than the cell, with checkpoints to jump to
//!   any operation or move, cancellation and progress.
//! - [`ToolSolid`]: any revolved tool profile (TL-03), from the tip to the top
//!   of its flutes.
//! - [`compare`]: the simulated stock against the target model by the
//!   closest distance to the stock's boundary at each surface sample, with
//!   the worst remaining and overcut values and where they are (SIM-05).
//! - [`stock_mesh`]: the stock as a mesh for the viewport; plain data only.
//!
//! Results never depend on the thread count (NFR-06): work runs on the
//! caller's rayon pool in fixed pieces, each column is cut alone, and span
//! endpoints depend only on the pose and the column.

#![warn(missing_docs)]

mod deviation;
mod dexel;
mod display;
mod sample;
mod simulation;
mod span;

pub use deviation::{
    Classify, Deviation, DeviationOptions, DeviationPoint, Leftover, MAX_DEVIATION_SAMPLES,
    SampleOn, compare, compare_cancellable, compare_classified, triangle_values,
};
pub use dexel::{DexelStock, MAX_COLUMNS, Pose};
pub use display::stock_mesh;
pub use sample::{MAX_SAMPLES_PER_MOVE, Sampling, sample_move};
pub use simulation::{BATCH_POSES, Checkpoint, GroupResult, RapidCut, Simulator};
pub use span::{MAX_SPANS, Spans, ToolSolid, cut_spans};

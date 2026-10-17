// SPDX-License-Identifier: AGPL-3.0-or-later
//! Test-only support shared by the OpenChuckCAM crates.
//!
//! This crate is used only as a dev-dependency; the `xtask` dependency check
//! fails if any crate takes it as a normal dependency. It holds:
//!
//! - [`oracle`]: the independent kinematic oracle (architecture section 9,
//!   work breakdown slice 0 item 11). It shares no code with
//!   `openchuck_types::kinematics`.
//! - [`determinism`]: the 1, 2 and N thread harness (NFR-06, GC-07).
//! - [`gouge`]: the independent gouge and coverage checker (OP-12,
//!   architecture section 9), which samples exact analytic surfaces.
//! - [`region`]: the independent oracle for 2D offsets on an unrolled
//!   cylinder, and the corpus of awkward regions (OP-04, from spike SP-7).
//! - [`golden`]: plain-text golden-file comparison (NFR-08).
//! - [`parts`]: procedurally generated analytic test parts and the
//!   1M-triangle benchmark model (NFR-08, NFR-03).

#![warn(missing_docs)]

pub mod brep;
pub mod determinism;
pub mod golden;
pub mod gouge;
pub mod oracle;
pub mod parts;
pub mod region;

// SPDX-License-Identifier: AGPL-3.0-or-later
//! SP-6 spike: which stock representation (rotary grid, dexel or voxel) gives
//! the best accuracy, speed and memory on a rotary part? Throwaway code; see
//! RESULT.md. The tool profile, its standard shapes and the deterministic
//! parallel map are copied unchanged from the SP-13 spike (decision 0012).

pub mod dexel;
pub mod par;
pub mod parts;
pub mod profile;
pub mod rotary;
pub mod shapes;
pub mod stock;
pub mod toolray;
pub mod voxel;

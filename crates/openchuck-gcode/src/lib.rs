// SPDX-License-Identifier: AGPL-3.0-or-later
//! G-code reader and interpreter: parses G-code with A words back into motion for replay and checks.
//!
//! Architecture section 5.3 and 9; requirements GC-08, SIM-09.
//!
//! This crate is a shell created in work breakdown slice 0. The reader and replay arrive in slice 6, after spike SP-5 (see decision record 0001, open item 5).

#![warn(missing_docs)]

use openchuck_types as _;

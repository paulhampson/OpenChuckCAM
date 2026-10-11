// SPDX-License-Identifier: AGPL-3.0-or-later
//! Material removal simulation, deviation measurement and the implementations of the pathing ports.
//!
//! Architecture section 5.5; requirements SIM-01 to SIM-10, COL-01 to COL-08.
//!
//! This crate is a shell created in work breakdown slice 0. The stock model arrives in slice 7, after spike SP-6.

#![warn(missing_docs)]

use openchuck_path as _;
use openchuck_types as _;

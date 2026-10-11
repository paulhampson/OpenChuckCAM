// SPDX-License-Identifier: AGPL-3.0-or-later
//! Project document model, text persistence, stable IDs and order keys, selection sets and content hashes.
//!
//! Architecture section 5.6; requirements PRJ-01 to PRJ-08, TL-01 to TL-10, UI-05, VC-03.
//!
//! This crate is a shell created in work breakdown slice 0. The project file format arrives in slice 1, after spike SP-8. Until then a project is an empty in-memory document.

#![warn(missing_docs)]

use openchuck_types as _;

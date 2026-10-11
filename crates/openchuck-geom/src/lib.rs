// SPDX-License-Identifier: AGPL-3.0-or-later
//! Mesh utilities: adjacency, validity checks, transforms, selection algorithms and ray-cast picking.
//!
//! Architecture section 5.4; requirements IMP-04, IMP-05, UI-04.
//!
//! This crate is a shell created in work breakdown slice 0. Mesh checks and transforms arrive in slice 2, selection and picking in slice 10.

#![warn(missing_docs)]

use openchuck_types as _;

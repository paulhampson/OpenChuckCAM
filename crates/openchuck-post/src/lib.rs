// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reusable post-processing library: neutral toolpath IR to G-code through user-editable post definitions.
//!
//! Architecture section 5.3; requirements GC-02 to GC-10, GC-13, INT-03.
//!
//! This crate is a shell created in work breakdown slice 0. The machine program, post definitions and feed modes arrive in slice 6.

#![warn(missing_docs)]

use openchuck_types as _;

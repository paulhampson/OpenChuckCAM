// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reusable 4-axis pathing library: strategies, tool-axis control, A-axis management, linking, gouge protection and feed intent.
//!
//! Architecture section 5.2; requirements OP-01 to OP-17, INT-01 to INT-04.
//!
//! This crate is a shell created in work breakdown slice 0. Strategies, the `SurfaceSource`, `StockModel` and `CollisionOracle` ports and the shared services arrive from slice 5.

#![warn(missing_docs)]

use openchuck_types as _;

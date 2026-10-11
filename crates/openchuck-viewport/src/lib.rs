// SPDX-License-Identifier: AGPL-3.0-or-later
//! wgpu renderer: scene, camera, shading, polyline toolpaths, tool and stock display. Draws data only.
//!
//! Architecture section 5.8; requirements UI-02, UI-03, SIM-01, SIM-02, NFR-03.
//!
//! This crate is a shell created in work breakdown slice 0. The renderer arrives in slice 8 (see decision record 0002). Slice 0 links wgpu so the three-platform CI proves it builds (NFR-02).

#![warn(missing_docs)]

use openchuck_types as _;

// wgpu is linked from slice 0 so the platform CI builds it (NFR-02, decision
// record 0002 outstanding item 2). No wgpu type is part of the public API.
use wgpu as _;

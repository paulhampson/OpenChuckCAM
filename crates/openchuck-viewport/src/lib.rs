// SPDX-License-Identifier: AGPL-3.0-or-later
//! wgpu renderer: scene, camera, shading, polyline toolpaths, tool and stock display. Draws data only.
//!
//! Architecture section 5.8; requirements UI-02, UI-03, SIM-01, SIM-02, NFR-03.
//!
//! - [`camera`]: orbit, pan, zoom, standard views, perspective and
//!   orthographic projection, all testable without a GPU.
//! - [`scene`]: what is drawn, as plain data: meshes with feature edges, and
//!   lines.
//! - [`renderer`]: uploads a scene and draws it into a wgpu render pass.
//! - [`bench`]: the NFR-03 camera path and frame-time statistics.
//!
//! The crate does not depend on egui or any window toolkit: the front end
//! hands it a device, a queue and a render pass (decision record 0026). No
//! wgpu type is stored in a public field; wgpu appears only in the
//! renderer's method signatures, which the front end needs to call it.
//!
//! The stack (egui, `egui_wgpu`, wgpu) is the one decision record 0002
//! accepted after the NFR-03 measurement (OI-09, OI-11).

#![warn(missing_docs)]

pub mod bench;
pub mod camera;
pub mod renderer;
pub mod scene;

pub use camera::{Camera, Projection, StandardView};
pub use renderer::{Frame, Renderer};
pub use scene::{Colour, DisplayMode, LineItem, MeshItem, Scene};
/// The wgpu version the renderer is built against, for front ends that hand
/// it a device.
pub use wgpu;

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reusable 4-axis pathing library: strategies, tool-axis control, A-axis
//! management, linking, gouge protection and feed intent.
//!
//! Architecture section 5.2; requirements OP-01 to OP-17, INT-01 to INT-04;
//! design in decision record 0018 (work breakdown slice 5).
//!
//! The library is built as if it belonged to another project (INT-01): plain
//! data in ([`openchuck_types`]), the neutral toolpath IR out, and three
//! ports a host may implement:
//!
//! - [`SurfaceSource`]: points, normals, rays, distances and tool drops on
//!   the selected surfaces. [`MeshSurface`] implements it for meshes.
//! - [`StockModel`]: the material still to be cut. [`AnalyticStock`] covers
//!   cylinder and box stock.
//! - [`CollisionOracle`]: whether a move is allowed. [`AcceptAll`] relies on
//!   the clearance geometry.
//!
//! A [`Strategy`] makes passes of cut points; [`generate`] checks its
//! parameters against its schema and hands the passes to the shared services
//! in [`assemble`]: A-axis management (OP-15), linking (OP-14) and contact
//! feed with segment durations (OP-13). Strategies are found by name in a
//! [`Registry`] (OP-16); the library's own is [`HelicalFinishing`]. Tool-centre
//! solving is the drop-cutter of [`cutter`], generic over the tool profile
//! (decision record 0012). Regions on the unrolled cylinder and their offsets
//! are in [`unroll`].
//!
//! ```
//! use openchuck_path::{MeshSurface, SurfaceSource, RotaryMapping};
//! use openchuck_types::machine::MachineProfile;
//! use openchuck_types::mesh::TriangleMesh;
//! use openchuck_types::geometry::Point3;
//! use openchuck_types::tool::{Tool, ToolShape};
//! use openchuck_types::units::{Degrees, Millimetres};
//!
//! // A flat square 10 mm above the rotary axis, facing up.
//! let mesh = TriangleMesh {
//!     vertices: vec![
//!         Point3::new(-5.0, -5.0, 10.0),
//!         Point3::new(5.0, -5.0, 10.0),
//!         Point3::new(5.0, 5.0, 10.0),
//!         Point3::new(-5.0, 5.0, 10.0),
//!     ],
//!     triangles: vec![[0, 1, 2], [0, 2, 3]],
//! };
//! let surface = MeshSurface::new(&mesh)?;
//! let tool = Tool {
//!     shape: ToolShape::BallEnd,
//!     diameter: Millimetres(6.0),
//!     flute_length: Millimetres(12.0),
//!     overall_length: Millimetres(50.0),
//!     shank_diameter: Millimetres(6.0),
//!     flute_count: 2,
//!     material: None,
//! }
//! .profile()?;
//! let rotary = RotaryMapping::of_profile(&MachineProfile::example_x_rotary());
//! let line = rotary.tool_line(Millimetres(0.0), Degrees(0.0), Millimetres(0.0));
//! let contact = surface.drop_tool(&tool, &line).expect("the tool lands on the square");
//! assert!((contact.tip.z - 10.0).abs() < 1e-12);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![warn(missing_docs)]

pub mod assemble;
pub mod cutter;
mod error;
pub mod helical;
pub mod parallel;
pub mod params;
pub mod region;
pub mod rotary;
pub mod stock;
pub mod strategy;
pub mod surface;
pub mod unroll;

pub use assemble::{GeneratedPath, PathNote};
pub use cutter::{Contact, ToolLine};
pub use error::{PathError, Result};
pub use helical::HelicalFinishing;
pub use params::{
    CommonParameters, EntryParameters, EntryStyle, ParameterKind, ParameterSchema, ParameterSpec,
    ParameterValue, Parameters, Unit,
};
pub use region::{RegionConstraint, Window};
pub use rotary::RotaryMapping;
pub use stock::{AcceptAll, AnalyticStock, CollisionOracle, StockModel};
pub use strategy::{
    CutPoint, Pass, Registry, Strategy, StrategyInput, generate, generate_in_region,
};
pub use surface::{
    FaceClosest, FacePosition, MeshSurface, ParametricFace, RayHit, SurfacePoint, SurfaceSource, Uv,
};

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
//!   [`CompositeSurface`] joins a model's source to analytic [`Solid`]s (a
//!   held region, retaining tabs) so strategies treat them as part.
//! - [`StockModel`]: the material still to be cut. [`AnalyticStock`] covers
//!   cylinder and box stock.
//! - [`CollisionOracle`]: whether a move is allowed. [`AcceptAll`] relies on
//!   the clearance geometry.
//!
//! A [`Strategy`] makes passes of cut points; [`generate`] checks its
//! parameters against its schema and hands the passes to the shared services
//! in [`assemble`]: A-axis management (OP-15), linking (OP-14) and contact
//! feed with segment durations (OP-13). Strategies are found by name in a
//! [`Registry`] (OP-16); the library's own are [`HelicalFinishing`],
//! [`SimultaneousFinishing`], [`RestMachining`], [`PencilFinishing`] and
//! [`RadialDrilling`], [`EdgeFollowing`] (edge chains from a [`CurveSource`], decision record 0047)
//! and [`Swarf`] (flank milling along the rulings of B-rep faces, decision record 0061). Tool-centre
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
//!     centre_cutting: None,
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
pub mod axial;
pub mod brep;
pub mod brep_edges;
mod candidates;
mod composite;
pub mod curves;
pub mod cutter;
pub mod drill;
pub mod edge_curve;
pub mod edge_following;
pub mod entry_safety;
mod error;
pub mod extend;
mod faces;
pub mod facing;
pub mod field;
pub mod finishing;
pub mod helical;
pub mod holes;
pub mod indexed;
pub mod parallel;
pub mod params;
pub mod pencil;
pub mod program;
pub mod region;
pub mod rest;
pub mod rotary;
pub mod roughing;
mod scallop;
mod section;
pub mod simultaneous;
mod solid;
pub mod stock;
pub mod strategy;
pub mod surface;
pub mod swarf;
pub mod unroll;
mod wrap_fit;

pub use assemble::{GeneratedPath, PathNote, PathWarning, ShankBreak, Unreached, UnreachedReason};
pub use axial::AxialFinishing;
pub use brep::{BrepSurface, DROP_TOLERANCE, ExactDrop, ExactFace, ball_radius};
pub use brep_edges::BrepEdges;
pub use composite::CompositeSurface;
pub use curves::{ChosenSegments, CurveSource, EdgeChain, MeshFeatureEdges};
pub use cutter::{Contact, ToolLine};
pub use drill::RadialDrilling;
pub use edge_following::EdgeFollowing;
pub use error::{PathError, Result};
pub use facing::Facing;
pub use field::HeightStock;
pub use helical::HelicalFinishing;
pub use holes::{Hole, HoleEnd, find_holes};
pub use indexed::IndexedFinishing;
pub use params::{
    CommonParameters, EntryParameters, EntryStyle, ParameterKind, ParameterSchema, ParameterSpec,
    ParameterValue, Parameters, Unit,
};
pub use pencil::PencilFinishing;
pub use region::{BoundaryLimits, Protection, RegionConstraint, WatchedSurface, Window};
pub use rest::{RemainingMap, RestMachining};
pub use rotary::RotaryMapping;
pub use roughing::RotaryRoughing;
pub use simultaneous::SimultaneousFinishing;
pub use solid::Solid;
pub use stock::{AcceptAll, AnalyticStock, CollisionOracle, StockModel};
pub use strategy::{
    CutPoint, LeaveZone, Pass, PassStock, Registry, Shank, Strategy, StrategyGroup, StrategyInput,
    StrategyOutput, generate, generate_in_region, generate_in_zones,
};
pub use surface::{
    FaceClosest, FacePosition, FaceRef, MeshSurface, ParametricFace, RayHit, SurfacePoint,
    SurfaceSource, Uv,
};
pub use swarf::Swarf;

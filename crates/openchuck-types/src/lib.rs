// SPDX-License-Identifier: AGPL-3.0-or-later
//! Plain data shared by every OpenChuckCAM library (architecture section 5.1).
//!
//! Everything here is serde-serialisable, uses double precision and works in
//! millimetres and degrees (NFR-05). The crate holds:
//!
//! - unit newtypes ([`units`]) and geometry ([`geometry`]),
//! - length units for imported models ([`units::LengthUnit`]),
//! - mesh and B-rep data ([`mesh`], [`brep`], [`model`]), model placement
//!   ([`transform`]), stock ([`stock`]) and tools ([`tool`]),
//! - the machine profile ([`machine`]) and the one shared forward-kinematics
//!   implementation ([`kinematics`]),
//! - the neutral toolpath IR ([`ir`], GC-01),
//! - the job context ([`job`]) and typed errors ([`Error`]).
//!
//! It depends on no other OpenChuckCAM crate and names no controller, axis
//! letter or G-code dialect.
//!
//! ```
//! use openchuck_types::kinematics::{machine_xyz, tool_tip_in_model, MachinePosition};
//! use openchuck_types::machine::MachineProfile;
//! use openchuck_types::geometry::Point3;
//! use openchuck_types::units::Degrees;
//!
//! let profile = MachineProfile::example_x_rotary();
//! let tip = Point3::new(10.0, 0.0, 15.0);
//! let xyz = machine_xyz(&profile, tip, Degrees(90.0));
//! let back = tool_tip_in_model(&profile, &MachinePosition { xyz, a: Degrees(90.0) });
//! assert!(back.distance(tip) < 1e-9);
//! ```

#![warn(missing_docs)]

pub mod brep;
mod error;
pub mod geometry;
pub mod ir;
pub mod job;
pub mod kinematics;
pub mod machine;
pub mod mesh;
pub mod model;
pub mod notice;
pub mod stock;
pub mod tool;
pub mod transform;
pub mod units;

pub use error::{Error, Result};

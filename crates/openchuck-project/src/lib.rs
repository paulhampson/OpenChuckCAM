// SPDX-License-Identifier: AGPL-3.0-or-later
//! Project document model, text persistence, stable IDs and order keys, selection sets and content hashes.
//!
//! Architecture section 5.6; requirements PRJ-01 to PRJ-08, TL-01 to TL-11, UI-05, VC-03.
//!
//! The crate provides the document ([`Project`]) with stable IDs ([`id`]) and
//! order keys ([`order`], PRJ-05), model content hashes ([`ContentHash`],
//! PRJ-02), selection sets with reserved descriptor fields (PRJ-06, PRJ-07),
//! the project folder as TOML files with schema versions and migration
//! ([`file`](mod@file), PRJ-01, PRJ-04), the cache folder for generated data
//! ([`cache`], PRJ-03), machine profile files ([`machines`], SET-01) and the
//! tool library, custom profile files and snapshot comparison ([`tools`],
//! TL-01, TL-03, TL-07). Decision records 0006, 0008, 0011 and 0014.
//!
//! ```
//! use openchuck_project::id::{OperationId, SetupId};
//! use openchuck_project::{Item, Operation, OrderKey, Placement, Project, Setup, file};
//!
//! let mut project = Project::new("bracket");
//! let setup = SetupId::new();
//! let new_setup = Setup::new(setup, "Setup 1");
//! project.insert(Item::Setup(new_setup), Placement::End)?;
//! project.insert(
//!     Item::Operation(Operation {
//!         enabled: true,
//!         id: OperationId::new(),
//!         order: OrderKey::default(),
//!         name: "Rough".into(),
//!         setup,
//!         strategy: "rotary_rough".into(),
//!         tool: None,
//!         selections: Vec::new(),
//!         boundary: None,
//!         avoid: Vec::new(),
//!         check: Vec::new(),
//!         restriction_options: Default::default(),
//!         spindle_speed: None,
//!         material: None,
//!         parameters: Default::default(),
//!     }),
//!     Placement::End,
//! )?;
//!
//! let folder = std::env::temp_dir().join(format!("openchuck-doc-{}", std::process::id()));
//! file::save(&project, &folder)?;
//! assert_eq!(file::load(&folder)?, project);
//! # std::fs::remove_dir_all(&folder).ok();
//! # Ok::<(), openchuck_project::ProjectError>(())
//! ```

#![warn(missing_docs)]

pub mod cache;
mod error;
pub mod file;
mod hash;
pub mod id;
pub mod machines;
mod model;
pub mod new_folder;
pub mod order;
pub mod tools;

pub use error::ProjectError;
pub use hash::ContentHash;
pub use model::{
    BoundaryOptions, Descriptor, Item, ItemId, ListKind, MeshEdge, ModelCheck, ModelRef, OldKeys,
    OldPlace, OnBreak, Operation, ParameterValue, Placement, Project, RelativePath,
    RestrictionOptions, SelectionSet, SelectionTarget, SetOptions, Setup, ToolSnapshot,
    is_set_colour,
};
pub use order::OrderKey;

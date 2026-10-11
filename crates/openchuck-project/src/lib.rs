// SPDX-License-Identifier: AGPL-3.0-or-later
//! Project document model, text persistence, stable IDs and order keys, selection sets and content hashes.
//!
//! Architecture section 5.6; requirements PRJ-01 to PRJ-08, TL-01 to TL-10, UI-05, VC-03.
//!
//! Slice 1 so far provides the in-memory document ([`Project`]) with stable IDs
//! ([`id`]), model content hashes ([`ContentHash`], PRJ-02), selection sets with
//! reserved descriptor fields (PRJ-06, PRJ-07) and the cache folder for
//! generated data ([`cache`], PRJ-03). The file format, order keys, schema
//! versions and migration wait for spike SP-8 (decision record 0005).
//!
//! ```
//! use openchuck_project::id::{OperationId, SetupId};
//! use openchuck_project::{Item, Operation, Project, Setup};
//!
//! let mut project = Project::new("bracket");
//! let setup = SetupId::new();
//! project.insert(Item::Setup(Setup { id: setup, name: "Setup 1".into() }), None)?;
//! project.insert(
//!     Item::Operation(Operation {
//!         id: OperationId::new(),
//!         name: "Rough".into(),
//!         setup,
//!         strategy: "rotary_rough".into(),
//!         tool: None,
//!         selections: Vec::new(),
//!         parameters: Default::default(),
//!     }),
//!     None,
//! )?;
//! assert_eq!(project.operations().len(), 1);
//! # Ok::<(), openchuck_project::ProjectError>(())
//! ```

#![warn(missing_docs)]

pub mod cache;
mod error;
mod hash;
pub mod id;
mod model;

pub use error::ProjectError;
pub use hash::ContentHash;
pub use model::{
    Descriptor, Item, ItemId, MachineRef, ModelCheck, ModelRef, Operation, ParameterValue, Project,
    RelativePath, SelectionSet, SelectionTarget, Setup, ToolSnapshot,
};

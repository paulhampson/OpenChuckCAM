// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::PathBuf;

/// An error from the project model, worded for the user (NFR-07).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProjectError {
    /// Text that should be an ID is not one.
    #[error("'{text}' is not a valid {kind} ID")]
    InvalidId {
        /// The kind of ID expected.
        kind: &'static str,
        /// The text found.
        text: String,
    },
    /// Text that should be a content hash is not one.
    #[error("'{0}' is not a valid content hash (expected 'blake3:' and 64 lower-case hex digits)")]
    InvalidHash(String),
    /// A model path is not a usable relative path.
    #[error("invalid model path '{path}': {reason}")]
    InvalidPath {
        /// The path given.
        path: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// An order key is not valid.
    #[error("invalid order key '{key}': {reason}")]
    InvalidOrderKey {
        /// The key found.
        key: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// A project file still holds Git merge conflict markers.
    #[error("'{}' has an unresolved merge conflict at line {line}; resolve it in Git, then open the project again", path.display())]
    MergeConflict {
        /// The file.
        path: PathBuf,
        /// The first marker's line, from 1.
        line: usize,
    },
    /// A project file was written by a newer version of OpenChuckCAM.
    #[error("'{}' uses project schema {found}, but this version reads up to {supported}; open it with a newer OpenChuckCAM", path.display())]
    SchemaTooNew {
        /// The file.
        path: PathBuf,
        /// Its schema version.
        found: i64,
        /// The newest this build reads.
        supported: i64,
    },
    /// A project file could not be read as a project file.
    #[error("cannot read '{}': {reason}", path.display())]
    BadFile {
        /// The file.
        path: PathBuf,
        /// What is wrong, from the parser or a check.
        reason: String,
    },
    /// The folder has no `project.toml`.
    #[error("'{}' is not a project folder: it has no project.toml", .0.display())]
    NotAProject(PathBuf),
    /// Two items share an ID.
    #[error("two items use the {kind} ID {id}")]
    DuplicateId {
        /// The kind of item.
        kind: &'static str,
        /// The repeated ID.
        id: String,
    },
    /// No item has the ID.
    #[error("there is no {kind} with ID {id}")]
    NotFound {
        /// The kind of item.
        kind: &'static str,
        /// The missing ID.
        id: String,
    },
    /// An item refers to an item that does not exist.
    #[error("{from} refers to {kind} {id}, which does not exist")]
    UnknownReference {
        /// The referring item, for example `operation 6f1c…`.
        from: String,
        /// The kind of item referred to.
        kind: &'static str,
        /// The missing ID.
        id: String,
    },
    /// An item cannot be removed because another refers to it.
    #[error("{kind} {id} cannot be removed because {by} uses it")]
    InUse {
        /// The kind of item.
        kind: &'static str,
        /// Its ID.
        id: String,
        /// The first item that uses it.
        by: String,
    },
    /// A position is past the end of a list.
    #[error("position {index} is past the end of the {kind} list ({len} items)")]
    BadIndex {
        /// The kind of item.
        kind: &'static str,
        /// The position given.
        index: usize,
        /// The list length.
        len: usize,
    },
    /// A value is invalid.
    #[error(transparent)]
    Invalid(#[from] openchuck_types::Error),
    /// A file could not be read or written.
    #[error("cannot access '{}': {source}", path.display())]
    Io {
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
}

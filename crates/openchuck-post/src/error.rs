// SPDX-License-Identifier: AGPL-3.0-or-later
//! Errors of the post-processing library (NFR-07).

/// An error from the post-processing library. Messages are written for the
/// user: they name the group, move or setting that is wrong.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PostError {
    /// Shared data was invalid (the toolpath, a value that is not finite).
    #[error(transparent)]
    Data(openchuck_types::Error),
    /// A group names a tool that has no entry in the tool settings (GC-14).
    #[error(
        "group {group} (operation \"{operation}\"): tool \"{tool}\" has no tool number or spindle setting"
    )]
    UnknownTool {
        /// Index of the group in the toolpath.
        group: usize,
        /// The operation the group came from.
        operation: String,
        /// The tool tag that has no settings.
        tool: String,
    },
    /// A move cannot be given a feed.
    #[error("group {group}, move {segment}: {reason}")]
    Feed {
        /// Index of the group in the toolpath.
        group: usize,
        /// Index of the move in the group.
        segment: usize,
        /// What is wrong.
        reason: String,
    },
    /// A post definition could not be read or is not valid (GC-03).
    #[error("post definition {source_name}: {reason}")]
    Definition {
        /// The file or folder the definition came from.
        source_name: String,
        /// What is wrong, naming the key or line.
        reason: String,
    },
    /// The template failed to compile or render; the message names the line.
    #[error("{0}")]
    Template(String),
    /// A setting of the conversion is not usable.
    #[error("{name}: {reason}")]
    Setting {
        /// The setting's name.
        name: String,
        /// What is wrong with it.
        reason: String,
    },
}

impl From<openchuck_types::Error> for PostError {
    fn from(error: openchuck_types::Error) -> Self {
        Self::Data(error)
    }
}

/// Result alias for the library.
pub type Result<T> = std::result::Result<T, PostError>;

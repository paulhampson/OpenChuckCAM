// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typed errors of the pathing library (NFR-07).

/// An error from the pathing library. Messages are written for the user: they
/// name the field or the place in the toolpath and say what is wrong.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PathError {
    /// Shared data was invalid (a tool, a mesh, a profile value).
    #[error(transparent)]
    Data(openchuck_types::Error),
    /// An operation parameter is missing, unknown, of the wrong type or out
    /// of range (OP-16).
    #[error("parameter {name}: {reason}")]
    Parameter {
        /// The parameter's name in the schema.
        name: String,
        /// What is wrong with it.
        reason: String,
    },
    /// No strategy of that name is registered (OP-16).
    #[error("no strategy named \"{0}\" is available")]
    UnknownStrategy(String),
    /// The surface cannot be used (for example an empty mesh).
    #[error("surface: {0}")]
    Surface(String),
    /// The rotary axis cannot follow the toolpath within its limits (OP-15).
    #[error("A axis: {0}")]
    RotaryLimit(String),
    /// The strategy could not produce a toolpath from its inputs.
    #[error("{0}")]
    Strategy(String),
    /// The job was cancelled.
    #[error("the job was cancelled")]
    Cancelled,
}

impl PathError {
    /// Shorthand for [`PathError::Parameter`].
    pub fn parameter(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Parameter {
            name: name.into(),
            reason: reason.into(),
        }
    }
}

/// Result type of the pathing library.
pub type Result<T> = std::result::Result<T, PathError>;

/// The shared cancel error becomes [`PathError::Cancelled`].
impl From<openchuck_types::Error> for PathError {
    fn from(error: openchuck_types::Error) -> Self {
        match error {
            openchuck_types::Error::Cancelled => Self::Cancelled,
            other => Self::Data(other),
        }
    }
}

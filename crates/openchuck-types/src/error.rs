// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typed errors shared by the library family (NFR-07).

/// An error from validating or processing shared data.
///
/// Messages are written for the user: they name the field and say what is wrong.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A value is out of range, not finite, or inconsistent with another value.
    #[error("invalid {field}: {reason}")]
    InvalidValue {
        /// Path to the offending field, for example `groups[0].segments[3].contact_feed`.
        field: String,
        /// What is wrong with it.
        reason: String,
    },
    /// The job was cancelled through its [`CancelToken`](crate::job::CancelToken).
    #[error("the job was cancelled")]
    Cancelled,
}

impl Error {
    /// Shorthand for [`Error::InvalidValue`].
    pub fn invalid(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidValue {
            field: field.into(),
            reason: reason.into(),
        }
    }
}

/// Result type used across the shared data crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Fails unless `value` is finite.
pub(crate) fn require_finite(field: &str, value: f64) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Error::invalid(
            field,
            format!("{value} is not a finite number"),
        ))
    }
}

/// Fails unless `value` is finite and strictly positive.
pub(crate) fn require_positive(field: &str, value: f64) -> Result<()> {
    require_finite(field, value)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(Error::invalid(
            field,
            format!("{value} must be greater than zero"),
        ))
    }
}

/// Fails unless `value` is finite and not negative.
pub(crate) fn require_non_negative(field: &str, value: f64) -> Result<()> {
    require_finite(field, value)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(Error::invalid(
            field,
            format!("{value} must not be negative"),
        ))
    }
}

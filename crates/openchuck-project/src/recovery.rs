// SPDX-License-Identifier: AGPL-3.0-or-later
//! Complete, checksummed project snapshots (PRJ-08, NFR-07).
//!
//! This is a separate recovery container, not a project-schema change.
//! It holds the existing persisted project texts and references to models;
//! it contains neither model bytes nor generated or UI draft state.

use crate::{Project, ProjectError, file};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod store;
pub use store::{Catalog, DIRECTORY, Entry, Store, read_choice};

/// Version of the independent recovery envelope.
pub const FORMAT: u32 = 1;
/// Bound input before allocating parser structures (64 MiB of text).
pub const MAX_BYTES: usize = 64 * 1024 * 1024;

/// Retention series, independent of the project file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// One of three scheduled states.
    Periodic,
    /// The separately replaced short-term state.
    Interim,
}

/// Capture identity; timestamps are for display, never for race ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    /// Stable identity in recovery storage, outside tracked project files.
    pub project_id: String,
    /// Identity of the instance's open session.
    pub session_id: String,
    /// Committed revision captured by the worker.
    pub revision: u64,
    /// Capture time, milliseconds after the Unix epoch.
    pub timestamp_ms: u64,
    /// Which retention series owns this checkpoint.
    pub kind: Kind,
}

/// A complete persisted state, validated on capture and read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    metadata: Metadata,
    files: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    format: u32,
    payload: String,
    checksum: String,
}

fn invalid(reason: impl ToString) -> ProjectError {
    ProjectError::BadFile {
        path: "recovery checkpoint".into(),
        reason: reason.to_string(),
    }
}

impl Metadata {
    fn validate(&self) -> Result<(), ProjectError> {
        for (label, id) in [("project", &self.project_id), ("session", &self.session_id)] {
            let parsed = uuid::Uuid::parse_str(id)
                .map_err(|_| invalid(format!("invalid {label} identity")))?;
            if parsed.is_nil() || parsed.to_string() != *id {
                return Err(invalid(format!("invalid {label} identity")));
            }
        }
        Ok(())
    }
}

impl Snapshot {
    /// Render only the committed document, using ordinary project persistence.
    pub fn capture(project: &Project, metadata: Metadata) -> Result<Self, ProjectError> {
        metadata.validate()?;
        Ok(Self {
            metadata,
            files: file::render(project)?,
        })
    }

    /// Metadata validated together with the snapshot's contents.
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Canonical project-file texts; these are never written by recovery itself.
    pub fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }

    /// Restore into memory. Relative model references still refer to the
    /// original project folder; the caller must report missing/changed files.
    pub fn project(&self) -> Result<Project, ProjectError> {
        file::load_files(&self.files)
    }

    /// Deterministic checksummed container ready for staged publication.
    pub fn encode(&self) -> Result<String, ProjectError> {
        self.validate()?;
        let payload = toml::to_string(self).map_err(invalid)?;
        let checksum = blake3::hash(payload.as_bytes()).to_hex().to_string();
        let text = toml::to_string(&Envelope {
            format: FORMAT,
            payload,
            checksum,
        })
        .map_err(invalid)?;
        if text.len() > MAX_BYTES {
            return Err(invalid("checkpoint exceeds the 64 MiB recovery limit"));
        }
        Ok(text)
    }

    /// Bounded malformed-input entry point, also used by the fuzz target.
    pub fn decode(text: &str) -> Result<Self, ProjectError> {
        Self::decode_with_project(text).map(|(snapshot, _)| snapshot)
    }

    fn decode_with_project(text: &str) -> Result<(Self, Project), ProjectError> {
        if text.len() > MAX_BYTES {
            return Err(invalid("checkpoint exceeds the 64 MiB recovery limit"));
        }
        let envelope: Envelope = toml::from_str(text).map_err(invalid)?;
        if envelope.format != FORMAT {
            return Err(invalid(format!(
                "unsupported recovery format {}; this build reads {FORMAT}",
                envelope.format
            )));
        }
        if blake3::hash(envelope.payload.as_bytes()).to_hex().as_str() != envelope.checksum {
            return Err(invalid(
                "checkpoint checksum does not match; retain this file and choose another checkpoint",
            ));
        }
        let snapshot: Self = toml::from_str(&envelope.payload).map_err(invalid)?;
        let project = snapshot.validate()?;
        Ok((snapshot, project))
    }

    fn validate(&self) -> Result<Project, ProjectError> {
        self.metadata.validate()?;
        // Reject arbitrary paths and ignored extra files, even though the
        // ordinary loader never writes them. Recovery must be complete and
        // must never become a general-purpose file extraction mechanism.
        for path in self.files.keys() {
            if path == "project.toml" || path == "machine.toml" {
                continue;
            }
            let Some((folder, name)) = path.split_once('/') else {
                return Err(invalid(format!("unexpected checkpoint file '{path}'")));
            };
            if !["setups", "tools", "selections", "operations"].contains(&folder)
                || name
                    .strip_suffix(".toml")
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
                    .is_none()
            {
                return Err(invalid(format!("unexpected checkpoint file '{path}'")));
            }
        }
        self.project()
    }
}

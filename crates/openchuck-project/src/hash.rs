// SPDX-License-Identifier: AGPL-3.0-or-later
//! Content hashes of model files (PRJ-02).
//!
//! A model file is referenced by relative path together with the BLAKE3 hash
//! of its bytes, so a changed model is detected when the project is loaded.
//! The text form names the algorithm, `blake3:` followed by 64 lower-case hex
//! digits, so another algorithm can be added later without ambiguity.

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ProjectError;

const PREFIX: &str = "blake3:";

/// The BLAKE3 hash of a file's contents.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// The hash of `bytes`.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    /// The hash of the file at `path`, read in chunks.
    pub fn of_file(path: &Path) -> Result<Self, ProjectError> {
        let io = |source| ProjectError::Io {
            path: path.to_path_buf(),
            source,
        };
        let file = std::fs::File::open(path).map_err(io)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update_reader(file).map_err(io)?;
        Ok(Self(*hasher.finalize().as_bytes()))
    }

    /// The raw hash.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(PREFIX)?;
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({self})")
    }
}

impl FromStr for ContentHash {
    type Err = ProjectError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || ProjectError::InvalidHash(text.to_owned());
        let hex = text.strip_prefix(PREFIX).ok_or_else(invalid)?;
        if hex.len() != 64 {
            return Err(invalid());
        }
        let mut bytes = [0u8; 32];
        for (byte, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
            let digits = std::str::from_utf8(pair).map_err(|_| invalid())?;
            // Upper-case digits would give a second spelling of the same hash.
            if digits.bytes().any(|d| d.is_ascii_uppercase()) {
                return Err(invalid());
            }
            *byte = u8::from_str_radix(digits, 16).map_err(|_| invalid())?;
        }
        Ok(Self(bytes))
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fresh project destinations for the measurement harness (NFR-07).
use std::path::Path;

/// Reserve a fresh STEP or STL destination without replacing caller data.
/// Existing directories (even empty ones), files and symlinks are refused.
/// Cleanup is therefore limited to destinations this run created itself.
pub fn create_destination(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = dir.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(dir).map_err(|error| {
        std::io::Error::new(error.kind(), format!(
            "cannot create fresh project directory {}: {error}; choose a new --keep destination (existing paths are never replaced)",
            dir.display()
        ))
    })?;
    Ok(())
}

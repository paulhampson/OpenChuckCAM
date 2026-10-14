// SPDX-License-Identifier: AGPL-3.0-or-later
//! `cargo xtask golden`: the golden file audit (work breakdown slice 6 review
//! checklist; decision record 0020 item 14).
//!
//! Runs `crates/openchuck-app/tests/golden_audit.rs`, which reads the committed
//! G-code files and checks them against things that do not come from the post
//! or the path library: the post conventions, the static checks, the exact
//! analytic parts (through the independent kinematic oracle and gouge
//! checker), the contact feed under the controller's rule, and agreement
//! between the Z1 and LinuxCNC programs. It also runs with the ordinary test
//! suite; this command prints what it measured.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::Findings;

/// Runs the audit and returns a finding if it fails.
pub fn check(root: &Path) -> Result<Findings> {
    let output = Command::new("cargo")
        .args([
            "test",
            "--quiet",
            "-p",
            "openchuck-app",
            "--test",
            "golden_audit",
            "--",
            "--nocapture",
            "--test-threads=1",
        ])
        .current_dir(root)
        .output()
        .context("cannot run cargo test")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout
        .lines()
        .filter(|l| l.starts_with("  ") || l.contains(".nc") || l.contains(".ngc"))
    {
        println!("{line}");
    }
    if output.status.success() {
        return Ok(Vec::new());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let why: Vec<&str> = stderr
        .lines()
        .chain(stdout.lines())
        .filter(|l| {
            l.contains("panicked at")
                || (!l.starts_with(' ') && l.contains(".nc") || l.contains(".ngc:"))
        })
        .take(6)
        .collect();
    Ok(vec![format!(
        "the golden audit failed: {}",
        why.join(" | ")
    )])
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! The NFR-12 notice, shared so the CLI, the UI and the post all use the same words.

/// The statement that generated G-code is the user's to verify (NFR-12).
pub const VERIFY_GCODE_NOTICE: &str = "Generated G-code must be verified by the user before it is run on a machine. \
OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup.";

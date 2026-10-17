// SPDX-License-Identifier: AGPL-3.0-or-later
//! The default tool library (TL-12; decision record 0081): common standard
//! tools of each type in TL-02 and TL-03, embedded in the program and
//! installed into an empty user library on first run. No network is used
//! (NFR-09).
//!
//! A small ledger beside the library, [`LEDGER_FILE`], lists the IDs of the
//! defaults already installed. An ID in the ledger is never installed again,
//! so a default the user deleted stays deleted, and an installed default is
//! never rewritten, so an edited one stays as edited. A default the ledger
//! does not know is installed, which is how a later release adds one. Further
//! embedded sets (the Makera Z1 tools and tool set) join [`EMBEDDED`] without
//! a format change.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::id::ToolId;

use crate::error::ProjectError;

use super::{LibraryTool, ToolLibrary, parse_tool_file};

/// The ledger file in the library folder. It does not end in `.toml`, so the
/// library does not read it as a tool.
pub const LEDGER_FILE: &str = ".installed-defaults";

/// The embedded default tool files, in the order they are installed.
const EMBEDDED: &[(&str, &str)] = &[
    (
        "01-1-mm-flat.toml",
        include_str!("../../defaults/tools/01-1-mm-flat.toml"),
    ),
    (
        "02-2-mm-flat.toml",
        include_str!("../../defaults/tools/02-2-mm-flat.toml"),
    ),
    (
        "03-3-mm-flat.toml",
        include_str!("../../defaults/tools/03-3-mm-flat.toml"),
    ),
    (
        "04-3-175-mm-flat.toml",
        include_str!("../../defaults/tools/04-3-175-mm-flat.toml"),
    ),
    (
        "05-4-mm-flat.toml",
        include_str!("../../defaults/tools/05-4-mm-flat.toml"),
    ),
    (
        "06-6-mm-flat.toml",
        include_str!("../../defaults/tools/06-6-mm-flat.toml"),
    ),
    (
        "07-1-mm-ball.toml",
        include_str!("../../defaults/tools/07-1-mm-ball.toml"),
    ),
    (
        "08-2-mm-ball.toml",
        include_str!("../../defaults/tools/08-2-mm-ball.toml"),
    ),
    (
        "09-3-mm-ball.toml",
        include_str!("../../defaults/tools/09-3-mm-ball.toml"),
    ),
    (
        "10-3-175-mm-ball.toml",
        include_str!("../../defaults/tools/10-3-175-mm-ball.toml"),
    ),
    (
        "11-6-mm-ball.toml",
        include_str!("../../defaults/tools/11-6-mm-ball.toml"),
    ),
    (
        "12-3-mm-bull-nose-r0-5.toml",
        include_str!("../../defaults/tools/12-3-mm-bull-nose-r0-5.toml"),
    ),
    (
        "13-6-mm-bull-nose-r1.toml",
        include_str!("../../defaults/tools/13-6-mm-bull-nose-r1.toml"),
    ),
    (
        "14-tapered-ball-r0-25-5-deg.toml",
        include_str!("../../defaults/tools/14-tapered-ball-r0-25-5-deg.toml"),
    ),
    (
        "15-tapered-ball-r0-25-10-deg.toml",
        include_str!("../../defaults/tools/15-tapered-ball-r0-25-10-deg.toml"),
    ),
    (
        "16-30-degree-v.toml",
        include_str!("../../defaults/tools/16-30-degree-v.toml"),
    ),
    (
        "17-60-degree-v.toml",
        include_str!("../../defaults/tools/17-60-degree-v.toml"),
    ),
    (
        "18-90-degree-v.toml",
        include_str!("../../defaults/tools/18-90-degree-v.toml"),
    ),
    (
        "19-90-degree-chamfer.toml",
        include_str!("../../defaults/tools/19-90-degree-chamfer.toml"),
    ),
    (
        "20-1-mm-drill.toml",
        include_str!("../../defaults/tools/20-1-mm-drill.toml"),
    ),
    (
        "21-2-mm-drill.toml",
        include_str!("../../defaults/tools/21-2-mm-drill.toml"),
    ),
    (
        "22-3-mm-drill.toml",
        include_str!("../../defaults/tools/22-3-mm-drill.toml"),
    ),
    (
        "23-5-mm-drill.toml",
        include_str!("../../defaults/tools/23-5-mm-drill.toml"),
    ),
];

/// What an installation did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallReport {
    /// Defaults written to the library.
    pub installed: Vec<ToolId>,
    /// Defaults left alone because an earlier run installed them, whether the
    /// user has since kept, edited or deleted them.
    pub already_installed: usize,
    /// Defaults the library already held, for example by import; they are
    /// recorded in the ledger and not rewritten.
    pub already_present: usize,
}

/// The embedded default tools, parsed. Fails only if the program was built
/// with a faulty file, which the tests rule out.
pub fn default_tools() -> Result<Vec<LibraryTool>, ProjectError> {
    EMBEDDED
        .iter()
        .map(|(name, text)| parse_tool_file(std::path::Path::new(name), text))
        .collect()
}

impl ToolLibrary {
    /// Installs the embedded default tools this library has not had before.
    /// Never overwrites or restores a tool (TL-12).
    pub fn install_defaults(&mut self) -> Result<InstallReport, ProjectError> {
        let ledger_path: PathBuf = self.dir().join(LEDGER_FILE);
        let mut ledger = read_ledger(&ledger_path)?;
        let mut report = InstallReport::default();
        let mut changed = false;
        for tool in default_tools()? {
            if ledger.contains(&tool.id) {
                report.already_installed += 1;
            } else if self.get(tool.id).is_some() {
                report.already_present += 1;
                ledger.insert(tool.id);
                changed = true;
            } else {
                let id = tool.id;
                self.save_tool(tool)?;
                report.installed.push(id);
                ledger.insert(id);
                changed = true;
            }
        }
        if changed {
            write_ledger(&ledger_path, &ledger)?;
        }
        Ok(report)
    }
}

fn read_ledger(path: &std::path::Path) -> Result<BTreeSet<ToolId>, ProjectError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(e) => return Err(super::library::io(path)(e)),
    };
    // A line that is not an ID is ignored: a damaged ledger can at worst
    // reinstall a default, never overwrite a tool.
    Ok(text
        .lines()
        .filter_map(|l| l.trim().parse::<ToolId>().ok())
        .collect())
}

fn write_ledger(path: &std::path::Path, ledger: &BTreeSet<ToolId>) -> Result<(), ProjectError> {
    let mut text = String::from(
        "# Default tools OpenChuckCAM has installed here (TL-12). Do not edit.\n# A listed tool is never installed again.\n",
    );
    for id in ledger {
        text.push_str(&format!("{id}\n"));
    }
    std::fs::write(path, text).map_err(super::library::io(path))
}

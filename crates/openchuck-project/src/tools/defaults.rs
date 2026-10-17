// SPDX-License-Identifier: AGPL-3.0-or-later
//! The default tool library (TL-12; decision record 0081): common standard
//! tools of each type in TL-02 and TL-03 and the Makera Z1 reference tools
//! with the Z1 default tool set (TL-11), embedded in the program and
//! installed into the user's library on first run. No network is used
//! (NFR-09).
//!
//! A small ledger beside the library, [`LEDGER_FILE`], lists the IDs of the
//! defaults already installed, and the machine of each default tool set. An
//! ID in the ledger is never installed again, so a default the user deleted
//! stays deleted, and an installed default is never rewritten, so an edited
//! one stays as edited. A default the ledger does not know is installed,
//! which is how a later release adds one. The explicit
//! [`ToolLibrary::restore_defaults`] reinstalls what is missing and still
//! never overwrites a file.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::id::ToolId;

use crate::error::ProjectError;

use super::{LibraryTool, ToolLibrary, ToolSet, parse_tool_file, parse_tool_set};

/// The ledger file in the library folder. It does not end in `.toml`, so the
/// library does not read it as a tool.
pub const LEDGER_FILE: &str = ".installed-defaults";

/// A ledger line for a default tool set starts with this word, then the
/// machine name; any other line is a tool ID.
const SET_PREFIX: &str = "tool-set ";

/// The embedded standard tools (common types and sizes), in install order.
const STANDARD: &[(&str, &str)] = &[
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

/// The embedded Makera Z1 reference tools, in install order.
const Z1: &[(&str, &str)] = &[
    (
        "101-z1-o-flute-metal-3-175x12.toml",
        include_str!("../../defaults/z1/101-z1-o-flute-metal-3-175x12.toml"),
    ),
    (
        "102-z1-o-flute-metal-2x8.toml",
        include_str!("../../defaults/z1/102-z1-o-flute-metal-2x8.toml"),
    ),
    (
        "103-z1-o-flute-metal-1x3.toml",
        include_str!("../../defaults/z1/103-z1-o-flute-metal-1x3.toml"),
    ),
    (
        "104-z1-o-flute-3-175x25.toml",
        include_str!("../../defaults/z1/104-z1-o-flute-3-175x25.toml"),
    ),
    (
        "105-z1-o-flute-3-175x42.toml",
        include_str!("../../defaults/z1/105-z1-o-flute-3-175x42.toml"),
    ),
    (
        "106-z1-corn-0-6x3.toml",
        include_str!("../../defaults/z1/106-z1-corn-0-6x3.toml"),
    ),
    (
        "107-z1-corn-1x7.toml",
        include_str!("../../defaults/z1/107-z1-corn-1x7.toml"),
    ),
    (
        "108-z1-corn-2x10-5.toml",
        include_str!("../../defaults/z1/108-z1-corn-2x10-5.toml"),
    ),
    (
        "109-z1-corn-3x12.toml",
        include_str!("../../defaults/z1/109-z1-corn-3x12.toml"),
    ),
    (
        "110-z1-o-flute-metal-4x12.toml",
        include_str!("../../defaults/z1/110-z1-o-flute-metal-4x12.toml"),
    ),
    (
        "111-z1-o-flute-metal-4x17.toml",
        include_str!("../../defaults/z1/111-z1-o-flute-metal-4x17.toml"),
    ),
    (
        "112-z1-o-flute-metal-4x22.toml",
        include_str!("../../defaults/z1/112-z1-o-flute-metal-4x22.toml"),
    ),
    (
        "113-z1-o-flute-4x22.toml",
        include_str!("../../defaults/z1/113-z1-o-flute-4x22.toml"),
    ),
    (
        "114-z1-o-flute-4x25.toml",
        include_str!("../../defaults/z1/114-z1-o-flute-4x25.toml"),
    ),
    (
        "115-z1-o-flute-4x32.toml",
        include_str!("../../defaults/z1/115-z1-o-flute-4x32.toml"),
    ),
    (
        "116-z1-o-flute-metal-6x17.toml",
        include_str!("../../defaults/z1/116-z1-o-flute-metal-6x17.toml"),
    ),
    (
        "117-z1-o-flute-metal-6x25.toml",
        include_str!("../../defaults/z1/117-z1-o-flute-metal-6x25.toml"),
    ),
    (
        "118-z1-o-flute-metal-6x30.toml",
        include_str!("../../defaults/z1/118-z1-o-flute-metal-6x30.toml"),
    ),
    (
        "119-z1-o-flute-6x22.toml",
        include_str!("../../defaults/z1/119-z1-o-flute-6x22.toml"),
    ),
    (
        "120-z1-o-flute-6x25.toml",
        include_str!("../../defaults/z1/120-z1-o-flute-6x25.toml"),
    ),
    (
        "121-z1-o-flute-6x32.toml",
        include_str!("../../defaults/z1/121-z1-o-flute-6x32.toml"),
    ),
    (
        "201-z1-ball-metal-3-175x10.toml",
        include_str!("../../defaults/z1/201-z1-ball-metal-3-175x10.toml"),
    ),
    (
        "202-z1-ball-metal-2x6.toml",
        include_str!("../../defaults/z1/202-z1-ball-metal-2x6.toml"),
    ),
    (
        "203-z1-ball-metal-1x3.toml",
        include_str!("../../defaults/z1/203-z1-ball-metal-1x3.toml"),
    ),
    (
        "204-z1-ball-3-175x22.toml",
        include_str!("../../defaults/z1/204-z1-ball-3-175x22.toml"),
    ),
    (
        "205-z1-ball-2x12.toml",
        include_str!("../../defaults/z1/205-z1-ball-2x12.toml"),
    ),
    (
        "206-z1-ball-1x4.toml",
        include_str!("../../defaults/z1/206-z1-ball-1x4.toml"),
    ),
    (
        "207-z1-ball-metal-4x12.toml",
        include_str!("../../defaults/z1/207-z1-ball-metal-4x12.toml"),
    ),
    (
        "208-z1-ball-metal-6x18.toml",
        include_str!("../../defaults/z1/208-z1-ball-metal-6x18.toml"),
    ),
    (
        "209-z1-ball-4x12.toml",
        include_str!("../../defaults/z1/209-z1-ball-4x12.toml"),
    ),
    (
        "210-z1-ball-4x22.toml",
        include_str!("../../defaults/z1/210-z1-ball-4x22.toml"),
    ),
    (
        "211-z1-ball-4x32.toml",
        include_str!("../../defaults/z1/211-z1-ball-4x32.toml"),
    ),
    (
        "212-z1-ball-6x12.toml",
        include_str!("../../defaults/z1/212-z1-ball-6x12.toml"),
    ),
    (
        "213-z1-ball-6x22.toml",
        include_str!("../../defaults/z1/213-z1-ball-6x22.toml"),
    ),
    (
        "214-z1-ball-6x32.toml",
        include_str!("../../defaults/z1/214-z1-ball-6x32.toml"),
    ),
    (
        "301-z1-v-bit-60-deg-tip-0-1.toml",
        include_str!("../../defaults/z1/301-z1-v-bit-60-deg-tip-0-1.toml"),
    ),
    (
        "302-z1-v-bit-30-deg-tip-0-2.toml",
        include_str!("../../defaults/z1/302-z1-v-bit-30-deg-tip-0-2.toml"),
    ),
    (
        "303-z1-v-bit-30-deg-tip-0-3.toml",
        include_str!("../../defaults/z1/303-z1-v-bit-30-deg-tip-0-3.toml"),
    ),
    (
        "304-z1-v-bit-metal-30-deg-tip-0-3-shank-4.toml",
        include_str!("../../defaults/z1/304-z1-v-bit-metal-30-deg-tip-0-3-shank-4.toml"),
    ),
    (
        "305-z1-v-bit-metal-30-deg-tip-0-4-shank-4.toml",
        include_str!("../../defaults/z1/305-z1-v-bit-metal-30-deg-tip-0-4-shank-4.toml"),
    ),
    (
        "306-z1-v-bit-metal-30-deg-tip-0-5-shank-4.toml",
        include_str!("../../defaults/z1/306-z1-v-bit-metal-30-deg-tip-0-5-shank-4.toml"),
    ),
    (
        "309-z1-v-bit-30-deg-tip-0-3-shank-4.toml",
        include_str!("../../defaults/z1/309-z1-v-bit-30-deg-tip-0-3-shank-4.toml"),
    ),
    (
        "310-z1-v-bit-30-deg-tip-0-4-shank-4.toml",
        include_str!("../../defaults/z1/310-z1-v-bit-30-deg-tip-0-4-shank-4.toml"),
    ),
    (
        "311-z1-v-bit-30-deg-tip-0-5-shank-4.toml",
        include_str!("../../defaults/z1/311-z1-v-bit-30-deg-tip-0-5-shank-4.toml"),
    ),
    (
        "312-z1-v-bit-20-deg-tip-0-3-shank-6.toml",
        include_str!("../../defaults/z1/312-z1-v-bit-20-deg-tip-0-3-shank-6.toml"),
    ),
    (
        "313-z1-v-bit-20-deg-tip-0-4-shank-6.toml",
        include_str!("../../defaults/z1/313-z1-v-bit-20-deg-tip-0-4-shank-6.toml"),
    ),
    (
        "314-z1-v-bit-20-deg-tip-0-5-shank-6.toml",
        include_str!("../../defaults/z1/314-z1-v-bit-20-deg-tip-0-5-shank-6.toml"),
    ),
    (
        "401-z1-chamfer-90-deg.toml",
        include_str!("../../defaults/z1/401-z1-chamfer-90-deg.toml"),
    ),
    (
        "501-z1-drill-1.toml",
        include_str!("../../defaults/z1/501-z1-drill-1.toml"),
    ),
    (
        "502-z1-drill-2.toml",
        include_str!("../../defaults/z1/502-z1-drill-2.toml"),
    ),
    (
        "503-z1-drill-2-5.toml",
        include_str!("../../defaults/z1/503-z1-drill-2-5.toml"),
    ),
    (
        "504-z1-drill-3.toml",
        include_str!("../../defaults/z1/504-z1-drill-3.toml"),
    ),
];

/// The embedded default tool sets (TL-11).
const TOOL_SETS: &[(&str, &str)] = &[(
    "makera-z1.toml",
    include_str!("../../defaults/tool-sets/makera-z1.toml"),
)];

/// What an installation did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallReport {
    /// Default tools written to the library.
    pub installed: Vec<ToolId>,
    /// Default tool sets written, by machine name.
    pub tool_sets_installed: Vec<String>,
    /// Default tools left alone because an earlier run installed them,
    /// whether the user has since kept, edited or deleted them.
    pub already_installed: usize,
    /// Default tools the library already held, for example by import; they
    /// are recorded in the ledger and not rewritten.
    pub already_present: usize,
    /// Default tool sets not written because a tool set for the machine
    /// already exists or a tool it names is not in the library.
    pub tool_sets_skipped: usize,
}

fn parse_all(files: &[(&str, &str)]) -> Result<Vec<LibraryTool>, ProjectError> {
    files
        .iter()
        .map(|(name, text)| parse_tool_file(std::path::Path::new(name), text))
        .collect()
}

/// The embedded standard tools, parsed. Fails only if the program was built
/// with a faulty file, which the tests rule out.
pub fn standard_tools() -> Result<Vec<LibraryTool>, ProjectError> {
    parse_all(STANDARD)
}

/// The embedded Makera Z1 reference tools, parsed.
pub fn z1_tools() -> Result<Vec<LibraryTool>, ProjectError> {
    parse_all(Z1)
}

/// Every embedded default tool: the standard tools, then the Z1 tools.
pub fn default_tools() -> Result<Vec<LibraryTool>, ProjectError> {
    let mut tools = standard_tools()?;
    tools.extend(z1_tools()?);
    Ok(tools)
}

/// The embedded default tool sets, parsed.
pub fn default_tool_sets() -> Result<Vec<ToolSet>, ProjectError> {
    TOOL_SETS
        .iter()
        .map(|(name, text)| parse_tool_set(std::path::Path::new(name), text))
        .collect()
}

#[derive(Default)]
struct Ledger {
    tools: BTreeSet<ToolId>,
    sets: BTreeSet<String>,
}

impl ToolLibrary {
    /// Installs the embedded default tools and tool sets this library has not
    /// had before. Never overwrites or restores a tool or tool set (TL-12).
    pub fn install_defaults(&mut self) -> Result<InstallReport, ProjectError> {
        let ledger_path: PathBuf = self.dir().join(LEDGER_FILE);
        let mut ledger = read_ledger(&ledger_path)?;
        let mut report = InstallReport::default();
        let mut changed = false;
        for tool in default_tools()? {
            if ledger.tools.contains(&tool.id) {
                report.already_installed += 1;
            } else if self.get(tool.id).is_some() {
                report.already_present += 1;
                ledger.tools.insert(tool.id);
                changed = true;
            } else {
                let id = tool.id;
                self.save_tool(tool)?;
                report.installed.push(id);
                ledger.tools.insert(id);
                changed = true;
            }
        }
        for set in default_tool_sets()? {
            let key = set.machine.to_ascii_lowercase();
            if ledger.sets.contains(&key) {
                report.tool_sets_skipped += 1;
            } else if self.tool_set(&set.machine).is_some() {
                report.tool_sets_skipped += 1;
                ledger.sets.insert(key);
                changed = true;
            } else if set.entries.iter().all(|e| self.get(e.tool).is_some()) {
                let machine = set.machine.clone();
                self.save_tool_set(set)?;
                report.tool_sets_installed.push(machine);
                ledger.sets.insert(key);
                changed = true;
            } else {
                report.tool_sets_skipped += 1;
            }
        }
        if changed {
            write_ledger(&ledger_path, &ledger)?;
        }
        Ok(report)
    }

    /// The explicit "restore default tools" action (TL-12): installs every
    /// embedded default tool and tool set the library does not hold now,
    /// including one the user deleted. A tool or tool set that is in the
    /// library is never touched, edited or not.
    pub fn restore_defaults(&mut self) -> Result<InstallReport, ProjectError> {
        let ledger_path: PathBuf = self.dir().join(LEDGER_FILE);
        let mut ledger = read_ledger(&ledger_path)?;
        let mut report = InstallReport::default();
        for tool in default_tools()? {
            let id = tool.id;
            if self.get(id).is_some() {
                report.already_present += 1;
            } else {
                self.save_tool(tool)?;
                report.installed.push(id);
            }
            ledger.tools.insert(id);
        }
        for set in default_tool_sets()? {
            let key = set.machine.to_ascii_lowercase();
            if self.tool_set(&set.machine).is_some() {
                report.tool_sets_skipped += 1;
            } else if set.entries.iter().all(|e| self.get(e.tool).is_some()) {
                let machine = set.machine.clone();
                self.save_tool_set(set)?;
                report.tool_sets_installed.push(machine);
            } else {
                report.tool_sets_skipped += 1;
            }
            ledger.sets.insert(key);
        }
        write_ledger(&ledger_path, &ledger)?;
        Ok(report)
    }
}

fn read_ledger(path: &std::path::Path) -> Result<Ledger, ProjectError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Ledger::default()),
        Err(e) => return Err(super::library::io(path)(e)),
    };
    // A line that is not understood is ignored: a damaged ledger can at worst
    // reinstall a default, never overwrite a tool or tool set.
    let mut ledger = Ledger::default();
    for line in text.lines() {
        let line = line.trim();
        if let Some(machine) = line.strip_prefix(SET_PREFIX) {
            ledger.sets.insert(machine.trim().to_ascii_lowercase());
        } else if let Ok(id) = line.parse::<ToolId>() {
            ledger.tools.insert(id);
        }
    }
    Ok(ledger)
}

fn write_ledger(path: &std::path::Path, ledger: &Ledger) -> Result<(), ProjectError> {
    let mut text = String::from(
        "# Default tools OpenChuckCAM has installed here (TL-12). Do not edit.\n# A listed tool or tool set is never installed again.\n",
    );
    for id in &ledger.tools {
        text.push_str(&format!("{id}\n"));
    }
    for machine in &ledger.sets {
        text.push_str(&format!("{SET_PREFIX}{machine}\n"));
    }
    std::fs::write(path, text).map_err(super::library::io(path))
}

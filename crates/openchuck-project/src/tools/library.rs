// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool library folder (TL-01), machine tool sets (TL-11) and the
//! comparison of project snapshots with the library (TL-07). Decision record
//! 0014.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use openchuck_types::tool::{ControllerMapping, CuttingTable, Tool, validate_entry};
use serde::{Deserialize, Serialize};

use crate::OrderKey;
use crate::error::ProjectError;
use crate::file::{FileKind, parse, to_text, write_if_changed};
use crate::id::ToolId;
use crate::model::ToolSnapshot;

/// The schema of tool files and tool set files this build writes and reads.
///
/// - 1: slice 4 (decision record 0014).
pub const LIBRARY_SCHEMA: i64 = 1;

/// The folder inside a library that holds machine tool sets (TL-11).
pub const TOOL_SETS_FOLDER: &str = "tool-sets";

/// A tool in a library: geometry and cutting data. Tool numbers belong to
/// the project and to machine tool sets, not to the library tool (decision
/// record 0014 item 4).
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryTool {
    /// Stable ID, kept by the project snapshots made from it.
    pub id: ToolId,
    /// Name shown to the user.
    pub name: String,
    /// Geometry and physical parameters (TL-02 to TL-04).
    pub tool: Tool,
    /// Cutting data by stock material name (TL-05).
    pub cutting_data: CuttingTable,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolFile {
    schema: i64,
    id: ToolId,
    name: String,
    tool: Tool,
    #[serde(default, skip_serializing_if = "CuttingTable::is_empty")]
    cutting_data: CuttingTable,
}

impl LibraryTool {
    /// Fails if the name is empty, or the tool or its cutting data is invalid.
    pub fn validate(&self) -> Result<(), ProjectError> {
        if self.name.trim().is_empty() {
            return Err(openchuck_types::Error::invalid("name", "a tool needs a name").into());
        }
        Ok(validate_entry(
            &self.tool,
            &ControllerMapping::numbered(1),
            &self.cutting_data,
        )?)
    }

    /// The project snapshot of this tool with the same ID and the given
    /// controller values (TL-06, TL-07). The project sets the order key when
    /// the snapshot is added.
    pub fn snapshot(&self, controller: ControllerMapping) -> ToolSnapshot {
        ToolSnapshot {
            id: self.id,
            order: OrderKey::default(),
            name: self.name.clone(),
            tool: self.tool.clone(),
            controller,
            cutting_data: self.cutting_data.clone(),
        }
    }

    /// The library tool made from a project snapshot; the snapshot's
    /// controller values stay with the project.
    pub fn from_snapshot(snapshot: &ToolSnapshot) -> Self {
        Self {
            id: snapshot.id,
            name: snapshot.name.clone(),
            tool: snapshot.tool.clone(),
            cutting_data: snapshot.cutting_data.clone(),
        }
    }
}

/// Reads one tool file's text. `path` names the file in messages.
pub fn parse_tool_file(path: &Path, text: &str) -> Result<LibraryTool, ProjectError> {
    let f: ToolFile = parse(path, text, FileKind::Tool, &[], LIBRARY_SCHEMA)?;
    let tool = LibraryTool {
        id: f.id,
        name: f.name,
        tool: f.tool,
        cutting_data: f.cutting_data,
    };
    tool.validate().map_err(|e| bad(path, e))?;
    Ok(tool)
}

/// The text of the tool file for `tool`.
pub fn render_tool_file(tool: &LibraryTool) -> Result<String, ProjectError> {
    tool.validate()?;
    to_text(&ToolFile {
        schema: LIBRARY_SCHEMA,
        id: tool.id,
        name: tool.name.clone(),
        tool: tool.tool.clone(),
        cutting_data: tool.cutting_data.clone(),
    })
}

/// One tool in a machine's tool set: which library tool, under which
/// controller values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolSetEntry {
    /// The library tool.
    pub tool: ToolId,
    /// Its tool number and offsets on this machine.
    pub controller: ControllerMapping,
}

/// A machine's default tool set (TL-11): the tools it normally holds and
/// their numbers, for example the tools kept in a tool changer. Adding one of
/// these tools to a project for this machine gives it this number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSet {
    /// The machine profile's name, matched without regard to case.
    pub machine: String,
    /// The tools, in file order.
    pub entries: Vec<ToolSetEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolSetFile {
    schema: i64,
    machine: String,
    #[serde(default)]
    tools: Vec<ToolSetLine>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolSetLine {
    number: u32,
    tool: ToolId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    length_offset: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    diameter_offset: Option<u32>,
}

impl ToolSet {
    /// Fails on an empty machine name, an invalid mapping, or a number or
    /// tool listed twice.
    pub fn validate(&self) -> Result<(), ProjectError> {
        if self.machine.trim().is_empty() {
            return Err(openchuck_types::Error::invalid(
                "machine",
                "a tool set names the machine profile it belongs to",
            )
            .into());
        }
        let mut numbers = BTreeSet::new();
        let mut tools = BTreeSet::new();
        for (i, e) in self.entries.iter().enumerate() {
            e.controller.validate().map_err(|err| {
                openchuck_types::Error::invalid(format!("tools[{i}]"), err.to_string())
            })?;
            if !numbers.insert(e.controller.number) {
                return Err(openchuck_types::Error::invalid(
                    format!("tools[{i}].number"),
                    format!("T{} is listed twice", e.controller.number),
                )
                .into());
            }
            if !tools.insert(e.tool) {
                return Err(openchuck_types::Error::invalid(
                    format!("tools[{i}].tool"),
                    format!("tool {} is listed twice", e.tool),
                )
                .into());
            }
        }
        Ok(())
    }

    /// The controller values this set gives `tool`.
    pub fn mapping_for(&self, tool: ToolId) -> Option<ControllerMapping> {
        self.entries
            .iter()
            .find(|e| e.tool == tool)
            .map(|e| e.controller)
    }

    /// The tool this set gives `number`.
    pub fn tool_for(&self, number: u32) -> Option<ToolId> {
        self.entries
            .iter()
            .find(|e| e.controller.number == number)
            .map(|e| e.tool)
    }

    /// Sets `tool`'s controller values, replacing any entry for that tool or
    /// that number.
    pub fn assign(&mut self, tool: ToolId, controller: ControllerMapping) {
        self.entries
            .retain(|e| e.tool != tool && e.controller.number != controller.number);
        self.entries.push(ToolSetEntry { tool, controller });
        self.entries.sort_by_key(|e| e.controller.number);
    }

    /// Removes the entry for tool number `number`; true if there was one.
    pub fn unassign(&mut self, number: u32) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.controller.number != number);
        self.entries.len() != before
    }
}

/// Reads one tool set file's text. `path` names the file in messages.
pub fn parse_tool_set(path: &Path, text: &str) -> Result<ToolSet, ProjectError> {
    let f: ToolSetFile = parse(path, text, FileKind::Tool, &[], LIBRARY_SCHEMA)?;
    let set = ToolSet {
        machine: f.machine,
        entries: f
            .tools
            .into_iter()
            .map(|l| ToolSetEntry {
                tool: l.tool,
                controller: ControllerMapping {
                    number: l.number,
                    length_offset: l.length_offset,
                    diameter_offset: l.diameter_offset,
                },
            })
            .collect(),
    };
    set.validate().map_err(|e| bad(path, e))?;
    Ok(set)
}

/// The text of the tool set file for `set`.
pub fn render_tool_set(set: &ToolSet) -> Result<String, ProjectError> {
    set.validate()?;
    to_text(&ToolSetFile {
        schema: LIBRARY_SCHEMA,
        machine: set.machine.clone(),
        tools: set
            .entries
            .iter()
            .map(|e| ToolSetLine {
                number: e.controller.number,
                tool: e.tool,
                length_offset: e.controller.length_offset,
                diameter_offset: e.controller.diameter_offset,
            })
            .collect(),
    })
}

/// What [`ToolLibrary::import`] did, by tool ID in the order read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportReport {
    /// Tools new to the library.
    pub added: Vec<ToolId>,
    /// Tools whose library file was replaced by a different import.
    pub replaced: Vec<ToolId>,
    /// Tools already in the library exactly as imported.
    pub unchanged: Vec<ToolId>,
}

/// A tool library: a folder with one TOML file per tool and machine tool
/// sets in `tool-sets/` (TL-01, TL-11; decision record 0014 item 7).
///
/// Files are read in file name order. A file that cannot be read, or that
/// repeats another file's tool ID, is skipped and listed in
/// [`ToolLibrary::problems`], so one broken file does not hide the rest.
/// Every change is written to its own file at once; other files are left
/// alone.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolLibrary {
    dir: PathBuf,
    tools: Vec<(PathBuf, LibraryTool)>,
    sets: Vec<(PathBuf, ToolSet)>,
    problems: Vec<String>,
}

impl ToolLibrary {
    /// Reads the library in the folder `dir`. A missing folder is an empty
    /// library; it is created when the first file is written.
    pub fn open(dir: &Path) -> Result<Self, ProjectError> {
        let mut library = Self {
            dir: dir.to_path_buf(),
            tools: Vec::new(),
            sets: Vec::new(),
            problems: Vec::new(),
        };
        for path in toml_files(dir)? {
            match read_text(&path).and_then(|text| parse_tool_file(&path, &text)) {
                Ok(tool) => {
                    if let Some((first, _)) = library.tools.iter().find(|(_, t)| t.id == tool.id) {
                        library.problems.push(format!(
                            "'{}' holds tool {}, which '{}' already holds; it is skipped",
                            path.display(),
                            tool.id,
                            first.display()
                        ));
                    } else {
                        library.tools.push((path, tool));
                    }
                }
                Err(e) => library.problems.push(e.to_string()),
            }
        }
        for path in toml_files(&dir.join(TOOL_SETS_FOLDER))? {
            match read_text(&path).and_then(|text| parse_tool_set(&path, &text)) {
                Ok(set) => {
                    if library.tool_set(&set.machine).is_some() {
                        library.problems.push(format!(
                            "'{}' is a second tool set for \"{}\"; it is skipped",
                            path.display(),
                            set.machine
                        ));
                        continue;
                    }
                    for e in &set.entries {
                        if library.get(e.tool).is_none() {
                            library.problems.push(format!(
                                "'{}': T{} names tool {}, which is not in the library",
                                path.display(),
                                e.controller.number,
                                e.tool
                            ));
                        }
                    }
                    library.sets.push((path, set));
                }
                Err(e) => library.problems.push(e.to_string()),
            }
        }
        Ok(library)
    }

    /// The library folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The tools in file name order.
    pub fn tools(&self) -> impl Iterator<Item = &LibraryTool> {
        self.tools.iter().map(|(_, t)| t)
    }

    /// The file holding the tool with `id`.
    pub fn path_of(&self, id: ToolId) -> Option<&Path> {
        self.tools
            .iter()
            .find(|(_, t)| t.id == id)
            .map(|(p, _)| p.as_path())
    }

    /// Files that could not be read or were skipped, with why.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// The tool with `id`.
    pub fn get(&self, id: ToolId) -> Option<&LibraryTool> {
        self.tools().find(|t| t.id == id)
    }

    /// The tool named by `text`: its ID, or its name ignoring case. Fails when
    /// nothing or more than one tool matches.
    pub fn find(&self, text: &str) -> Result<&LibraryTool, String> {
        let text = text.trim();
        if let Ok(id) = text.parse::<ToolId>() {
            return self
                .get(id)
                .ok_or_else(|| format!("the library has no tool with ID {id}"));
        }
        let matches: Vec<&LibraryTool> = self
            .tools()
            .filter(|t| t.name.eq_ignore_ascii_case(text))
            .collect();
        match matches.as_slice() {
            [one] => Ok(one),
            [] => Err(format!("the library has no tool named \"{text}\"")),
            many => Err(format!(
                "\"{text}\" matches {} tools; give the tool's ID",
                many.len()
            )),
        }
    }

    /// Writes `tool` to its file: the file that already holds its ID, or a
    /// new file named after the tool. Returns the file.
    pub fn save_tool(&mut self, tool: LibraryTool) -> Result<PathBuf, ProjectError> {
        let text = render_tool_file(&tool)?;
        let path = match self.path_of(tool.id) {
            Some(path) => path.to_path_buf(),
            None => self.free_path(&self.dir, &tool.name),
        };
        std::fs::create_dir_all(&self.dir).map_err(io(&self.dir))?;
        write_if_changed(&path, &text)?;
        match self.tools.iter_mut().find(|(_, t)| t.id == tool.id) {
            Some(entry) => entry.1 = tool,
            None => {
                self.tools.push((path.clone(), tool));
                self.tools.sort_by(|a, b| a.0.cmp(&b.0));
            }
        }
        Ok(path)
    }

    /// Deletes the file of the tool with `id`, returning the tool.
    pub fn remove_tool(&mut self, id: ToolId) -> Result<Option<LibraryTool>, ProjectError> {
        let Some(i) = self.tools.iter().position(|(_, t)| t.id == id) else {
            return Ok(None);
        };
        let (path, tool) = self.tools.remove(i);
        std::fs::remove_file(&path).map_err(io(&path))?;
        Ok(Some(tool))
    }

    /// Merges tools from `from`, a library folder or a single tool file: a
    /// new ID gets a new file, a known ID rewrites that tool's file, an
    /// identical tool is left alone. Fails, writing nothing, if any file in
    /// `from` cannot be read.
    pub fn import(&mut self, from: &Path) -> Result<ImportReport, ProjectError> {
        let incoming: Vec<LibraryTool> = if from.is_file() {
            vec![parse_tool_file(from, &read_text(from)?)?]
        } else {
            let other = Self::open(from)?;
            if let Some(problem) = other.problems.first() {
                return Err(ProjectError::BadFile {
                    path: from.to_path_buf(),
                    reason: format!("nothing imported: {problem}"),
                });
            }
            other.tools().cloned().collect()
        };
        let mut report = ImportReport::default();
        for tool in incoming {
            match self.get(tool.id) {
                Some(t) if *t == tool => report.unchanged.push(tool.id),
                Some(_) => {
                    report.replaced.push(tool.id);
                    self.save_tool(tool)?;
                }
                None => {
                    report.added.push(tool.id);
                    self.save_tool(tool)?;
                }
            }
        }
        Ok(report)
    }

    /// Writes the tools with `ids` (all when empty) into the folder `to`, as a
    /// library another user can import. Returns the files written.
    pub fn export(&self, ids: &[ToolId], to: &Path) -> Result<Vec<PathBuf>, ProjectError> {
        for id in ids {
            if self.get(*id).is_none() {
                return Err(ProjectError::NotFound {
                    kind: ToolId::KIND,
                    id: id.to_string(),
                });
            }
        }
        let mut target = Self::open(to)?;
        self.tools()
            .filter(|t| ids.is_empty() || ids.contains(&t.id))
            .map(|t| target.save_tool(t.clone()))
            .collect()
    }

    /// The machine tool sets, in file name order.
    pub fn tool_sets(&self) -> impl Iterator<Item = &ToolSet> {
        self.sets.iter().map(|(_, s)| s)
    }

    /// The tool set for the machine profile named `machine` (ignoring case).
    pub fn tool_set(&self, machine: &str) -> Option<&ToolSet> {
        self.tool_sets()
            .find(|s| s.machine.trim().eq_ignore_ascii_case(machine.trim()))
    }

    /// Writes `set` to its file in `tool-sets/`, the one that already holds
    /// its machine or a new file named after the machine. Fails if it names
    /// a tool that is not in the library.
    pub fn save_tool_set(&mut self, set: ToolSet) -> Result<PathBuf, ProjectError> {
        if let Some(e) = set.entries.iter().find(|e| self.get(e.tool).is_none()) {
            return Err(ProjectError::NotFound {
                kind: ToolId::KIND,
                id: e.tool.to_string(),
            });
        }
        let text = render_tool_set(&set)?;
        let i = self
            .sets
            .iter()
            .position(|(_, s)| s.machine.eq_ignore_ascii_case(&set.machine));
        let path = match i.and_then(|i| self.sets.get(i)) {
            Some((path, _)) => path.clone(),
            None => self.free_path(&self.dir.join(TOOL_SETS_FOLDER), &set.machine),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io(parent))?;
        }
        write_if_changed(&path, &text)?;
        match i.and_then(|i| self.sets.get_mut(i)) {
            Some(entry) => entry.1 = set,
            None => {
                self.sets.push((path.clone(), set));
                self.sets.sort_by(|a, b| a.0.cmp(&b.0));
            }
        }
        Ok(path)
    }

    /// Renames the machine that a tool set belongs to, from `old` to `new`
    /// (names matched without regard to case), rewriting the set's own file
    /// (TL-11; a renamed machine profile keeps its tool set, OI-1474,
    /// decision 0380). Returns the file, or `None` when no set names `old`.
    /// Fails, writing nothing, if `new` is blank or another set already
    /// names it.
    pub fn rename_tool_set(
        &mut self,
        old: &str,
        new: &str,
    ) -> Result<Option<PathBuf>, ProjectError> {
        let Some(i) = self
            .sets
            .iter()
            .position(|(_, s)| s.machine.trim().eq_ignore_ascii_case(old.trim()))
        else {
            return Ok(None);
        };
        let Some((path, set)) = self.sets.get(i) else {
            return Ok(None);
        };
        let path = path.clone();
        let bad = |reason: String| ProjectError::BadFile {
            path: path.clone(),
            reason,
        };
        if self
            .sets
            .iter()
            .enumerate()
            .any(|(j, (_, s))| j != i && s.machine.trim().eq_ignore_ascii_case(new.trim()))
        {
            return Err(bad(format!(
                "a tool set for the machine \"{}\" already exists",
                new.trim()
            )));
        }
        let mut renamed = set.clone();
        renamed.machine = new.trim().to_owned();
        renamed.validate()?;
        let text = render_tool_set(&renamed)?;
        write_if_changed(&path, &text)?;
        if let Some(entry) = self.sets.get_mut(i) {
            entry.1 = renamed;
        }
        Ok(Some(path))
    }

    /// A file in `dir` named after `name` that no tool or tool set uses and
    /// that does not exist.
    fn free_path(&self, dir: &Path, name: &str) -> PathBuf {
        let stem = file_stem_for(name);
        let taken = |p: &Path| {
            p.exists()
                || self.tools.iter().any(|(q, _)| q == p)
                || self.sets.iter().any(|(q, _)| q == p)
        };
        let mut n = 1;
        loop {
            let candidate = if n == 1 {
                dir.join(format!("{stem}.toml"))
            } else {
                dir.join(format!("{stem}-{n}.toml"))
            };
            if !taken(&candidate) {
                return candidate;
            }
            n += 1;
        }
    }
}

/// A file stem for `name`: lower-case letters and digits, other runs as one
/// hyphen. "6 mm Flat (2F)" gives "6-mm-flat-2f".
pub fn file_stem_for(name: &str) -> String {
    let mut stem = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            stem.push(c.to_ascii_lowercase());
        } else if !stem.is_empty() && !stem.ends_with('-') {
            stem.push('-');
        }
    }
    let stem = stem.trim_end_matches('-');
    if stem.is_empty() {
        "tool".to_owned()
    } else {
        stem.to_owned()
    }
}

/// How a project's tool snapshot compares with a library (TL-07). The
/// snapshot always wins; this only reports. Controller values are the
/// project's own and are not compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotStatus {
    /// The library holds the same tool, identical.
    Same,
    /// The library's tool with the same ID differs in these fields, for
    /// example `tool.diameter` or `cutting_data.oak`.
    Differs(Vec<String>),
    /// The library has no tool with the snapshot's ID.
    NotInLibrary,
}

/// Compares `snapshot` with the tool of the same ID in `library`.
pub fn compare_snapshot(snapshot: &ToolSnapshot, library: &ToolLibrary) -> SnapshotStatus {
    let Some(entry) = library.get(snapshot.id) else {
        return SnapshotStatus::NotInLibrary;
    };
    let ours = LibraryTool::from_snapshot(snapshot);
    if &ours == entry {
        return SnapshotStatus::Same;
    }
    let as_value = |t: &LibraryTool| {
        let mut table = BTreeMap::new();
        table.insert("name", toml::Value::String(t.name.clone()));
        table.insert("tool", toml::Value::try_from(&t.tool).ok()?);
        table.insert("cutting_data", toml::Value::try_from(&t.cutting_data).ok()?);
        toml::Value::try_from(table).ok()
    };
    let (Some(a), Some(b)) = (as_value(&ours), as_value(entry)) else {
        return SnapshotStatus::Differs(vec!["tool".to_owned()]);
    };
    let mut fields = Vec::new();
    differences("", &a, &b, &mut fields);
    SnapshotStatus::Differs(fields)
}

/// Appends the dotted paths where `a` and `b` differ. Tables are compared key
/// by key; a changed tool shape type is reported once, as `tool.shape`; an
/// emptied table is reported by its own name.
fn differences(prefix: &str, a: &toml::Value, b: &toml::Value, out: &mut Vec<String>) {
    let join = |k: &str| {
        if prefix.is_empty() {
            k.to_owned()
        } else {
            format!("{prefix}.{k}")
        }
    };
    match (a, b) {
        (toml::Value::Table(ta), toml::Value::Table(tb))
            if ta.get("type") == tb.get("type") && !ta.is_empty() && !tb.is_empty() =>
        {
            let keys: BTreeSet<&String> = ta.keys().chain(tb.keys()).collect();
            for k in keys {
                match (ta.get(k), tb.get(k)) {
                    (Some(x), Some(y)) => differences(&join(k), x, y, out),
                    _ => out.push(join(k)),
                }
            }
        }
        _ if a == b => {}
        _ => out.push(prefix.to_owned()),
    }
}

fn bad(path: &Path, e: ProjectError) -> ProjectError {
    ProjectError::BadFile {
        path: path.to_path_buf(),
        reason: e.to_string(),
    }
}

fn read_text(path: &Path) -> Result<String, ProjectError> {
    std::fs::read_to_string(path).map_err(io(path))
}

pub(super) fn io(path: &Path) -> impl Fn(std::io::Error) -> ProjectError + '_ {
    move |source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// The `.toml` files directly in `dir`, sorted. A missing folder has none.
fn toml_files(dir: &Path) -> Result<Vec<PathBuf>, ProjectError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(dir)(e)),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry.map_err(io(dir))?.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("toml") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

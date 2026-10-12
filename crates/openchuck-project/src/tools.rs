// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool library, stored separately from projects (TL-01), custom profile
//! text files (TL-03) and the comparison of a project's tool snapshots with a
//! library (TL-07). Decision records 0012 and 0014.
//!
//! A library is one TOML file with its own schema version. Export writes the
//! chosen tools to a file in the same format, and import merges such a file
//! into a library, so a library file is also the exchange format:
//!
//! ```toml
//! schema = 1
//!
//! [[tools]]
//! id = "6f1c2e4a-0b7d-4c3e-9a51-2d8f0e6b7c19"
//! name = "6 mm flat"
//!
//! [tools.tool]
//! diameter = 6.0
//! flute_length = 20.0
//! overall_length = 50.0
//! shank_diameter = 6.0
//! flute_count = 2
//! material = "carbide"
//!
//! [tools.tool.shape]
//! type = "flat_end"
//!
//! [tools.controller]
//! number = 3
//!
//! [tools.cutting_data.oak]
//! spindle_speed = 18000.0
//! feed_per_tooth = 0.03
//! plunge_feed = 300.0
//! stepover = 2.4
//! stepdown = 1.5
//! ```
//!
//! Tools keep their order in the file: an edit never moves a tool, and new
//! tools go at the end, so a one-value edit is a one-line diff. A tool's ID is
//! kept when it is copied into a project, so the snapshot and its library
//! tool can be matched later.
//!
//! A custom profile file holds the [`ToolProfile`] fields. An arc gives its
//! `centre`, or its `radius` and whether it is `convex` (bulging away from the
//! tool, like a ball end) or `concave` (like a corner-rounding cutter):
//!
//! ```toml
//! schema = 1
//!
//! [[segments]]
//! type = "line"
//! to = { radius = 1.0, height = 0.0 }
//!
//! [[segments]]
//! type = "arc"
//! to = { radius = 3.0, height = 2.0 }
//! radius = 2.0
//! curve = "concave"
//! ```

use std::collections::BTreeSet;
use std::path::Path;

use openchuck_types::tool::profile::arc_centre;
use openchuck_types::tool::{
    ControllerMapping, CuttingTable, ProfilePoint, ProfileSegment, Tool, ToolProfile,
    validate_entry,
};
use serde::{Deserialize, Serialize};

use crate::OrderKey;
use crate::error::ProjectError;
use crate::file::{FileKind, parse, to_text};
use crate::id::ToolId;
use crate::model::ToolSnapshot;

/// The tool library file schema this build writes and reads.
///
/// - 1: slice 4 (decision record 0014).
pub const LIBRARY_SCHEMA: i64 = 1;

/// The custom profile file schema this build writes and reads.
///
/// - 1: slice 4 (decision record 0014).
pub const PROFILE_SCHEMA: i64 = 1;

/// A tool in a library: what a project snapshot copies (TL-01, TL-07).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryTool {
    /// Stable ID, kept by the project snapshots made from it.
    pub id: ToolId,
    /// Name shown to the user.
    pub name: String,
    /// Geometry and physical parameters (TL-02 to TL-04).
    pub tool: Tool,
    /// Controller values for the post (TL-06).
    pub controller: ControllerMapping,
    /// Cutting data by stock material name (TL-05).
    #[serde(default, skip_serializing_if = "CuttingTable::is_empty")]
    pub cutting_data: CuttingTable,
}

impl LibraryTool {
    /// Fails if the tool, its controller mapping or its cutting data is
    /// invalid, or the name is empty.
    pub fn validate(&self) -> Result<(), ProjectError> {
        if self.name.trim().is_empty() {
            return Err(openchuck_types::Error::invalid("name", "a tool needs a name").into());
        }
        Ok(validate_entry(
            &self.tool,
            &self.controller,
            &self.cutting_data,
        )?)
    }

    /// The project snapshot of this tool, with the same ID (TL-07). The
    /// project sets the order key when the snapshot is added.
    pub fn snapshot(&self) -> ToolSnapshot {
        ToolSnapshot {
            id: self.id,
            order: OrderKey::default(),
            name: self.name.clone(),
            tool: self.tool.clone(),
            controller: self.controller,
            cutting_data: self.cutting_data.clone(),
        }
    }

    /// The library tool made from a project snapshot, for adding a tool
    /// defined in a project to a library.
    pub fn from_snapshot(snapshot: &ToolSnapshot) -> Self {
        Self {
            id: snapshot.id,
            name: snapshot.name.clone(),
            tool: snapshot.tool.clone(),
            controller: snapshot.controller,
            cutting_data: snapshot.cutting_data.clone(),
        }
    }
}

/// A tool library: tools in file order, each ID once (TL-01).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ToolLibrary {
    tools: Vec<LibraryTool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryFile {
    schema: i64,
    #[serde(default)]
    tools: Vec<LibraryTool>,
}

/// What [`ToolLibrary::import`] did, by tool ID in the imported file's order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportReport {
    /// Tools new to the library, added at the end.
    pub added: Vec<ToolId>,
    /// Tools whose library entry was replaced by a different import.
    pub replaced: Vec<ToolId>,
    /// Tools already in the library exactly as imported.
    pub unchanged: Vec<ToolId>,
}

impl ToolLibrary {
    /// A library holding `tools` in order. Fails on an invalid tool or a
    /// repeated ID.
    pub fn new(tools: Vec<LibraryTool>) -> Result<Self, ProjectError> {
        let mut ids = BTreeSet::new();
        for t in &tools {
            t.validate().map_err(|e| ProjectError::BadFile {
                path: Path::new("library").to_path_buf(),
                reason: format!("tool \"{}\" ({}): {e}", t.name, t.id),
            })?;
            if !ids.insert(t.id) {
                return Err(ProjectError::DuplicateId {
                    kind: ToolId::KIND,
                    id: t.id.to_string(),
                });
            }
        }
        Ok(Self { tools })
    }

    /// The tools in file order.
    pub fn tools(&self) -> &[LibraryTool] {
        &self.tools
    }

    /// The tool with `id`.
    pub fn get(&self, id: ToolId) -> Option<&LibraryTool> {
        self.tools.iter().find(|t| t.id == id)
    }

    /// The tool named by `text`: its ID, its name (ignoring case), or `T<n>`
    /// for its tool number. Fails when nothing or more than one tool matches.
    pub fn find(&self, text: &str) -> Result<&LibraryTool, String> {
        let text = text.trim();
        if let Ok(id) = text.parse::<ToolId>() {
            return self
                .get(id)
                .ok_or_else(|| format!("the library has no tool with ID {id}"));
        }
        let number = text
            .strip_prefix(['T', 't'])
            .and_then(|n| n.parse::<u32>().ok());
        let matches: Vec<&LibraryTool> = self
            .tools
            .iter()
            .filter(|t| t.name.eq_ignore_ascii_case(text) || Some(t.controller.number) == number)
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

    /// Adds `tool` at the end, or replaces the tool with its ID in place.
    pub fn upsert(&mut self, tool: LibraryTool) -> Result<(), ProjectError> {
        tool.validate()?;
        match self.tools.iter_mut().find(|t| t.id == tool.id) {
            Some(t) => *t = tool,
            None => self.tools.push(tool),
        }
        Ok(())
    }

    /// Removes the tool with `id`, returning it.
    pub fn remove(&mut self, id: ToolId) -> Option<LibraryTool> {
        let i = self.tools.iter().position(|t| t.id == id)?;
        Some(self.tools.remove(i))
    }

    /// Merges `other` into this library: a tool with a new ID is added at the
    /// end, and a tool whose ID is already here replaces that entry in place.
    pub fn import(&mut self, other: &ToolLibrary) -> ImportReport {
        let mut report = ImportReport::default();
        for tool in &other.tools {
            match self.tools.iter_mut().find(|t| t.id == tool.id) {
                Some(t) if t == tool => report.unchanged.push(tool.id),
                Some(t) => {
                    *t = tool.clone();
                    report.replaced.push(tool.id);
                }
                None => {
                    self.tools.push(tool.clone());
                    report.added.push(tool.id);
                }
            }
        }
        report
    }

    /// A library holding only the tools with `ids`, in this library's order,
    /// for export. Fails if an ID is not here.
    pub fn export(&self, ids: &[ToolId]) -> Result<ToolLibrary, ProjectError> {
        for id in ids {
            if self.get(*id).is_none() {
                return Err(ProjectError::NotFound {
                    kind: ToolId::KIND,
                    id: id.to_string(),
                });
            }
        }
        Ok(Self {
            tools: self
                .tools
                .iter()
                .filter(|t| ids.contains(&t.id))
                .cloned()
                .collect(),
        })
    }
}

/// Reads a tool library file's text. `path` names the file in messages.
pub fn parse_library(path: &Path, text: &str) -> Result<ToolLibrary, ProjectError> {
    let file: LibraryFile = parse(path, text, FileKind::Tool, &[], LIBRARY_SCHEMA)?;
    ToolLibrary::new(file.tools).map_err(|e| match e {
        ProjectError::BadFile { reason, .. } => ProjectError::BadFile {
            path: path.to_path_buf(),
            reason,
        },
        ProjectError::DuplicateId { id, .. } => ProjectError::BadFile {
            path: path.to_path_buf(),
            reason: format!("two tools use the ID {id}"),
        },
        other => other,
    })
}

/// The text of a tool library file for `library`.
pub fn render_library(library: &ToolLibrary) -> Result<String, ProjectError> {
    to_text(&LibraryFile {
        schema: LIBRARY_SCHEMA,
        tools: library.tools.clone(),
    })
}

/// Reads the tool library file at `path`.
pub fn load_library(path: &Path) -> Result<ToolLibrary, ProjectError> {
    let text = std::fs::read_to_string(path).map_err(|source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_library(path, &text)
}

/// Writes `library` to the file at `path`.
pub fn save_library(library: &ToolLibrary, path: &Path) -> Result<(), ProjectError> {
    let text = render_library(library)?;
    std::fs::write(path, text).map_err(|source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Which way an arc given by radius bulges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcCurve {
    /// Away from the tool, like a ball end: the centre is above and inside.
    Convex,
    /// Into the tool, like a corner-rounding cutter: the centre is below and
    /// outside.
    Concave,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum SegmentText {
    Line {
        to: ProfilePoint,
    },
    Arc {
        to: ProfilePoint,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        centre: Option<ProfilePoint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        radius: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        curve: Option<ArcCurve>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileFile {
    schema: i64,
    #[serde(default)]
    segments: Vec<SegmentText>,
}

/// Reads a custom profile file's text (TL-03) and checks the monotone rule.
/// Errors name the file and the segment.
pub fn parse_profile(path: &Path, text: &str) -> Result<ToolProfile, ProjectError> {
    let bad = |reason: String| ProjectError::BadFile {
        path: path.to_path_buf(),
        reason,
    };
    let file: ProfileFile = parse(path, text, FileKind::Tool, &[], PROFILE_SCHEMA)?;
    let mut prev = ProfilePoint::TIP;
    let mut segments = Vec::with_capacity(file.segments.len());
    for (i, segment) in file.segments.into_iter().enumerate() {
        let segment = match segment {
            SegmentText::Line { to } => ProfileSegment::Line { to },
            SegmentText::Arc {
                to,
                centre,
                radius,
                curve,
            } => {
                let centre = match (centre, radius, curve) {
                    (Some(centre), None, None) => centre,
                    (None, Some(radius), Some(curve)) => {
                        arc_centre(prev, to, radius, curve == ArcCurve::Convex)
                            .map_err(|e| bad(format!("segment {}: {e}", i + 1)))?
                    }
                    _ => {
                        return Err(bad(format!(
                            "segment {}: an arc gives either `centre`, or `radius` and \
                             `curve` (\"convex\" or \"concave\")",
                            i + 1
                        )));
                    }
                };
                ProfileSegment::Arc { to, centre }
            }
        };
        prev = segment.end();
        segments.push(segment);
    }
    let profile = ToolProfile { segments };
    profile.validate().map_err(|e| bad(e.to_string()))?;
    Ok(profile)
}

/// The text of a custom profile file for `profile`. Arcs are written with
/// their centre.
pub fn render_profile(profile: &ToolProfile) -> Result<String, ProjectError> {
    profile.validate()?;
    to_text(&ProfileFile {
        schema: PROFILE_SCHEMA,
        segments: profile
            .segments
            .iter()
            .map(|s| match *s {
                ProfileSegment::Line { to } => SegmentText::Line { to },
                ProfileSegment::Arc { to, centre } => SegmentText::Arc {
                    to,
                    centre: Some(centre),
                    radius: None,
                    curve: None,
                },
            })
            .collect(),
    })
}

/// Reads the custom profile file at `path`.
pub fn load_profile(path: &Path) -> Result<ToolProfile, ProjectError> {
    let text = std::fs::read_to_string(path).map_err(|source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_profile(path, &text)
}

/// How a project's tool snapshot compares with a library (TL-07). The
/// snapshot always wins; this only reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotStatus {
    /// The library holds the same tool, identical.
    Same,
    /// The library's tool with the same ID differs in these fields, for
    /// example `tool.diameter`, `controller.number` or `cutting_data.oak`.
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
    let (Ok(a), Ok(b)) = (toml::Value::try_from(&ours), toml::Value::try_from(entry)) else {
        return SnapshotStatus::Differs(vec!["tool".to_owned()]);
    };
    let mut fields = Vec::new();
    differences("", &a, &b, &mut fields);
    SnapshotStatus::Differs(fields)
}

/// Appends the dotted paths where `a` and `b` differ. Tables are compared key
/// by key; a changed tool shape type is reported once, as `tool.shape`.
fn differences(prefix: &str, a: &toml::Value, b: &toml::Value, out: &mut Vec<String>) {
    let join = |k: &str| {
        if prefix.is_empty() {
            k.to_owned()
        } else {
            format!("{prefix}.{k}")
        }
    };
    match (a, b) {
        (toml::Value::Table(ta), toml::Value::Table(tb)) if ta.get("type") == tb.get("type") => {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use openchuck_types::tool::{CuttingData, ToolShape};
    use openchuck_types::units::{Degrees, FeedRate, Millimetres, SpindleSpeed};

    fn flat(id: u128, number: u32) -> LibraryTool {
        LibraryTool {
            id: ToolId::from_u128(id),
            name: format!("{number}: 6 mm flat"),
            tool: Tool {
                shape: ToolShape::FlatEnd,
                diameter: Millimetres(6.0),
                flute_length: Millimetres(20.0),
                overall_length: Millimetres(50.0),
                shank_diameter: Millimetres(6.0),
                flute_count: 2,
                material: Some("carbide".into()),
            },
            controller: ControllerMapping::numbered(number),
            cutting_data: CuttingTable::from([(
                "oak".to_owned(),
                CuttingData {
                    spindle_speed: SpindleSpeed(18000.0),
                    feed_per_tooth: Millimetres(0.03),
                    plunge_feed: FeedRate(300.0),
                    stepover: Millimetres(2.4),
                    stepdown: Millimetres(1.5),
                },
            )]),
        }
    }

    fn vbit(id: u128) -> LibraryTool {
        LibraryTool {
            id: ToolId::from_u128(id),
            name: "60 degree V".into(),
            tool: Tool {
                shape: ToolShape::VBit {
                    included_angle: Degrees(60.0),
                    tip_diameter: Millimetres(0.2),
                },
                material: None,
                ..flat(0, 1).tool
            },
            controller: ControllerMapping {
                number: 5,
                length_offset: Some(15),
                diameter_offset: None,
            },
            cutting_data: CuttingTable::new(),
        }
    }

    fn library() -> ToolLibrary {
        ToolLibrary::new(vec![flat(1, 3), vbit(2)]).unwrap()
    }

    #[test]
    fn req_tl_01_library_file_round_trips_byte_for_byte() {
        let lib = library();
        let text = render_library(&lib).unwrap();
        assert!(text.starts_with("schema = 1\n"), "{text}");
        let back = parse_library(Path::new("lib.toml"), &text).unwrap();
        assert_eq!(back, lib);
        assert_eq!(render_library(&back).unwrap(), text);
    }

    #[test]
    fn req_tl_01_a_one_value_edit_is_a_one_line_diff() {
        let mut lib = library();
        let before = render_library(&lib).unwrap();
        let mut t = lib.get(ToolId::from_u128(1)).unwrap().clone();
        t.tool.flute_length = Millimetres(22.0);
        lib.upsert(t).unwrap();
        let after = render_library(&lib).unwrap();
        let changed: Vec<_> = before
            .lines()
            .zip(after.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(before.lines().count(), after.lines().count());
        assert_eq!(
            changed,
            vec![("flute_length = 20.0", "flute_length = 22.0")]
        );
    }

    #[test]
    fn req_tl_01_export_then_import_is_lossless() {
        let lib = library();
        let exported = lib.export(&[ToolId::from_u128(2)]).unwrap();
        let text = render_library(&exported).unwrap();
        let mut other = ToolLibrary::new(vec![flat(9, 1)]).unwrap();
        let report = other.import(&parse_library(Path::new("x.toml"), &text).unwrap());
        assert_eq!(report.added, vec![ToolId::from_u128(2)]);
        assert_eq!(
            other.get(ToolId::from_u128(2)),
            lib.get(ToolId::from_u128(2))
        );
        // Importing again changes nothing; a changed tool replaces in place.
        let again = other.import(&exported);
        assert_eq!(again.unchanged, vec![ToolId::from_u128(2)]);
        let mut changed = exported.clone();
        let mut v = vbit(2);
        v.name = "V renamed".into();
        changed.upsert(v).unwrap();
        let report = other.import(&changed);
        assert_eq!(report.replaced, vec![ToolId::from_u128(2)]);
        assert_eq!(other.tools()[1].name, "V renamed");
        assert!(lib.export(&[ToolId::from_u128(77)]).is_err());
    }

    #[test]
    fn req_tl_01_find_by_id_name_or_number() {
        let lib = library();
        assert_eq!(lib.find("T5").unwrap().id, ToolId::from_u128(2));
        assert_eq!(lib.find("60 DEGREE v").unwrap().id, ToolId::from_u128(2));
        assert_eq!(
            lib.find(&ToolId::from_u128(1).to_string()).unwrap().id,
            ToolId::from_u128(1)
        );
        assert!(lib.find("T9").is_err());
        let twice = ToolLibrary::new(vec![flat(1, 3), flat(4, 3)]).unwrap();
        assert!(twice.find("T3").unwrap_err().contains("matches 2"));
    }

    #[test]
    fn req_nfr_07_bad_library_files_give_clear_errors() {
        let text = render_library(&library()).unwrap();
        let cases = [
            ("schema = 2\n", "newer"),
            ("not toml [", "lib.toml"),
            (
                &text.replace("diameter = 6.0\nflute", "diameter = -6.0\nflute"),
                "tool.diameter",
            ),
            (
                &text.replace("number = 3", "number = 0"),
                "controller.number",
            ),
            (
                &text.replace("stepover = 2.4", "stepover = 9.0"),
                "stepover",
            ),
            (
                &text.replace("flute_count = 2\nmaterial", "flutes = 2\nmaterial"),
                "flutes",
            ),
            (
                &text.replace(
                    &ToolId::from_u128(2).to_string(),
                    &ToolId::from_u128(1).to_string(),
                ),
                "two tools",
            ),
        ];
        for (text, expected) in cases {
            let err = parse_library(Path::new("lib.toml"), text)
                .unwrap_err()
                .to_string();
            assert!(err.contains(expected), "{expected}: {err}");
        }
    }

    #[test]
    fn req_tl_03_profile_file_reads_arcs_by_centre_or_radius() {
        let text = "schema = 1\n\n[[segments]]\ntype = \"line\"\nto = { radius = 1.0, height = 0.0 }\n\n\
                    [[segments]]\ntype = \"arc\"\nto = { radius = 3.0, height = 2.0 }\nradius = 2.0\ncurve = \"concave\"\n\n\
                    [[segments]]\ntype = \"line\"\nto = { radius = 3.0, height = 12.0 }\n";
        let profile = parse_profile(Path::new("p.toml"), text).unwrap();
        let ProfileSegment::Arc { to, centre } = profile.segments[1] else {
            panic!("{profile:?}");
        };
        assert_eq!(to, ProfilePoint::new(3.0, 2.0));
        assert!(
            centre.distance(ProfilePoint::new(3.0, 0.0)) < 1e-12,
            "{centre:?}"
        );
        let written = render_profile(&profile).unwrap();
        assert!(written.contains("centre"), "{written}");
        assert_eq!(
            parse_profile(Path::new("p.toml"), &written).unwrap(),
            profile
        );
        assert_eq!(render_profile(&profile).unwrap(), written);
    }

    #[test]
    fn req_tl_03_req_nfr_07_bad_profile_files_name_the_segment() {
        let line = |r: f64, z: f64| {
            format!("[[segments]]\ntype = \"line\"\nto = {{ radius = {r:?}, height = {z:?} }}\n")
        };
        let cases = [
            (
                format!(
                    "schema = 1\n{}{}{}",
                    line(3.0, 0.0),
                    line(3.0, 5.0),
                    line(2.0, 9.0)
                ),
                "segment 3",
            ),
            (
                format!(
                    "schema = 1\n{}[[segments]]\ntype = \"arc\"\nto = {{ radius = 3.0, height = 2.0 }}\nradius = 2.0\n",
                    line(1.0, 0.0)
                ),
                "segment 2: an arc gives",
            ),
            (
                format!(
                    "schema = 1\n{}[[segments]]\ntype = \"arc\"\nto = {{ radius = 9.0, height = 9.0 }}\nradius = 1.0\ncurve = \"convex\"\n",
                    line(1.0, 0.0)
                ),
                "segment 2: the arc's ends",
            ),
            ("schema = 1\n".to_owned(), "at least one segment"),
            (
                "schema = 1\n[[segments]]\ntype = \"spline\"\n".to_owned(),
                "spline",
            ),
        ];
        for (text, expected) in cases {
            let err = parse_profile(Path::new("p.toml"), &text)
                .unwrap_err()
                .to_string();
            assert!(
                err.contains(expected) && err.contains("p.toml"),
                "{expected}: {err}"
            );
        }
    }

    #[test]
    fn req_tl_07_snapshot_comparison_names_the_changed_fields() {
        let lib = library();
        let snapshot = lib.get(ToolId::from_u128(1)).unwrap().snapshot();
        assert_eq!(compare_snapshot(&snapshot, &lib), SnapshotStatus::Same);
        let mut edited = snapshot.clone();
        edited.tool.diameter = Millimetres(5.0);
        edited.controller.number = 4;
        edited.cutting_data.clear();
        assert_eq!(
            compare_snapshot(&edited, &lib),
            SnapshotStatus::Differs(vec![
                "controller.number".into(),
                "cutting_data".into(),
                "tool.diameter".into()
            ])
        );
        let mut reshaped = snapshot.clone();
        reshaped.tool.shape = ToolShape::BallEnd;
        assert_eq!(
            compare_snapshot(&reshaped, &lib),
            SnapshotStatus::Differs(vec!["tool.shape".into()])
        );
        assert_eq!(
            compare_snapshot(&snapshot, &ToolLibrary::default()),
            SnapshotStatus::NotInLibrary
        );
    }

    proptest::proptest! {
        #[test]
        fn req_nfr_07_library_parser_never_panics_on_damaged_files(
            cut in 0usize..2000,
            insert in ".{0,12}",
        ) {
            let text = render_library(&library()).unwrap();
            let mut cut = cut.min(text.len());
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            let damaged = format!("{}{insert}{}", &text[..cut], &text[cut..]);
            if let Ok(lib) = parse_library(Path::new("fuzz.toml"), &damaged) {
                let written = render_library(&lib).unwrap();
                proptest::prop_assert_eq!(parse_library(Path::new("x.toml"), &written).unwrap(), lib);
            }
        }

        #[test]
        fn req_nfr_07_profile_parser_never_panics_on_text(text in ".{0,400}") {
            let _ = parse_profile(Path::new("fuzz.toml"), &text);
        }
    }
}

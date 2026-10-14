// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tools: the library, machine tool sets, the project's tool snapshots and
//! their checks (TL-01 to TL-07, TL-11; work breakdown slice 4, decision
//! record 0014).
//!
//! A project carries a snapshot of every tool it uses, with the tool number
//! it uses on this project's machine; the snapshot always wins, and
//! [`check_tools`] only reports how it differs from a library (TL-07). A new
//! snapshot takes its number from the machine's tool set in the library, or
//! the next free number ([`default_controller`], TL-11). Adding, updating and removing a snapshot are ordinary
//! [`Command`](crate::Command)s (`Add`, `Replace`, `Remove`), so they undo.
//!
//! The types a front end needs are re-exported here, so it needs no engine
//! crate of its own (architecture section 5.8).

use std::collections::BTreeMap;

use openchuck_project::id::ToolId;
use openchuck_project::{Project, ToolSnapshot};

pub use openchuck_project::tools::{
    ArcCurve, ImportReport, LIBRARY_SCHEMA, LibraryTool, PROFILE_SCHEMA, SnapshotStatus,
    TOOL_SETS_FOLDER, ToolLibrary, ToolSet, ToolSetEntry, compare_snapshot, load_profile,
    parse_profile, parse_tool_file, parse_tool_set, render_profile, render_tool_file,
    render_tool_set,
};
pub use openchuck_types::tool::{
    ControllerMapping, CuttingData, CuttingTable, ProfilePoint, ProfileSegment, RevolvedProfile,
    Tool, ToolProfile, ToolShape,
};

/// What checking one tool snapshot found.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolReport {
    /// The snapshot.
    pub tool: ToolId,
    /// Its name.
    pub name: String,
    /// How it compares with the library, when one was given.
    pub library: Option<SnapshotStatus>,
    /// Problems the user should look at, in a fixed order.
    pub warnings: Vec<String>,
}

/// Checks every tool snapshot in `project`: tool numbers shared by two
/// snapshots, operations whose setup's stock material has no cutting data
/// for the tool, and, when `library` is given, how each snapshot differs from
/// it (TL-07). Warnings only; nothing is changed.
pub fn check_tools(project: &Project, library: Option<&ToolLibrary>) -> Vec<ToolReport> {
    let mut numbers: BTreeMap<u32, Vec<&str>> = BTreeMap::new();
    for t in project.tools() {
        numbers
            .entry(t.controller.number)
            .or_default()
            .push(&t.name);
    }
    project
        .tools()
        .iter()
        .map(|t| {
            let mut warnings = Vec::new();
            let sharing = numbers
                .get(&t.controller.number)
                .map_or(&[][..], Vec::as_slice);
            if sharing.len() > 1 {
                let others: Vec<String> = sharing
                    .iter()
                    .filter(|n| **n != t.name)
                    .map(|n| format!("\"{n}\""))
                    .collect();
                warnings.push(format!(
                    "tool number T{} is also used by {}; the machine cannot tell them apart",
                    t.controller.number,
                    if others.is_empty() {
                        "another tool of the same name".to_owned()
                    } else {
                        others.join(", ")
                    }
                ));
            }
            let mut missing = std::collections::BTreeSet::new();
            for op in project.operations().iter().filter(|o| o.tool == Some(t.id)) {
                let material = project
                    .setups()
                    .iter()
                    .find(|s| s.id == op.setup)
                    .and_then(|s| s.material.as_deref());
                if let Some(material) = material
                    && !t.cutting_data.contains_key(material)
                {
                    missing.insert(material.to_owned());
                }
            }
            for material in missing {
                warnings.push(format!(
                    "no cutting data for stock material \"{material}\", which an operation using this tool cuts"
                ));
            }
            let machine = project.machine().map(|m| m.name.as_str());
            if let Some(set) = machine.and_then(|m| library.and_then(|l| l.tool_set(m)))
                && let Some(mapping) = set.mapping_for(t.id)
                && mapping != t.controller
            {
                warnings.push(format!(
                    "the {} tool set in the library gives this tool {}; the project uses {}",
                    set.machine,
                    describe_controller(&mapping),
                    describe_controller(&t.controller)
                ));
            }
            ToolReport {
                tool: t.id,
                name: t.name.clone(),
                library: library.map(|lib| compare_snapshot(t, lib)),
                warnings,
            }
        })
        .collect()
}

/// Where [`default_controller`] took a tool's number from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberSource {
    /// The project machine's tool set in the library.
    ToolSet,
    /// The tool set gives `wanted`, but another tool in the project already
    /// uses it, so the next free number was taken.
    ToolSetTaken {
        /// The number the tool set gives.
        wanted: u32,
    },
    /// No tool set covers the tool: the lowest number that no project tool
    /// uses and the tool set does not keep for another tool.
    NextFree,
}

/// The controller values for adding library tool `tool` to `project`
/// (TL-06, TL-11): the number the project machine's tool set gives it, unless
/// another project tool already uses that number; otherwise the lowest free
/// number, skipping numbers the tool set keeps for other tools. The user can
/// change it afterwards.
pub fn default_controller(
    project: &Project,
    library: Option<&ToolLibrary>,
    tool: ToolId,
) -> (ControllerMapping, NumberSource) {
    let set = project
        .machine()
        .and_then(|m| library.and_then(|l| l.tool_set(&m.name)));
    let used: std::collections::BTreeSet<u32> = project
        .tools()
        .iter()
        .filter(|t| t.id != tool)
        .map(|t| t.controller.number)
        .collect();
    let wanted = set.and_then(|s| s.mapping_for(tool));
    if let Some(mapping) = wanted
        && !used.contains(&mapping.number)
    {
        return (mapping, NumberSource::ToolSet);
    }
    let reserved = |n: u32| set.is_some_and(|s| s.tool_for(n).is_some_and(|t| t != tool));
    let number = (1..=u32::MAX)
        .find(|n| !used.contains(n) && !reserved(*n))
        .unwrap_or(u32::MAX);
    let source = match wanted {
        Some(m) => NumberSource::ToolSetTaken { wanted: m.number },
        None => NumberSource::NextFree,
    };
    (ControllerMapping::numbered(number), source)
}

/// The snapshot in `project` named by `text`: its ID, its name (ignoring
/// case), or `T<n>` for its tool number.
pub fn find_tool<'a>(project: &'a Project, text: &str) -> Option<&'a ToolSnapshot> {
    let text = text.trim();
    if let Ok(id) = text.parse::<ToolId>() {
        return project.tools().iter().find(|t| t.id == id);
    }
    let number = text
        .strip_prefix(['T', 't'])
        .and_then(|n| n.parse::<u32>().ok());
    let matches: Vec<&ToolSnapshot> = project
        .tools()
        .iter()
        .filter(|t| t.name.eq_ignore_ascii_case(text) || Some(t.controller.number) == number)
        .collect();
    match matches.as_slice() {
        [one] => Some(one),
        _ => None,
    }
}

/// One line describing a tool's shape and size, for listings.
pub fn describe(tool: &Tool) -> String {
    let shape = match &tool.shape {
        ToolShape::FlatEnd => "flat end".to_owned(),
        ToolShape::BallEnd => "ball end".to_owned(),
        ToolShape::BullNose { corner_radius } => format!("bull-nose r{}", corner_radius.0),
        ToolShape::TaperedBall {
            tip_radius,
            taper_half_angle,
        } => format!(
            "tapered ball r{} {} deg per side",
            tip_radius.0, taper_half_angle.0
        ),
        ToolShape::VBit {
            included_angle,
            tip_diameter,
        } => format!("V-bit {} deg, tip {} mm", included_angle.0, tip_diameter.0),
        ToolShape::Chamfer {
            included_angle,
            tip_diameter,
        } => format!(
            "chamfer {} deg, tip {} mm",
            included_angle.0, tip_diameter.0
        ),
        ToolShape::Drill { point_angle } => format!("drill {} deg point", point_angle.0),
        ToolShape::Custom { profile } => {
            format!("custom profile, {} segments", profile.segments.len())
        }
        _ => tool.shape.type_name().to_owned(),
    };
    format!(
        "{shape}, diameter {} mm, flutes {} mm x {}, overall {} mm, shank {} mm{}",
        tool.diameter.0,
        tool.flute_length.0,
        tool.flute_count,
        tool.overall_length.0,
        tool.shank_diameter.0,
        tool.material
            .as_deref()
            .map(|m| format!(", {m}"))
            .unwrap_or_default()
    )
}

/// The controller values in the form the post writes them, for listings.
pub fn describe_controller(controller: &ControllerMapping) -> String {
    format!(
        "T{} H{} D{}",
        controller.number,
        controller.length_offset(),
        controller.diameter_offset()
    )
}

/// The tool with its shape replaced by the custom `profile` and its diameter
/// set to where the profile ends (TL-03: a profile imported from a text file).
pub fn with_custom_profile(tool: &Tool, profile: ToolProfile) -> Tool {
    let mut tool = tool.clone();
    tool.diameter = openchuck_types::units::Millimetres(2.0 * profile.end().radius.0);
    tool.shape = ToolShape::Custom { profile };
    tool
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use openchuck_project::id::{OperationId, SetupId};
    use openchuck_project::{Item, Operation, OrderKey, Placement, Setup};
    use openchuck_types::units::Millimetres;

    fn snapshot(id: u128, name: &str, number: u32) -> ToolSnapshot {
        ToolSnapshot {
            id: ToolId::from_u128(id),
            order: OrderKey::default(),
            name: name.into(),
            tool: Tool {
                shape: ToolShape::FlatEnd,
                diameter: Millimetres(6.0),
                flute_length: Millimetres(20.0),
                overall_length: Millimetres(50.0),
                shank_diameter: Millimetres(6.0),
                flute_count: 2,
                material: None,
            },
            controller: ControllerMapping::numbered(number),
            cutting_data: CuttingTable::new(),
        }
    }

    #[test]
    fn req_tl_06_shared_tool_numbers_and_missing_cutting_data_are_warned() {
        let mut p = Project::new("p");
        let mut setup = Setup::new(SetupId::from_u128(1), "s");
        setup.material = Some("oak".into());
        p.insert(Item::Setup(setup), Placement::End).unwrap();
        for (id, name, number) in [(2, "a", 3), (3, "b", 3), (4, "c", 4)] {
            p.insert(Item::Tool(snapshot(id, name, number)), Placement::End)
                .unwrap();
        }
        p.insert(
            Item::Operation(Operation {
                id: OperationId::from_u128(5),
                order: OrderKey::default(),
                name: "op".into(),
                setup: SetupId::from_u128(1),
                strategy: "helical".into(),
                tool: Some(ToolId::from_u128(4)),
                selections: Vec::new(),
                boundary: None,
                avoid: Vec::new(),
                check: Vec::new(),
                parameters: Default::default(),
            }),
            Placement::End,
        )
        .unwrap();
        let reports = check_tools(&p, None);
        assert!(reports[0].warnings[0].contains("T3 is also used by \"b\""));
        assert!(reports[1].warnings[0].contains("\"a\""));
        assert_eq!(
            reports[2].warnings,
            vec![
                "no cutting data for stock material \"oak\", which an operation using this tool cuts"
                    .to_owned()
            ]
        );
        assert!(reports.iter().all(|r| r.library.is_none()));
        assert_eq!(find_tool(&p, "T4").unwrap().name, "c");
        assert!(find_tool(&p, "T3").is_none());
        assert_eq!(find_tool(&p, "B").unwrap().controller.number, 3);
    }

    #[test]
    fn req_tl_11_numbers_come_from_the_machine_tool_set_or_the_next_free() {
        let dir = std::env::temp_dir().join(format!("openchuck-app-set-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut lib = ToolLibrary::open(&dir).unwrap();
        for id in [10, 11, 12] {
            let s = snapshot(id, &format!("tool {id}"), 1);
            lib.save_tool(LibraryTool::from_snapshot(&s)).unwrap();
        }
        let mut set = ToolSet {
            machine: "Makera Z1".into(),
            entries: Vec::new(),
        };
        set.assign(ToolId::from_u128(10), ControllerMapping::numbered(1));
        set.assign(ToolId::from_u128(11), ControllerMapping::numbered(2));
        lib.save_tool_set(set).unwrap();

        let mut p = Project::new("p");
        let id = |n| ToolId::from_u128(n);
        // No machine: the next free number.
        assert_eq!(
            default_controller(&p, Some(&lib), id(11)),
            (ControllerMapping::numbered(1), NumberSource::NextFree)
        );
        let z1 = openchuck_project::machines::builtin(openchuck_project::machines::MAKERA_Z1)
            .unwrap()
            .unwrap();
        p.set_machine(Some(z1)).unwrap();
        assert_eq!(
            default_controller(&p, Some(&lib), id(11)),
            (ControllerMapping::numbered(2), NumberSource::ToolSet)
        );
        // A tool outside the set skips the numbers the set keeps.
        assert_eq!(
            default_controller(&p, Some(&lib), id(12)),
            (ControllerMapping::numbered(3), NumberSource::NextFree)
        );
        // The set's number is taken by another project tool.
        p.insert(Item::Tool(snapshot(20, "other", 2)), Placement::End)
            .unwrap();
        assert_eq!(
            default_controller(&p, Some(&lib), id(11)),
            (
                ControllerMapping::numbered(3),
                NumberSource::ToolSetTaken { wanted: 2 }
            )
        );
        // A project tool numbered differently from the set is reported.
        p.insert(Item::Tool(snapshot(10, "tool 10", 5)), Placement::End)
            .unwrap();
        let report = check_tools(&p, Some(&lib));
        let warned = &report[1].warnings;
        assert!(
            warned[0].contains("gives this tool T1 H1 D1; the project uses T5"),
            "{warned:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

/// One tool the program will use and the number it will have (GC-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolNumberRow {
    /// The snapshot.
    pub tool: ToolId,
    /// Its name.
    pub name: String,
    /// The number it has now.
    pub number: u32,
}

/// The tools the program will use, in order of first use by the operations in
/// machining order, with their current numbers (GC-14). A tool no operation
/// uses is not listed: it does not appear in the program.
pub fn tool_number_plan(project: &Project) -> Vec<ToolNumberRow> {
    let mut rows: Vec<ToolNumberRow> = Vec::new();
    for operation in project.operations() {
        let Some(id) = operation.tool else { continue };
        if rows.iter().any(|r| r.tool == id) {
            continue;
        }
        if let Some(tool) = project.tools().iter().find(|t| t.id == id) {
            rows.push(ToolNumberRow {
                tool: id,
                name: tool.name.clone(),
                number: tool.controller.number,
            });
        }
    }
    rows
}

/// How the numbers of the tools in use are chosen (GC-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolNumbering {
    /// The user confirms the numbers as they are; nothing changes.
    Keep,
    /// Renumber in order of first use, counting up from `first`.
    InOrderOfUse {
        /// The number of the first tool used (at least 1).
        first: u32,
    },
    /// The user gives numbers by hand for some or all of the tools in use.
    Manual(Vec<(ToolId, u32)>),
}

/// The command that gives the tools in use their numbers, or an error that
/// names the problem: a number below 1, above `max_number` (the machine's
/// range, OI-62), used twice, or a tool the program does not use. The
/// command is one undo step; a tool whose number does not change is not
/// replaced. `Ok(None)` means nothing changes.
pub fn tool_numbering_command(
    project: &Project,
    numbering: &ToolNumbering,
    max_number: Option<u32>,
) -> Result<Option<crate::Command>, crate::AppError> {
    let plan = tool_number_plan(project);
    let refuse = crate::AppError::ToolNumbers;
    let mut wanted: BTreeMap<ToolId, u32> = plan.iter().map(|r| (r.tool, r.number)).collect();
    match numbering {
        ToolNumbering::Keep => {}
        ToolNumbering::InOrderOfUse { first } => {
            if *first < 1 {
                return Err(refuse("the first tool number must be at least 1".into()));
            }
            for (i, row) in plan.iter().enumerate() {
                let n = u32::try_from(i)
                    .ok()
                    .and_then(|i| first.checked_add(i))
                    .ok_or_else(|| refuse("the tool numbers run past the largest number".into()))?;
                wanted.insert(row.tool, n);
            }
        }
        ToolNumbering::Manual(pairs) => {
            for (id, number) in pairs {
                if !wanted.contains_key(id) {
                    return Err(refuse(format!(
                        "tool {id} is not used by any operation, so it has no number in the program"
                    )));
                }
                wanted.insert(*id, *number);
            }
        }
    }
    let mut seen: BTreeMap<u32, &str> = BTreeMap::new();
    for row in &plan {
        let number = wanted.get(&row.tool).copied().unwrap_or(row.number);
        if number < 1 {
            return Err(refuse(format!(
                "'{}' would get T{number}; tool numbers start at 1",
                row.name
            )));
        }
        if let Some(max) = max_number
            && number > max
        {
            return Err(refuse(format!(
                "'{}' would get T{number}, above the machine's largest tool number {max}",
                row.name
            )));
        }
        if let Some(other) = seen.insert(number, &row.name) {
            return Err(refuse(format!(
                "T{number} would be used by both '{other}' and '{}'; a number can belong to one tool",
                row.name
            )));
        }
    }
    // A tool the program does not use may still hold a number a used tool
    // would take; the machine could not tell them apart if it is ever used.
    for tool in project.tools() {
        if wanted.contains_key(&tool.id) {
            continue;
        }
        if let Some(other) = seen.get(&tool.controller.number) {
            return Err(refuse(format!(
                "T{} would be used by '{other}', but '{}' also holds it and no operation uses it; give one of them another number",
                tool.controller.number, tool.name
            )));
        }
    }
    let mut commands = Vec::new();
    for row in &plan {
        let number = wanted.get(&row.tool).copied().unwrap_or(row.number);
        if number == row.number {
            continue;
        }
        if let Some(snapshot) = project.tools().iter().find(|t| t.id == row.tool) {
            let mut changed = snapshot.clone();
            changed.controller.number = number;
            commands.push(crate::Command::Replace(openchuck_project::Item::Tool(
                changed,
            )));
        }
    }
    Ok(match commands.len() {
        0 => None,
        1 => commands.pop(),
        _ => Some(crate::Command::Batch(commands)),
    })
}

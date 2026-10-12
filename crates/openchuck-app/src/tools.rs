// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tools: the library, the project's tool snapshots and their checks (TL-01
//! to TL-07; work breakdown slice 4, decision record 0014).
//!
//! A project carries a snapshot of every tool it uses; the snapshot always
//! wins, and [`check_tools`] only reports how it differs from a library
//! (TL-07). Adding, updating and removing a snapshot are ordinary
//! [`Command`](crate::Command)s (`Add`, `Replace`, `Remove`), so they undo.
//!
//! The types a front end needs are re-exported here, so it needs no engine
//! crate of its own (architecture section 5.8).

use std::collections::BTreeMap;

use openchuck_project::id::ToolId;
use openchuck_project::{Project, ToolSnapshot};

pub use openchuck_project::tools::{
    ArcCurve, ImportReport, LIBRARY_SCHEMA, LibraryTool, PROFILE_SCHEMA, SnapshotStatus,
    ToolLibrary, compare_snapshot, load_library, load_profile, parse_library, parse_profile,
    render_library, render_profile, save_library,
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
            ToolReport {
                tool: t.id,
                name: t.name.clone(),
                library: library.map(|lib| compare_snapshot(t, lib)),
                warnings,
            }
        })
        .collect()
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
}

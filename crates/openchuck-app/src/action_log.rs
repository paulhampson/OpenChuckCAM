// SPDX-License-Identifier: AGPL-3.0-or-later
//! Short text for the user-action log (requirement NFR-18; decision record
//! 0157).
//!
//! A line names the action and the kind and name of the item it acts on, and
//! the names of parameters: never a parameter's value, a geometry value or
//! any file contents. The application keeps the lines of the commands, undos
//! and redos it ran until the front end drains them
//! ([`crate::Application::drain_actions`]); the front end decides whether to
//! write them to the log (the user's switch), so this crate logs nothing.

use openchuck_project::{Item, ItemId, Project};

use crate::command::Command;

/// How many undrained lines are kept; the oldest are dropped beyond this.
pub(crate) const KEPT: usize = 256;

/// The last component of a model's path: the log names the file, never the
/// folders above it (NFR-18; an import can reach outside the project).
fn model_file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned()
}

/// The name of the item `id` as the user sees it: the name of a setup, tool,
/// selection set or operation, the file name of a model. `None` when the
/// project has no such item.
pub(crate) fn item_name(project: &Project, id: ItemId) -> Option<String> {
    match id {
        ItemId::Model(m) => project
            .models()
            .iter()
            .find(|x| x.id == m)
            .map(|x| model_file_name(x.path.as_str())),
        ItemId::Setup(s) => project.setup(s).map(|x| x.name.clone()),
        ItemId::Tool(t) => project
            .tools()
            .iter()
            .find(|x| x.id == t)
            .map(|x| x.name.clone()),
        ItemId::SelectionSet(s) => project
            .selection_sets()
            .iter()
            .find(|x| x.id == s)
            .map(|x| x.name.clone()),
        ItemId::Operation(o) => project.operation(o).map(|x| x.name.clone()),
    }
}

/// `kind 'name'` for an item of `project`; the kind alone when it has no
/// name (it is not in the project, or is about to be added).
fn item(project: &Project, id: ItemId) -> String {
    match item_name(project, id) {
        Some(name) => format!("{} '{name}'", id.kind()),
        None => id.kind().to_owned(),
    }
}

fn new_item(item: &Item) -> String {
    let id = item.id();
    let name = match item {
        Item::Model(m) => model_file_name(m.path.as_str()),
        Item::Setup(s) => s.name.clone(),
        Item::Tool(t) => t.name.clone(),
        Item::SelectionSet(s) => s.name.clone(),
        Item::Operation(o) => o.name.clone(),
    };
    format!("{} '{name}'", id.kind())
}

/// The log text of `command`, read against the project it is about to be
/// applied to: its name, the kind and name of what it acts on, and the names
/// of parameters. Never values.
pub(crate) fn describe(command: &Command, project: &Project) -> String {
    match command {
        Command::RenameProject(_) => "RenameProject".to_owned(),
        Command::SetMachine(machine) | Command::SetMachineFrom { machine, .. } => format!(
            "SetMachine {}",
            if machine.is_some() { "set" } else { "clear" }
        ),
        Command::SetSimulation(_) => "SetSimulation".to_owned(),
        Command::SetKeepTangentEdges(_) => "SetKeepTangentEdges".to_owned(),
        Command::SetWarnOnSelectionReset(_) => "SetWarnOnSelectionReset".to_owned(),
        Command::Add { item, .. } => format!("Add {}", new_item(item)),
        Command::Remove(id) => format!("Remove {}", self::item(project, *id)),
        Command::Replace(new) => format!("Replace {}", self::item(project, new.id())),
        Command::Move { id, .. } => format!("Move {}", self::item(project, *id)),
        Command::SetOrderKeys(keys) => format!("SetOrderKeys items={}", keys.len()),
        Command::Renumber(_) => "Renumber".to_owned(),
        Command::SetParameter {
            operation, name, ..
        } => format!(
            "SetParameter {} parameter='{name}'",
            self::item(project, ItemId::Operation(*operation))
        ),
        Command::SetOperationEnabled { operation, .. } => format!(
            "SetOperationEnabled {}",
            self::item(project, ItemId::Operation(*operation))
        ),
        Command::SetModelUnits { model, .. } => format!(
            "SetModelUnits {}",
            self::item(project, ItemId::Model(*model))
        ),
        Command::SetModelTransform { model, .. } => format!(
            "SetModelTransform {}",
            self::item(project, ItemId::Model(*model))
        ),
        Command::Batch(commands) => {
            let parts: Vec<String> = commands.iter().map(|c| describe(c, project)).collect();
            format!("Batch [{}]", parts.join("; "))
        }
    }
}

#[cfg(test)]
mod tests {
    use openchuck_project::id::SetupId;
    use openchuck_project::{ParameterValue, Placement, Setup};

    use super::*;

    #[test]
    fn req_nfr_18_a_model_is_named_by_its_file_only() {
        assert_eq!(model_file_name("part.step"), "part.step");
        assert_eq!(model_file_name("models/sub/part.step"), "part.step");
        assert_eq!(model_file_name("../private\\dir/part.stl"), "part.stl");
    }

    fn project_with_setup(name: &str) -> (Project, SetupId) {
        let mut project = Project::new("Demo");
        let id = SetupId::new();
        project
            .insert(Item::Setup(Setup::new(id, name)), Placement::End)
            .unwrap();
        (project, id)
    }

    #[test]
    fn req_nfr_18_a_command_names_the_item_kind_and_name() {
        let (project, id) = project_with_setup("Setup 1");
        let text = describe(&Command::Remove(ItemId::Setup(id)), &project);
        assert_eq!(
            text,
            format!("Remove {} 'Setup 1'", ItemId::Setup(id).kind())
        );
        let add = Command::Add {
            item: Item::Setup(Setup::new(SetupId::new(), "Flip")),
            placement: Placement::End,
        };
        assert!(describe(&add, &project).ends_with(" 'Flip'"));
    }

    #[test]
    fn req_nfr_18_a_parameter_is_named_never_valued() {
        let (project, _) = project_with_setup("S");
        let op = openchuck_project::id::OperationId::new();
        let text = describe(
            &Command::SetParameter {
                operation: op,
                name: "stepover".to_owned(),
                value: Some(ParameterValue::Number(1234.5678)),
            },
            &project,
        );
        assert!(text.contains("parameter='stepover'"), "{text}");
        assert!(!text.contains("1234"), "{text}");
    }

    #[test]
    fn req_nfr_18_values_of_other_commands_are_not_logged() {
        let project = Project::new("Secret name");
        assert_eq!(
            describe(&Command::RenameProject("Private".into()), &project),
            "RenameProject"
        );
        assert_eq!(
            describe(&Command::SetKeepTangentEdges(true), &project),
            "SetKeepTangentEdges"
        );
        let text = describe(
            &Command::Batch(vec![
                Command::Renumber(openchuck_project::ListKind::Setups),
                Command::SetWarnOnSelectionReset(false),
            ]),
            &project,
        );
        assert_eq!(text, "Batch [Renumber; SetWarnOnSelectionReset]");
        assert!(!text.contains("Secret"));
    }

    #[test]
    fn req_nfr_18_an_unknown_item_gives_its_kind_only() {
        let project = Project::new("Demo");
        let id = ItemId::Setup(SetupId::new());
        assert_eq!(
            describe(&Command::Remove(id), &project),
            format!("Remove {}", id.kind())
        );
        assert_eq!(item_name(&project, id), None);
    }
}

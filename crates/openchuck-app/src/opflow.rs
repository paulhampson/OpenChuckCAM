// SPDX-License-Identifier: AGPL-3.0-or-later
//! Adding and deleting operations, and the list of strategies to choose from
//! (UI-06; work breakdown slice 9 item 7, the operation add, edit, generate
//! and regenerate flow; decision record 0040 item 7).
//!
//! Editing is [`Application::set_parameter`](crate::Application::set_parameter)
//! and the selection roles; generating is
//! [`start_regenerate`](crate::Application::start_regenerate) and
//! [`start_simulate`](crate::Application::start_simulate). What the front end
//! still needs is a way to make an operation and to remove one, each as one
//! undoable step, and to know which strategies it may offer.

use std::collections::BTreeMap;

use openchuck_project::id::{OperationId, SetupId, ToolId};
use openchuck_project::{Item, ItemId, Operation, OrderKey, Placement, RestrictionOptions};

use openchuck_path::StrategyGroup;

use crate::{AppError, Application, Command};

/// A strategy a user can choose for an operation (OP-16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyChoice {
    /// The registry name, which the operation stores.
    pub name: String,
    /// The name shown to the user.
    pub label: String,
    /// What the strategy does, in plain words (OP-16; decision record 0204).
    /// The same text is shown in the list and in the operation's panel.
    pub description: String,
    /// Where it sits in the list: roughing, finishing or specialist.
    pub group: StrategyGroup,
    /// Whether the strategy can cut in a selection (decision record 0032
    /// item 6); one that cannot is listed with "ignores selections" and takes
    /// none (interface specification 11.4).
    pub takes_selections: bool,
    /// Why the strategy cannot be used in the setup the list was made for,
    /// in plain words: it needs a STEP part and the setup's part is a mesh
    /// (decision record 0174). The list shows it disabled with this reason,
    /// not hidden. `None` when it can be used.
    pub unavailable: Option<String>,
}

impl Application {
    /// The strategies in the registry, in the order the "add operation" list
    /// shows them (roughing, then finishing, then the specialist strategies;
    /// decision record 0204), with none marked unavailable.
    pub fn strategy_choices(&self) -> Vec<StrategyChoice> {
        self.strategy_choices_for(None)
    }

    /// As [`strategy_choices`](Self::strategy_choices), for an operation in
    /// `setup`: a strategy that needs a STEP part, by its registration, is
    /// marked unavailable with the reason when the setup's part is a mesh
    /// (decision record 0174).
    pub fn strategy_choices_for(&self, setup: Option<SetupId>) -> Vec<StrategyChoice> {
        let project = &self.document.project;
        self.strategies
            .in_menu_order()
            .map(|s| StrategyChoice {
                name: s.name().to_owned(),
                label: s.label().to_owned(),
                description: s.description().to_owned(),
                group: s.group(),
                takes_selections: s.honours_region(),
                unavailable: setup
                    .and_then(|id| crate::operation::unavailable_on_setup(project, s, id)),
            })
            .collect()
    }

    /// The name the user reads for the strategy `name` (specification section
    /// 15): its label, or `name` itself when no such strategy is registered.
    pub fn strategy_label(&self, name: &str) -> String {
        self.strategies
            .iter()
            .find(|s| s.name() == name)
            .map_or_else(|| name.to_owned(), |s| s.label().to_owned())
    }

    /// The plain-language description of strategy `name` (the one-line text
    /// under its name in the New operation list and in the operation's panel;
    /// issues 195 and 242), or an empty string when no such strategy is
    /// registered.
    pub fn strategy_description(&self, name: &str) -> String {
        self.strategies
            .iter()
            .find(|s| s.name() == name)
            .map_or_else(String::new, |s| s.description().to_owned())
    }

    /// Whether strategy `name` takes options on its boundary, avoid and check
    /// sets (decision 0176; swarf so far, OI-637), so the Geometry section
    /// shows them.
    pub fn strategy_takes_set_options(&self, name: &str) -> bool {
        self.strategies
            .iter()
            .find(|s| s.name() == name)
            .is_some_and(|s| s.honours_restriction_options())
    }

    /// Adds an operation at the end of the list, using `strategy` in `setup`,
    /// with `tool` if given and the strategy's own defaults for everything
    /// else (UI-06, UI-07). One undoable step. Fails, saying why, for an
    /// empty name, a strategy that is not registered, or a setup or tool the
    /// project does not have.
    pub fn add_operation(
        &mut self,
        name: &str,
        strategy: &str,
        setup: SetupId,
        tool: Option<ToolId>,
    ) -> Result<OperationId, AppError> {
        self.add_operation_on(name, strategy, setup, tool, None)
    }

    /// [`Application::add_operation`] with the selection set `cut_on` in its
    /// Cut on role: the new operation takes the selection (interface
    /// specification 11.4; decision record 0114 C16). One undoable step.
    /// Fails, saying why, for a set the project does not have or a strategy
    /// that cannot cut in a selection.
    pub fn add_operation_on(
        &mut self,
        name: &str,
        strategy: &str,
        setup: SetupId,
        tool: Option<ToolId>,
        cut_on: Option<openchuck_project::id::SelectionSetId>,
    ) -> Result<OperationId, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::Operation {
                name: String::new(),
                reason: "an operation needs a name".to_owned(),
            });
        }
        let fail = |reason: String| AppError::Operation {
            name: name.to_owned(),
            reason,
        };
        if self.strategies.get(strategy).is_err() {
            let known: Vec<String> = self
                .strategy_choices()
                .into_iter()
                .map(|c| c.name)
                .collect();
            return Err(fail(format!(
                "there is no strategy '{strategy}' (available: {})",
                known.join(", ")
            )));
        }
        if self.document.project.setup(setup).is_none() {
            return Err(fail("its setup is not in the project".to_owned()));
        }
        // A strategy that needs a STEP part is not added to a setup whose
        // part is a mesh (decision record 0174), so no generation is wasted.
        if let Some(why) =
            self.strategies.get(strategy).ok().and_then(|s| {
                crate::operation::unavailable_on_setup(&self.document.project, s, setup)
            })
        {
            return Err(fail(why.replacen("Needs", "it needs", 1)));
        }
        if let Some(tool) = tool
            && !self.document.project.tools().iter().any(|t| t.id == tool)
        {
            return Err(fail("its tool is not in the project".to_owned()));
        }
        if let Some(set) = cut_on {
            let Some(found) = self
                .document
                .project
                .selection_sets()
                .iter()
                .find(|s| s.id == set)
            else {
                return Err(fail("its selection set is not in the project".to_owned()));
            };
            // A set made on another model can never be resolved against the
            // setup's part (as `set_operation_selection` refuses it).
            if let Some(part) = self.document.project.setup(setup).and_then(|s| s.part)
                && found.model != part
            {
                return Err(fail(format!(
                    "its selection set '{}' was made on a different model from the setup's part",
                    found.name
                )));
            }
            if !self
                .strategies
                .get(strategy)
                .is_ok_and(|s| s.honours_region())
            {
                return Err(fail(format!(
                    "the strategy '{strategy}' ignores selections, so it cannot take one"
                )));
            }
        }
        // Rotary roughing starts with a stock to leave for the finishing that
        // follows, a tenth of the tool's diameter, written into the operation
        // so the inspector shows it as set (decision record 0205).
        let mut parameters = BTreeMap::new();
        if strategy == openchuck_path::roughing::NAME
            && let Some(snapshot) =
                tool.and_then(|id| self.document.project.tools().iter().find(|t| t.id == id))
        {
            parameters.insert(
                openchuck_path::params::names::STOCK_TO_LEAVE.to_owned(),
                openchuck_project::ParameterValue::Number(default_roughing_stock_to_leave(
                    snapshot.tool.diameter.0,
                )),
            );
        }
        let id = OperationId::new();
        self.execute(Command::Add {
            item: Item::Operation(Operation {
                id,
                order: OrderKey::default(),
                name: name.to_owned(),
                setup,
                strategy: strategy.to_owned(),
                tool,
                selections: cut_on.into_iter().collect(),
                boundary: None,
                avoid: Vec::new(),
                check: Vec::new(),
                restriction_options: RestrictionOptions::default(),
                parameters,
                enabled: true,
            }),
            placement: Placement::End,
        })?;
        Ok(id)
    }

    /// Removes operation `id` (UI-06). One undoable step.
    pub fn delete_operation(&mut self, id: OperationId) -> Result<(), AppError> {
        if self.document.project.operation(id).is_none() {
            return Err(AppError::UnknownOperation(id.to_string()));
        }
        self.execute(Command::Remove(ItemId::Operation(id)))
    }
}

/// The stock to leave a new rotary roughing operation starts with: 10 percent
/// of the tool's diameter, rounded to 0.05 mm, at least 0.1 mm and at most
/// 1 mm (decision record 0205, option B; issue 223; OP-11).
pub(crate) fn default_roughing_stock_to_leave(diameter: f64) -> f64 {
    let twentieths = (diameter * 0.1 / 0.05).round();
    (twentieths / 20.0).clamp(0.1, 1.0)
}

#[cfg(test)]
mod tests {
    use super::default_roughing_stock_to_leave as leave;

    /// OP-11, decision 0205: a tenth of the diameter on a 0.05 mm step, kept
    /// between 0.1 and 1 mm.
    #[test]
    fn req_op_11_the_default_roughing_stock_to_leave_follows_the_tool_diameter() {
        assert_eq!(leave(3.175), 0.3);
        assert_eq!(leave(4.0), 0.4);
        assert_eq!(leave(6.0), 0.6);
        assert_eq!(leave(0.5), 0.1);
        assert_eq!(leave(25.0), 1.0);
    }
}

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
use openchuck_project::{Item, ItemId, Operation, OrderKey, Placement};

use crate::{AppError, Application, Command};

/// A strategy a user can choose for an operation (OP-16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyChoice {
    /// The registry name, which the operation stores.
    pub name: String,
    /// The name shown to the user.
    pub label: String,
}

impl Application {
    /// The strategies in the registry, in name order, for an "add operation"
    /// list.
    pub fn strategy_choices(&self) -> Vec<StrategyChoice> {
        self.strategies
            .iter()
            .map(|s| StrategyChoice {
                name: s.name().to_owned(),
                label: s.label().to_owned(),
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
        if let Some(tool) = tool
            && !self.document.project.tools().iter().any(|t| t.id == tool)
        {
            return Err(fail("its tool is not in the project".to_owned()));
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
                selections: Vec::new(),
                boundary: None,
                avoid: Vec::new(),
                check: Vec::new(),
                parameters: BTreeMap::new(),
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

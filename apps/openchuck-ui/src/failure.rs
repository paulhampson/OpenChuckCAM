// SPDX-License-Identifier: AGPL-3.0-or-later
//! What an error dialog says: a command the user gave failed outright
//! (specification section 9, the only pop-up errors). The dialog's title
//! names the action that failed ("Save failed"), and its text takes the one
//! form of every message, led by that action: "Save: the folder is read-only."
//! (decision record 0536 sections 3 and 7, build step 5).

use openchuck_app::lead_in::{Context, in_form};

/// A command that failed outright, for its error dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The action that failed, in the user's words: "Save", "Undo",
    /// "Import model". It is the topic of the text and names the title.
    pub action: String,
    /// The text in the one form: the action, the items it was about, and
    /// what happened.
    pub text: String,
}

impl Failure {
    /// `action` failed because of `reason`, about no item in particular.
    pub fn new(action: &str, reason: impl std::fmt::Display) -> Self {
        Self::about(action, &Context::default(), reason)
    }

    /// `action` failed on the items of `context` because of `reason`.
    pub fn about(action: &str, context: &Context, reason: impl std::fmt::Display) -> Self {
        Self {
            action: action.to_owned(),
            text: sentence(&in_form(action, context, &reason.to_string())),
        }
    }

    /// The dialog's title: the action that failed.
    pub fn title(&self) -> String {
        format!("{} failed", self.action)
    }
}

/// `text` ending in a full stop, unless it ends in other punctuation
/// already (specification section 15: messages are sentences).
fn sentence(text: &str) -> String {
    let text = text.trim_end();
    if text.ends_with(['.', '?', '!', ')', '"', '”']) {
        text.to_owned()
    } else {
        format!("{text}.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_12_req_nfr_07_an_error_dialog_names_the_action_that_failed() {
        let failure = Failure::new("Undo", "the project has no operation 'Rough'");
        assert_eq!(failure.title(), "Undo failed");
        assert_eq!(failure.text, "Undo: the project has no operation 'Rough'.");
    }

    #[test]
    fn req_ui_12_an_error_dialog_takes_the_one_form_with_its_items() {
        let failure = Failure::about(
            "Delete operation",
            &Context::operation("1 Rough"),
            "it is not in the project.",
        );
        assert_eq!(
            failure.text,
            "Delete operation (operation '1 Rough'): it is not in the project."
        );
        assert_eq!(failure.title(), "Delete operation failed");
    }
}

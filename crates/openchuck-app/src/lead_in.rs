// SPDX-License-Identifier: AGPL-3.0-or-later
//! The one form every message takes: `<Topic> (<context>): <text>`
//! (decision record 0536 section 3, extending decision record 0522's
//! "Flute length (operation '<name>', tool '<tool name>'): ...").
//!
//! The topic names the check, the same wherever the message is shown; the
//! context names the items concerned, as they are named now, so a rename is
//! followed (decision record 0522 item 3). The library crates give the topic
//! and the text; the application adds the context when it shows a message.

/// The items a message is about, named as they are now (decision record
/// 0536 section 3). Each is written in a fixed order, so the same items
/// always read the same way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    operation: Option<String>,
    tool: Option<String>,
    setup: Option<String>,
    model: Option<String>,
    post: Option<String>,
    machine: Option<String>,
}

impl Context {
    /// A message about the operation named `name`.
    pub fn operation(name: impl Into<String>) -> Self {
        Self::default().and_operation(name)
    }

    /// A message about the setup named `name`.
    pub fn setup(name: impl Into<String>) -> Self {
        Self::default().and_setup(name)
    }

    /// A message about the model named `name`.
    pub fn model(name: impl Into<String>) -> Self {
        Self {
            model: Some(name.into()),
            ..Self::default()
        }
    }

    /// A message about the post named `name`.
    pub fn post(name: impl Into<String>) -> Self {
        Self {
            post: Some(name.into()),
            ..Self::default()
        }
    }

    /// A message about the machine profile named `name`.
    pub fn machine(name: impl Into<String>) -> Self {
        Self {
            machine: Some(name.into()),
            ..Self::default()
        }
    }

    /// This context, naming the operation `name` too.
    #[must_use]
    pub fn and_operation(mut self, name: impl Into<String>) -> Self {
        self.operation = Some(name.into());
        self
    }

    /// This context, naming the tool `name` too: for a message where the
    /// tool's size, flutes or number matter.
    #[must_use]
    pub fn and_tool(mut self, name: impl Into<String>) -> Self {
        self.tool = Some(name.into());
        self
    }

    /// This context, naming the setup `name` too.
    #[must_use]
    pub fn and_setup(mut self, name: impl Into<String>) -> Self {
        self.setup = Some(name.into());
        self
    }

    /// The items, as written in the lead-in: "operation 'Rough', tool 'T1'".
    /// Empty when it names none.
    pub fn written(&self) -> String {
        [
            ("operation", &self.operation),
            ("tool", &self.tool),
            ("setup", &self.setup),
            ("model", &self.model),
            ("post", &self.post),
            ("machine", &self.machine),
        ]
        .into_iter()
        .filter_map(|(kind, name)| name.as_ref().map(|n| format!("{kind} '{n}'")))
        .collect::<Vec<_>>()
        .join(", ")
    }
}

/// The lead-in of a message: "Flute length (operation 'Rough', tool 'T1')",
/// or the topic alone when the context names no item.
pub fn lead_in(topic: &str, context: &Context) -> String {
    let items = context.written();
    if items.is_empty() {
        topic.to_owned()
    } else {
        format!("{topic} ({items})")
    }
}

/// The topics of the warnings the application adds to an operation's path,
/// beside those of the pathing library ([`openchuck_path::topic`]): short
/// names of the checks, in sentence case (decision record 0536 section 3).
pub mod topic {
    /// A tab shorter than the tool is wide (decision 0523).
    pub const TAB_LENGTH: &str = "Tab length";
    /// A tool that cannot cut down beside a tab between a held region and
    /// the part (decision 0503).
    pub const TAB_CLEARANCE: &str = "Tab clearance";
    /// A stock to leave other than the operation's.
    pub const STOCK_TO_LEAVE: &str = "Stock to leave";
    /// Exact STEP surface contact asked for that does not apply.
    pub const EXACT_CONTACT: &str = "Exact contact";
    /// A setting the strategy does not use.
    pub const IGNORED_SETTING: &str = "Ignored setting";
    /// A strategy that works on what earlier operations left, with none
    /// before it in the setup.
    pub const EARLIER_OPERATIONS: &str = "Earlier operations";
}

/// The topics whose messages name the tool too, because the tool's size,
/// flutes or number matter to them (decision record 0536 section 3).
pub const TOOL_TOPICS: &[&str] = &[openchuck_path::topic::FLUTE_LENGTH, topic::TAB_LENGTH];

/// Whether a message under `topic` names the operation's tool as well.
pub fn names_the_tool(topic: &str) -> bool {
    TOOL_TOPICS.contains(&topic)
}

/// A whole message in the one form: the lead-in, a colon, then `text`.
pub fn in_form(topic: &str, context: &Context, text: &str) -> String {
    format!("{}: {text}", lead_in(topic, context))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_12_the_form_names_the_operation_and_tool_as_decision_0522() {
        let context = Context::operation("1 Rough").and_tool("T2 Flat 6");
        assert_eq!(
            in_form(
                "Flute length",
                &context,
                "the stock stands 3.000 mm above the flutes"
            ),
            "Flute length (operation '1 Rough', tool 'T2 Flat 6'): the stock stands 3.000 mm \
             above the flutes"
        );
    }

    #[test]
    fn req_ui_12_only_a_topic_about_the_tool_names_the_tool() {
        // Decision record 0536 section 3: `tool '<name>'` when the tool's
        // size, flutes or number matter.
        assert!(names_the_tool("Flute length"));
        assert!(names_the_tool("Tab length"));
        assert!(!names_the_tool("Overcut"));
    }

    #[test]
    fn req_ui_12_every_item_kind_has_its_word_in_a_fixed_order() {
        assert_eq!(
            lead_in("Clearance", &Context::setup("Setup 1")),
            "Clearance (setup 'Setup 1')"
        );
        assert_eq!(
            lead_in("Model file changed", &Context::model("part.stl")),
            "Model file changed (model 'part.stl')"
        );
        assert_eq!(
            lead_in("Short blocks", &Context::post("Makera Z1")),
            "Short blocks (post 'Makera Z1')"
        );
        assert_eq!(
            lead_in("Travel", &Context::machine("Z1")),
            "Travel (machine 'Z1')"
        );
        // The order is fixed whatever order the items were given in.
        let given = Context::setup("S").and_tool("T").and_operation("O");
        assert_eq!(given.written(), "operation 'O', tool 'T', setup 'S'");
    }

    #[test]
    fn req_ui_12_a_message_about_no_item_is_led_by_its_topic_alone() {
        assert_eq!(
            in_form("Recovery", &Context::default(), "text"),
            "Recovery: text"
        );
    }
}

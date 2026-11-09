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

/// The topics of the warnings and information the application adds to an
/// operation's path, beside those of the pathing library
/// ([`openchuck_path::topic`]): short names of the checks, in sentence case
/// (decision record 0536 section 3).
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
    /// An operation's own value that overrides the tool's cutting data for
    /// the stock material.
    pub const CUTTING_DATA: &str = "Cutting data";
    /// A setting the strategy does not use.
    pub const IGNORED_SETTING: &str = "Ignored setting";
    /// A strategy that works on what earlier operations left, with none
    /// before it in the setup.
    pub const EARLIER_OPERATIONS: &str = "Earlier operations";
    /// A setup's workholding: its draft is not applied.
    pub const WORKHOLDING: &str = "Workholding";
    /// What the 3D view cannot draw yet.
    pub const VIEW: &str = "View";
    /// A finished or failed export, in Messages.
    pub const EXPORT: &str = "Export";
    /// A generation that was refused or failed.
    pub const GENERATION: &str = "Generation";
    /// A strategy that needs a STEP part, on a mesh part.
    pub const STEP_PART: &str = "STEP part";
    /// A ramp entry that cannot be used on this part or with this tool.
    pub const RAMP_ENTRY: &str = "Ramp entry";
    /// Material no tool on the machine can reach, enclosed by the part.
    pub const ENCLOSED_MATERIAL: &str = "Enclosed material";
    /// A setup with no machine profile, or a profile's own values.
    pub const MACHINE_PROFILE: &str = "Machine profile";
    /// A tool too long for the safe Z or the Z travel.
    pub const TOOL_LENGTH: &str = "Tool length";
    /// The Makera Z1 copy's old placeholder axis height.
    pub const AXIS_HEIGHT: &str = "Axis height";
    /// A setup's stock.
    pub const STOCK: &str = "Stock";
    /// A setup's clearance (the safe cylinder).
    pub const CLEARANCE: &str = "Clearance";
    /// A position outside the machine's travel.
    pub const TRAVEL: &str = "Travel";
    /// A tab's position, or two tabs over each other.
    pub const TAB_POSITION: &str = "Tab position";
    /// A model linked from outside the project folder.
    pub const LINKED_MODEL: &str = "Linked model";
    /// A model's file: missing, unreadable or changed.
    pub const MODEL_FILE: &str = "Model file";
    /// What the mesh check found in a model.
    pub const MESH_CHECK: &str = "Mesh check";
    /// A model whose units are in doubt.
    pub const UNITS: &str = "Units";
    /// What reading a model's file noted.
    pub const MODEL_NOTES: &str = "Model notes";
    /// A model that cannot be imported.
    pub const IMPORT: &str = "Import";
    /// The project's checks, when they fail to run.
    pub const PROJECT_CHECKS: &str = "Project checks";
    /// A rapid move the stock simulation found removing material.
    pub const RAPID_CUT: &str = "Rapid cut";
    /// Moves shorter than the controller can run at their feed.
    pub const SHORT_BLOCKS: &str = "Short blocks";
    /// An operation itself: its name, strategy or whether it is turned on.
    pub const OPERATION: &str = "Operation";
    /// An operation's setup, missing from the project.
    pub const SETUP: &str = "Setup";
    /// An operation's tool, not chosen or missing.
    pub const TOOL: &str = "Tool";
    /// A setup's part: not chosen, missing or with no extent.
    pub const PART: &str = "Part";
    /// The stock material whose cutting data an operation takes.
    pub const MATERIAL: &str = "Material";

    /// Every topic above, for a check that a message takes one.
    pub const ALL: &[&str] = &[
        TAB_LENGTH,
        TAB_CLEARANCE,
        STOCK_TO_LEAVE,
        EXACT_CONTACT,
        CUTTING_DATA,
        IGNORED_SETTING,
        EARLIER_OPERATIONS,
        WORKHOLDING,
        VIEW,
        EXPORT,
        GENERATION,
        STEP_PART,
        RAMP_ENTRY,
        ENCLOSED_MATERIAL,
        MACHINE_PROFILE,
        TOOL_LENGTH,
        AXIS_HEIGHT,
        STOCK,
        CLEARANCE,
        TRAVEL,
        TAB_POSITION,
        LINKED_MODEL,
        MODEL_FILE,
        MESH_CHECK,
        UNITS,
        MODEL_NOTES,
        IMPORT,
        PROJECT_CHECKS,
        RAPID_CUT,
        SHORT_BLOCKS,
        OPERATION,
        SETUP,
        TOOL,
        PART,
        MATERIAL,
    ];
}

/// The topics whose messages name the tool too, because the tool's size,
/// flutes or number matter to them (decision record 0536 section 3).
pub const TOOL_TOPICS: &[&str] = &[
    openchuck_path::topic::FLUTE_LENGTH,
    topic::TAB_LENGTH,
    topic::TAB_CLEARANCE,
];

/// Whether a message under `topic` names the operation's tool as well.
pub fn names_the_tool(topic: &str) -> bool {
    TOOL_TOPICS.contains(&topic)
}

/// A whole message in the one form: the lead-in, a colon, then `text` as a
/// clause ([`clause`]) ending with a full stop, so every message reads the
/// same wherever it is shown, on the command line too (OWNERCHECKS2).
pub fn in_form(topic: &str, context: &Context, text: &str) -> String {
    let mut text = clause(text);
    if !text.ends_with(['.', '!', '?']) {
        text.push('.');
    }
    format!("{}: {text}", lead_in(topic, context))
}

/// `text` as the clause after a message's lead-in (decision record 0536
/// section 3): its first letter in lower case, unless the first word is a
/// name in capitals ("STEP", "X") or the text opens with a quote or a
/// number.
pub fn clause(text: &str) -> String {
    let text = text.trim();
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(first), Some(second)) if first.is_uppercase() && second.is_lowercase() => first
            .to_lowercase()
            .chain(text[first.len_utf8()..].chars())
            .collect(),
        _ => text.to_owned(),
    }
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
             above the flutes."
        );
    }

    #[test]
    fn req_ui_12_only_a_topic_about_the_tool_names_the_tool() {
        // Decision record 0536 section 3: `tool '<name>'` when the tool's
        // size, flutes or number matter.
        assert!(names_the_tool("Flute length"));
        assert!(names_the_tool("Tab length"));
        assert!(names_the_tool("Tab clearance"));
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
    fn req_ui_12_the_clause_after_the_lead_in_is_lower_case_and_ends_with_a_full_stop() {
        // OWNERCHECKS2 on main 81bade8a: "Held region (operation 'X'):
        // Nothing ..." started with a capital, and some lines ended with no
        // full stop on the command line.
        let held = Context::operation("Finish");
        assert_eq!(
            in_form(
                "Held region",
                &held,
                "Nothing within one tool radius is cut"
            ),
            "Held region (operation 'Finish'): nothing within one tool radius is cut."
        );
        // A name in capitals, a quote or a number keeps its case; an ending
        // is never doubled.
        assert_eq!(
            in_form("T", &held, "STEP faces only."),
            "T (operation 'Finish'): STEP faces only."
        );
        assert_eq!(
            in_form("T", &held, "X 10.000 is past the end"),
            "T (operation 'Finish'): X 10.000 is past the end."
        );
        assert_eq!(
            in_form("T", &held, "'Top' limits nothing?"),
            "T (operation 'Finish'): 'Top' limits nothing?"
        );
        assert_eq!(
            in_form("T", &held, "2 chains"),
            "T (operation 'Finish'): 2 chains."
        );
    }

    #[test]
    fn req_ui_12_a_message_about_no_item_is_led_by_its_topic_alone() {
        assert_eq!(
            in_form("Recovery", &Context::default(), "text"),
            "Recovery: text."
        );
    }
}

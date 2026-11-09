// SPDX-License-Identifier: AGPL-3.0-or-later
//! What generation tells the user about a path: one type for every warning
//! and every piece of information a strategy or the shared generation gives
//! (decision record 0536 item 6.1).
//!
//! A [`Notice`] carries its own level, so a host shows it where the level
//! belongs (decision record 0536 section 5): a warning in Messages and the
//! Results tab, information in the Results tab only. A warning that must be
//! acknowledged before its program is written says why
//! ([`Acknowledgement`]), so a partial path is never confused with a
//! possible collision (decision records 0239 item 10 and 0538).
//!
//! The library does not know the names of operations, tools or setups: a
//! host adds them when it shows a notice, so a rename is followed (decision
//! record 0522 item 3). Nothing a notice says may call a path safe or
//! verified (NFR-12).

/// The topics of the warnings and the information this crate makes: short
/// names of the checks, in sentence case, the same for the same check
/// wherever it is made (decision record 0536 section 3). A host puts the
/// topic first, then the items concerned, then the text.
pub mod topic {
    /// The path stops short of the material it was asked to cut, or could
    /// not confirm it, and is kept as a partial path.
    pub const PARTIAL_PATH: &str = "Partial path";
    /// The tool reaches into an avoid set that only warns.
    pub const AVOID_SET: &str = "Avoid set";
    /// The tool touches, or comes closer than the stock to leave to, a
    /// check set that only warns.
    pub const CHECK_SET: &str = "Check set";
    /// A selection set whose options leave it limiting nothing.
    pub const SELECTION_SET: &str = "Selection set";
    /// More material is left than the operation asks for.
    pub const MATERIAL_LEFT: &str = "Material left";
    /// Areas of the part the tool cannot reach stay uncut, or are not
    /// checked.
    pub const UNREACHED_AREAS: &str = "Unreached areas";
    /// The stock stands above the tool's flutes.
    pub const FLUTE_LENGTH: &str = "Flute length";
    /// The tool holder is not checked against the part.
    pub const TOOL_HOLDER: &str = "Tool holder";
    /// The tool's shank would touch the part, so the path breaks round it.
    pub const SHANK_CLEARANCE: &str = "Shank clearance";
    /// What is left uncut beside a held region, where the workholding grips
    /// the stock.
    pub const HELD_REGION: &str = "Held region";
    /// The tool cuts into the finished surface or a face beside it by more
    /// than the tolerance.
    pub const OVERCUT: &str = "Overcut";
    /// Inside corners a tool leaves material in, or does not follow.
    pub const INSIDE_CORNERS: &str = "Inside corners";
    /// An end face with nothing beyond it that is not faced, or is faced
    /// from the setup's own stock.
    pub const FREE_ENDS: &str = "Free ends";
    /// Facing an end down to the rotary axis may cut the part or the stock
    /// free.
    pub const PART_CUT_FREE: &str = "Part cut free";
    /// Selected faces that lie across the rotary axis.
    pub const FACES_ACROSS_THE_AXIS: &str = "Faces across the axis";
    /// Selected holes the drilling leaves out.
    pub const HOLES_LEFT_OUT: &str = "Holes left out";
    /// Selected chains the edge following leaves out.
    pub const CHAINS_LEFT_OUT: &str = "Chains left out";
    /// The machine's feed limits slow a move below the feed set.
    pub const FEED_LIMIT: &str = "Feed limit";
    /// A turn of A with the contact point at the rotary axis, whose time
    /// the rotary axis's speed limit sets.
    pub const ROTARY_AXIS: &str = "Rotary axis";
    /// Links between passes asked to be low that retract as usual.
    pub const LINKS: &str = "Links";
    /// The passes adaptive spacing added between the passes, or that it
    /// added none.
    pub const ADAPTIVE_SPACING: &str = "Adaptive spacing";
    /// The extra turns round the axis at steps across it, so each shoulder
    /// keeps only the stock to leave.
    pub const SHOULDERS: &str = "Shoulders";
    /// What cutting by region did to the order of the passes, or why it
    /// did not.
    pub const PASS_ORDER: &str = "Pass order";
    /// Rings cut as whole turns at one place along the part, and how far
    /// apart.
    pub const RINGS: &str = "Rings";
    /// The finishing layers above the final pass.
    pub const FINISHING_LAYERS: &str = "Finishing layers";
    /// Passes that run on past the ends of the part.
    pub const PAST_THE_ENDS: &str = "Past the ends";
    /// How the tool axis leans from the surface normal.
    pub const TOOL_TILT: &str = "Tool tilt";
    /// A tolerance measured against the mesh, not the design surface.
    pub const TESSELLATION: &str = "Tessellation";
    /// A section that does not surround the rotary axis, followed round its
    /// own centre.
    pub const OFF_AXIS_SECTION: &str = "Off-axis section";
    /// Surfaces hidden from the drive, finished with passes of their own.
    pub const HIDDEN_SURFACES: &str = "Hidden surfaces";
    /// What rest machining found left and cut.
    pub const REST_MACHINING: &str = "Rest machining";
    /// The chains, passes and depth of edge following.
    pub const EDGE_FOLLOWING: &str = "Edge following";
    /// The faces and passes of face-flow finishing.
    pub const FACE_FLOW: &str = "Face-flow finishing";
    /// The creases and passes of pencil clean-up.
    pub const PENCIL: &str = "Pencil clean-up";
    /// How swarf cutting follows a ruled face.
    pub const SWARF: &str = "Swarf";
    /// How a ramp entry followed the pass's own toolpath (decision record
    /// 0543): passes plunged as single points, ramps that followed a steep
    /// drop of the pass, A reversed during an entry.
    pub const RAMP_ENTRY: &str = "Ramp entry";
    /// How a lead entry was made (decision record 0544): passes plunged as
    /// single points, which have no cut to lead into.
    pub const LEAD_ENTRY: &str = "Lead entry";
    /// The move to an entry's start, raised to clear the stock beside the
    /// first cut point (decision record 0544 item 6).
    pub const ENTRY: &str = "Entry";
    /// Nubs of stock round the rotary axis removed by a plunge instead of
    /// ring layers (decision record 0543 section 4, point 3).
    pub const FACE_NUBS: &str = "Face nubs";
    /// The bands End facing adds where the stock rises above the requested
    /// area (decision record 0547, question 4).
    pub const STOCK_ABOVE_THE_AREA: &str = "Stock above the area";

    // The topics of a refusal to generate (decision record 0644), beside
    // those above that a refusal shares with a warning or a note.

    /// End facing's own checks: its settings, the tool, the layers.
    pub const END_FACING: &str = "End facing";
    /// Whether Facing covers the end faces it was asked to.
    pub const FACING_COMPLETION: &str = "Facing completion";
    /// The moves Facing makes, and what keeps them off the part.
    pub const FACING_MOTION: &str = "Facing motion";
    /// Facing of a box's sides.
    pub const BOX_FACING: &str = "Box facing";
    /// Facing's check of the path it made, before it is used.
    pub const FACING_CHECK: &str = "Facing check";
    /// How far a Facing or roughing operation reaches along the part.
    pub const FACING_EXTENT: &str = "Facing extent";
    /// The surface a strategy works on: the part's mesh or its faces.
    pub const SURFACE: &str = "Surface";
    /// A region on the unrolled surface (a boundary and its offsets).
    pub const REGION: &str = "Region";
    /// A helix entry of a pass.
    pub const HELIX_ENTRY: &str = "Helix entry";
    /// A setting of the operation that is missing or out of range, when no
    /// label names it.
    pub const SETTING: &str = "Setting";
}

/// How serious a [`Notice`] is (decision record 0536 section 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {
    /// The path is made, but something about it may be a problem during
    /// machining or cause unexpected behaviour.
    Warning,
    /// The path's goal is met, with a compromise or a choice the user may
    /// want to know about.
    Information,
}

/// Why a warning must be acknowledged before its program is written
/// (GC-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Acknowledgement {
    /// The path is partial: it is kept but never called complete (adaptive
    /// spacing that met a limit or could not confirm a place; decision
    /// record 0239 item 10).
    PartialPath,
    /// The finding may be a collision of the tool, its shank or its holder
    /// with the part, the stock, a tab or a held region (decision record
    /// 0538).
    PossibleCollision,
}

/// Something the user should know about a generated path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// How serious it is.
    pub level: Level,
    /// The short name of the check, in sentence case ("Flute length"), that
    /// a host puts first; `None` when the notice has none yet.
    pub topic: Option<String>,
    /// What happened, why, and what the user can do, in plain words.
    pub text: String,
    /// The operation parameter that, changed, would remove or accept it
    /// (`tolerance`), for the host to point at; `None` when no one does.
    pub setting: Option<String>,
    /// Why it must be acknowledged before its program is written; `None`
    /// when it need not be. Only a warning carries one.
    pub acknowledgement: Option<Acknowledgement>,
    /// The segment it concerns, by index in the group; `None` when it is
    /// about the whole path.
    pub segment: Option<usize>,
}

impl Notice {
    /// Information about the whole path.
    pub fn information(text: impl Into<String>) -> Self {
        Self {
            level: Level::Information,
            topic: None,
            text: text.into(),
            setting: None,
            acknowledgement: None,
            segment: None,
        }
    }

    /// A warning about the whole path.
    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            level: Level::Warning,
            ..Self::information(text)
        }
    }

    /// This notice, naming `topic` as its check.
    #[must_use]
    pub fn about(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// This notice, pointing at the operation parameter `setting`.
    #[must_use]
    pub fn with_setting(mut self, setting: impl Into<String>) -> Self {
        self.setting = Some(setting.into());
        self
    }

    /// This notice, about segment `segment` of the group.
    #[must_use]
    pub fn at_segment(mut self, segment: usize) -> Self {
        self.segment = Some(segment);
        self
    }

    /// This notice as a warning that must be acknowledged, for `why`,
    /// before its program is written (GC-16). A notice that must be
    /// acknowledged is always a warning.
    #[must_use]
    pub fn to_acknowledge(mut self, why: Acknowledgement) -> Self {
        self.level = Level::Warning;
        self.acknowledgement = Some(why);
        self
    }

    /// Whether it is a warning.
    pub fn is_warning(&self) -> bool {
        self.level == Level::Warning
    }

    /// Whether its path is partial (decision record 0239 item 10).
    pub fn is_partial_path(&self) -> bool {
        self.acknowledgement == Some(Acknowledgement::PartialPath)
    }

    /// The notice as one sentence: the topic, when it has one, then the
    /// text ("Flute length: the stock ...").
    pub fn sentence(&self) -> String {
        match &self.topic {
            Some(topic) => format!("{topic}: {}", self.text),
            None => self.text.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_12_a_notice_carries_its_own_level() {
        assert_eq!(Notice::warning("w").level, Level::Warning);
        assert_eq!(Notice::information("i").level, Level::Information);
        assert!(Notice::warning("w").is_warning());
        assert!(!Notice::information("i").is_warning());
    }

    #[test]
    fn req_gc_16_a_notice_to_acknowledge_is_always_a_warning() {
        let n = Notice::information("i").to_acknowledge(Acknowledgement::PossibleCollision);
        assert_eq!(n.level, Level::Warning);
        assert_eq!(n.acknowledgement, Some(Acknowledgement::PossibleCollision));
        assert!(!n.is_partial_path());
    }

    #[test]
    fn req_gc_16_only_a_partial_path_is_partial() {
        let partial = Notice::warning("p").to_acknowledge(Acknowledgement::PartialPath);
        assert!(partial.is_partial_path());
        assert!(!Notice::warning("w").is_partial_path());
    }

    #[test]
    fn req_ui_12_the_sentence_puts_the_topic_first() {
        let n = Notice::warning("the stock stands 3 mm above the flutes").about("Flute length");
        assert_eq!(
            n.sentence(),
            "Flute length: the stock stands 3 mm above the flutes"
        );
        assert_eq!(Notice::information("plain").sentence(), "plain");
    }

    #[test]
    fn req_ui_12_setting_and_segment_are_kept() {
        let n = Notice::information("i")
            .with_setting("tolerance")
            .at_segment(4);
        assert_eq!(n.setting.as_deref(), Some("tolerance"));
        assert_eq!(n.segment, Some(4));
        assert_eq!(Notice::warning("w").segment, None);
    }
}

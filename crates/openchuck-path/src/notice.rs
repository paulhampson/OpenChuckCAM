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

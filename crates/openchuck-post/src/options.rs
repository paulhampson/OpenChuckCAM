// SPDX-License-Identifier: AGPL-3.0-or-later
//! Post options (GC-04; decision record 0019 item 4, OI-85).
//!
//! The options are a typed struct, not a loose map: a mistyped or unknown key
//! in a post definition is refused when it is loaded, naming the key, and the
//! template always sees every key (an unset one as `none`), so strict
//! undefined variables cannot hide a mistake.

use serde::{Deserialize, Serialize};

use crate::error::{PostError, Result};

/// How the rotary axis is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnwindMode {
    /// A rapid rotation of A by whole turns.
    Rotate,
    /// The controller drops whole turns with no motion, by `unwind_command`
    /// (the Z1's `G92.4 A0 S0`, decision 0007 item 4).
    DropTurns,
}

/// Which way the post writes positive A, relative to the machine profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ADirection {
    /// A as the profile defines it.
    Same,
    /// The sign of every A word is flipped.
    Reversed,
}

/// Units the program is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Units {
    /// Millimetres (G21).
    Mm,
}

/// Coolant in the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coolant {
    /// No coolant codes.
    Off,
    /// Flood, M8, off with M9.
    Flood,
    /// Mist, M7, off with M9.
    Mist,
}

/// Feed word mode the template writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedMode {
    /// Units per minute (G94).
    G94,
    /// Inverse time (G93): needs the `inverse_time` rule.
    G93,
}

/// Line numbering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineNumbers {
    /// Whether lines carry N words.
    pub enabled: bool,
    /// The first number.
    pub start: u32,
    /// The step between numbers (at least 1).
    pub step: u32,
}

/// Decimal places per kind of word (0 to 9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decimals {
    /// X, Y and Z.
    pub pos: u32,
    /// The rotary axis.
    pub a: u32,
    /// Feed.
    pub f: u32,
}

/// The tool change sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolChange {
    /// Write `M5` before `T<n> M6`.
    pub stop_spindle_first: bool,
    /// Write `G43 H<n>` after the change.
    pub length_offset: bool,
}

/// Every option of a post (GC-04).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostOptions {
    /// Optional minimum block duration override, milliseconds (GC-17).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_block_ms: Option<f64>,
    /// Optional controller window override (1 to 4096).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_window: Option<u32>,
    /// Optional command-rate action override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_block_mode: Option<openchuck_types::machine::ShortBlockMode>,
    /// Units of the program.
    pub units: Units,
    /// The rotary axis letter.
    pub a_letter: String,
    /// Which way positive A is written.
    pub a_direction: ADirection,
    /// How whole turns of A are removed.
    pub unwind: UnwindMode,
    /// The command for [`UnwindMode::DropTurns`]; required with it.
    #[serde(default)]
    pub unwind_command: Option<String>,
    /// Lines written first, after the title comment.
    pub header: Vec<String>,
    /// Lines written last.
    pub footer: Vec<String>,
    /// The work offset line, such as `G54`.
    #[serde(default)]
    pub work_offset: Option<String>,
    /// The tool change sequence.
    pub tool_change: ToolChange,
    /// Coolant.
    pub coolant: Coolant,
    /// Line numbering.
    pub line_numbers: LineNumbers,
    /// Decimal places.
    pub decimals: Decimals,
    /// The most F may be, in units per minute; higher feeds are lowered.
    #[serde(default)]
    pub max_feed: Option<f64>,
    /// The feed word mode; must agree with the post's feed rule.
    pub feed_mode: FeedMode,
    /// Write comments.
    pub comments: bool,
    /// The extension of program files, without the dot (`ngc`, `nc`).
    pub file_extension: String,
}

fn bad(name: &str, reason: impl Into<String>) -> PostError {
    PostError::Setting {
        name: format!("options.{name}"),
        reason: reason.into(),
    }
}

/// A line of fixed text from a definition: one line, printable ASCII.
fn check_line(name: &str, text: &str) -> Result<()> {
    if text.is_empty()
        || text.len() > 200
        || !text.chars().all(|c| c.is_ascii() && !c.is_ascii_control())
    {
        return Err(bad(
            name,
            format!("\"{text}\" must be 1 to 200 printable ASCII characters on one line"),
        ));
    }
    Ok(())
}

impl PostOptions {
    /// Resolve each optional post override against the machine profile (GC-17),
    /// retaining its source even when the action or threshold disables checking.
    pub fn effective_short_blocks(
        &self,
        profile: crate::ShortBlockSettings,
    ) -> (crate::ShortBlockSettings, crate::ShortBlockSources) {
        let mut settings = profile;
        let mut sources = crate::ShortBlockSources::default();
        if let Some(v) = self.min_block_ms {
            settings.min_block_ms = v;
            sources.min_block_ms = crate::SettingSource::Post;
        }
        if let Some(v) = self.block_window {
            settings.block_window = v as usize;
            sources.block_window = crate::SettingSource::Post;
        }
        if let Some(v) = self.short_block_mode {
            settings.mode = v;
            sources.mode = crate::SettingSource::Post;
        }
        (settings, sources)
    }

    /// Resolve each post override independently over the canonical machine settings.
    pub fn block_time(&self, profile: crate::BlockTimeSettings) -> crate::BlockTimeSettings {
        crate::BlockTimeSettings {
            min_block_ms: self.min_block_ms.unwrap_or(profile.min_block_ms),
            block_window: self.block_window.unwrap_or(profile.block_window),
            mode: self.short_block_mode.unwrap_or(profile.mode),
        }
    }

    /// Checks every value; the message names the option.
    pub fn validate(&self) -> Result<()> {
        crate::ShortBlockSettings {
            min_block_ms: self.min_block_ms.unwrap_or(0.0),
            block_window: self.block_window.unwrap_or(1) as usize,
            mode: self.short_block_mode.unwrap_or_default(),
        }
        .validate()?;
        if !matches!(self.a_letter.as_str(), "A" | "B" | "C") {
            return Err(bad("a_letter", "must be A, B or C"));
        }
        for (i, line) in self.header.iter().enumerate() {
            check_line(&format!("header[{i}]"), line)?;
        }
        for (i, line) in self.footer.iter().enumerate() {
            check_line(&format!("footer[{i}]"), line)?;
        }
        if let Some(offset) = &self.work_offset {
            let ok = matches!(
                offset.as_str(),
                "G54" | "G55" | "G56" | "G57" | "G58" | "G59"
            );
            if !ok {
                return Err(bad(
                    "work_offset",
                    format!("\"{offset}\" is not one of G54 to G59"),
                ));
            }
        }
        match (&self.unwind, &self.unwind_command) {
            (UnwindMode::DropTurns, None) => {
                return Err(bad(
                    "unwind_command",
                    "is required when unwind is drop_turns",
                ));
            }
            (_, Some(command)) => check_line("unwind_command", command)?,
            _ => {}
        }
        if self.line_numbers.step == 0 {
            return Err(bad("line_numbers.step", "must be at least 1"));
        }
        for (name, value) in [
            ("decimals.pos", self.decimals.pos),
            ("decimals.a", self.decimals.a),
            ("decimals.f", self.decimals.f),
        ] {
            if value > 9 {
                return Err(bad(name, format!("{value} is outside 0 to 9")));
            }
        }
        let ext_ok = !self.file_extension.is_empty()
            && self.file_extension.len() <= 8
            && self
                .file_extension
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        if !ext_ok {
            return Err(bad(
                "file_extension",
                "must be 1 to 8 lower-case letters or digits, without a dot",
            ));
        }
        if let Some(max) = self.max_feed
            && !(max.is_finite() && max > 0.0)
        {
            return Err(bad("max_feed", "must be a number greater than 0"));
        }
        Ok(())
    }
}

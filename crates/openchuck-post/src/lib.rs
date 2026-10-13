// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reusable post-processing library: neutral toolpath IR to G-code through user-editable post definitions.
//!
//! Architecture section 5.3; requirements GC-02 to GC-10, GC-13, INT-03.
//!
//! Slice 6 is under way. So far: the neutral machine program stage
//! ([`to_machine_program`]) and the per-controller feed rules ([`FeedRule`],
//! decision 0019 item 5). The template engine, post definitions and the
//! `PostProcessor` trait follow.

#![warn(missing_docs)]

mod builtin;
mod convert;
mod definition;
mod engine;
mod error;
mod feed;
mod options;
mod program;
mod render;

pub use builtin::{builtin, builtin_ids};
pub use convert::{ConvertOptions, ToolSetting, to_machine_program};
pub use definition::{MAX_FILE_BYTES, PostDefinition, SCHEMA};
pub use engine::fixed_str;
pub use error::{PostError, Result};
pub use feed::{FeedRule, FeedWord, MoveMetrics, NO_MOTION};
pub use options::{
    ADirection, Coolant, Decimals, FeedMode, LineNumbers, PostOptions, ToolChange, Units,
    UnwindMode,
};
pub use program::{MachineProgram, Step, Target};
pub use render::{PostJob, PostOutput, PostProcessor, TemplatePost, sanitise_comment};

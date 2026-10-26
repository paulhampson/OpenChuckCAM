// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reusable post-processing library: neutral toolpath IR to G-code through user-editable post definitions.
//!
//! Architecture section 5.3; requirements GC-02 to GC-10, GC-13, INT-03.
//!
//! The neutral machine program stage ([`to_machine_program`]), the
//! per-controller feed rules ([`FeedRule`], decision 0019 item 5), the template
//! engine, post definitions ([`PostDefinition`]) and the built-in posts
//! ([`builtin`]), which this crate carries in its `posts/` folder (decision
//! 0103). Generated G-code is the user's to verify before it runs on a machine
//! (NFR-12).

#![warn(missing_docs)]

mod block_time;
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
pub use convert::{ConvertOptions, ToolSetting, round_written, to_machine_program};
pub use definition::{MAX_FILE_BYTES, PostDefinition, SCHEMA};
pub use engine::{FEED_SIGNIFICANT_FIGURES, feed_str, fixed_str};
pub use error::{PostError, Result};
pub use feed::{FeedRule, FeedWord, MoveMetrics, NO_MOTION};
pub use options::{
    ADirection, Coolant, Decimals, FeedMode, LineNumbers, PostOptions, ToolChange, Units,
    UnwindMode,
};
pub use program::{MachineProgram, Step, Target};
pub use render::{PostJob, PostOutput, PostProcessor, TemplatePost, sanitise_comment};

pub use block_time::{BlockTimeReport, BlockTimeSettings, block_time_reports};

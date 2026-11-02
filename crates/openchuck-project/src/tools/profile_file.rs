// SPDX-License-Identifier: AGPL-3.0-or-later
//! Custom tool-profile text files (TL-03; decision records 0012 and 0014).

use std::path::Path;

use openchuck_types::tool::profile::arc_centre;
use openchuck_types::tool::{ProfilePoint, ProfileSegment, ToolProfile};
use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::file::{FileKind, parse, to_text};

/// The custom profile file schema this build writes and reads.
///
/// - 1: slice 4 (decision record 0014).
pub const PROFILE_SCHEMA: i64 = 1;

/// Which way an arc given by radius bulges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcCurve {
    /// Away from the tool, like a ball end: the centre is above and inside.
    Convex,
    /// Into the tool, like a corner-rounding cutter: the centre is below and
    /// outside.
    Concave,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum SegmentText {
    Line {
        to: ProfilePoint,
    },
    Arc {
        to: ProfilePoint,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        centre: Option<ProfilePoint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        radius: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        curve: Option<ArcCurve>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileFile {
    schema: i64,
    #[serde(default)]
    segments: Vec<SegmentText>,
}

/// Reads a custom profile file's text (TL-03) and checks the monotone rule.
/// Errors name the file and the segment.
pub fn parse_profile(path: &Path, text: &str) -> Result<ToolProfile, ProjectError> {
    let bad = |reason: String| ProjectError::BadFile {
        path: path.to_path_buf(),
        reason,
    };
    let file: ProfileFile = parse(path, text, FileKind::Tool, &[], PROFILE_SCHEMA)?;
    let mut prev = ProfilePoint::TIP;
    let mut segments = Vec::with_capacity(file.segments.len());
    for (i, segment) in file.segments.into_iter().enumerate() {
        let segment = match segment {
            SegmentText::Line { to } => ProfileSegment::Line { to },
            SegmentText::Arc {
                to,
                centre,
                radius,
                curve,
            } => {
                let centre = match (centre, radius, curve) {
                    (Some(centre), None, None) => centre,
                    (None, Some(radius), Some(curve)) => {
                        arc_centre(prev, to, radius, curve == ArcCurve::Convex)
                            .map_err(|e| bad(format!("segment {}: {e}", i + 1)))?
                    }
                    _ => {
                        return Err(bad(format!(
                            "segment {}: an arc gives either `centre`, or `radius` and \
                             `curve` (\"convex\" or \"concave\")",
                            i + 1
                        )));
                    }
                };
                ProfileSegment::Arc { to, centre }
            }
        };
        prev = segment.end();
        segments.push(segment);
    }
    let profile = ToolProfile { segments };
    profile.validate().map_err(|e| bad(e.to_string()))?;
    Ok(profile)
}

/// The text of a custom profile file for `profile`. Arcs are written with
/// their centre.
pub fn render_profile(profile: &ToolProfile) -> Result<String, ProjectError> {
    profile.validate()?;
    to_text(&ProfileFile {
        schema: PROFILE_SCHEMA,
        segments: profile
            .segments
            .iter()
            .map(|s| match *s {
                ProfileSegment::Line { to } => SegmentText::Line { to },
                ProfileSegment::Arc { to, centre } => SegmentText::Arc {
                    to,
                    centre: Some(centre),
                    radius: None,
                    curve: None,
                },
            })
            .collect(),
    })
}

/// Reads the custom profile file at `path`.
pub fn load_profile(path: &Path) -> Result<ToolProfile, ProjectError> {
    let text = std::fs::read_to_string(path).map_err(|source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_profile(path, &text)
}

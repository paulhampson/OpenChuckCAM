// SPDX-License-Identifier: AGPL-3.0-or-later
//! Post definitions: a folder with `post.toml` and a template (GC-03;
//! decision record 0019 items 6 and 9, OI-84, OI-85).
//!
//! ```toml
//! schema = 1
//! name = "LinuxCNC verification"
//! template = "main.post"
//! feed_rule = "linuxcnc"
//!
//! [options]
//! # every option of `PostOptions`
//! ```
//!
//! Loading normalises newlines to LF, refuses unknown keys, checks every
//! option, checks the feed mode against the feed rule, and compiles the
//! template so a mistake in it is reported when the post is loaded.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine;
use crate::error::{PostError, Result};
use crate::feed::FeedRule;
use crate::options::{FeedMode, PostOptions};

/// The schema version this release reads.
pub const SCHEMA: u32 = 1;
/// Largest definition file read, bytes (NFR-07).
pub const MAX_FILE_BYTES: u64 = 1 << 20;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    name: String,
    template: String,
    feed_rule: FeedRule,
    options: PostOptions,
}

/// A loaded, checked post definition.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PostDefinition {
    /// Display name.
    pub name: String,
    /// The named feed rule (decision 0019 item 5).
    pub feed_rule: FeedRule,
    /// The options.
    pub options: PostOptions,
    /// The template file name, for messages.
    pub template_name: String,
    /// The template source, with LF newlines.
    pub template: String,
}

fn normalise(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn definition_error(source: &str, reason: impl Into<String>) -> PostError {
    PostError::Definition {
        source_name: source.to_string(),
        reason: reason.into(),
    }
}

impl PostDefinition {
    /// Builds a definition from the text of `post.toml` and its template.
    /// `source` names where they came from, for messages.
    pub fn from_parts(source: &str, manifest: &str, template: &str) -> Result<Self> {
        let manifest: Manifest = toml::from_str(&normalise(manifest))
            .map_err(|e| definition_error(source, e.message().to_string()))?;
        if manifest.schema != SCHEMA {
            return Err(definition_error(
                source,
                format!(
                    "schema {} is not supported; this release reads schema {SCHEMA}",
                    manifest.schema
                ),
            ));
        }
        if manifest.name.trim().is_empty() {
            return Err(definition_error(source, "name must not be empty"));
        }
        manifest.options.validate()?;
        let mode_ok = matches!(
            (manifest.feed_rule, manifest.options.feed_mode),
            (FeedRule::InverseTime, FeedMode::G93)
                | (
                    FeedRule::Linuxcnc | FeedRule::Makera | FeedRule::Contact,
                    FeedMode::G94
                )
        );
        if !mode_ok {
            return Err(definition_error(
                source,
                format!(
                    "feed_rule \"{}\" cannot be written in feed_mode {:?}: inverse_time goes with g93, the others with g94",
                    manifest.feed_rule.name(),
                    manifest.options.feed_mode
                ),
            ));
        }
        let template = normalise(template);
        // Compile now so a syntax error names its line at load time.
        let mut env = engine::environment(0)?;
        env.add_template_owned(manifest.template.clone(), template.clone())
            .map_err(|e| PostError::Template(engine::describe(&e)))?;
        Ok(Self {
            name: manifest.name,
            feed_rule: manifest.feed_rule,
            options: manifest.options,
            template_name: manifest.template,
            template,
        })
    }

    /// Loads a post folder (`post.toml` and the template it names).
    pub fn load_dir(dir: &Path) -> Result<Self> {
        let source = dir.display().to_string();
        let manifest = read_text(&dir.join("post.toml"), &source)?;
        // Read the template name first so the second file can be found.
        let name = toml::from_str::<toml::Table>(&manifest).ok().and_then(|t| {
            t.get("template")
                .and_then(|v| v.as_str().map(str::to_string))
        });
        let Some(name) = name else {
            // Let the full parse produce the message.
            return Self::from_parts(&source, &manifest, "");
        };
        let relative = Path::new(&name);
        let plain = relative.components().count() == 1
            && relative
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)));
        if !plain {
            return Err(definition_error(
                &source,
                format!("template \"{name}\" must be a file name in the post folder"),
            ));
        }
        let template = read_text(&dir.join(relative), &source)?;
        Self::from_parts(&source, &manifest, &template)
    }
}

fn read_text(path: &Path, source: &str) -> Result<String> {
    let meta = std::fs::metadata(path)
        .map_err(|e| definition_error(source, format!("{}: {e}", path.display())))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(definition_error(
            source,
            format!("{} is larger than {MAX_FILE_BYTES} bytes", path.display()),
        ));
    }
    std::fs::read_to_string(path)
        .map_err(|e| definition_error(source, format!("{}: {e}", path.display())))
}

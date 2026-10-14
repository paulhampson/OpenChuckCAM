// SPDX-License-Identifier: AGPL-3.0-or-later
//! The posts that ship with the application (GC-09, GC-10), embedded from the
//! `posts/` folder at the repository root. They are ordinary post folders: a
//! user can copy one, edit it and load it from disk without recompiling.

use crate::definition::PostDefinition;
use crate::error::Result;

/// The built-in posts as `(id, manifest, template)`.
const BUILTIN: [(&str, &str, &str); 2] = [
    (
        "makera-z1",
        include_str!("../../../posts/makera-z1/post.toml"),
        include_str!("../../../posts/makera-z1/main.post"),
    ),
    (
        "linuxcnc",
        include_str!("../../../posts/linuxcnc/post.toml"),
        include_str!("../../../posts/linuxcnc/main.post"),
    ),
];

/// The ids of the built-in posts, in a fixed order.
pub fn builtin_ids() -> impl Iterator<Item = &'static str> {
    BUILTIN.iter().map(|(id, _, _)| *id)
}

/// Loads a built-in post by id, or `None` for an unknown id.
pub fn builtin(id: &str) -> Option<Result<PostDefinition>> {
    BUILTIN
        .iter()
        .find(|(name, _, _)| *name == id)
        .map(|(name, manifest, template)| {
            PostDefinition::from_parts(&format!("built-in {name}"), manifest, template)
        })
}

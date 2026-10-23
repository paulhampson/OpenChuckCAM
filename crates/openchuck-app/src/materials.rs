// SPDX-License-Identifier: AGPL-3.0-or-later
//! The stock material names the project and the tool library know, so a
//! setup's material and a tool's cutting data are chosen from one list and a
//! typing difference cannot leave a tool without data (decision 0196; TL-05,
//! TL-09; issue 249).
//!
//! The list is not stored. It is every material name in the cutting data of
//! the library's tools and the project's tools, and the materials the project's
//! setups already use, so adding a material to one tool's cutting data adds it
//! to the list for every tool and setup, and no file format changes. Names
//! that differ only by case or spacing are one material (see
//! [`openchuck_types::tool::material_key`]); the list holds one spelling of
//! each, the first in name order.

use std::collections::BTreeMap;

use openchuck_project::Project;
use openchuck_project::tools::ToolLibrary;
use openchuck_types::tool::material_key;

/// The known material names, one spelling per material, in name order.
pub fn known_materials(project: Option<&Project>, library: Option<&ToolLibrary>) -> Vec<String> {
    let mut names: Vec<&str> = Vec::new();
    if let Some(library) = library {
        for tool in library.tools() {
            names.extend(tool.cutting_data.keys().map(String::as_str));
        }
    }
    if let Some(project) = project {
        for tool in project.tools() {
            names.extend(tool.cutting_data.keys().map(String::as_str));
        }
        names.extend(
            project
                .setups()
                .iter()
                .filter_map(|s| s.material.as_deref()),
        );
    }
    let mut by_key: BTreeMap<String, &str> = BTreeMap::new();
    for name in names {
        by_key
            .entry(material_key(name))
            .and_modify(|first| {
                if name < *first {
                    *first = name;
                }
            })
            .or_insert(name);
    }
    let mut out: Vec<String> = by_key.into_values().map(str::to_owned).collect();
    out.sort();
    out
}

/// `typed`, trimmed, in the spelling of the known material it matches, or as
/// typed (trimmed, with repeated spaces made one) when it is new.
pub fn canonical_material(known: &[String], typed: &str) -> String {
    let key = material_key(typed);
    known
        .iter()
        .find(|name| material_key(name) == key)
        .cloned()
        .unwrap_or_else(|| typed.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// The name an edited cutting data entry is saved under: the entry's own
/// spelling when `typed` is that name (a speed edit never renames it), else
/// as [`canonical_material`].
pub fn edited_material(known: &[String], typed: &str, editing: Option<&str>) -> String {
    match editing {
        Some(old) if material_key(old) == material_key(typed) => old.to_owned(),
        _ => canonical_material(known, typed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_tl_05_an_edit_keeps_the_entrys_own_spelling() {
        let known = vec!["Oak".to_owned()];
        assert_eq!(edited_material(&known, "oak", Some("oak")), "oak");
        assert_eq!(edited_material(&known, " OAK ", Some("oak")), "oak");
        assert_eq!(edited_material(&known, "oak", Some("pine")), "Oak");
        assert_eq!(edited_material(&known, "oak", None), "Oak");
    }

    #[test]
    fn req_tl_05_a_typed_name_takes_the_known_spelling() {
        let known = vec!["Oak".to_owned(), "Softwood".to_owned()];
        assert_eq!(canonical_material(&known, " softwood "), "Softwood");
        assert_eq!(canonical_material(&known, "OAK"), "Oak");
        assert_eq!(canonical_material(&known, "  Soft   wood "), "Soft wood");
        assert_eq!(canonical_material(&[], ""), "");
    }

    #[test]
    fn req_tl_05_known_materials_of_nothing_is_empty() {
        assert!(known_materials(None, None).is_empty());
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Saving and loading the project folder in either candidate format, under
//! either order scheme.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::model::{Operation, Project, ProjectFile, SCHEMA, SelectionSet, Setup};
use crate::order;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Toml,
}

impl Format {
    pub fn ext(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Toml => "toml",
        }
    }

    pub fn to_text<T: Serialize>(self, v: &T) -> Result<String> {
        let mut s = match self {
            Format::Json => serde_json::to_string_pretty(v)?,
            Format::Toml => toml::to_string_pretty(v)?,
        };
        if !s.ends_with('\n') {
            s.push('\n');
        }
        Ok(s)
    }

    pub fn from_text<T: DeserializeOwned>(self, s: &str) -> Result<T> {
        Ok(match self {
            Format::Json => serde_json::from_str(s)?,
            Format::Toml => toml::from_str(s)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// An order key stored in each item's own file.
    Keys,
    /// One ordered list of operation IDs in the project file.
    List,
}

/// Writes only files whose bytes change, and removes files of deleted items.
fn write_dir<T: Serialize>(
    dir: &Path,
    fmt: Format,
    items: &[(String, &T)],
) -> Result<()> {
    fs::create_dir_all(dir)?;
    let mut keep = Vec::new();
    for (id, item) in items {
        let name = format!("{id}.{}", fmt.ext());
        write_if_changed(&dir.join(&name), &fmt.to_text(item)?)?;
        keep.push(name);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !keep.contains(&name) {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn write_if_changed(path: &Path, text: &str) -> Result<()> {
    if fs::read_to_string(path).ok().as_deref() != Some(text) {
        fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}

pub fn save(p: &Project, root: &Path, fmt: Format, scheme: Scheme) -> Result<()> {
    let mut file = p.file.clone();
    let mut ops = p.operations.clone();
    match scheme {
        Scheme::Keys => file.operation_order = None,
        Scheme::List => {
            file.operation_order = Some(p.op_order());
            for o in &mut ops {
                o.order.clear();
            }
        }
    }
    fs::create_dir_all(root)?;
    write_if_changed(&root.join(format!("project.{}", fmt.ext())), &fmt.to_text(&file)?)?;
    let setups: Vec<_> = p.setups.iter().map(|s| (s.id.clone(), s)).collect();
    write_dir(&root.join("setups"), fmt, &setups)?;
    let ops: Vec<_> = ops.iter().map(|o| (o.id.clone(), o)).collect();
    write_dir(&root.join("operations"), fmt, &ops)?;
    let sels: Vec<_> = p.selections.iter().map(|s| (s.id.clone(), s)).collect();
    write_dir(&root.join("selections"), fmt, &sels)?;
    Ok(())
}

fn read_dir<T: DeserializeOwned>(dir: &Path, fmt: Format) -> Result<Vec<T>> {
    let mut names: Vec<_> = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<_>>()?;
    names.sort();
    let mut out = Vec::new();
    for path in names {
        if path.extension().and_then(|e| e.to_str()) != Some(fmt.ext()) {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        out.push(fmt.from_text(&text).with_context(|| format!("reading {}", path.display()))?);
    }
    Ok(out)
}

pub fn load(root: &Path, fmt: Format, scheme: Scheme) -> Result<Project> {
    let file: ProjectFile =
        fmt.from_text(&fs::read_to_string(root.join(format!("project.{}", fmt.ext())))?)?;
    if file.schema != SCHEMA {
        bail!("project schema {} is not {SCHEMA}", file.schema);
    }
    let mut setups: Vec<Setup> = read_dir(&root.join("setups"), fmt)?;
    setups.sort_by(|a, b| (&a.order, &a.id).cmp(&(&b.order, &b.id)));
    let mut selections: Vec<SelectionSet> = read_dir(&root.join("selections"), fmt)?;
    selections.sort_by(|a, b| (&a.order, &a.id).cmp(&(&b.order, &b.id)));
    let mut operations: Vec<Operation> = read_dir(&root.join("operations"), fmt)?;
    match scheme {
        Scheme::Keys => {
            for o in &operations {
                order::validate(&o.order).with_context(|| format!("operation {}", o.id))?;
            }
            operations.sort_by(|a, b| (&a.order, &a.id).cmp(&(&b.order, &b.id)));
        }
        Scheme::List => {
            let list = file.operation_order.clone().unwrap_or_default();
            // Listed operations first, in list order; unlisted ones after, by ID.
            operations.sort_by_key(|o| {
                (list.iter().position(|id| *id == o.id).unwrap_or(usize::MAX), o.id.clone())
            });
            for (i, id) in list.iter().enumerate() {
                if list[..i].contains(id) {
                    bail!("operation_order lists {id} twice");
                }
                if !operations.iter().any(|o| &o.id == id) {
                    bail!("operation_order names missing operation {id}");
                }
            }
        }
    }
    let mut file = file;
    file.operation_order = None;
    Ok(Project { file, setups, operations, selections })
}

/// Moves operation `id` to `to` (index in the list without it), choosing a new
/// key from its neighbours. Returns how many files get a new key.
pub fn move_op(p: &mut Project, id: &str, to: usize, scheme: Scheme) -> Result<usize> {
    let Some(from) = p.operations.iter().position(|o| o.id == id) else {
        bail!("no operation {id}");
    };
    let item = p.operations.remove(from);
    insert_op(p, item, to, scheme)
}

/// Inserts an operation at index `to`. Returns how many files get a new key.
pub fn insert_op(p: &mut Project, mut item: Operation, to: usize, scheme: Scheme) -> Result<usize> {
    let to = to.min(p.operations.len());
    let touched = if scheme == Scheme::Keys { place(&mut p.operations, to, &mut item)? } else { 1 };
    p.operations.insert(to, item);
    Ok(touched)
}

fn key_at(ops: &[Operation], i: Option<usize>) -> Option<String> {
    i.and_then(|i| ops.get(i)).map(|o| o.order.clone())
}

fn place(ops: &mut [Operation], to: usize, item: &mut Operation) -> Result<usize> {
    let lo = key_at(ops, to.checked_sub(1));
    let hi = key_at(ops, Some(to));
    let mut touched = 1;
    let hi = if lo.is_some() && lo == hi {
        // Neighbours share a key (two branches inserted at the same spot and
        // the merge kept both): move the upper neighbour up first.
        let next = key_at(ops, Some(to + 1));
        let k = order::key_between(hi.as_deref(), next.as_deref())?;
        if let Some(o) = ops.get_mut(to) {
            o.order = k.clone();
        }
        touched += 1;
        Some(k)
    } else {
        hi
    };
    item.order = order::key_between(lo.as_deref(), hi.as_deref())?;
    Ok(touched)
}

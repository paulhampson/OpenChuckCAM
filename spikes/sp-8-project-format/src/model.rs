// SPDX-License-Identifier: AGPL-3.0-or-later
//! A sample project shaped like architecture 5.6: project, setups, one file per
//! operation, selection sets and a machine reference. Typed structs give a
//! fixed field order; maps are `BTreeMap` so output order is sorted.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub units: String,
    pub machine: String,
    pub models: Vec<ModelRef>,
    /// Only used by the "ordered list" scheme; absent with per-item keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_order: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRef {
    pub id: String,
    pub path: String,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Setup {
    pub schema: u32,
    pub id: String,
    pub order: String,
    pub name: String,
    pub model: String,
    pub stock: Stock,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stock {
    pub kind: String,
    pub diameter: f64,
    pub length: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub schema: u32,
    pub id: String,
    /// Per-item order key. Empty under the "ordered list" scheme.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub order: String,
    pub name: String,
    pub setup: String,
    pub strategy: String,
    pub enabled: bool,
    pub selection: String,
    pub tool: ToolSnapshot,
    pub params: Params,
    /// Strategy-specific values with no typed field yet.
    pub extra: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSnapshot {
    pub library_id: String,
    pub kind: String,
    pub diameter: f64,
    pub flutes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Params {
    pub tolerance: f64,
    pub stepover: f64,
    pub stepdown: f64,
    pub feed: f64,
    pub plunge_feed: f64,
    pub spindle: f64,
    pub safe_radius: f64,
    pub a_start: f64,
    pub a_end: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectionSet {
    pub schema: u32,
    pub id: String,
    pub order: String,
    pub name: String,
    pub model: String,
    pub faces: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub file: ProjectFile,
    pub setups: Vec<Setup>,
    pub operations: Vec<Operation>,
    pub selections: Vec<SelectionSet>,
}

impl Project {
    /// Operation IDs in display order.
    pub fn op_order(&self) -> Vec<String> {
        self.operations.iter().map(|o| o.id.clone()).collect()
    }
}

fn op(id: &str, order: &str, name: &str, strategy: &str, d: f64, stepover: f64) -> Operation {
    let mut extra = BTreeMap::new();
    extra.insert("wrap_overlap".to_string(), 0.5);
    Operation {
        schema: SCHEMA,
        id: id.to_string(),
        order: order.to_string(),
        name: name.to_string(),
        setup: "s-7d2a".to_string(),
        strategy: strategy.to_string(),
        enabled: true,
        selection: "sel-41c0".to_string(),
        tool: ToolSnapshot {
            library_id: format!("t-{d}"),
            kind: "flat".to_string(),
            diameter: d,
            flutes: 2,
        },
        params: Params {
            tolerance: 0.01,
            stepover,
            stepdown: 0.5,
            feed: 800.0,
            plunge_feed: 200.0,
            spindle: 18000.0,
            safe_radius: 30.0,
            a_start: 0.0,
            a_end: 360.0,
        },
        extra,
    }
}

/// The sample project. IDs are fixed so runs are repeatable (in the product
/// they are UUIDs made when the user adds an item).
pub fn sample() -> Project {
    Project {
        file: ProjectFile {
            schema: SCHEMA,
            id: "p-0001".to_string(),
            name: "Chess pawn".to_string(),
            units: "mm".to_string(),
            machine: "makera-z1".to_string(),
            models: vec![ModelRef {
                id: "m-9f31".to_string(),
                path: "models/pawn.stl".to_string(),
                blake3: "4f1c0e3b9a".to_string(),
            }],
            operation_order: None,
        },
        setups: vec![Setup {
            schema: SCHEMA,
            id: "s-7d2a".to_string(),
            order: "V".to_string(),
            name: "Setup 1".to_string(),
            model: "m-9f31".to_string(),
            stock: Stock { kind: "cylinder".to_string(), diameter: 40.0, length: 80.0 },
        }],
        operations: vec![
            op("op-a1", "F", "Rough", "wrap-rough", 6.0, 2.4),
            op("op-b2", "V", "Finish", "wrap-finish", 3.0, 0.3),
            op("op-c3", "k", "Engrave", "wrap-engrave", 0.5, 0.1),
            op("op-d4", "s", "Chamfer", "wrap-chamfer", 1.0, 0.2),
        ],
        selections: vec![SelectionSet {
            schema: SCHEMA,
            id: "sel-41c0".to_string(),
            order: "V".to_string(),
            name: "Body".to_string(),
            model: "m-9f31".to_string(),
            faces: vec![1, 2, 3, 5, 8, 13, 21],
        }],
    }
}

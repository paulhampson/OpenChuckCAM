// SPDX-License-Identifier: AGPL-3.0-or-later
//! A small host program that uses only the public API of `openchuck-path`
//! (INT-01, INT-08; work breakdown slice 5 review checklist).
//!
//! It builds a cylinder mesh of its own, adds its own strategy (one ring of
//! cuts around the part, OP-16), and asks the library for a toolpath in the
//! neutral IR: A-axis management, links and feed come from the library.
//!
//! Run with `cargo run -p openchuck-path --example host`.
#![allow(clippy::print_stdout)]

use std::collections::BTreeMap;
use std::sync::Arc;

use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, CutPoint, MeshSurface, ParameterSchema, ParameterValue, Parameters,
    Pass, Registry, Result, RotaryMapping, Strategy, StrategyInput, generate,
};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};

/// One ring of cuts at X = 10 mm, every 5 degrees.
#[derive(Debug)]
struct OneRing;

impl Strategy for OneRing {
    fn name(&self) -> &str {
        "one_ring"
    }

    fn label(&self) -> &str {
        "One ring"
    }

    fn description(&self) -> &str {
        "A strategy for tests."
    }

    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(Vec::new())
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        _parameters: &Parameters,
        _context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let points = (0..=72)
            .filter_map(|i| {
                let a = Degrees(5.0 * f64::from(i));
                let line = input
                    .rotary
                    .tool_line(Millimetres(10.0), a, Millimetres(0.0));
                let c = input.surface.drop_tool(input.tool, &line)?;
                Some(CutPoint {
                    tip: c.tip,
                    a,
                    contact: c.contact,
                })
            })
            .collect();
        Ok(vec![Pass {
            helical_pass_start: false,
            points,
            stock: None,
            rapids: Vec::new(),
        }])
    }
}

/// A closed cylinder of `radius` about X from 0 to `length`.
fn cylinder(radius: f64, length: f64, around: u32) -> TriangleMesh {
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for x in [0.0, length] {
        for j in 0..around {
            let t = std::f64::consts::TAU * f64::from(j) / f64::from(around);
            vertices.push(Point3::new(x, radius * t.cos(), radius * t.sin()));
        }
    }
    vertices.push(Point3::new(0.0, 0.0, 0.0));
    vertices.push(Point3::new(length, 0.0, 0.0));
    let (c0, c1) = (2 * around, 2 * around + 1);
    for j in 0..around {
        let k = (j + 1) % around;
        triangles.push([j, around + k, around + j]);
        triangles.push([j, k, around + k]);
        triangles.push([c0, k, j]);
        triangles.push([c1, around + j, around + k]);
    }
    TriangleMesh {
        vertices,
        triangles,
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let surface = MeshSurface::new(&cylinder(15.0, 20.0, 360))?;
    let stock = AnalyticStock::Cylinder {
        radius: 18.0,
        x_start: 0.0,
        x_end: 20.0,
    };
    let tool = Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(6.0),
        flute_length: Millimetres(12.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(6.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
    .profile()?;
    let machine = MachineProfile::example_x_rotary();
    let tags = PathTags {
        operation: "ring".to_owned(),
        setup: "setup".to_owned(),
        tool: "ball 6".to_owned(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &tool,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: Clearance {
            radius: Millimetres(25.0),
            safe_z: Millimetres(30.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        keep_out: &[],
    };

    let mut registry = Registry::builtin();
    registry.register(Arc::new(OneRing))?;
    let strategy = registry.get("one_ring")?;
    let values: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 1.0),
        (names::FEED, 400.0),
        (names::PLUNGE_FEED, 150.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), ParameterValue::Number(v)))
    .collect();

    let cancel = CancelToken::new();
    let context = JobContext::new(&cancel, &NoProgress);
    let path = generate(strategy, &input, &values, &context)?;
    let seconds: f64 = path.group.segments.iter().map(|s| s.duration.0).sum();
    println!(
        "{}: {} moves, {:.1} s, A from {} to {}",
        strategy.label(),
        path.group.segments.len(),
        seconds,
        path.group.start.a,
        path.group
            .segments
            .last()
            .map_or(path.group.start.a, |s| s.end.a)
    );
    Ok(())
}

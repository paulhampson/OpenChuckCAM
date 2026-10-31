// SPDX-License-Identifier: AGPL-3.0-or-later
//! Test support: a stub strategy that uses only the public API (OP-16), and
//! the standard inputs.
#![allow(dead_code, clippy::unwrap_used)]

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, CutPoint, MeshSurface, ParameterKind, ParameterSchema, ParameterSpec,
    ParameterValue, Parameters, Pass, Result, RotaryMapping, Strategy, StrategyInput,
    SurfaceSource, Unit, parallel,
};
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::tool::{RevolvedProfile, Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};

/// Rings: at X stations `stepover` apart from `x_start` to `x_end`, a full
/// turn of A with the tool `across` from the axis, dropped onto the surface
/// at each angle step. Not a product strategy: it exists to prove that a
/// strategy plugs in through the public API alone.
#[derive(Debug)]
pub struct Rings;

impl Strategy for Rings {
    fn name(&self) -> &str {
        "test_rings"
    }

    fn label(&self) -> &str {
        "Rings (test)"
    }

    fn description(&self) -> &str {
        "A strategy for tests."
    }

    fn schema(&self) -> ParameterSchema {
        let num = |name: &str, default: Option<f64>| ParameterSpec {
            name: name.to_owned(),
            label: name.to_owned(),
            description: String::new(),
            kind: ParameterKind::Number {
                unit: Unit::Millimetres,
                min: None,
                min_exclusive: false,
                max: None,
                default,
            },
        };
        ParameterSchema::with_common(vec![
            num("x_start", None),
            num("x_end", None),
            num("across", Some(0.0)),
            ParameterSpec {
                name: "angle_step".to_owned(),
                label: "Angle step".to_owned(),
                description: String::new(),
                kind: ParameterKind::Number {
                    unit: Unit::Degrees,
                    min: Some(0.0),
                    min_exclusive: true,
                    max: Some(90.0),
                    default: Some(2.0),
                },
            },
        ])
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        p: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let x0 = p.number("x_start")?;
        let x1 = p.number("x_end")?;
        let across = p.number("across")?;
        let step = p.number("angle_step")?;
        let stepover = p.number(names::STEPOVER)?;
        let rings = ((x1 - x0) / stepover).round().max(0.0) as u32;
        let around = (360.0 / step).round().max(1.0) as u32;
        let mut stations = Vec::new();
        for i in 0..=rings {
            for j in 0..=around {
                stations.push((
                    x0 + f64::from(i) * stepover,
                    f64::from(j) * 360.0 / f64::from(around),
                    i,
                ));
            }
        }
        let points = parallel::map_in_order(&stations, |(x, a, ring)| {
            let line = input
                .rotary
                .tool_line(Millimetres(*x), Degrees(*a), Millimetres(across));
            input.surface.drop_tool(input.tool, &line).map(|c| {
                (
                    *ring,
                    CutPoint {
                        tip: c.tip,
                        a: Degrees(*a),
                        contact: c.contact,
                    },
                )
            })
        });
        context.check_cancelled()?;
        let mut passes: Vec<Pass> = (0..=rings).map(|_| Pass::default()).collect();
        for (ring, point) in points.into_iter().flatten() {
            if let Some(pass) = passes.get_mut(ring as usize) {
                pass.points.push(point);
            }
        }
        Ok(passes)
    }
}

pub fn ball(diameter: f64) -> Tool {
    Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(diameter),
        flute_length: Millimetres(12.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(diameter),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}

pub fn tool(shape: ToolShape, diameter: f64) -> Tool {
    Tool {
        shape,
        ..ball(diameter)
    }
}

pub fn machine() -> MachineProfile {
    MachineProfile::example_x_rotary()
}

pub fn tags() -> PathTags {
    PathTags {
        operation: "op".to_owned(),
        setup: "setup".to_owned(),
        tool: "tool".to_owned(),
    }
}

/// Values for the rings stub.
pub fn ring_values(
    x_start: f64,
    x_end: f64,
    stepover: f64,
    across: f64,
) -> BTreeMap<String, ParameterValue> {
    let mut v = BTreeMap::new();
    for (k, x) in [
        ("x_start", x_start),
        ("x_end", x_end),
        ("across", across),
        (names::STEPOVER, stepover),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ] {
        v.insert(k.to_owned(), ParameterValue::Number(x));
    }
    v
}

pub struct Fixture {
    pub surface: MeshSurface,
    pub stock: AnalyticStock,
    pub tool: RevolvedProfile,
    pub rotary: RotaryMapping,
    pub clearance: Clearance,
    pub tags: PathTags,
    pub machine: MachineProfile,
}

impl Fixture {
    pub fn new(surface: MeshSurface, stock_radius: f64, tool: &Tool) -> Self {
        let machine = machine();
        let b = surface.bounds();
        Self {
            surface,
            stock: AnalyticStock::Cylinder {
                radius: stock_radius,
                x_start: b.min.x - 1.0,
                x_end: b.max.x + 1.0,
            },
            tool: tool.profile().unwrap(),
            rotary: RotaryMapping::of_profile(&machine),
            clearance: Clearance {
                radius: Millimetres(stock_radius + 5.0),
                safe_z: Millimetres(stock_radius + 15.0),
            },
            tags: tags(),
            machine,
        }
    }

    pub fn input(&self) -> StrategyInput<'_> {
        StrategyInput {
            surface: &self.surface,
            stock: &self.stock,
            collision: &AcceptAll,
            tool: &self.tool,
            rotary: self.rotary,
            clearance: self.clearance,
            max_feeds: self.machine.max_feeds,
            tags: &self.tags,
            entry_safety: None,
            facing: None,
            keep_out: &[],
            shank: None,
        }
    }
}

pub fn run<T>(f: impl FnOnce(&JobContext<'_>) -> T) -> T {
    let cancel = CancelToken::new();
    let context = JobContext::new(&cancel, &NoProgress);
    f(&context)
}

/// Reads a binary STL as a mesh, one vertex per corner.
pub fn read_binary_stl(path: &std::path::Path) -> openchuck_types::mesh::TriangleMesh {
    use openchuck_types::geometry::Point3;
    let bytes = std::fs::read(path).unwrap();
    let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    let mut vertices = Vec::with_capacity(3 * count);
    let mut triangles = Vec::with_capacity(count);
    for t in 0..count {
        let base = 84 + 50 * t + 12;
        for v in 0..3 {
            let at = base + 12 * v;
            let f = |k: usize| {
                f64::from(f32::from_le_bytes(
                    bytes[at + 4 * k..at + 4 * k + 4].try_into().unwrap(),
                ))
            };
            vertices.push(Point3::new(f(0), f(1), f(2)));
        }
        let i = (3 * t) as u32;
        triangles.push([i, i + 1, i + 2]);
    }
    openchuck_types::mesh::TriangleMesh {
        vertices,
        triangles,
    }
}

/// The owner's blisk (`testdata/stl/blisk.stl`; OI-311): a disc with a
/// shoulder at X 10 and ten thin, leaning blades on a conical hub, 40 mm
/// across and 50 mm long.
pub fn blisk() -> openchuck_types::mesh::TriangleMesh {
    read_binary_stl(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/blisk.stl"),
    )
}

/// `values` with adaptive spacing off where `strategy` has it and the test
/// sets none, for tests that are not about it: a new operation has it on
/// (decision record 0239), which changes the passes these tests pin.
fn spacing_off(
    strategy: &dyn Strategy,
    values: &BTreeMap<String, ParameterValue>,
) -> BTreeMap<String, ParameterValue> {
    let mut values = values.clone();
    if strategy.schema().get("adaptive_spacing").is_some() {
        values
            .entry("adaptive_spacing".to_owned())
            .or_insert(ParameterValue::Flag(false));
    }
    values
}

/// [`openchuck_path::generate`] with adaptive spacing off unless asked for.
pub fn generate(
    strategy: &dyn Strategy,
    input: &StrategyInput<'_>,
    values: &BTreeMap<String, ParameterValue>,
    context: &JobContext<'_>,
) -> Result<openchuck_path::GeneratedPath> {
    openchuck_path::generate(strategy, input, &spacing_off(strategy, values), context)
}

/// [`openchuck_path::generate_in_region`] with adaptive spacing off unless
/// asked for.
pub fn generate_in_region(
    strategy: &dyn Strategy,
    input: &StrategyInput<'_>,
    region: &openchuck_path::RegionConstraint<'_>,
    values: &BTreeMap<String, ParameterValue>,
    context: &JobContext<'_>,
) -> Result<openchuck_path::GeneratedPath> {
    openchuck_path::generate_in_region(
        strategy,
        input,
        region,
        &spacing_off(strategy, values),
        context,
    )
}

/// [`openchuck_path::generate_in_zones`] with adaptive spacing off unless
/// asked for.
pub fn generate_in_zones(
    strategy: &dyn Strategy,
    input: &StrategyInput<'_>,
    region: Option<&openchuck_path::RegionConstraint<'_>>,
    zones: &[openchuck_path::LeaveZone<'_>],
    values: &BTreeMap<String, ParameterValue>,
    context: &JobContext<'_>,
) -> Result<openchuck_path::GeneratedPath> {
    openchuck_path::generate_in_zones(
        strategy,
        input,
        region,
        zones,
        &spacing_off(strategy, values),
        context,
    )
}

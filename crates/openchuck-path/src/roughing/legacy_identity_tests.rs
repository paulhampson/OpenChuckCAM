// SPDX-License-Identifier: AGPL-3.0-or-later
//! OP-24/GC-07: exact legacy default IR/diagnostics on each supported platform.
//! The pre-scope entry is computed in-process: native sin/cos and hypot may
//! differ in the last bit across platforms (0113), while raw JSON includes all
//! those digits. No rounding or tolerance is allowed in the paired comparison.
#![allow(clippy::unwrap_used, clippy::float_cmp, clippy::indexing_slicing)]
use super::*;
use crate::{AcceptAll, Facing, GeneratedPath, MeshSurface, ParameterValue, generate};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;
use std::collections::BTreeMap;

fn input_test(test: impl FnOnce(&StrategyInput<'_>)) {
    let mesh = TriangleMesh {
        vertices: vec![
            Point3::new(10., -2., 0.),
            Point3::new(20., -2., 0.),
            Point3::new(20., 2., 0.),
        ],
        triangles: vec![[0, 1, 2]],
    };
    let surface = MeshSurface::new(&mesh).unwrap();
    let stock = crate::AnalyticStock::Cylinder {
        radius: 10.,
        x_start: 0.,
        x_end: 30.,
    };
    let machine = MachineProfile::example_x_rotary();
    let tool = Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(2.),
        flute_length: Millimetres(12.),
        overall_length: Millimetres(50.),
        shank_diameter: Millimetres(2.),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
    .profile()
    .unwrap();
    let tags = PathTags {
        operation: "op".into(),
        setup: "setup".into(),
        tool: "tool".into(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &tool,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: Clearance {
            radius: Millimetres(15.),
            safe_z: Millimetres(25.),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        facing: None,
        keep_out: &[],
        shank: None,
    };
    test(&input);
}

fn bytes(path: &GeneratedPath) -> Vec<u8> {
    let notes = path
        .notes
        .iter()
        .map(|n| (n.segment, n.message.as_str()))
        .collect::<Vec<_>>();
    let warnings = path
        .warnings
        .iter()
        .map(|w| (&w.message, &w.parameter))
        .collect::<Vec<_>>();
    serde_json::to_vec(&(&path.group, notes, warnings)).unwrap()
}

#[test]
fn req_op_24_req_gc_07_legacy_defaults_match_pre_scope_ir_and_diagnostics() {
    input_test(|input| {
        for (shape, size) in [("flat", 6.), ("round", 12.)] {
            let values: BTreeMap<String, ParameterValue> = [
                ("shape", ParameterValue::Text(shape.into())),
                ("size", ParameterValue::Number(size)),
                ("angles", ParameterValue::Text("0,90".into())),
                ("feed", ParameterValue::Number(600.)),
                ("plunge_feed", ParameterValue::Number(200.)),
                ("stepover", ParameterValue::Number(1.)),
                ("stepdown", ParameterValue::Number(2.)),
                ("max_angle_step", ParameterValue::Number(30.)),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect();
            // Resolve the old and current schemas separately. New defaults cannot
            // silently change the reference's meaning or activation behaviour.
            let reference_pool = rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .build()
                .unwrap();
            let token = CancelToken::new();
            let ctx = JobContext::new(&token, &NoProgress);
            let reference = reference_pool
                .install(|| generate(&legacy::LegacyFacing, input, &values, &ctx))
                .unwrap();
            let expected = bytes(&reference);
            for threads in [1, 2, 4] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();
                let current = pool
                    .install(|| generate(&Facing, input, &values, &ctx))
                    .unwrap();
                let actual = bytes(&current);
                // Compare complete bytes (including diagnostics), not just hashes.
                // Keep the fingerprint as useful CI diagnostics, never acceptance.
                let hash = expected.iter().fold(0xcbf29ce484222325u64, |h, b| {
                    (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
                });
                println!(
                    "{shape} threads={threads}: native pre-scope {hash:016x}, bytes={}, segments={}",
                    expected.len(),
                    current.group.segments.len()
                );
                assert!(
                    actual == expected,
                    "Exact native pre-scope {shape} baseline at {threads} threads: first differing byte {:?}; lengths {}/{}",
                    actual.iter().zip(&expected).position(|(a, b)| a != b),
                    actual.len(),
                    expected.len()
                );
            }
        }
    });
}

// Frozen Facing schema/dispatch and roughing::face entry from merged main
// b2278b89c252d2d2c3b67ea2e6ed2368c9d4ecba, before optional coverage scope.
// It deliberately does not call the current face() entry or pass a scope.
// Shared roughing, layer, kinematic and assembly services remain the legacy
// services: this oracle isolates the FACING scope/schema integration, while
// separate analytic tests verify the underlying geometric algorithms.
mod legacy {
    use openchuck_types::job::JobContext;

    use crate::error::Result;
    use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit};
    use crate::roughing::parameter as roughing;
    use crate::strategy::{Pass, Strategy, StrategyGroup, StrategyInput};

    /// The registry name of the frozen pre-scope Facing strategy.
    pub const NAME: &str = "facing";

    /// Parameter names specific to facing.
    pub mod parameter {
        /// [`FLAT`] or [`ROUND`].
        pub const SHAPE: &str = "shape";
        /// Flat: the face's distance from the rotary axis; round: the diameter.
        pub const SIZE: &str = "size";
        /// Flat: the A positions to face at, degrees.
        pub const ANGLES: &str = "angles";
        /// A flat face at each listed A.
        pub const FLAT: &str = "flat";
        /// The stock turned down to a diameter.
        pub const ROUND: &str = "round";
    }

    /// Facing of the stock (OP-24).
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct LegacyFacing;

    impl Strategy for LegacyFacing {
        fn name(&self) -> &str {
            NAME
        }

        fn label(&self) -> &str {
            "Facing"
        }

        fn description(&self) -> &str {
            "Reduces the stock along its length to flat faces at chosen rotary angles, or to a round diameter. Use it to prepare stock before roughing. The part and workholding can limit the cut."
        }

        fn group(&self) -> StrategyGroup {
            StrategyGroup::Roughing
        }

        fn schema(&self) -> ParameterSchema {
            let own = crate::roughing::RotaryRoughing.schema();
            let borrowed = [
                roughing::STEPDOWN,
                roughing::MAX_ANGLE_STEP,
                roughing::START_ANGLE,
                roughing::MIN_CUT,
            ]
            .into_iter()
            .filter_map(|name| own.get(name).cloned());
            let mut parameters = vec![
                ParameterSpec {
                    name: parameter::SHAPE.to_owned(),
                    label: "Shape".to_owned(),
                    description: "Flat: a flat face at each angle you list, the part held still. \
                              Round: the stock turned down to a cylinder as the part turns."
                        .to_owned(),
                    kind: ParameterKind::Choice {
                        options: vec![parameter::FLAT.to_owned(), parameter::ROUND.to_owned()],
                        default: parameter::FLAT.to_owned(),
                    },
                },
                ParameterSpec {
                    name: parameter::SIZE.to_owned(),
                    label: "Finished size".to_owned(),
                    description: "Flat: how far the face is from the rotary axis. Round: the \
                              diameter to turn the stock down to."
                        .to_owned(),
                    kind: ParameterKind::Number {
                        unit: Unit::Millimetres,
                        min: Some(0.0),
                        min_exclusive: true,
                        max: None,
                        default: None,
                    },
                },
                ParameterSpec {
                    name: parameter::ANGLES.to_owned(),
                    label: "Angles".to_owned(),
                    description: "Flat: the A positions to face at, in degrees, separated by \
                              commas, in the order to cut them."
                        .to_owned(),
                    kind: ParameterKind::AngleList {
                        default: "0".to_owned(),
                    },
                },
            ];
            parameters.extend(borrowed);
            parameters.extend(crate::extend::specs(true));
            ParameterSchema::with_common(parameters)
        }

        fn passes(
            &self,
            input: &StrategyInput<'_>,
            parameters: &Parameters,
            context: &JobContext<'_>,
        ) -> Result<Vec<Pass>> {
            let round = parameters.text(parameter::SHAPE)? == parameter::ROUND;
            let size = parameters.number(parameter::SIZE)?;
            let level = if round { 0.5 * size } else { size };
            legacy_face(input, parameters, round, level, context)
        }
    }

    fn legacy_face(
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        round: bool,
        level: f64,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let label = "facing";
        let mut job = super::super::Job::new(input, None, parameters, label)?;
        let ends = input.stock.x_extent().ok_or_else(|| {
            crate::PathError::Strategy(
                "facing needs the stock's ends, and the stock model gives none".into(),
            )
        })?;
        job.x_range = job.extension.map_or(ends, |e| e.widen(ends));
        if !level.is_finite() || level <= 0.0 || level >= job.r_top - job.min_cut {
            return Err(crate::PathError::Parameter {
                name: crate::facing::parameter::SIZE.to_owned(),
                reason: format!(
                    "the face would be {level:.3} mm from the rotary axis, and the stock reaches \
                 {:.3} mm: there is nothing to face",
                    job.r_top
                ),
            });
        }
        job.face = Some(level);
        let passes = if round {
            job.continuous(parameters, context)?.0
        } else {
            job.flat(parameters, level, context)?
        };
        if passes.is_empty() {
            return Err(crate::PathError::Strategy(format!(
                "facing has no cut to make: no material stands more than {} mm above the face",
                job.min_cut
            )));
        }
        Ok(passes)
    }
}

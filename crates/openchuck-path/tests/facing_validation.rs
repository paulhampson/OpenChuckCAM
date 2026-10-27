// SPDX-License-Identifier: AGPL-3.0-or-later
//! OP-24: a final-path refusal must inspect actual assembled output on every
//! generation entry point, without changing existing strategy diagnostics.
#![allow(clippy::unwrap_used, clippy::float_cmp, clippy::indexing_slicing)]

mod support;

use std::collections::BTreeMap;
use std::sync::Mutex;

use openchuck_path::{
    CutPoint, GeneratedPath, LeaveZone, MeshSurface, ParameterSchema, ParameterValue, Parameters,
    Pass, PathError, PathWarning, RegionConstraint, Result, Strategy, StrategyInput,
    StrategyOutput, generate, generate_in_region, generate_in_zones,
};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::MoveKind;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::units::{Degrees, Millimetres};
use support::{Fixture, ball, run};

fn fixture() -> Fixture {
    let mesh = TriangleMesh {
        vertices: vec![
            Point3::new(-10., -1., 0.),
            Point3::new(10., -1., 0.),
            Point3::new(10., 1., 0.),
        ],
        triangles: vec![[0, 1, 2]],
    };
    Fixture::new(MeshSurface::new(&mesh).unwrap(), 2., &ball(2.))
}
fn values() -> BTreeMap<String, ParameterValue> {
    [("feed", 300.), ("plunge_feed", 100.), ("stepover", 1.)]
        .into_iter()
        .map(|(name, v)| (name.into(), ParameterValue::Number(v)))
        .collect()
}

#[derive(Debug)]
struct Plain;
impl Strategy for Plain {
    fn name(&self) -> &str {
        "validation_fixture"
    }
    fn label(&self) -> &str {
        "Validation fixture"
    }
    fn description(&self) -> &str {
        "Explicit path for validation-contract tests."
    }
    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(vec![])
    }
    fn honours_region(&self) -> bool {
        true
    }
    fn honours_leave_zones(&self) -> bool {
        true
    }
    fn passes(
        &self,
        _: &StrategyInput<'_>,
        _: &Parameters,
        _: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        Ok(vec![Pass {
            points: [-8., -4., 0., 4., 8.]
                .into_iter()
                .map(|x| {
                    let tip = Point3::new(x, 0., 3.);
                    CutPoint {
                        tip,
                        a: Degrees(0.),
                        contact: tip,
                    }
                })
                .collect(),
            ..Pass::default()
        }])
    }
    fn passes_in_region(
        &self,
        i: &StrategyInput<'_>,
        _: &RegionConstraint<'_>,
        p: &Parameters,
        c: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        self.passes(i, p, c)
    }
    fn passes_in_zones(
        &self,
        i: &StrategyInput<'_>,
        _: Option<&RegionConstraint<'_>>,
        _: &[LeaveZone<'_>],
        p: &Parameters,
        c: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        self.passes(i, p, c)
    }
    fn passes_with_notes(
        &self,
        i: &StrategyInput<'_>,
        _: Option<&RegionConstraint<'_>>,
        p: &Parameters,
        c: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        Ok(StrategyOutput {
            passes: self.passes(i, p, c)?,
            notes: vec!["Existing strategy note".into()],
            warnings: vec![PathWarning {
                message: "Existing strategy warning".into(),
                parameter: Some("tolerance".into()),
            }],
        })
    }
}

#[derive(Debug, Default)]
struct Validating {
    seen: Mutex<Vec<GeneratedPath>>,
    refuse: bool,
    cancel: Option<CancelToken>,
    empty: bool,
}
impl Strategy for Validating {
    fn name(&self) -> &str {
        Plain.name()
    }
    fn label(&self) -> &str {
        Plain.label()
    }
    fn description(&self) -> &str {
        Plain.description()
    }
    fn schema(&self) -> ParameterSchema {
        Plain.schema()
    }
    fn honours_region(&self) -> bool {
        true
    }
    fn honours_leave_zones(&self) -> bool {
        true
    }
    fn passes(
        &self,
        i: &StrategyInput<'_>,
        p: &Parameters,
        c: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        Plain.passes(i, p, c)
    }
    fn passes_in_region(
        &self,
        i: &StrategyInput<'_>,
        r: &RegionConstraint<'_>,
        p: &Parameters,
        c: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        Plain.passes_in_region(i, r, p, c)
    }
    fn passes_in_zones(
        &self,
        i: &StrategyInput<'_>,
        r: Option<&RegionConstraint<'_>>,
        z: &[LeaveZone<'_>],
        p: &Parameters,
        c: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        Plain.passes_in_zones(i, r, z, p, c)
    }
    fn passes_with_notes(
        &self,
        i: &StrategyInput<'_>,
        r: Option<&RegionConstraint<'_>>,
        p: &Parameters,
        c: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        let mut output = Plain.passes_with_notes(i, r, p, c)?;
        if self.empty {
            output.passes.clear();
        }
        Ok(output)
    }
    fn validate_path(
        &self,
        i: &StrategyInput<'_>,
        p: &Parameters,
        path: &GeneratedPath,
        _: &JobContext<'_>,
    ) -> Result<()> {
        assert_eq!(path.group.tags, *i.tags);
        assert_eq!(p.number("feed")?, 300.);
        self.seen.lock().unwrap().push(path.clone());
        if let Some(token) = &self.cancel {
            token.cancel();
        }
        if self.refuse {
            return Err(PathError::Strategy("Required face is not complete".into()));
        }
        Ok(())
    }
}

#[test]
fn req_op_24_req_op_16_final_validation_runs_after_keepout_assembly_and_diagnostics_on_all_routes()
{
    let f = fixture();
    let mut input = f.input();
    input.keep_out = &[(-1., 1.)];
    let region = RegionConstraint::default();
    let zones = [LeaveZone {
        surface: &f.surface,
        leave: 0.1,
    }];
    for route in 0..3 {
        let check = Validating::default();
        let invoke = |s: &dyn Strategy| {
            run(|c| match route {
                0 => generate(s, &input, &values(), c),
                1 => generate_in_region(s, &input, &region, &values(), c),
                _ => generate_in_zones(s, &input, Some(&region), &zones, &values(), c),
            })
            .unwrap()
        };
        let baseline = invoke(&Plain);
        let path = invoke(&check);
        assert_eq!(
            path, baseline,
            "Provided no-op and observing hook preserve the final path and diagnostics"
        );
        let seen = check.seen.lock().unwrap();
        assert_eq!(
            seen.as_slice(),
            std::slice::from_ref(&path),
            "Exactly once, with the returned final path"
        );
        assert!(
            path.group
                .segments
                .iter()
                .any(|s| s.kind == MoveKind::Rapid)
        );
        assert!(
            path.group
                .segments
                .iter()
                .any(|s| s.kind == MoveKind::Plunge)
        );
        assert!(
            path.group
                .segments
                .iter()
                .any(|s| s.kind == MoveKind::Cutting)
        );
        assert!(
            path.group
                .segments
                .iter()
                .filter(|s| s.kind == MoveKind::Cutting)
                .all(|s| s.end.tip.x.abs() >= 1.)
        );
        assert!(path.notes.iter().any(|n| n.message.contains("held region")));
        if route < 2 {
            assert_eq!(
                path.warnings,
                vec![PathWarning {
                    message: "Existing strategy warning".into(),
                    parameter: Some("tolerance".into())
                }]
            );
            assert!(
                path.notes
                    .iter()
                    .any(|n| n.message == "Existing strategy note")
            );
        }
    }
}

#[test]
fn req_op_24_final_validation_refusal_returns_no_usable_path() {
    let f = fixture();
    let check = Validating {
        refuse: true,
        ..Validating::default()
    };
    let result = run(|c| generate(&check, &f.input(), &values(), c));
    assert!(
        matches!(result,Err(PathError::Strategy(message)) if message=="Required face is not complete")
    );
    assert_eq!(check.seen.lock().unwrap().len(), 1);
}

#[test]
fn req_op_24_empty_passes_retain_existing_no_cut_refusal() {
    let f = fixture();
    let check = Validating {
        empty: true,
        refuse: true,
        ..Validating::default()
    };
    let result = run(|c| generate(&check, &f.input(), &values(), c));
    assert!(
        matches!(result,Err(PathError::Strategy(message)) if message=="the strategy produced no cut for this selection")
    );
    let seen = check.seen.lock().unwrap();
    assert!(
        seen.is_empty(),
        "Assembly already refused; there is no final path to validate"
    );
}

#[test]
fn req_op_24_req_nfr_04_cancellation_during_final_validation_returns_no_path() {
    let f = fixture();
    let token = CancelToken::new();
    let check = Validating {
        cancel: Some(token.clone()),
        ..Validating::default()
    };
    let context = JobContext::new(&token, &NoProgress);
    assert!(matches!(
        generate(&check, &f.input(), &values(), &context),
        Err(PathError::Cancelled)
    ));
    assert_eq!(check.seen.lock().unwrap().len(), 1);
}

#[test]
fn req_op_24_final_validation_is_not_called_for_parameter_or_assembly_failure() {
    let f = fixture();
    let check = Validating::default();
    let mut bad = values();
    bad.insert("feed".into(), ParameterValue::Number(0.));
    assert!(run(|c| generate(&check, &f.input(), &bad, c)).is_err());
    let mut input = f.input();
    input.clearance.radius = Millimetres(0.1);
    input.clearance.safe_z = Millimetres(0.1);
    assert!(run(|c| generate(&check, &input, &values(), c)).is_err());
    assert!(check.seen.lock().unwrap().is_empty());
}

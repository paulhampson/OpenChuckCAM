// SPDX-License-Identifier: AGPL-3.0-or-later
//! FINSTEP consumer contract; no production adaptive strategy is implemented here.
#![allow(clippy::unwrap_used, clippy::float_cmp)]
use crate::{Command, form};
use openchuck_path::params::{ParameterKind, ParameterSpec, Unit};
use openchuck_path::{
    ParameterSchema, ParameterValue, Parameters, Pass, PathError, Strategy, StrategyInput,
};
use std::collections::BTreeMap;

#[derive(Debug)]
struct Probe;
impl Strategy for Probe {
    fn name(&self) -> &str {
        "facing"
    }
    fn label(&self) -> &str {
        "Consumer contract"
    }
    fn description(&self) -> &str {
        "Test-only active-schema consumer"
    }
    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(vec![
            ParameterSpec {
                name: "adaptive".into(),
                label: "Adaptive".into(),
                description: String::new(),
                kind: ParameterKind::Flag { default: false },
            },
            ParameterSpec {
                name: "height".into(),
                label: "Scallop".into(),
                description: String::new(),
                kind: ParameterKind::Number {
                    unit: Unit::Millimetres,
                    min: Some(0.),
                    min_exclusive: false,
                    max: None,
                    default: Some(0.),
                },
            },
        ])
    }
    fn schema_for(&self, values: &BTreeMap<String, ParameterValue>) -> ParameterSchema {
        let mut schema = self.schema();
        if values.get("adaptive") == Some(&ParameterValue::Flag(true)) {
            for spec in &mut schema.parameters {
                if spec.name == "height" {
                    spec.label = "Allowed excess".into();
                    spec.kind = ParameterKind::Number {
                        unit: Unit::Millimetres,
                        min: Some(0.),
                        min_exclusive: true,
                        max: None,
                        default: Some(0.01),
                    };
                }
            }
        }
        schema
    }
    fn passes(
        &self,
        _: &StrategyInput<'_>,
        _: &Parameters,
        _: &openchuck_types::job::JobContext<'_>,
    ) -> openchuck_path::Result<Vec<Pass>> {
        Err(PathError::Strategy("test-only schema consumer".into()))
    }
}

#[test]
fn req_op_11_req_ui_07_active_single_height_defaults_and_edit_constraints_match_generation() {
    let mut p = crate::test_project::TestProject::new("mode-schema-consumer");
    let op = p.add_operation("Consumer", "facing");
    let mut registry = openchuck_path::Registry::new();
    registry.register(std::sync::Arc::new(Probe)).unwrap();
    let form = form::parameter_form(p.app.project(), &registry, op).unwrap();
    let height = form.fields.iter().find(|f| f.name == "height").unwrap();
    assert_eq!(height.label, "Scallop");
    assert_eq!(
        height.value,
        Some(openchuck_project::ParameterValue::Number(0.))
    );
    p.app
        .execute(Command::SetParameter {
            operation: op,
            name: "adaptive".into(),
            value: Some(openchuck_project::ParameterValue::Flag(true)),
        })
        .unwrap();
    let form = form::parameter_form(p.app.project(), &registry, op).unwrap();
    let height = form.fields.iter().find(|f| f.name == "height").unwrap();
    assert_eq!(height.label, "Allowed excess");
    assert_eq!(
        height.value,
        Some(openchuck_project::ParameterValue::Number(0.01))
    );
    assert_eq!(
        Probe
            .resolve_parameters(
                &[
                    ("adaptive".into(), ParameterValue::Flag(true)),
                    ("feed".into(), ParameterValue::Number(100.)),
                    ("plunge_feed".into(), ParameterValue::Number(50.)),
                    ("stepover".into(), ParameterValue::Number(1.))
                ]
                .into_iter()
                .collect()
            )
            .unwrap()
            .number("height")
            .unwrap(),
        0.01
    );
    assert!(
        form::check_parameter(
            p.app.project(),
            &registry,
            op,
            "height",
            Some(&openchuck_project::ParameterValue::Number(0.))
        )
        .is_err(),
        "An edit must use active constraints, not the legacy union range"
    );
    assert!(
        form::check_parameter(
            p.app.project(),
            &registry,
            op,
            "height",
            Some(&openchuck_project::ParameterValue::Number(0.01))
        )
        .is_ok()
    );
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Immutable Facing verification inputs retained with generated results.
//! Coverage never replaces original stock or protected whole-setup geometry.

pub(crate) mod display;
pub(crate) mod written;
pub use display::FacingDisplay;
#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod parameter_contract_tests;

use crate::AppError;
use openchuck_path::facing::description::{FacingDescription, describe};
use openchuck_path::facing::plan::FacingSetup;
use openchuck_path::{Facing, Parameters, Solid};
use openchuck_types::ir::ToolpathGroup;
use openchuck_types::job::JobContext;
use openchuck_types::machine::MachineProfile;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;
use openchuck_types::tool::Tool;
use std::sync::Arc;

/// Whole-setup inputs from the SAME generation as an operation's path. File
/// connections and reporting must not borrow freshly edited project geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct FacingVerification(Arc<Inputs>);
#[derive(Debug, PartialEq)]
struct Inputs {
    part: TriangleMesh,
    stock: Stock,
    tool: Tool,
    machine: MachineProfile,
    tabs: Vec<Solid>,
    protected: Vec<Solid>,
    held: [Option<(f64, f64)>; 2],
    projection_error: Option<f64>,
    parameters: Parameters,
}
/// Stored settings needing a retained Facing certificate. Inactive side edits
/// do not change legacy defaults; malformed active settings cannot bypass it.
pub(crate) fn needs_verification(operation: &openchuck_project::Operation) -> bool {
    use openchuck_path::facing::parameter;
    use openchuck_project::ParameterValue as Value;
    if operation.strategy != openchuck_path::facing::NAME {
        return false;
    }
    let legacy_shape = match operation.parameters.get(parameter::SHAPE) {
        None => true,
        Some(Value::Text(shape)) => matches!(shape.as_str(), parameter::FLAT | parameter::ROUND),
        _ => false,
    };
    let whole = match operation.parameters.get(parameter::EXTENT) {
        None => true,
        Some(Value::Text(extent)) => extent == "whole_stock",
        _ => false,
    };
    let zero_margin = match operation.parameters.get(parameter::MARGIN) {
        None | Some(Value::Integer(0)) => true,
        Some(Value::Number(margin)) => *margin == 0.,
        _ => false,
    };
    !legacy_shape || !whole || !zero_margin
}

impl FacingVerification {
    /// Capture only modes requiring the new complete Facing checks. Legacy
    /// Whole stock/zero-margin defaults keep their existing path and metadata.
    pub(crate) fn capture(
        setup: &FacingSetup<'_>,
        parameters: &Parameters,
    ) -> Result<Option<Self>, AppError> {
        use openchuck_path::facing::parameter;
        let shape = parameters.text(parameter::SHAPE)?;
        if !matches!(shape, parameter::END | parameter::BOX)
            && parameters.text(parameter::EXTENT)? == "whole_stock"
            && parameters.number(parameter::MARGIN)? == 0.
        {
            return Ok(None);
        }
        Ok(Some(Self(Arc::new(Inputs {
            part: setup.part.clone(),
            stock: setup.stock.clone(),
            tool: setup.tool.clone(),
            machine: setup.machine.clone(),
            tabs: setup.tabs.to_vec(),
            protected: setup.protected.to_vec(),
            held: setup.held,
            projection_error: setup.projection_error,
            parameters: parameters.clone(),
        }))))
    }
    fn setup(&self) -> FacingSetup<'_> {
        let i = &self.0;
        FacingSetup {
            part: &i.part,
            stock: &i.stock,
            tool: &i.tool,
            machine: &i.machine,
            tabs: &i.tabs,
            protected: &i.protected,
            held: i.held,
            projection_error: i.projection_error,
        }
    }
    /// Independently recheck an actual canonical or connected group. This is
    /// nominal motion validation; written/controller precision is separate.
    pub(crate) fn validate(
        &self,
        group: &ToolpathGroup,
        context: &JobContext<'_>,
    ) -> Result<(), AppError> {
        Facing.validate_group(&self.setup(), &self.0.parameters, group, context)?;
        Ok(())
    }
    /// The same nominal coverage/target geometry for both host diagrams.
    /// This does not describe actual additional cutter sweep or certify it.
    pub fn description(&self) -> Result<FacingDescription, AppError> {
        Ok(describe(&self.setup(), &self.0.parameters)?)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]
    use super::*;
    use openchuck_path::facing::plan::{BoxFacingParameters, plan_box_sides};
    use openchuck_path::params::{CommonParameters, EntryParameters};
    use openchuck_path::{
        AcceptAll, AnalyticStock, MeshSurface, ParameterValue, RotaryMapping, Strategy,
        StrategyInput,
    };
    use openchuck_types::geometry::{Aabb, Point3};
    use openchuck_types::ir::{MoveKind, PathTags};
    use openchuck_types::job::{CancelToken, NoProgress};
    use openchuck_types::tool::ToolShape;
    use openchuck_types::units::{Millimetres, Seconds};
    fn fixture(ctx: &JobContext<'_>) -> (FacingVerification, ToolpathGroup) {
        fixture_with(ctx, &[], [None, None])
    }
    fn fixture_with(
        ctx: &JobContext<'_>,
        changes: &[(&str, ParameterValue)],
        held: [Option<(f64, f64)>; 2],
    ) -> (FacingVerification, ToolpathGroup) {
        fixture_with_tabs(ctx, changes, held, &[])
    }
    fn fixture_with_tabs(
        ctx: &JobContext<'_>,
        changes: &[(&str, ParameterValue)],
        held: [Option<(f64, f64)>; 2],
        tabs: &[Solid],
    ) -> (FacingVerification, ToolpathGroup) {
        let mut part = TriangleMesh {
            vertices: vec![
                Point3::new(10., -2., 0.),
                Point3::new(20., -2., 2.),
                Point3::new(20., 2., 2.),
            ],
            triangles: vec![[0, 1, 2]],
        };
        let stock = Stock::Box {
            bounds: Aabb {
                min: Point3::new(5., -4., -5.),
                max: Point3::new(25., 4., 5.),
            },
        };
        let tool = Tool {
            shape: ToolShape::FlatEnd,
            diameter: Millimetres(3.),
            flute_length: Millimetres(10.),
            overall_length: Millimetres(30.),
            shank_diameter: Millimetres(3.),
            flute_count: 2,
            material: None,
            centre_cutting: Some(true),
        };
        let machine = MachineProfile::example_x_rotary();
        let setup = FacingSetup {
            part: &part,
            stock: &stock,
            tool: &tool,
            machine: &machine,
            tabs,
            protected: tabs,
            held,
            projection_error: Some(0.),
        };
        let mut values: std::collections::BTreeMap<String, ParameterValue> = [
            ("shape", ParameterValue::Text("box".into())),
            ("positive_z", ParameterValue::Flag(true)),
            ("feed", ParameterValue::Number(600.)),
            ("plunge_feed", ParameterValue::Number(200.)),
            ("stepover", ParameterValue::Number(1.5)),
            ("stepdown", ParameterValue::Number(1.3)),
            ("stock_to_leave", ParameterValue::Number(0.2)),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect();
        for (key, value) in changes {
            values.insert((*key).into(), value.clone());
        }
        let p = Facing.resolve_parameters(&values).unwrap();
        let passes = if p.text("shape").unwrap() == "end" {
            openchuck_path::facing::end::plan_end(
                &setup,
                &openchuck_path::facing::end::EndFacingParameters::read(&p).unwrap(),
                ctx,
            )
            .unwrap()
            .passes
        } else {
            plan_box_sides(&setup, &BoxFacingParameters::read(&p).unwrap(), ctx)
                .unwrap()
                .passes
        };
        let surface = MeshSurface::new(&part).unwrap();
        let profile = tool.profile().unwrap();
        let analytic = AnalyticStock::from_stock(&stock, None).unwrap();
        let tags = PathTags {
            operation: "op".into(),
            setup: "setup".into(),
            tool: "tool".into(),
        };
        let input = StrategyInput {
            surface: &surface,
            stock: &analytic,
            collision: &AcceptAll,
            tool: &profile,
            rotary: RotaryMapping::of_profile(&machine),
            clearance: machine.clearance,
            max_feeds: machine.max_feeds,
            tags: &tags,
            entry_safety: None,
            facing: Some(&setup),
            keep_out: &[],
        };
        let path = openchuck_path::assemble::assemble_with_entry(
            &passes,
            &input,
            &CommonParameters::read(&p).unwrap(),
            &EntryParameters::read(&p).unwrap(),
            ctx,
        )
        .unwrap();
        let proof = FacingVerification::capture(&setup, &p).unwrap().unwrap();
        // Later caller mutations cannot alter the captured projection/target.
        part.vertices.iter_mut().for_each(|p| p.z += 100.);
        (proof, path.group)
    }
    #[test]
    fn req_op_24_req_op_14_result_retains_geometry_for_actual_connected_groups() {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let (proof, group) = fixture(&ctx);
        proof.validate(&group, &ctx).unwrap();
        let mut prior = group.clone();
        // A preceding operation can change equivalent whole-turn placement.
        prior.start.a.0 = 350.;
        let rotary = RotaryMapping::of_profile(&proof.0.machine);
        prior.start.axis = rotary.tool_axis(prior.start.a);
        prior.start.tip = rotary.to_model(
            openchuck_path::rotary::AxisPosition {
                along: 15.,
                across: 0.,
                height: 50.,
            },
            prior.start.a,
        );
        prior.segments.clear();
        let connected = openchuck_path::program::connect_group(
            Some(&prior),
            &group,
            &openchuck_path::program::ConnectionOptions {
                machine: &proof.0.machine,
                clearance: proof.0.machine.clearance,
                enclosing_radius: 10.,
                required_start_a: None,
            },
            &ctx,
        )
        .unwrap();
        proof.validate(&connected.group, &ctx).unwrap();
        let mut bad = connected.group;
        let leg = bad
            .segments
            .iter_mut()
            .find(|s| s.kind == MoveKind::Rapid)
            .unwrap();
        leg.end.tip = Point3::new(15., 0., 0.);
        leg.duration = Seconds(100.);
        assert!(proof.validate(&bad, &ctx).is_err());
        let openchuck_path::facing::description::FacingGeometry::Box { faces } =
            proof.description().unwrap().geometry
        else {
            panic!("box")
        };
        assert_eq!(faces[0].target.offset, 2.2);
        assert_eq!(faces[0].coverage.min(), [5., -4.]);
        cancel.cancel();
        assert!(proof.validate(&group, &ctx).unwrap_err().is_cancelled());
    }
    #[test]
    fn req_op_24_req_op_14_file_assembly_rechecks_actual_motion_with_generation_inputs() {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let (proof, mut group) = fixture(&ctx);
        let mut project = crate::test_project::TestProject::new("facing-file-check");
        let id = project.add_operation("Facing", "facing");
        group.tags = PathTags {
            operation: id.to_string(),
            setup: project.setup.to_string(),
            tool: project.tool.to_string(),
        };
        let metadata = crate::GroupInfo {
            operation: id,
            setup: project.setup,
            tool: proof.0.tool.profile().unwrap(),
            part_bounds: Aabb {
                min: Point3::new(10., -2., 0.),
                max: Point3::new(20., 2., 2.),
            },
            warnings: vec![],
            facing: Some(proof),
        };
        let canonical = group.clone();
        let output = crate::connections::assemble(
            project.app.project(),
            vec![group.clone()],
            std::slice::from_ref(&metadata),
            &ctx,
        )
        .unwrap();
        assert_eq!(output.groups.len(), 1);
        assert_eq!(
            canonical, group,
            "File placement cannot change cached canonical paths"
        );
        let leg = group
            .segments
            .iter_mut()
            .find(|s| s.kind == MoveKind::Rapid)
            .unwrap();
        leg.end.tip = Point3::new(15., 0., 0.);
        leg.duration = Seconds(100.);
        assert!(
            crate::connections::assemble(project.app.project(), vec![group], &[metadata], &ctx)
                .is_err(),
            "File assembly must not return an unchecked actual Facing group"
        );
    }

    #[test]
    fn req_op_24_legacy_default_does_not_capture_new_verification_context() {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let (proof, _) = fixture(&ctx);
        let values = [
            ("size", ParameterValue::Number(3.)),
            ("feed", ParameterValue::Number(600.)),
            ("plunge_feed", ParameterValue::Number(200.)),
            ("stepover", ParameterValue::Number(1.5)),
            ("stepdown", ParameterValue::Number(1.3)),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect();
        let p = Facing.resolve_parameters(&values).unwrap();
        assert!(
            FacingVerification::capture(&proof.setup(), &p)
                .unwrap()
                .is_none()
        );
    }
    fn written_case(
        proof: FacingVerification,
        mut group: ToolpathGroup,
        name: &str,
        context: &JobContext<'_>,
        f: impl FnOnce(
            &openchuck_post::PostDefinition,
            &openchuck_post::PostJob,
            &openchuck_types::ir::Toolpath,
            &crate::GroupInfo,
            &str,
        ),
    ) {
        use openchuck_post::{PostJob, PostProcessor, TemplatePost, ToolSetting, builtin};
        use openchuck_types::ir::Toolpath;
        use openchuck_types::kinematics::RotaryFrame;
        let id = openchuck_project::id::OperationId::new();
        let setup = openchuck_project::id::SetupId::new();
        group.tags.operation = id.to_string();
        group.tags.setup = setup.to_string();
        let metadata = crate::GroupInfo {
            operation: id,
            setup,
            tool: proof.0.tool.profile().unwrap(),
            part_bounds: openchuck_geom::summary(&proof.0.part).bounds.unwrap(),
            warnings: vec![],
            facing: Some(proof.clone()),
        };
        let path = Toolpath {
            groups: vec![group],
        };
        let job = PostJob {
            block_time: Default::default(),
            a_range: proof.0.machine.a_range,
            safe_z: 50.,
            frame: RotaryFrame::work(&proof.0.machine, &Default::default()),
            tools: [(
                "tool".into(),
                ToolSetting {
                    number: 1,
                    name: "Flat".into(),
                    spindle_rpm: 10000.,
                },
            )]
            .into_iter()
            .collect(),
            title: "Facing verification".into(),
            operation_names: Default::default(),
            operation_spindle: Default::default(),
            revision: None,
            generator: None,
            work_offset: Some("G54".into()),
        };

        let mut definition = builtin(name).unwrap().unwrap();
        // TEST post: accepted shipped footer remains unchanged pending policy.
        definition.options.footer = vec!["M5".into(), "M2".into()];
        let output = TemplatePost::new(definition.clone())
            .post(&path, &job)
            .unwrap();
        super::written::verify_output(
            &output.text,
            &definition,
            &job,
            &path,
            std::slice::from_ref(&metadata),
            context,
        )
        .unwrap();
        f(&definition, &job, &path, &metadata, &output.text);
    }
    #[test]
    fn req_op_24_req_ui_03_req_sim_05_facing_display_keeps_coverage_sweep_and_global_deviation_distinct()
     {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let (proof, group) = fixture_with(
            &ctx,
            &[
                ("extent", ParameterValue::Text("part".into())),
                ("extent_margin", ParameterValue::Number(0.5)),
            ],
            [None, None],
        );
        let display = proof.display(&group, &ctx).unwrap();
        assert!(display.intentionally_unfaced);
        assert!(
            display
                .lines
                .iter()
                .any(|l| l.role == crate::scene::SceneRole::FacingSweep)
        );
        let coverage = display
            .lines
            .iter()
            .find(|l| l.role == crate::scene::SceneRole::FacingExtent)
            .unwrap();
        assert_eq!(coverage.points[0], Point3::new(9.5, -2.5, 2.2));
        assert!(display.labels.iter().any(|l| l.text.contains("0.500")));
        assert!(display.completion.contains("complete"));
        let project = crate::test_project::TestProject::new("facing-pane-agreement");
        let view = crate::scene::SceneView {
            setup: Some(project.setup),
            lines: display.lines.clone(),
            labels: display.labels.clone(),
            ..Default::default()
        };
        let panes =
            crate::setup_panes::setup_panes(project.app.project(), &view, &[], &[], &[]).unwrap();
        assert_eq!(
            panes.front.facing[0].points[0],
            [coverage.points[0].x, coverage.points[0].z]
        );
        assert_eq!(
            panes.end.facing[0].points[0],
            [coverage.points[0].y, coverage.points[0].z]
        );
        let mut bad = group.clone();
        bad.segments.retain(|s| s.kind != MoveKind::Cutting);
        assert!(
            proof.display(&bad, &ctx).is_err(),
            "Display must not promote an unfinished actual path to completion"
        );
    }
    #[test]
    fn req_op_24_req_set_06_z1_footer_home_must_fit_all_machine_axes() {
        use openchuck_post::{PostProcessor, TemplatePost, builtin};
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let (mut proof, group) = fixture(&ctx);
        let data = Arc::get_mut(&mut proof.0).unwrap();
        data.machine.rotary_origin.x = 100.;
        data.machine.rotary_origin.z = -55.;
        data.machine.travel.min.x = 1.;
        written_case(
            proof,
            group,
            "makera-z1",
            &ctx,
            |_, job, path, metadata, _| {
                let definition = builtin("makera-z1").unwrap().unwrap();
                let text = TemplatePost::new(definition.clone())
                    .post(path, job)
                    .unwrap()
                    .text;
                assert!(
                    super::written::verify_output(
                        &text,
                        &definition,
                        job,
                        path,
                        std::slice::from_ref(metadata),
                        &ctx
                    )
                    .is_err(),
                    "Clear Z alone cannot authorise a home X outside configured travel"
                );
            },
        );
    }
    #[test]
    fn req_op_24_req_gc_09_shipped_z1_footer_clears_the_actual_setup() {
        use openchuck_post::{PostProcessor, TemplatePost, builtin};
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let (mut proof, group) = fixture(&ctx);
        Arc::get_mut(&mut proof.0).unwrap().machine.rotary_origin.z = -55.;
        written_case(
            proof,
            group,
            "makera-z1",
            &ctx,
            |_, job, path, metadata, _| {
                let definition = builtin("makera-z1").unwrap().unwrap();
                let text = TemplatePost::new(definition.clone())
                    .post(path, job)
                    .unwrap()
                    .text;
                super::written::verify_output(
                    &text,
                    &definition,
                    job,
                    path,
                    std::slice::from_ref(metadata),
                    &ctx,
                )
                .unwrap();
                let mut high = job.clone();
                high.safe_z = 53.;
                assert!(
                    super::written::verify_output(
                        &text,
                        &definition,
                        &high,
                        path,
                        std::slice::from_ref(metadata),
                        &ctx
                    )
                    .is_err(),
                    "The fixed footer must not lower below a higher configured clearance"
                );
                let extra = text.replacen("G90 G21 G17", "G90 G21 G17\nG28", 1);
                assert!(
                    super::written::verify_output(
                        &extra,
                        &definition,
                        job,
                        path,
                        std::slice::from_ref(metadata),
                        &ctx
                    )
                    .is_err(),
                    "A known footer cannot authorise an extra mid-program home"
                );
            },
        );
    }
    #[test]
    fn req_op_24_req_gc_09_actual_written_and_float_words_must_complete_the_face() {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        for name in ["linuxcnc", "makera-z1"] {
            let (proof, group) = fixture(&ctx);
            written_case(
                proof,
                group,
                name,
                &ctx,
                |definition, job, path, metadata, text| {
                    let verify = |text: &str| {
                        super::written::verify_output(
                            text,
                            definition,
                            job,
                            path,
                            std::slice::from_ref(metadata),
                            &ctx,
                        )
                    };
                    let corrupt = text.replacen("Z2.200", "Z2.180", 1);
                    assert_ne!(corrupt, text);
                    assert!(verify(&corrupt).is_err());
                    for code in ["G28", "G53 G0 Z-3", "G43 H1", "G91", "G20", "G55"] {
                        assert!(
                            verify(&text.replace("M2", &format!("{code}\nM2"))).is_err(),
                            "{code} is not a verified extra move/frame change"
                        );
                    }
                    assert!(verify(&text.replace("G90 G21 G17", "G17")).is_err());
                },
            );
        }
    }
    #[test]
    fn req_op_24_req_op_21_unheld_cylindrical_tab_is_preserved_through_both_posts() {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        for radius in [1., 5.] {
            for (held, base) in [
                ([Some((5., 8.)), None], 20.),
                ([None, Some((22., 25.))], 4.),
            ] {
                let tab = Solid::cylinder(
                    Point3::new(base, 0., 0.),
                    openchuck_types::geometry::UnitVec3::X,
                    6.,
                    radius,
                )
                .unwrap();
                for name in ["linuxcnc", "makera-z1"] {
                    let (proof, group) = fixture_with_tabs(
                        &ctx,
                        &[("shape", ParameterValue::Text("end".into()))],
                        held,
                        std::slice::from_ref(&tab),
                    );
                    proof.validate(&group, &ctx).unwrap();
                    written_case(proof, group, name, &ctx, |_, _, _, _, _| {});
                }
            }
        }
    }
    #[test]
    fn req_op_24_req_gc_09_end_written_motion_covers_both_workholding_directions() {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        for held in [[Some((5., 8.)), None], [None, Some((22., 25.))]] {
            for post in ["linuxcnc", "makera-z1"] {
                let (proof, group) =
                    fixture_with(&ctx, &[("shape", ParameterValue::Text("end".into()))], held);
                let display = proof.display(&group, &ctx).unwrap();
                let openchuck_path::facing::description::FacingGeometry::End { target, .. } =
                    display.description.geometry
                else {
                    panic!("End")
                };
                assert!(
                    display
                        .lines
                        .iter()
                        .filter(|l| l.role == crate::scene::SceneRole::FacingExtent)
                        .flat_map(|l| &l.points)
                        .all(|p| p.x == target.x),
                    "Both end diagrams must use the signed model X target"
                );
                written_case(proof, group, post, &ctx, |_, _, _, _, _| {});
            }
        }
    }
    #[test]
    fn req_op_24_req_nfr_12_missing_generation_context_cannot_bypass_written_validation() {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let (proof, mut group) = fixture(&ctx);
        let mut p = crate::test_project::TestProject::new("facing-missing-post-context");
        let op = p.add_operation("Facing", "facing");
        p.app
            .execute(crate::Command::SetParameter {
                operation: op,
                name: "shape".into(),
                value: Some(openchuck_project::ParameterValue::Text("box".into())),
            })
            .unwrap();
        group.tags = PathTags {
            operation: op.to_string(),
            setup: p.setup.to_string(),
            tool: p.tool.to_string(),
        };
        let metadata = crate::GroupInfo {
            operation: op,
            setup: p.setup,
            tool: proof.0.tool.profile().unwrap(),
            part_bounds: openchuck_geom::summary(&proof.0.part).bounds.unwrap(),
            warnings: vec![],
            facing: None,
        };
        let run = crate::Regeneration {
            canonical_toolpath: openchuck_types::ir::Toolpath {
                groups: vec![group],
            },
            groups: vec![metadata],
            project: std::sync::Arc::new(p.app.project().clone()),
            ..Default::default()
        };
        let result = crate::posting::post_toolpath(
            &run,
            &crate::PostSelection::Builtin("linuxcnc".into()),
            crate::OutputGrouping::default(),
            &ctx,
        );
        assert!(
            result.is_err(),
            "A path without its saved verification context cannot become usable Facing output"
        );
    }

    #[test]
    fn req_op_24_req_gc_09_fractional_a_written_projection_and_reversed_rotary_letter_are_verified()
    {
        use openchuck_post::{ADirection, PostProcessor, TemplatePost};
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        for post in ["linuxcnc", "makera-z1"] {
            let (proof, group) = fixture_with(
                &ctx,
                &[
                    ("positive_z", ParameterValue::Flag(false)),
                    (
                        "arbitrary_faces",
                        ParameterValue::Text(r#"[{"angle":33.3333}]"#.into()),
                    ),
                    ("layer_distribution", ParameterValue::Text("equal".into())),
                ],
                [None, None],
            );
            written_case(
                proof,
                group,
                post,
                &ctx,
                |definition, job, path, metadata, _| {
                    let mut reverse = definition.clone();
                    reverse.options.a_direction = ADirection::Reversed;
                    reverse.options.a_letter = "B".into();
                    let output = TemplatePost::new(reverse.clone()).post(path, job).unwrap();
                    super::written::verify_output(
                        &output.text,
                        &reverse,
                        job,
                        path,
                        std::slice::from_ref(metadata),
                        &ctx,
                    )
                    .unwrap();
                },
            );
        }
    }
    #[test]
    fn req_op_24_req_nfr_12_written_precision_and_feed_limits_cannot_be_waived_by_replay_tolerance()
    {
        use openchuck_post::{PostProcessor, TemplatePost};
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let (proof, group) = fixture(&ctx);
        written_case(
            proof,
            group,
            "linuxcnc",
            &ctx,
            |definition, job, path, metadata, text| {
                let mut remote = job.clone();
                remote.frame.origin.x = -1e6;
                let output = TemplatePost::new(definition.clone())
                    .post(path, &remote)
                    .unwrap();
                let error = super::written::verify_output(
                    &output.text,
                    definition,
                    &remote,
                    path,
                    std::slice::from_ref(metadata),
                    &ctx,
                )
                .unwrap_err()
                .to_string();
                assert!(error.contains("float precision"), "{error}");
                let faster = text.replace("F600.0", "F60000.0");
                assert_ne!(faster, text);
                assert!(
                    super::written::verify_output(
                        &faster,
                        definition,
                        job,
                        path,
                        std::slice::from_ref(metadata),
                        &ctx
                    )
                    .is_err()
                );
                cancel.cancel();
                assert!(
                    super::written::verify_output(
                        text,
                        definition,
                        job,
                        path,
                        std::slice::from_ref(metadata),
                        &ctx
                    )
                    .unwrap_err()
                    .is_cancelled()
                );
            },
        );
    }

    #[test]
    fn req_op_24_req_op_11_end_full_coverage_cannot_replace_the_selected_axial_slices() {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let (proof, mut group) = fixture_with(
            &ctx,
            &[("shape", ParameterValue::Text("end".into()))],
            [Some((5., 8.)), None],
        );
        proof.validate(&group, &ctx).unwrap();
        // Keep near planes22.6 and20.2, remove23.8 and21.4. The Ø3 tool
        // still covers all original X20.2..25 stock, but each actual2.4mm
        // bite exceeds the selected1.3mm. Preserve harmless air entry/links.
        group
            .segments
            .retain(|s| ![25.3, 22.9].iter().any(|x| (s.end.tip.x - x).abs() < 1e-10));
        for segment in &mut group.segments {
            segment.duration = Seconds(100.);
        }
        assert!(
            proof.validate(&group, &ctx).is_err(),
            "A complete final end slab is insufficient evidence of bounded axial engagement"
        );
    }
}

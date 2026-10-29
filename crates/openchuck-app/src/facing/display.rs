// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared, plain geometry for Facing viewport/Setup and operation-local results.
//! Display outlines approximate curves; they never replace the analytic checks.

use super::*;
use crate::scene::{SceneLabel, ScenePolyline, SceneRole};
use openchuck_path::facing::description::FacingGeometry;
use openchuck_path::facing::geometry::{CoverageRectangle, FacingPlane};
use openchuck_types::geometry::{Point3, Vec3};

/// One operation's shared diagram and separate completion result. Global
/// simulation deviation, including overcut, is never filtered by this value.
#[derive(Debug, Clone, PartialEq)]
pub struct FacingDisplay {
    /// Requested geometry, independently of cutter overlap.
    pub description: FacingDescription,
    /// Coverage/margin and actual cutter outlines in model coordinates.
    pub lines: Vec<ScenePolyline>,
    /// Targets, dimensions and margin with units.
    pub labels: Vec<SceneLabel>,
    /// Geometric completion status; no simulation accuracy claim.
    pub completion: String,
    /// Stock outside this operation's selected extent is intentionally retained.
    pub intentionally_unfaced: bool,
}
impl FacingVerification {
    /// Recheck actual motion and describe it without hiding global deviation.
    pub fn display(
        &self,
        group: &ToolpathGroup,
        context: &JobContext<'_>,
    ) -> Result<FacingDisplay, AppError> {
        self.validate(group, context)?;
        let mut display = nominal(self.description()?)?;
        let mut from = group.start;
        for segment in &group.segments {
            context.check_cancelled()?;
            let to = segment.end;
            if segment.kind.is_fed() && from.a == to.a {
                let delta = to.tip - from.tip;
                let normal = from.axis.get();
                let across = normal.cross(delta);
                // Finite straight cylindrical cutter envelope, including
                // approach/entry overlap. A display, never a proof by samples.
                let u = if across.length() > 1e-12 {
                    across * (1. / across.length())
                } else {
                    Vec3::new(1., 0., 0.)
                };
                let v = normal.cross(u);
                let r = 0.5 * self.0.tool.diameter.0;
                for h in [0., self.0.tool.flute_length.0] {
                    for tip in [from.tip, to.tip] {
                        let mut circle = Vec::with_capacity(33);
                        for i in 0..=32 {
                            let a = std::f64::consts::TAU * f64::from(i) / 32.;
                            circle.push(tip + normal * h + (u * a.cos() + v * a.sin()) * r);
                        }
                        display.lines.push(ScenePolyline {
                            role: SceneRole::FacingSweep,
                            points: circle,
                        });
                    }
                    for offset in [u * r, u * (-r), v * r, v * (-r)] {
                        display.lines.push(ScenePolyline {
                            role: SceneRole::FacingSweep,
                            points: vec![
                                from.tip + normal * h + offset,
                                to.tip + normal * h + offset,
                            ],
                        });
                    }
                }
                if display.lines.len() > 200_000 {
                    return Err(AppError::Post(
                        "Facing diagram exceeds its work budget".into(),
                    ));
                }
            }
            if segment.kind.is_fed() && from.a != to.a {
                let count = ((to.a.0 - from.a.0).abs() / 5.).ceil();
                if !count.is_finite() || count > 72. {
                    return Err(AppError::Post(
                        "Facing rotary diagram exceeds its angular work budget".into(),
                    ));
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let count = count as u32;
                let frame = openchuck_types::kinematics::RotaryFrame::of_profile(&self.0.machine);
                for i in 0..=count {
                    context.check_cancelled()?;
                    let (tip, normal, _, _) = crate::playback::between(
                        &frame,
                        &from,
                        &to,
                        f64::from(i) / f64::from(count.max(1)),
                    );
                    let u = Vec3::new(1., 0., 0.);
                    let v = normal.cross(u);
                    for h in [0., self.0.tool.flute_length.0] {
                        let points = (0..=32)
                            .map(|i| {
                                let a = std::f64::consts::TAU * f64::from(i) / 32.;
                                tip + normal * h
                                    + (u * a.cos() + v * a.sin()) * (0.5 * self.0.tool.diameter.0)
                            })
                            .collect();
                        display.lines.push(ScenePolyline {
                            role: SceneRole::FacingSweep,
                            points,
                        });
                    }
                    if display.lines.len() > 200_000 {
                        return Err(AppError::Post(
                            "Facing rotary sweep preview exceeds its work budget".into(),
                        ));
                    }
                }
            }
            from = to;
        }
        display.completion = "Facing coverage complete (verified path geometry)".into();
        Ok(display)
    }
}

pub(super) fn nominal(description: FacingDescription) -> Result<FacingDisplay, AppError> {
    use openchuck_path::facing::extents::FacingExtent;
    let mut lines = Vec::new();
    let mut labels = Vec::new();
    let margin = description.margin;
    let mut rectangle = |plane: FacingPlane,
                         extent: CoverageRectangle,
                         h: f64,
                         text: String|
     -> Result<(), AppError> {
        let lo = extent.min();
        let hi = extent.max();
        for (m, role) in [
            (0., SceneRole::FacingExtent),
            (margin, SceneRole::FacingMargin),
        ] {
            if m == 0. && role == SceneRole::FacingMargin {
                continue;
            }
            let points = [
                (lo[0] + m, lo[1] + m),
                (hi[0] - m, lo[1] + m),
                (hi[0] - m, hi[1] - m),
                (lo[0] + m, hi[1] - m),
                (lo[0] + m, lo[1] + m),
            ]
            .into_iter()
            .map(|(u, v)| plane.point(u, v, h))
            .collect::<Result<Vec<_>, _>>()?;
            lines.push(ScenePolyline { role, points });
        }
        labels.push(SceneLabel {
            role: SceneRole::FacingExtent,
            at: plane.point(hi[0], hi[1], h)?,
            text: format!("{text} · margin {margin:.3} mm"),
        });
        Ok(())
    };
    match &description.geometry {
        FacingGeometry::Flat { faces } | FacingGeometry::Box { faces } => {
            for face in faces {
                rectangle(
                    face.target.plane,
                    face.coverage,
                    face.target.offset,
                    format!(
                        "A {:.3}° · plane offset {:.3} mm",
                        face.target.angle.0, face.target.offset
                    ),
                )?;
            }
            for (i, a) in faces.iter().enumerate() {
                for b in faces.iter().skip(i + 1) {
                    if (a.target.plane.normal() + b.target.plane.normal()).length() < 1e-9 {
                        labels.push(SceneLabel {
                            role: SceneRole::FacingMargin,
                            at: a.corners()?[0],
                            text: format!(
                                "Opposite-plane separation {:.3} mm",
                                a.target.offset + b.target.offset
                            ),
                        });
                    }
                }
            }
        }
        FacingGeometry::End {
            end,
            target,
            coverage,
        } => {
            let plane = FacingPlane::end(*end);
            rectangle(
                plane,
                *coverage,
                plane.normal().x * target.x,
                format!(
                    "Target X {:.3} mm · length to opposite model end {:.3} mm",
                    target.x, target.length
                ),
            )?;
        }
        FacingGeometry::Round { diameter, x_extent } => {
            for (m, role) in [
                (0., SceneRole::FacingExtent),
                (margin, SceneRole::FacingMargin),
            ] {
                if m == 0. && role == SceneRole::FacingMargin {
                    continue;
                }
                for x in [x_extent.0 + m, x_extent.1 - m] {
                    let points = (0..=64)
                        .map(|i| {
                            let a = std::f64::consts::TAU * f64::from(i) / 64.;
                            Point3::new(x, 0.5 * diameter * a.cos(), 0.5 * diameter * a.sin())
                        })
                        .collect();
                    lines.push(ScenePolyline { role, points });
                }
            }
            for (y, z) in [
                (0.5 * diameter, 0.),
                (-0.5 * diameter, 0.),
                (0., 0.5 * diameter),
                (0., -0.5 * diameter),
            ] {
                lines.push(ScenePolyline {
                    role: SceneRole::FacingExtent,
                    points: vec![Point3::new(x_extent.0, y, z), Point3::new(x_extent.1, y, z)],
                });
            }
            labels.push(SceneLabel {
                role: SceneRole::FacingExtent,
                at: Point3::new(x_extent.1, 0., 0.5 * diameter),
                text: format!(
                    "Nominal Ø{diameter:.3} mm · X {:.3}…{:.3} mm · axial margin {margin:.3} mm",
                    x_extent.0, x_extent.1
                ),
            });
        }
    }
    Ok(FacingDisplay {
        intentionally_unfaced: description.extent != FacingExtent::WholeStock,
        description,
        lines,
        labels,
        completion: "Draft Facing extent; generate to verify completion".into(),
    })
}

/// Build draft geometry inside the existing scene worker from its immutable
/// project snapshot. Missing/invalid settings produce a visible reason.
pub(crate) fn preview(
    project: &openchuck_project::Project,
    setup: &openchuck_project::Setup,
    part: &TriangleMesh,
    projection_error: Option<f64>,
    view: &mut crate::scene::SceneView,
    context: &JobContext<'_>,
) -> Result<(), AppError> {
    use openchuck_path::{ParameterValue, Strategy};
    let registry = openchuck_path::Registry::builtin();
    for operation in project
        .operations()
        .iter()
        .filter(|o| o.setup == setup.id && o.strategy == openchuck_path::facing::NAME)
    {
        context.check_cancelled()?;
        let result = (|| -> Result<FacingDisplay, AppError> {
            let machine = project
                .machine()
                .ok_or_else(|| AppError::Post("choose a machine for the Facing preview".into()))?;
            let stock = setup
                .stock
                .as_ref()
                .ok_or_else(|| AppError::Post("define stock for the Facing preview".into()))?;
            let tool = operation
                .tool
                .and_then(|id| project.tools().iter().find(|t| t.id == id))
                .ok_or_else(|| AppError::Post("choose a tool for the Facing preview".into()))?;
            let extent = openchuck_geom::part_extent(part)
                .ok_or_else(|| AppError::Post("Facing part has no extent".into()))?;
            let protected = crate::workholding::solids(&setup.workholding, Some(stock), &extent)?;
            let mut tabs = setup.workholding.resolved(Some(stock), Some(&extent));
            tabs.start = None;
            tabs.end = None;
            for tab in &mut tabs.tabs {
                tab.auto = None;
            }
            let tabs = crate::workholding::solids(&tabs, Some(stock), &extent)?;
            let bounds = stock
                .bounds(Some(&extent))
                .ok_or_else(|| AppError::Post("Facing stock has no bounds".into()))?;
            let held = setup
                .workholding
                .held_intervals((bounds.min.x, bounds.max.x));
            let values = crate::operation::parameter_values(project, operation, &registry)?.values;
            let mut schema = Facing.schema_for(&values);
            // Only target/extent settings are required to draw a DRAFT.
            // No invented feed, stepdown or tool-size default is inserted.
            schema.parameters.retain(|s| {
                matches!(
                    s.name.as_str(),
                    "shape"
                        | "size"
                        | "angles"
                        | "extent"
                        | "extent_margin"
                        | "stock_to_leave"
                        | "arbitrary_faces"
                        | "positive_z"
                        | "positive_y"
                        | "negative_z"
                        | "negative_y"
                ) || s.name.ends_with("_offset")
                    || s.name.ends_with("_from_part")
            });
            let values: std::collections::BTreeMap<String, ParameterValue> = values
                .into_iter()
                .filter(|(name, _)| schema.get(name).is_some())
                .collect();
            let parameters = schema.resolve(&values)?;
            let shape = parameters.text(openchuck_path::facing::parameter::SHAPE)?;
            if shape == "box"
                && !matches!(stock, Stock::Box { .. } | Stock::OffsetBoundingBox { .. })
            {
                return Err(AppError::Post("Box sides requires box/block stock".into()));
            }
            if matches!(shape, "box" | "end")
                && !matches!(tool.tool.shape, openchuck_types::tool::ToolShape::FlatEnd)
            {
                return Err(AppError::Post(
                    "this Facing construction requires a flat end mill".into(),
                ));
            }
            let data = FacingSetup {
                part,
                stock,
                tool: &tool.tool,
                machine,
                tabs: &tabs,
                protected: &protected,
                held: [held.0, held.1],
                projection_error,
            };
            nominal(describe(&data, &parameters)?)
        })();
        match result {
            Ok(mut display) => {
                for label in &mut display.labels {
                    label.text = format!("{} · draft · {}", operation.name, label.text);
                    if display.description.source_projection_error.is_none() {
                        label
                            .text
                            .push_str(" · tessellation bounds; source accuracy unverified");
                    }
                }
                if display.description.source_projection_error.is_none() {
                    view.notes.push(format!("{}: Facing source projection accuracy is unknown; these tessellation bounds do not enable certified generation",operation.name));
                }
                view.lines.extend(display.lines);
                view.labels.extend(display.labels);
            }
            Err(error) => view.notes.push(format!(
                "{}: Facing preview unavailable: {}",
                operation.name,
                error.user_message()
            )),
        }
    }
    Ok(())
}

pub(crate) fn add_generated(
    view: &mut crate::scene::SceneView,
    run: &crate::Regeneration,
    context: &JobContext<'_>,
) -> Result<(), AppError> {
    for (group, info) in run.toolpath.groups.iter().zip(&run.groups) {
        if Some(info.setup) != view.setup {
            continue;
        }
        if let Some(proof) = &info.facing {
            let display = proof.display(group, context)?;
            view.lines.extend(
                display
                    .lines
                    .into_iter()
                    .filter(|l| l.role == SceneRole::FacingSweep),
            );
        }
    }
    Ok(())
}

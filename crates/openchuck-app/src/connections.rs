// SPDX-License-Identifier: AGPL-3.0-or-later
//! Application inputs to shared operation-boundary assembly (decision 0241).

use openchuck_path::program::{ConnectedGroup, ConnectionOptions, connect_group};
use openchuck_path::{AnalyticStock, StockModel};
use openchuck_project::Project;
use openchuck_types::geometry::Aabb;
use openchuck_types::ir::{Toolpath, ToolpathGroup};
use openchuck_types::job::JobContext;
use openchuck_types::setup::PartExtent;
use openchuck_types::units::Millimetres;

use crate::{AppError, GroupInfo};

fn radius(bounds: Aabb) -> f64 {
    bounds
        .min
        .y
        .abs()
        .max(bounds.max.y.abs())
        .hypot(bounds.min.z.abs().max(bounds.max.z.abs()))
}

pub(crate) fn connect(
    project: &Project,
    previous: Option<&ToolpathGroup>,
    next: &ToolpathGroup,
    part_bounds: Aabb,
    context: &JobContext<'_>,
) -> Result<ConnectedGroup, AppError> {
    let operation = project
        .operations()
        .iter()
        .find(|o| o.id.to_string() == next.tags.operation)
        .ok_or_else(|| {
            AppError::Post(format!(
                "operation '{}' of the toolpath is missing",
                next.tags.operation
            ))
        })?;
    let (setup, stock, machine) = crate::pipeline::setup_parts(project, operation)?;
    let part = PartExtent {
        bounds: part_bounds,
        radius: Millimetres(radius(part_bounds)),
    };
    let stock = AnalyticStock::from_stock(stock, Some(part_bounds))?;
    let mut enclosing_radius = radius(part_bounds).max(
        stock
            .max_radius()
            .ok_or_else(|| AppError::Post("the stock's enclosing radius is unknown".into()))?,
    );
    // Keep target, stock and protected geometry separate. The connection's
    // conservative envelope includes all three, independent of the selected
    // cutting region or intended stock outside a facing extent.
    for solid in crate::workholding::solids(&setup.workholding, setup.stock.as_ref(), &part)? {
        enclosing_radius = enclosing_radius.max(radius(solid.bounds()));
    }
    let options = ConnectionOptions {
        machine,
        clearance: crate::setup::effective_clearance(machine, setup, Some(&part_bounds)).clearance,
        enclosing_radius,
        required_start_a: None,
    };
    connect_group(
        previous.filter(|g| g.tags.setup == next.tags.setup),
        next,
        &options,
        context,
    )
    .map_err(|e| match e {
        openchuck_path::PathError::Cancelled => AppError::Path(e),
        other => AppError::Operation {
            name: operation.name.clone(),
            reason: other.to_string(),
        },
    })
}

/// Assemble one file from canonical operation groups. Metadata comes from
/// the same regeneration, never a fresh model read or a different project.
pub(crate) fn assemble(
    project: &Project,
    groups: Vec<ToolpathGroup>,
    info: &[GroupInfo],
    context: &JobContext<'_>,
) -> Result<Toolpath, AppError> {
    let mut result = Toolpath::default();
    for group in groups {
        context.check_cancelled()?;
        let data = info
            .iter()
            .find(|i| i.operation.to_string() == group.tags.operation)
            .ok_or_else(|| {
                AppError::Post(format!(
                    "operation '{}' has no generation geometry",
                    group.tags.operation
                ))
            })?;
        let linked = connect(
            project,
            result.groups.last(),
            &group,
            data.part_bounds,
            context,
        )?;
        if let Some(facing) = &data.facing {
            facing.validate(&linked.group, context)?;
        }
        result.groups.push(linked.group);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;
    use openchuck_types::job::{CancelToken, NoProgress};
    #[test]
    fn req_nfr_04_req_ui_09_connection_preserves_cancellation_kind() {
        let mut p = crate::test_project::TestProject::new("cancel-connection");
        p.add_operation("Finish", "helical_finishing");
        let run = p.app.regenerate().unwrap();
        let cancel = CancelToken::new();
        cancel.cancel();
        let context = JobContext::new(&cancel, &NoProgress);
        let error = connect(
            p.app.project(),
            None,
            &run.canonical_toolpath.groups[0],
            run.groups[0].part_bounds,
            &context,
        )
        .unwrap_err();
        assert!(error.is_cancelled(), "{error}");
    }
}

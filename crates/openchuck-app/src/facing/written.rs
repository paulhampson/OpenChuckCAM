// SPDX-License-Identifier: AGPL-3.0-or-later
//! Verify actual post words, including float-controller parsing, against the
//! retained Facing setup. No sampling, decimal-count certificate or mutable
//! project geometry is used. Unsupported program motion refuses.

use crate::{AppError, GroupInfo};
use openchuck_gcode::{
    CompareError, ComparedMove, Dialect, Motion, Program, Replay, TimingRule, Tolerances,
};
use openchuck_post::{ADirection, FeedRule, PostDefinition, PostJob, UnwindMode, round_written};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::{Segment, ToolPose, Toolpath, ToolpathGroup};
use openchuck_types::job::JobContext;
use openchuck_types::kinematics::{
    MachinePosition, RotaryFrame, machine_xyz, tool_axis_in_model, tool_tip_in_model,
};
use openchuck_types::units::{Degrees, FeedRate, Millimetres, Seconds};

fn refuse(reason: impl std::fmt::Display) -> AppError {
    AppError::Post(format!("Facing written-motion verification: {reason}"))
}
fn observer_error(reason: impl std::fmt::Display) -> CompareError {
    CompareError::Structure(reason.to_string())
}

#[allow(clippy::float_cmp)] // Exact parsed decimal identity, not proximity, is required.
pub(crate) fn verify_output(
    text: &str,
    definition: &PostDefinition,
    job: &PostJob,
    path: &Toolpath,
    info: &[GroupInfo],
    context: &JobContext<'_>,
) -> Result<(), AppError> {
    if info.iter().all(|i| i.facing.is_none()) {
        return Ok(());
    }
    let inputs = path
        .groups
        .iter()
        .map(|group| {
            info.iter()
                .find(|i| i.operation.to_string() == group.tags.operation)
                .and_then(|i| i.facing.as_ref())
        })
        .collect::<Vec<_>>();
    if inputs.iter().all(Option::is_none) {
        return Ok(());
    }
    context.check_cancelled()?;
    if text.len() > 256 * 1024 * 1024 {
        return Err(refuse("program exceeds the verification work limit"));
    }
    let mut program = openchuck_gcode::read(text).map_err(|e| {
        refuse(
            e.first()
                .map_or_else(|| "unreadable program".into(), ToString::to_string),
        )
    })?;
    // The established Z1 footer is checked separately, not silently omitted
    // by generic replay. Other home/machine-coordinate commands still refuse.
    verify_footer(&mut program, definition, job, path, &inputs, context)?;
    for block in &program.blocks {
        context.check_cancelled()?;
        if block.block_delete {
            return Err(refuse(
                "optional block deletion cannot certify complete Facing",
            ));
        }
        for w in &block.words {
            if w.letter == 'G' && [20., 28., 43., 53., 91.].contains(&w.value) {
                return Err(refuse(format!(
                    "line {}: G{} has no supported complete-motion certificate",
                    w.line, w.text
                )));
            }
        }
    }
    let dialect = if definition.options.unwind == UnwindMode::DropTurns {
        Dialect::MakeraZ1
    } else {
        Dialect::Linuxcnc
    };
    let axis = definition.options.a_letter.chars().next().unwrap_or('A');
    let mut raw = openchuck_gcode::replay_axis(&program, dialect, axis).map_err(refuse)?;
    normalize(&mut raw, definition.options.a_direction);
    if !raw.work_offset_before_motion || !raw.absolute_at_first_move || !raw.units_before_motion {
        return Err(refuse(
            "explicit work offset, metric units and absolute mode are required before motion",
        ));
    }
    for block in &program.blocks {
        for word in &block.words {
            if word.letter == 'G'
                && (54. ..=59.).contains(&word.value)
                && job
                    .work_offset
                    .as_deref()
                    .or(definition.options.work_offset.as_deref())
                    != Some(format!("G{:.0}", word.value).as_str())
            {
                return Err(refuse(
                    "the program selects a different work offset from its verification frame",
                ));
            }
        }
    }
    let rule = match definition.feed_rule {
        FeedRule::Makera => TimingRule::Makera,
        FeedRule::InverseTime => TimingRule::InverseTime,
        FeedRule::Linuxcnc | FeedRule::Contact => TimingRule::Linuxcnc,
    };
    let mut tolerances = Tolerances::for_decimals(
        definition.options.decimals.pos,
        definition.options.decimals.a,
    );
    // Exact decimal-word identity below replaces a loose XYZ distance test.
    // Feed rounding/clamping is checked against actual machine constraints.
    tolerances.position_mm = f64::INFINITY;
    tolerances.time_relative = f64::INFINITY;
    let raw_result = openchuck_gcode::compare_with_moves(
        &raw,
        path,
        &job.frame,
        rule,
        tolerances,
        &mut |matched| {
            context.check_cancelled().map_err(observer_error)?;
            let group = path
                .groups
                .get(matched.group)
                .ok_or_else(|| observer_error("missing source group"))?;
            let source = match matched.segment {
                Some(i) => {
                    group
                        .segments
                        .get(i)
                        .ok_or_else(|| observer_error("missing source segment"))?
                        .end
                }
                None => group.start,
            };
            // Main's #324 correction locates the requested tip at the A
            // word the controller receives, before rounding linear words.
            // Preserve exact identity against that convention; actual reached
            // decimal/float tool sweeps are still validated independently.
            let a = round_written(source.a.0, definition.options.decimals.a);
            let expected = machine_xyz(&job.frame, source.tip, openchuck_types::units::Degrees(a));
            for (actual, value) in matched
                .movement
                .to
                .iter()
                .take(3)
                .zip([expected.x, expected.y, expected.z])
            {
                if *actual != round_written(value, definition.options.decimals.pos) {
                    return Err(observer_error(format!(
                        "line {}: a position word differs from the generated motion",
                        matched.movement.line
                    )));
                }
            }
            let allowance = 128. * f64::EPSILON * a.abs().max(1.);
            if (matched.movement.to[3] + matched.coordinate_shift - a).abs() > allowance {
                return Err(observer_error(
                    "a rotary word differs from the generated physical motion",
                ));
            }
            let fed = matched
                .segment
                .and_then(|i| group.segments.get(i))
                .is_some_and(|s| s.kind.is_fed());
            if !matched.movement.metric
                || matched.movement.motion != if fed { Motion::Linear } else { Motion::Rapid }
            {
                return Err(observer_error(
                    "only matching metric linear/rapid motion is certified",
                ));
            }
            Ok(())
        },
    );
    context.check_cancelled()?;
    raw_result.map_err(refuse)?;
    verify_replay(&raw, path, &inputs, job, false, rule, definition, context)?;
    {
        // Verify binary32 too, rather than inferring controller precision
        // from a post's feed formula or display name.
        let float_program = float_words(program, axis, context)?;
        // Quantize BEFORE modal replay: this preserves unchanged modal words
        // and the controller's float-valued nonmoving counter resets.
        let mut actual =
            openchuck_gcode::replay_axis(&float_program, dialect, axis).map_err(refuse)?;
        normalize(&mut actual, definition.options.a_direction);
        verify_replay(&actual, path, &inputs, job, true, rule, definition, context)?;
    }
    Ok(())
}

#[allow(clippy::float_cmp)] // Recognise the exact established footer words.
fn verify_footer(
    program: &mut Program,
    definition: &PostDefinition,
    job: &PostJob,
    path: &Toolpath,
    inputs: &[Option<&super::FacingVerification>],
    context: &JobContext<'_>,
) -> Result<(), AppError> {
    let indices = program
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| !b.words.is_empty())
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let Some(tail) = indices
        .get(indices.len().saturating_sub(4)..)
        .filter(|t| t.len() == 4)
    else {
        return Ok(());
    };
    let expected: [&[(char, f64)]; 4] = [
        &[('M', 5.)],
        &[('G', 53.), ('G', 0.), ('Z', -3.)],
        &[('G', 28.)],
        &[('M', 2.)],
    ];
    if !tail.iter().zip(expected).all(|(i, words)| {
        program.blocks.get(*i).is_some_and(|b| {
            !b.block_delete
                && b.words
                    .iter()
                    .filter(|w| w.letter != 'N')
                    .map(|w| (w.letter, w.value))
                    .eq(words.iter().copied())
        })
    }) {
        return Ok(());
    }
    if definition.feed_rule != FeedRule::Makera
        || definition.options.unwind != UnwindMode::DropTurns
    {
        return Err(refuse(
            "this home footer requires the established Z1 controller conventions",
        ));
    }
    let last = path
        .groups
        .last()
        .map(|g| g.segments.last().map_or(g.start, |s| s.end))
        .ok_or_else(|| refuse("footer has no preceding path"))?;
    for proof in inputs.iter().flatten() {
        context.check_cancelled()?;
        let setup = proof.setup();
        let radial = openchuck_path::facing::motion::enclosing_radius(&setup, context)?;
        let origin = setup.machine.rotary_origin;
        let physical = machine_xyz(&job.frame, last.tip, last.a) + (origin - job.frame.origin);
        let height = -3. - origin.z;
        if !setup.machine.travel.contains(Point3::ORIGIN) {
            return Err(refuse(
                "the Z1 home position (machine XYZ0) lies outside configured travel",
            ));
        }
        // This global all-angle bound covers the vertical retract and the
        // established Z1 return at/above retract height. All supported tool
        // bodies extend upwards from the tip. No sampled-clearance inference.
        if !height.is_finite()
            || height < job.safe_z
            || physical.z - origin.z < job.safe_z - 0.001
            || height.min(physical.z - origin.z) <= radial + 0.001
            || setup.projection_error != Some(0.)
            || setup.machine.travel.min.z > -3.
            || setup.machine.travel.max.z < 0.
        {
            return Err(refuse(
                "the Z1 footer retract does not establish clearance above the actual whole setup/configured safe height",
            ));
        }
    }
    let [_, retract, home, _] = tail else {
        return Err(refuse("invalid established footer structure"));
    };
    let (retract, home) = (*retract, *home);
    program.blocks = std::mem::take(&mut program.blocks)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| *i != retract && *i != home)
        .map(|(_, b)| b)
        .collect();
    Ok(())
}

fn normalize(replay: &mut Replay, direction: ADirection) {
    if direction != ADirection::Reversed {
        return;
    }
    for step in &mut replay.steps {
        match step {
            openchuck_gcode::Step::Move(m) => {
                m.from[3] = -m.from[3];
                m.to[3] = -m.to[3];
            }
            openchuck_gcode::Step::Event(openchuck_gcode::Event::DropTurns {
                from, to, ..
            }) => {
                *from = -*from;
                *to = -*to;
            }
            _ => {}
        }
    }
}
fn float_words(
    mut program: Program,
    axis: char,
    context: &JobContext<'_>,
) -> Result<Program, AppError> {
    for block in &mut program.blocks {
        context.check_cancelled()?;
        for word in &mut block.words {
            if ['X', 'Y', 'Z', 'F', axis].contains(&word.letter) {
                if word.letter == axis && float_ulp(word.value)? > 0.01 {
                    return Err(refuse(
                        "float rotary counter exceeds the supported 0.01-degree numeric resolution; use a controller counter reset",
                    ));
                }
                if word.letter == 'F' && !(1e-6..=1e9).contains(&word.value) {
                    return Err(refuse(
                        "feed has no supported normal float arithmetic bound",
                    ));
                }
                word.value = as_float(word.value)?;
            }
        }
    }
    Ok(program)
}
#[allow(clippy::cast_possible_truncation)]
fn as_float(value: f64) -> Result<f64, AppError> {
    let value = value as f32;
    if !value.is_finite() {
        return Err(refuse(
            "a word exceeds the controller's float representation",
        ));
    }
    Ok(f64::from(value))
}
#[allow(clippy::cast_possible_truncation)]
fn float_ulp(value: f64) -> Result<f64, AppError> {
    let f = as_float(value.abs())? as f32;
    Ok(f64::from(f32::from_bits(f.to_bits().saturating_add(1))) - f64::from(f))
}
#[allow(clippy::cast_possible_truncation)]
fn linear_float(value: f64) -> Result<f64, AppError> {
    let word = (value.abs() + 1.) as f32;
    let ulp = f64::from(f32::from_bits(word.to_bits().saturating_add(1))) - f64::from(word);
    if !ulp.is_finite() || ulp > 0.0001 {
        return Err(refuse(
            "work/machine coordinates exceed the supported float precision; change the work offset or post",
        ));
    }
    as_float(value)
}
#[allow(clippy::too_many_arguments)]
fn verify_replay(
    replay: &Replay,
    path: &Toolpath,
    inputs: &[Option<&super::FacingVerification>],
    job: &PostJob,
    float: bool,
    rule: TimingRule,
    definition: &PostDefinition,
    context: &JobContext<'_>,
) -> Result<(), AppError> {
    let mut reached: Vec<Option<ToolpathGroup>> = path
        .groups
        .iter()
        .zip(inputs)
        .map(|(g, input)| {
            input.map(|_| ToolpathGroup {
                tags: g.tags.clone(),
                start: g.start,
                segments: vec![],
            })
        })
        .collect();
    let mut tolerances = Tolerances::for_decimals(
        definition.options.decimals.pos,
        definition.options.decimals.a,
    );
    tolerances.position_mm = f64::INFINITY;
    tolerances.time_relative = f64::INFINITY;
    if float {
        tolerances.a_deg += 0.01;
    }
    let result = openchuck_gcode::compare_with_moves(
        replay,
        path,
        &job.frame,
        rule,
        tolerances,
        &mut |m: ComparedMove<'_>| {
            context.check_cancelled().map_err(observer_error)?;
            let Some(proof) = inputs.get(m.group).copied().flatten() else {
                return Ok(());
            };
            let machine = &proof.0.machine;
            if job.frame.axis != machine.rotary_axis || job.frame.sense != machine.a_sense {
                return Err(observer_error(
                    "post frame differs from the verified machine",
                ));
            }
            let fixed = RotaryFrame::of_profile(machine);
            let zero = machine.rotary_origin - job.frame.origin;
            let xyz = |words: &[f64; 4]| -> Result<Point3, AppError> {
                let coordinate = |word, offset| -> Result<f64, AppError> {
                    if float {
                        linear_float(linear_float(word)? + linear_float(offset)?)
                    } else {
                        Ok(word + offset)
                    }
                };
                Ok(Point3::new(
                    coordinate(words[0], zero.x)?,
                    coordinate(words[1], zero.y)?,
                    coordinate(words[2], zero.z)?,
                ))
            };
            let physical = xyz(&m.movement.to).map_err(observer_error)?;
            let a = Degrees(m.movement.to[3] + m.coordinate_shift);
            let pose = ToolPose {
                tip: tool_tip_in_model(&fixed, &MachinePosition { xyz: physical, a }),
                axis: tool_axis_in_model(&fixed, a),
                a,
            };
            let group = reached
                .get_mut(m.group)
                .and_then(Option::as_mut)
                .ok_or_else(|| observer_error("missing reached Facing group"))?;
            if m.segment.is_none() {
                group.start = pose;
                return Ok(());
            }
            // A connected group can share the preceding group's reached start,
            // in which case no separate source-start block is emitted.
            let from_xyz = xyz(&m.movement.from).map_err(observer_error)?;
            let from_a = Degrees(m.movement.from[3] + m.coordinate_shift);
            let from = ToolPose {
                tip: tool_tip_in_model(
                    &fixed,
                    &MachinePosition {
                        xyz: from_xyz,
                        a: from_a,
                    },
                ),
                axis: tool_axis_in_model(&fixed, from_a),
                a: from_a,
            };
            if group.segments.is_empty() {
                group.start = from;
            }
            let source = path
                .groups
                .get(m.group)
                .and_then(|g| m.segment.and_then(|i| g.segments.get(i)))
                .ok_or_else(|| observer_error("missing reached source move"))?;
            let delta = physical - from_xyz;
            let (duration, feed) = if source.kind.is_fed() {
                // A single changing linear axis at fixed A sweeps the ENTIRE
                // interval despite monotone float interpolation. General fed
                // XYZ/A interpolation needs a separate certificate; never accept
                // a chord-only or rounded-endpoint argument for it.
                // The coverage/body checks use common height intervals and
                // axis ranges, so monotone Z drift does not require a sampled
                // chord argument. The XY footprint must still be straight:
                // at most one horizontal axis may change at fixed A.
                let fixed_prism = from.a == pose.a && (delta.x == 0. || delta.y == 0.);
                let round = proof
                    .0
                    .parameters
                    .text(openchuck_path::facing::parameter::SHAPE)
                    .map_err(observer_error)?
                    == openchuck_path::facing::parameter::ROUND;
                let radial_helix = round && delta.y == 0. && delta.z == 0. && from.a != pose.a;
                let axial_air = axial_air(proof, from, pose).map_err(observer_error)?;
                if !fixed_prism && !radial_helix && !axial_air {
                    return Err(observer_error(format!(
                        "line {}: Facing fed multi-axis interpolation is not yet certified ({:?}, delta {:?}, A {} to {}); use supported fixed-A straight entry/cuts",
                        m.movement.line, source.kind, delta, from.a.0, pose.a.0,
                    )));
                }
                if float && radial_helix {
                    let a_error = float_ulp(m.movement.from[3])
                        .and_then(|a| Ok(a.max(float_ulp(m.movement.to[3])?)))
                        .map_err(observer_error)?;
                    let x_error = float_ulp(from_xyz.x)
                        .and_then(|x| Ok(x.max(float_ulp(physical.x)?)))
                        .map_err(observer_error)?;
                    let interpolation = 2. * x_error
                        + delta.x.abs() * (1e-5 + 2. * a_error / (pose.a.0 - from.a.0).abs());
                    if interpolation > 0.001 {
                        return Err(observer_error(
                            "Round interpolation exceeds its independently reserved axial error budget",
                        ));
                    }
                }
                let seconds = m
                    .seconds
                    .ok_or_else(|| observer_error("fed motion has no duration"))?;
                // At most 128 normal rounded primitives: gamma128 < 1e-5 in
                // binary32 (input range checked above). Reserve fastest timing.
                let reserve = if float { 1e-5 } else { 1e-10 };
                (
                    seconds * (1. - reserve),
                    Some(FeedRate(
                        m.movement
                            .feed
                            .ok_or_else(|| observer_error("missing feed"))?,
                    )),
                )
            } else {
                let f = machine.max_feeds;
                (
                    60. * (delta.length() / f.rapid_linear.0)
                        .max(delta.z.abs() / f.rapid_z.0)
                        .max((pose.a.0 - from.a.0).abs() / f.rapid_rotary.0),
                    None,
                )
            };
            group.segments.push(Segment {
                kind: source.kind,
                end: pose,
                contact_feed: feed,
                duration: Seconds(duration),
                contact_length: Millimetres(0.),
            });
            Ok(())
        },
    );
    context.check_cancelled()?;
    result.map_err(refuse)?;
    for (group, proof) in reached.iter().zip(inputs) {
        if let (Some(group), Some(proof)) = (group, proof) {
            proof.validate(group, context).map_err(refuse)?;
        }
    }
    Ok(())
}

fn axial_air(
    proof: &super::FacingVerification,
    from: ToolPose,
    to: ToolPose,
) -> Result<bool, AppError> {
    use openchuck_path::facing::extents::FacingExtent;
    let setup = proof.setup();
    let stock = FacingExtent::WholeStock.axial(setup.stock, setup.part, &[], 0.)?;
    let part = FacingExtent::Part.axial(setup.stock, setup.part, &[], 0.)?;
    let mut lo = stock.0.min(part.0);
    let mut hi = stock.1.max(part.1);
    for solid in setup.protected.iter().chain(setup.tabs) {
        let b = solid.bounds();
        lo = lo.min(b.min.x);
        hi = hi.max(b.max.x);
    }
    let r = 0.5 * setup.tool.diameter.0.max(setup.tool.shank_diameter.0) + 0.001;
    Ok(from.tip.x.min(to.tip.x) - r > hi || from.tip.x.max(to.tip.x) + r < lo)
}

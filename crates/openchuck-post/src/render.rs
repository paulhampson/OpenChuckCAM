// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rendering the neutral program to text, and the `PostProcessor` trait
//! (GC-02, GC-03, GC-07; architecture section 5.3).

use std::collections::BTreeMap;

use minijinja::value::Serde;
use serde::Serialize;

use openchuck_types::ir::Toolpath;
use openchuck_types::kinematics::RotaryFrame;

use crate::convert::{ConvertOptions, ToolSetting, to_machine_program};
use crate::definition::PostDefinition;
use crate::engine;
use crate::error::{PostError, Result};
use crate::options::{ADirection, PostOptions, UnwindMode};
use crate::program::{MachineProgram, Step, Target};

/// What a post needs to know about the job beyond the toolpath.
#[derive(Debug, Clone)]
pub struct PostJob {
    /// The work frame the program is written in.
    pub frame: RotaryFrame,
    /// Tool numbers and spindle speeds by the IR's tool tag (GC-14).
    pub tools: BTreeMap<String, ToolSetting>,
    /// The program title, written as a comment.
    pub title: String,
    /// Display names of operations by the IR's operation tag, written in the
    /// comments that name each operation (GC-06). A tag with no name is
    /// written as it is.
    pub operation_names: BTreeMap<String, String>,
    /// A revision identifier for the header, when supplied (VC-06).
    pub revision: Option<String>,
    /// The tool and its version, written as a comment after the title, such as
    /// `OpenChuckCAM 0.10.0` (NFR-16). The host passes the software version only, never a
    /// commit or a date, so one version always gives the same program (GC-07, NFR-06).
    pub generator: Option<String>,
    /// The setup's work offset register (`G54` to `G59`), which replaces the
    /// post's own `work_offset` when given (SET-05).
    pub work_offset: Option<String>,
}

/// The result of posting.
#[derive(Debug, Clone, PartialEq)]
pub struct PostOutput {
    /// The program text.
    pub text: String,
    /// Fed moves whose feed was lowered to the post's maximum.
    pub clamped_feeds: usize,
    /// Sum of the IR's move durations, seconds.
    pub estimated_seconds: f64,
}

/// Something that turns a toolpath into program text (GC-02, GC-13). The
/// template implementation is first; a scripting engine can follow.
pub trait PostProcessor {
    /// A stable name.
    fn name(&self) -> &str;
    /// Posts a toolpath.
    fn post(&self, toolpath: &Toolpath, job: &PostJob) -> Result<PostOutput>;
}

/// Replaces what a G-code comment cannot hold: parentheses, control
/// characters and anything that is not ASCII (decision 0019 item 4, OI-84).
pub fn sanitise_comment(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '(' => '[',
            ')' => ']',
            c if c.is_ascii() && !c.is_ascii_control() => c,
            c if c.is_control() => ' ',
            _ => '?',
        })
        .collect()
}

#[derive(Serialize)]
struct Context<'a> {
    title: String,
    revision: Option<String>,
    generator: Option<String>,
    opt: &'a PostOptions,
    steps: &'a [Step],
}

fn map_a(step: &mut Step, f: impl Fn(f64) -> f64) {
    match step {
        Step::Rapid { target } | Step::Feed { target, .. } => target.a = f(target.a),
        Step::Unwind { from, to, at } => {
            *from = f(*from);
            *to = f(*to);
            at.a = f(at.a);
        }
        _ => {}
    }
}

/// Applies the post's A conventions to the neutral program: direction, and
/// how whole turns are removed.
fn apply_a_conventions(program: &MachineProgram, options: &PostOptions) -> Vec<Step> {
    let sign = match options.a_direction {
        ADirection::Same => 1.0,
        ADirection::Reversed => -1.0,
    };
    let mut steps = program.steps.clone();
    for step in &mut steps {
        map_a(step, |a| a * sign);
    }
    match options.unwind {
        UnwindMode::Rotate => steps
            .into_iter()
            .map(|step| match step {
                Step::Unwind { at, to, .. } => Step::Rapid {
                    target: Target { a: to, ..at },
                },
                other => other,
            })
            .collect(),
        UnwindMode::DropTurns => {
            // The controller keeps the remainder of A with its sign, which may
            // differ from the IR's chosen turn; shifting what follows by whole
            // turns keeps every machine position the same.
            let mut shift = 0.0;
            steps
                .into_iter()
                .map(|mut step| match &mut step {
                    Step::Unwind { from, to, at } => {
                        let held = *from + shift;
                        let remainder = held - 360.0 * (held / 360.0).trunc();
                        shift = remainder - *to;
                        *from = held;
                        *to = remainder;
                        at.a = remainder;
                        step
                    }
                    _ => {
                        map_a(&mut step, |a| a + shift);
                        step
                    }
                })
                .collect()
        }
    }
}

/// A post defined by a template and options (GC-03).
#[derive(Debug, Clone, PartialEq)]
pub struct TemplatePost {
    definition: PostDefinition,
}

impl TemplatePost {
    /// Wraps a checked definition.
    pub fn new(definition: PostDefinition) -> Self {
        Self { definition }
    }

    /// The definition.
    pub fn definition(&self) -> &PostDefinition {
        &self.definition
    }
}

impl PostProcessor for TemplatePost {
    fn name(&self) -> &str {
        &self.definition.name
    }

    fn post(&self, toolpath: &Toolpath, job: &PostJob) -> Result<PostOutput> {
        let d = &self.definition;
        let convert = ConvertOptions {
            frame: job.frame,
            feed_rule: d.feed_rule,
            max_feed: d.options.max_feed,
            tools: job.tools.clone(),
            decimals: Some((d.options.decimals.pos, d.options.decimals.a)),
            operation_names: job.operation_names.clone(),
        };
        let mut program = to_machine_program(toolpath, &convert)?;
        for step in &mut program.steps {
            match step {
                Step::Comment { text } => *text = sanitise_comment(text),
                Step::ToolChange { name, .. } => *name = sanitise_comment(name),
                _ => {}
            }
        }
        let mut options = d.options.clone();
        if let Some(offset) = &job.work_offset {
            options.work_offset = Some(offset.clone());
            options.validate()?;
        }
        let steps = apply_a_conventions(&program, &options);
        let context = Context {
            title: sanitise_comment(&job.title),
            revision: job.revision.as_deref().map(sanitise_comment),
            generator: job.generator.as_deref().map(sanitise_comment),
            opt: &options,
            steps: &steps,
        };
        let mut env = engine::environment(steps.len())?;
        env.add_template_owned(d.template_name.clone(), d.template.clone())
            .map_err(|e| PostError::Template(engine::describe(&e)))?;
        let template = env
            .get_template(&d.template_name)
            .map_err(|e| PostError::Template(engine::describe(&e)))?;
        let text = template
            .render(Serde(&context))
            .map_err(|e| PostError::Template(engine::describe(&e)))?;
        Ok(PostOutput {
            text,
            clamped_feeds: program.clamped_feeds,
            estimated_seconds: toolpath.total_duration().0,
        })
    }
}

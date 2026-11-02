// SPDX-License-Identifier: AGPL-3.0-or-later
//! Post each measured form for M4, including a finer tessellation which the
//! application cannot select.
//!
use openchuck_post::{PostJob, PostProcessor, TemplatePost, builtin};
use openchuck_types::ir::{Toolpath, ToolpathGroup};

pub fn post_run(
    groups: &[Result<ToolpathGroup, String>],
    job: &PostJob,
    id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let groups: Vec<_> = groups
        .iter()
        .filter_map(|g| g.as_ref().ok().cloned())
        .collect();
    if groups.is_empty() {
        return Err("M4 cannot post a run with no successful operations".into());
    }
    let post = TemplatePost::new(builtin(id).ok_or("no such built-in post")??);
    Ok(post.post(&Toolpath { groups }, job)?.text)
}

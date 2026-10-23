// SPDX-License-Identifier: AGPL-3.0-or-later
//! Post each measured form for M4, including a finer tessellation which the
//! application cannot select.
//!
//! UNFINISHED TRANSFER SNAPSHOT: this is the deliberate empty-output stub from
//! the red test phase. It is not wired into main.rs. Implement posting, then
//! integrate it into M4 before claiming form-c G-code determinism. The contract
//! tests currently fail; no ignore or successful verification is claimed.

use openchuck_post::PostJob;
use openchuck_types::ir::ToolpathGroup;

pub fn post_run(
    _groups: &[Result<ToolpathGroup, String>],
    _job: &PostJob,
) -> Result<String, Box<dyn std::error::Error>> {
    Ok(String::new())
}

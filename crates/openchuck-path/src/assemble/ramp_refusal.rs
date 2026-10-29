// SPDX-License-Identifier: AGPL-3.0-or-later
//! Every refusal of the optional back-and-forth ramp (decisions 0242 and
//! 0248), in plain words: what is wrong, where the user changes it and what
//! to try (#291, lane RAMPERRORS; NFR-07, decision 0114). The checks are in
//! `entry` and `entry_safety`; only the wording is here.

use crate::entry_safety::{Blocked, ENTRY_FIX, lower_first};

use super::ramp::Refusal;

/// The host gave no whole part, complete tool and machine travel to check
/// the ramp against. The application refuses a part it cannot supply them
/// for before it generates (a STEP part); a library host sees this.
pub(super) fn missing_inputs() -> String {
    format!(
        "The back-and-forth ramp cannot check its clearance in this operation: it needs the \
         whole part, the complete tool and the machine's travel, and they were not supplied. \
         {ENTRY_FIX}"
    )
}

/// The pass has no straight line at its start to ramp along: it is one
/// point, or it only turns the part.
pub(super) fn no_direction(pass: usize, points: usize) -> String {
    let what = if points < 2 {
        "is a single point"
    } else {
        "only turns the part about the rotary axis, without moving the tool along a straight \
         line"
    };
    format!(
        "Pass {} {what}, so the back-and-forth ramp, which keeps the A angle fixed while it \
         descends, has no straight line to ramp along. {ENTRY_FIX}",
        pass + 1
    )
}

/// The stock gives no largest radius to start the ramp above.
pub(super) fn no_stock_size() -> String {
    format!(
        "The back-and-forth ramp starts above the whole stock, and this stock has no size to \
         start above. In the Setup step, under Stock, set the stock's shape and size, or change \
         the entry: {}",
        lower_first(ENTRY_FIX)
    )
}

/// The ramp would start above the height the tool is at before the pass:
/// the safe Z before the first pass, else the height its links retract to.
pub(super) fn above_clearance(pass: usize, top: f64, at: f64, field: &str) -> String {
    format!(
        "Pass {}: the back-and-forth ramp starts {top:.3} mm from the rotary axis (the stock's \
         largest radius plus the approach distance), above the {at:.3} mm the tool is at before \
         this pass. In the Setup step, under Work offset and clearance, set {field} to at least \
         {top:.3} mm; or, in the operation's Entry, links and retract settings, lower Approach \
         distance, turn off Back-and-forth ramp or set Entry to Plunge, Lead or Helix.",
        pass + 1
    )
}

/// The first cut point is at or above the height the ramp would start at,
/// `above` mm over the stock (owner's choice D3 = A: refused, reworded).
pub(super) fn nothing_to_descend(pass: usize, above: f64) -> String {
    format!(
        "Pass {} starts {above:.3} mm above the stock, so the back-and-forth ramp has nothing \
         to ramp down into. In the operation's Entry, links and retract settings, turn off \
         Back-and-forth ramp or set Entry to Plunge.",
        pass + 1
    )
}

/// The first straight run of one tool diameter at the cut's depth could
/// not be proved clear. The first sentence is the one the owner approved
/// (decision 0242 item 3); the cause and the fix replace its second
/// (owner's choice D2 = A, #291).
pub(super) fn corridor(length: f64, blocked: Blocked) -> String {
    let fix = match blocked {
        Blocked::Target => format!("{ENTRY_FIX} A smaller tool needs a shorter run."),
        other => other.fix(),
    };
    format!(
        "Back-and-forth ramp needs at least {length:.3} mm of usable tool-centre travel; only \
         0.000 mm could be verified: within one tool diameter behind the first cut, {}. {fix}",
        blocked.cause()
    )
}

/// The descending legs could not be planned.
pub(super) fn plan(pass: usize, refusal: Refusal, rise: f64, length: f64, angle: f64) -> String {
    let n = pass + 1;
    let angle_fix = "In the operation's Entry, links and retract settings, raise Ramp angle, or \
         turn off Back-and-forth ramp or set Entry to Plunge, Lead or Helix.";
    match refusal {
        Refusal::TooMany => format!(
            "Pass {n}: the back-and-forth ramp would need more than 10000 runs back and forth to \
             descend {rise:.3} mm at {angle:.2} degrees along {length:.3} mm (one tool \
             diameter). {angle_fix}"
        ),
        Refusal::NoProgress => format!(
            "Pass {n}: at {angle:.2} degrees, each run of {length:.3} mm of the back-and-forth \
             ramp descends too little to be written in the G-code. {angle_fix}"
        ),
        Refusal::TooShort => format!(
            "Pass {n}: the back-and-forth ramp has less than one tool diameter ({length:.3} mm) \
             to run along. {ENTRY_FIX}"
        ),
        Refusal::Invalid | Refusal::Cancelled => format!(
            "Pass {n}: the back-and-forth ramp cannot be planned to descend {rise:.3} mm at \
             {angle:.2} degrees. In the operation's Entry, links and retract settings, check \
             Ramp angle, or turn off Back-and-forth ramp or set Entry to Plunge, Lead or Helix."
        ),
    }
}

/// A descending run, a reversal, the approach or the final connection could
/// not be proved clear.
pub(super) fn blocked(pass: usize, blocked: Blocked) -> String {
    format!(
        "Pass {}: the back-and-forth ramp cannot be used here: {}. {}",
        pass + 1,
        blocked.cause(),
        blocked.fix()
    )
}

/// The additional finished-surface check found a cut into the part.
pub(super) fn into_part(pass: usize, depth: f64, tolerance: f64) -> String {
    format!(
        "Pass {}: the back-and-forth ramp cannot be used here: the tool would cut {depth:.3} mm \
         into the finished part (the tolerance is {tolerance:.3} mm). {ENTRY_FIX}",
        pass + 1
    )
}

/// The additional held-region check found the ramp over a held end.
pub(super) fn over_held(pass: usize, held: &str, x: f64) -> String {
    format!(
        "Pass {}: the back-and-forth ramp cannot be used here: the tool would run over {held} \
         at X {x:.3} mm, where the workholding grips the stock. In the Setup step, under \
         Workholding and tabs, move the held region clear of the start of the pass, or change \
         the entry: {}",
        pass + 1,
        lower_first(ENTRY_FIX)
    )
}

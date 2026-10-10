//! Candidate neutral toolpath IR fields (GC-01, architecture section 3). Everything here is
//! in the *model frame*; no axis letters, no controller names.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Rapid,
    Plunge,
    Cut,
    Retract,
}

/// One segment = the move *to* `tip`. The start is the previous segment's end.
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub kind: Kind,
    /// Tool tip, model frame, mm.
    pub tip: [f64; 3],
    /// Unit tool axis (tip towards holder), model frame.
    pub axis: [f64; 3],
    /// Explicit, unwrapped A angle in degrees (continuous; not reduced mod 360).
    pub a_deg: f64,
    /// Feed at the cutter contact point, mm/min. None for rapids.
    pub contact_feed: Option<f64>,
    /// Time for the move at the contact feed, seconds. None for rapids.
    pub duration_s: Option<f64>,
    /// Length of the contact-point path in this move, mm (what the duration was derived from).
    pub contact_len: Option<f64>,
}

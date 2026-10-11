// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tool-centre solving from a surface point and its normal: the tool touches
//! where its outward normal is opposite the surface normal. Valid for convex
//! profiles only (`Profile::contact_for_normal` refuses others), and local:
//! a convex tool on a concave region still needs a drop or a gouge check.

use crate::mesh::V3;
use crate::profile::Profile;

/// Tip position for a tool with axis `axis` (unit) touching surface point `p`
/// whose outward unit normal is `n`. `None` when the surface faces away from
/// the tool or the profile is not convex.
pub fn tip_from_contact(profile: &Profile, p: V3, n: V3, axis: V3) -> Option<V3> {
    // Tool outward normal at the contact.
    let m = [-n[0], -n[1], -n[2]];
    let ma = m[0] * axis[0] + m[1] * axis[1] + m[2] * axis[2];
    if ma > 1e-12 {
        return None; // The surface faces away from the tool.
    }
    let mr = [
        m[0] - ma * axis[0],
        m[1] - ma * axis[1],
        m[2] - ma * axis[2],
    ];
    let rho = (mr[0] * mr[0] + mr[1] * mr[1] + mr[2] * mr[2]).sqrt();
    let phi = ma.atan2(rho);
    let c = profile.contact_for_normal(phi)?;
    let e = if rho > 1e-15 {
        [mr[0] / rho, mr[1] / rho, mr[2] / rho]
    } else {
        [0.0; 3]
    };
    Some([
        p[0] - c.z * axis[0] - c.r * e[0],
        p[1] - c.z * axis[1] - c.r * e[1],
        p[2] - c.z * axis[2] - c.r * e[2],
    ])
}

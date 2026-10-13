// SPDX-License-Identifier: AGPL-3.0-or-later
//! The test surfaces and tools shared by the tests and the report.

use crate::place::Tool;
use crate::surface::*;

pub struct Case {
    pub id: &'static str,
    pub surface: Box<dyn Surface>,
    pub tool: Tool,
    /// `+1` or `-1`: which side of the geometric normal the tool sits on.
    pub side: f64,
}

fn deg(d: f64) -> f64 {
    d.to_radians()
}

pub const TOL: f64 = 0.01;

pub fn cases() -> Vec<Case> {
    let cone = || RadialCone {
        x0: 10.0,
        beta: deg(6.0),
        u: (0.4, 2.7),
        v: (15.0, 30.0),
    };
    let tool = |r: f64, a: f64| Tool {
        radius: r,
        half_angle: deg(a),
    };
    vec![
        Case {
            id: "cylinder",
            surface: Box::new(RadialCylinder {
                x0: 10.0,
                r: 8.0,
                u: (0.4, 2.7),
                v: (15.0, 30.0),
            }),
            tool: tool(3.0, 0.0),
            side: -1.0,
        },
        Case {
            id: "cone, matching taper",
            surface: Box::new(cone()),
            tool: tool(3.0, 6.0),
            side: -1.0,
        },
        Case {
            id: "cone, taper 4 deg",
            surface: Box::new(cone()),
            tool: tool(3.0, 4.0),
            side: -1.0,
        },
        Case {
            id: "cone, straight tool",
            surface: Box::new(cone()),
            tool: tool(3.0, 0.0),
            side: -1.0,
        },
        Case {
            id: "helicoid p=1.5",
            surface: Box::new(Helicoid {
                pitch: 1.5,
                u: (0.2, 2.9),
                v: (12.0, 30.0),
            }),
            tool: tool(3.0, 0.0),
            side: 1.0,
        },
        Case {
            id: "helicoid p=0.3",
            surface: Box::new(Helicoid {
                pitch: 0.3,
                u: (0.2, 2.9),
                v: (12.0, 30.0),
            }),
            tool: tool(3.0, 0.0),
            side: 1.0,
        },
        Case {
            id: "spline bump 0.004",
            surface: Box::new(near_ruled(0.004)),
            tool: tool(3.0, 0.0),
            side: 1.0,
        },
        Case {
            id: "spline bump 0.05",
            surface: Box::new(near_ruled(0.05)),
            tool: tool(3.0, 0.0),
            side: 1.0,
        },
        Case {
            id: "coaxial cylinder",
            surface: Box::new(CoaxialCylinder {
                r: 10.0,
                u: (0.4, 2.7),
                v: (5.0, 25.0),
            }),
            tool: tool(3.0, 0.0),
            side: -1.0,
        },
        Case {
            id: "plane",
            surface: Box::new(Plane),
            tool: tool(3.0, 0.0),
            side: 1.0,
        },
        Case {
            id: "sphere",
            surface: Box::new(Sphere { r: 15.0 }),
            tool: tool(3.0, 0.0),
            side: -1.0,
        },
        Case {
            id: "torus",
            surface: Box::new(Torus {
                major: 20.0,
                minor: 6.0,
            }),
            tool: tool(3.0, 0.0),
            side: -1.0,
        },
    ]
}

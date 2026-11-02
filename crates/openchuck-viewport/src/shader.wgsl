// SPDX-License-Identifier: AGPL-3.0-or-later
// Viewport shaders (architecture section 5.8).
//
// Meshes carry positions only: the flat normal comes from the screen-space
// derivatives of the position, which saves the memory of a normal per vertex
// on large meshes. Shading is a two-sided headlight, so a part whose normals
// point inward still reads correctly.

struct Uniforms {
    view_proj: mat4x4<f32>,
    colour: vec4<f32>,
    // xyz: the eye (perspective, w = 1) or the direction toward the eye
    // (orthographic, w = 0).
    eye: vec4<f32>,
    // x: the width of a wide line in logical pixels; yz: the viewport's size
    // in logical pixels; w: physical pixels per logical pixel.
    line: vec4<f32>,
    // A line's dash pattern in logical pixels: on, off, on, off. All zero is
    // a solid line.
    pattern: vec4<f32>,
    // The colour of a mesh's hatch lines.
    hatch: vec4<f32>,
    // x: logical pixels per millimetre at the camera's target, which the
    // dash pattern is measured in; y: the hatch, 0 none, 1 one direction,
    // 2 both directions.
    scale: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
};

@vertex
fn vs_mesh(@location(0) position: vec3<f32>) -> MeshOut {
    var out: MeshOut;
    out.clip = u.view_proj * vec4<f32>(position, 1.0);
    out.world = position;
    return out;
}

fn shade(in: MeshOut) -> vec4<f32> {
    let n = normalize(cross(dpdx(in.world), dpdy(in.world)));
    let toward_eye = select(u.eye.xyz, normalize(u.eye.xyz - in.world), u.eye.w > 0.5);
    let light = abs(dot(n, toward_eye));
    return vec4<f32>(u.colour.rgb * (0.3 + 0.7 * light), u.colour.a);
}

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    return shade(in);
}

// The fill with a hatch over it: diagonal lines 1 logical pixel wide and 6
// apart, fixed to the screen so they keep their spacing at any zoom
// (specification 7.4 and 7.5).
@fragment
fn fs_hatch(in: MeshOut) -> @location(0) vec4<f32> {
    // Whole logical pixels along each diagonal, so the bands are exact.
    let p = in.clip.xy / max(u.line.w, 1e-3);
    var on = i32(floor(p.x + p.y)) % 6 == 0;
    if u.scale.y > 1.5 {
        // Offset so the difference is never negative.
        on = on || (i32(floor(p.x - p.y)) + 600000) % 6 == 0;
    }
    return select(shade(in), u.hatch, on);
}

@vertex
fn vs_line(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return u.view_proj * vec4<f32>(position, 1.0);
}

@fragment
fn fs_line() -> @location(0) vec4<f32> {
    return u.colour;
}

struct PathOut {
    @builtin(position) clip: vec4<f32>,
    // Millimetres along the line from its start.
    @location(0) along: f32,
};

@vertex
fn vs_path(@location(0) position: vec3<f32>, @location(1) along: f32) -> PathOut {
    var out: PathOut;
    out.clip = u.view_proj * vec4<f32>(position, 1.0);
    out.along = along;
    return out;
}

// A line in its dash pattern: the distance along it, in millimetres, is
// turned into logical pixels at the camera's target, so the dashes keep
// their size on screen as the view zooms.
@fragment
fn fs_path(in: PathOut) -> @location(0) vec4<f32> {
    let p = u.pattern;
    let period = p.x + p.y + p.z + p.w;
    if period > 0.0 {
        let t = fract(in.along * u.scale.x / period) * period;
        let first = t < p.x;
        let second = t >= p.x + p.y && t < p.x + p.y + p.z;
        if !(first || second) {
            discard;
        }
    }
    return u.colour;
}

// The corner of a quad round the segment from a to b, `width` logical
// pixels wide, in clip space; see vs_wide.
fn wide_corner(corner: u32, a: vec3<f32>, b: vec3<f32>, width: f32) -> vec4<f32> {
    var ca = u.view_proj * vec4<f32>(a, 1.0);
    var cb = u.view_proj * vec4<f32>(b, 1.0);
    let near = 1e-4;
    if ca.w < near && cb.w < near {
        return vec4<f32>(0.0, 0.0, 2.0, 1.0);
    }
    // A segment that crosses the eye plane is cut where it meets it.
    if ca.w < near {
        ca = mix(ca, cb, (near - ca.w) / (cb.w - ca.w));
    } else if cb.w < near {
        cb = mix(cb, ca, (near - cb.w) / (ca.w - cb.w));
    }
    let size = max(u.line.yz, vec2<f32>(1.0, 1.0));
    let sa = ca.xy / ca.w * 0.5 * size;
    let sb = cb.xy / cb.w * 0.5 * size;
    var along = sb - sa;
    if length(along) < 1e-6 {
        along = vec2<f32>(1.0, 0.0);
    }
    along = normalize(along);
    let side = vec2<f32>(-along.y, along.x);
    var ends = array<u32, 6>(0u, 1u, 0u, 0u, 1u, 1u);
    var sides = array<f32, 6>(-1.0, -1.0, 1.0, 1.0, -1.0, 1.0);
    let end = ends[corner];
    let across = sides[corner];
    let c = select(ca, cb, end == 1u);
    let radius = 0.5 * width;
    // Square ends: the quad reaches half a width past each end.
    let shift = (side * across + along * select(-1.0, 1.0, end == 1u)) * radius;
    let ndc = c.xy / c.w + shift * 2.0 / size;
    return vec4<f32>(ndc * c.w, c.z, c.w);
}

// A wide line: one quad for each segment, from the segment's two ends given
// per instance. The width is in logical pixels, so it does not change with
// the zoom. Six corners make two triangles: (a, -), (b, -), (a, +) and
// (a, +), (b, -), (b, +), where the sign is the side of the segment.
@vertex
fn vs_wide(
    @builtin(vertex_index) corner: u32,
    @location(0) a: vec3<f32>,
    @location(1) along_a: f32,
    @location(2) b: vec3<f32>,
    @location(3) along_b: f32,
) -> PathOut {
    var ends = array<u32, 6>(0u, 1u, 0u, 0u, 1u, 1u);
    var out: PathOut;
    out.clip = wide_corner(corner, a, b, u.line.x);
    out.along = select(along_a, along_b, ends[corner] == 1u);
    return out;
}

// An outline edge: drawn as a wide line where it is a silhouette, that is
// where one of the two faces meeting at it looks toward the eye and the
// other away, and always where both normals are zero (a crease or an open
// boundary). Elsewhere the quad collapses to nothing.
@vertex
fn vs_outline(
    @builtin(vertex_index) corner: u32,
    @location(0) a: vec3<f32>,
    @location(1) b: vec3<f32>,
    @location(2) n1: vec3<f32>,
    @location(3) n2: vec3<f32>,
    @location(4) along: f32,
) -> PathOut {
    var ends = array<u32, 6>(0u, 1u, 0u, 0u, 1u, 1u);
    var out: PathOut;
    out.along = select(along, along + distance(a, b), ends[corner] == 1u);
    let always = dot(n1, n1) < 1e-12 && dot(n2, n2) < 1e-12;
    let toward_eye = select(u.eye.xyz, u.eye.xyz - 0.5 * (a + b), u.eye.w > 0.5);
    let f1 = dot(n1, toward_eye);
    let f2 = dot(n2, toward_eye);
    if !always && f1 * f2 > 0.0 {
        out.clip = vec4<f32>(0.0, 0.0, 2.0, 1.0);
        return out;
    }
    out.clip = wide_corner(corner, a, b, u.line.x);
    return out;
}

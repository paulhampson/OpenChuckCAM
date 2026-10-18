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
    // in logical pixels.
    line: vec4<f32>,
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

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    let n = normalize(cross(dpdx(in.world), dpdy(in.world)));
    let toward_eye = select(u.eye.xyz, normalize(u.eye.xyz - in.world), u.eye.w > 0.5);
    let light = abs(dot(n, toward_eye));
    return vec4<f32>(u.colour.rgb * (0.3 + 0.7 * light), u.colour.a);
}

@vertex
fn vs_line(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return u.view_proj * vec4<f32>(position, 1.0);
}

@fragment
fn fs_line() -> @location(0) vec4<f32> {
    return u.colour;
}

// A wide line: one quad for each segment, from the segment's two ends given
// per instance. The width is in logical pixels, so it does not change with
// the zoom. Six corners make two triangles: (a, -), (b, -), (a, +) and
// (a, +), (b, -), (b, +), where the sign is the side of the segment.
@vertex
fn vs_wide(
    @builtin(vertex_index) corner: u32,
    @location(0) a: vec3<f32>,
    @location(1) b: vec3<f32>,
) -> @builtin(position) vec4<f32> {
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
    let radius = 0.5 * u.line.x;
    // Square ends: the quad reaches half a width past each end.
    let shift = (side * across + along * select(-1.0, 1.0, end == 1u)) * radius;
    let ndc = c.xy / c.w + shift * 2.0 / size;
    return vec4<f32>(ndc * c.w, c.z, c.w);
}

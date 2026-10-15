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

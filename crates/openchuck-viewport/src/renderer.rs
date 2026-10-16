// SPDX-License-Identifier: AGPL-3.0-or-later
//! The wgpu renderer: uploads a [`Scene`] and draws it into a render pass
//! (UI-02, UI-03, NFR-02, NFR-03).
//!
//! It knows nothing of the window toolkit: the front end hands it a device,
//! a queue and a render pass. With egui that is a paint callback inside the
//! egui pass (decision record 0002, "Findings for later slices"), so a
//! fallback toolkit (OI-11) would only change that glue.

use wgpu::util::DeviceExt;

use crate::camera::{
    Camera, IDENTITY, Mat4, Projection, multiply, rigid_inverse, to_f32, transform,
};
use crate::scene::{Colour, DisplayMode, Scene, Slot, wireframe_indices};

/// Bytes of one draw's uniforms: a 4 by 4 matrix, a colour and the eye.
const UNIFORM_SIZE: u64 = 96;

/// What a frame needs besides the scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The camera.
    pub camera: Camera,
    /// Viewport width over height.
    pub aspect: f64,
    /// How meshes are drawn.
    pub mode: DisplayMode,
    /// The rigid transform of each [`Slot`], indexed by [`Slot::index`]: the
    /// tool's placement and the workpiece's turn with A. Items in a slot are
    /// drawn as if their points were moved by it.
    pub transforms: [Mat4; Slot::COUNT],
}

impl Frame {
    /// Sets the transform of `slot`.
    pub fn set_transform(&mut self, slot: Slot, transform: Mat4) {
        if let Some(t) = self.transforms.get_mut(slot.index()) {
            *t = transform;
        }
    }

    /// A frame with every slot at the identity.
    pub fn new(camera: Camera, aspect: f64, mode: DisplayMode) -> Self {
        Self {
            camera,
            aspect,
            mode,
            transforms: [IDENTITY; Slot::COUNT],
        }
    }
}

/// The parts of the picture, uploaded and replaced separately so a change to
/// one never uploads the others again. Drawn in this order: opaque meshes,
/// then translucent meshes, then the highlights, then lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layer {
    /// The part, the stock outline and the guides.
    Scene,
    /// The simulated stock, usually translucent.
    Stock,
    /// Toolpaths.
    Paths,
    /// The tool, which moves with the playback position.
    Tool,
    /// Selection and hover highlights.
    Overlay,
}

impl Layer {
    /// Every layer, in drawing order.
    pub const ALL: [Self; 5] = [
        Self::Scene,
        Self::Stock,
        Self::Paths,
        Self::Tool,
        Self::Overlay,
    ];

    /// The layer's position in [`Layer::ALL`], for front ends that keep a
    /// record per layer.
    pub const fn index(self) -> usize {
        match self {
            Self::Scene => 0,
            Self::Stock => 1,
            Self::Paths => 2,
            Self::Tool => 3,
            Self::Overlay => 4,
        }
    }
}

/// One draw call's uniforms.
#[derive(Debug)]
struct Uniform {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    colour: Colour,
}

#[derive(Debug)]
struct GpuMesh {
    slot: Slot,
    translucent: bool,
    vertices: wgpu::Buffer,
    triangles: Option<(wgpu::Buffer, u32)>,
    feature_edges: Option<(wgpu::Buffer, u32)>,
    /// Built the first time the wireframe mode is drawn.
    wireframe: Option<(wgpu::Buffer, u32)>,
    /// Kept to build the wireframe from.
    indices: Vec<u32>,
    fill: Uniform,
    edges: Uniform,
}

#[derive(Debug)]
struct GpuLines {
    slot: Slot,
    vertices: wgpu::Buffer,
    count: u32,
    on_top: bool,
    uniform: Uniform,
}

/// Draws a scene. Create it once per device; replace the scene with
/// [`Renderer::set_scene`]; call [`Renderer::prepare`] then
/// [`Renderer::paint`] each frame.
#[derive(Debug)]
pub struct Renderer {
    layout: wgpu::BindGroupLayout,
    fill: wgpu::RenderPipeline,
    edges: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    lines_on_top: wgpu::RenderPipeline,
    highlight: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    layers: [GpuLayer; 5],
    mode: DisplayMode,
}

#[derive(Debug, Default)]
struct GpuLayer {
    meshes: Vec<GpuMesh>,
    lines: Vec<GpuLines>,
}

impl Renderer {
    /// Builds the pipelines for a colour target of `target_format` with a
    /// depth buffer of `depth_format` and `sample_count` samples.
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("openchuck viewport"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("openchuck viewport uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(UNIFORM_SIZE),
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("openchuck viewport"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x3];
        let buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: 12,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        })];
        let make = |label: &str,
                    vertex: &str,
                    fragment: &str,
                    topology: wgpu::PrimitiveTopology,
                    compare: wgpu::CompareFunction,
                    write: bool,
                    bias: wgpu::DepthBiasState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vertex),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    ..wgpu::PrimitiveState::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: wgpu::StencilState::default(),
                    bias,
                }),
                multisample: wgpu::MultisampleState {
                    count: sample_count.max(1),
                    ..wgpu::MultisampleState::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fragment),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        // Surfaces are pushed back a little so edges on them win the depth test.
        let push_back = wgpu::DepthBiasState {
            constant: 2,
            slope_scale: 1.5,
            clamp: 0.0,
        };
        let none = wgpu::DepthBiasState::default();
        // Highlights lie on the part's own surface: pulled forward so they win
        // the depth test over the pushed-back surface they cover.
        let pull_forward = wgpu::DepthBiasState {
            constant: -4,
            slope_scale: -2.0,
            clamp: 0.0,
        };
        let (triangles, line_list) = (
            wgpu::PrimitiveTopology::TriangleList,
            wgpu::PrimitiveTopology::LineList,
        );
        use wgpu::CompareFunction::{Always, Less, LessEqual};
        Self {
            fill: make(
                "fill", "vs_mesh", "fs_mesh", triangles, Less, true, push_back,
            ),
            edges: make(
                "edges", "vs_line", "fs_line", line_list, LessEqual, true, none,
            ),
            lines: make(
                "lines", "vs_line", "fs_line", line_list, LessEqual, true, none,
            ),
            lines_on_top: make(
                "guides", "vs_line", "fs_line", line_list, Always, false, none,
            ),
            highlight: make(
                "highlight",
                "vs_mesh",
                "fs_mesh",
                triangles,
                LessEqual,
                false,
                pull_forward,
            ),
            glass: make(
                "glass", "vs_mesh", "fs_mesh", triangles, Less, false, push_back,
            ),
            layout,
            layers: Default::default(),
            mode: DisplayMode::default(),
        }
    }

    /// Replaces the scene ([`Layer::Scene`]), uploading it to the GPU.
    pub fn set_scene(&mut self, device: &wgpu::Device, scene: &Scene) {
        self.set_layer(device, Layer::Scene, scene);
    }

    /// Replaces the highlights ([`Layer::Overlay`]). Its meshes win the depth
    /// test over the surface they lie on (a selection or a hover highlight)
    /// and are still hidden by anything in front of them. Small and quick to
    /// replace, so moving the pointer does not upload the scene again.
    pub fn set_overlay(&mut self, device: &wgpu::Device, overlay: &Scene) {
        self.set_layer(device, Layer::Overlay, overlay);
    }

    /// Replaces one layer, uploading it to the GPU.
    pub fn set_layer(&mut self, device: &wgpu::Device, layer: Layer, scene: &Scene) {
        let uploaded = self.upload(device, scene);
        if let Some(slot) = self.layers.get_mut(layer.index()) {
            *slot = uploaded;
        }
    }

    fn upload(&self, device: &wgpu::Device, scene: &Scene) -> GpuLayer {
        let meshes = scene
            .meshes
            .iter()
            .filter(|m| !m.positions.is_empty())
            .map(|m| GpuMesh {
                slot: m.slot,
                translucent: m.translucent,
                vertices: vertex_buffer(device, "mesh vertices", &m.positions),
                triangles: index_buffer(device, "mesh triangles", &m.indices),
                feature_edges: index_buffer(device, "mesh feature edges", &m.feature_edges),
                wireframe: None,
                indices: m.indices.clone(),
                fill: self.uniform(device, m.colour),
                edges: self.uniform(device, m.edge_colour),
            })
            .collect();
        let lines = scene
            .lines
            .iter()
            .filter(|l| l.segments.len() >= 2)
            .map(|l| GpuLines {
                slot: l.slot,
                vertices: vertex_buffer(device, "lines", &l.segments),
                // An odd last point has no partner and is left out.
                count: u32::try_from(l.segments.len() & !1).unwrap_or(!1),
                on_top: l.on_top,
                uniform: self.uniform(device, l.colour),
            })
            .collect();
        GpuLayer { meshes, lines }
    }

    /// Writes the frame's camera and slot transforms to the GPU, and builds
    /// the wireframe the first time it is needed.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &Frame) {
        self.mode = frame.mode;
        let view_projection = frame.camera.view_projection(frame.aspect);
        let eye = match frame.camera.projection {
            Projection::Perspective => frame.camera.eye(),
            Projection::Orthographic => frame.camera.basis().2,
        };
        let w = match frame.camera.projection {
            Projection::Perspective => 1.0,
            Projection::Orthographic => 0.0,
        };
        // Per slot: the matrix that takes the item's own points to the screen,
        // and the eye in the item's own frame, so the shading follows the
        // item when it moves.
        #[allow(clippy::cast_possible_truncation)]
        let per_slot: Vec<([[f32; 4]; 4], [f32; 3])> = frame
            .transforms
            .iter()
            .map(|t| {
                let matrix = to_f32(&multiply(&view_projection, t));
                let local = transform(&rigid_inverse(t), [eye[0], eye[1], eye[2], w]);
                (matrix, [local[0] as f32, local[1] as f32, local[2] as f32])
            })
            .collect();
        #[allow(clippy::cast_possible_truncation)]
        let write = |u: &Uniform, slot: Slot| {
            let Some((matrix, eye)) = per_slot.get(slot.index()) else {
                return;
            };
            let mut floats = Vec::with_capacity(24);
            floats.extend(matrix.iter().flatten());
            floats.extend(u.colour);
            floats.extend(eye);
            floats.push(w as f32);
            queue.write_buffer(&u.buffer, 0, &f32_bytes(&floats));
        };
        for layer in &mut self.layers {
            for mesh in &mut layer.meshes {
                write(&mesh.fill, mesh.slot);
                write(&mesh.edges, mesh.slot);
                if frame.mode == DisplayMode::Wireframe && mesh.wireframe.is_none() {
                    mesh.wireframe =
                        index_buffer(device, "wireframe", &wireframe_indices(&mesh.indices));
                }
            }
            for lines in &layer.lines {
                write(&lines.uniform, lines.slot);
            }
        }
    }

    /// Draws the scene into `pass`, which must have the colour and depth
    /// formats the renderer was built for.
    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        // Opaque meshes of every layer but the highlights.
        for layer in Layer::ALL.iter().filter(|l| **l != Layer::Overlay) {
            for mesh in self.layer(*layer).meshes.iter().filter(|m| !m.translucent) {
                self.paint_mesh(pass, mesh);
            }
        }
        // Translucent meshes over them, without hiding what is behind.
        for layer in Layer::ALL {
            for mesh in self.layer(layer).meshes.iter().filter(|m| m.translucent) {
                if let Some((buffer, count)) = &mesh.triangles {
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_pipeline(&self.glass);
                    pass.set_bind_group(0, &mesh.fill.bind_group, &[]);
                    pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*count, 0, 0..1);
                }
            }
        }
        // Highlights over the surfaces they cover.
        for mesh in self
            .layer(Layer::Overlay)
            .meshes
            .iter()
            .filter(|m| !m.translucent)
        {
            if let Some((buffer, count)) = &mesh.triangles {
                pass.set_pipeline(&self.highlight);
                pass.set_bind_group(0, &mesh.fill.bind_group, &[]);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*count, 0, 0..1);
            }
        }
        // Lines tested against depth first, then the guides drawn over everything.
        for on_top in [false, true] {
            for layer in Layer::ALL {
                for lines in self
                    .layer(layer)
                    .lines
                    .iter()
                    .filter(|l| l.on_top == on_top)
                {
                    pass.set_pipeline(if on_top {
                        &self.lines_on_top
                    } else {
                        &self.lines
                    });
                    pass.set_bind_group(0, &lines.uniform.bind_group, &[]);
                    pass.set_vertex_buffer(0, lines.vertices.slice(..));
                    pass.draw(0..lines.count, 0..1);
                }
            }
        }
    }

    fn layer(&self, layer: Layer) -> &GpuLayer {
        static EMPTY: GpuLayer = GpuLayer {
            meshes: Vec::new(),
            lines: Vec::new(),
        };
        self.layers.get(layer.index()).unwrap_or(&EMPTY)
    }

    fn paint_mesh(&self, pass: &mut wgpu::RenderPass<'_>, mesh: &GpuMesh) {
        pass.set_vertex_buffer(0, mesh.vertices.slice(..));
        match self.mode {
            DisplayMode::Shaded | DisplayMode::ShadedWithEdges => {
                if let Some((buffer, count)) = &mesh.triangles {
                    pass.set_pipeline(&self.fill);
                    pass.set_bind_group(0, &mesh.fill.bind_group, &[]);
                    pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*count, 0, 0..1);
                }
                if self.mode == DisplayMode::ShadedWithEdges
                    && let Some((buffer, count)) = &mesh.feature_edges
                {
                    pass.set_pipeline(&self.edges);
                    pass.set_bind_group(0, &mesh.edges.bind_group, &[]);
                    pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*count, 0, 0..1);
                }
            }
            DisplayMode::Wireframe => {
                if let Some((buffer, count)) = &mesh.wireframe {
                    pass.set_pipeline(&self.edges);
                    pass.set_bind_group(0, &mesh.edges.bind_group, &[]);
                    pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*count, 0, 0..1);
                }
            }
        }
    }

    fn uniform(&self, device: &wgpu::Device, colour: Colour) -> Uniform {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport uniforms"),
            size: UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport uniforms"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Uniform {
            buffer,
            bind_group,
            colour,
        }
    }
}

fn vertex_buffer(device: &wgpu::Device, label: &str, positions: &[[f32; 3]]) -> wgpu::Buffer {
    let floats: Vec<f32> = positions.iter().flatten().copied().collect();
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: &f32_bytes(&floats),
        usage: wgpu::BufferUsages::VERTEX,
    })
}

/// An index buffer and its index count, or `None` when there are no indices.
fn index_buffer(
    device: &wgpu::Device,
    label: &str,
    indices: &[u32],
) -> Option<(wgpu::Buffer, u32)> {
    let count = u32::try_from(indices.len()).ok().filter(|c| *c > 0)?;
    let bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: &bytes,
        usage: wgpu::BufferUsages::INDEX,
    });
    Some((buffer, count))
}

/// Little-endian bytes, as the GPU reads them on every platform wgpu targets.
fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

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
use crate::scene::{
    Colour, DisplayMode, Hatch, Scene, Slot, batch_lines, outline_edges, wireframe_indices,
};

/// Bytes of one draw's uniforms: a 4 by 4 matrix, then six vectors of four:
/// the colour, the eye, the line width and viewport, the dash pattern, the
/// hatch colour, and the scale and hatch kind.
const UNIFORM_SIZE: u64 = 160;

/// What a frame needs besides the scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The camera.
    pub camera: Camera,
    /// Viewport width over height.
    pub aspect: f64,
    /// How meshes are drawn.
    pub mode: DisplayMode,
    /// The viewport's size in logical pixels, which a wide line
    /// ([`crate::LineItem::width`]) is measured in.
    pub viewport: [f32; 2],
    /// The rigid transform of each [`Slot`], indexed by [`Slot::index`]: the
    /// tool's placement and the workpiece's turn with A. Items in a slot are
    /// drawn as if their points were moved by it.
    pub transforms: [Mat4; Slot::COUNT],
    /// Physical pixels per logical pixel, which a screen-space hatch
    /// ([`crate::Hatch`]) is measured with.
    pub pixels_per_point: f32,
}

impl Frame {
    /// Logical pixels per millimetre at the camera's target: the scale a
    /// dash pattern ([`crate::LineStyle`]) is drawn at, so its dashes keep
    /// their size on screen as the view zooms.
    pub fn pixels_per_mm(&self) -> f32 {
        let height = self.camera.visible_height();
        if height > 0.0 && height.is_finite() {
            (f64::from(self.viewport[1]) / height) as f32
        } else {
            1.0
        }
    }

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
            // Until the front end says, the view is taken as 1000 pixels wide.
            viewport: [1000.0, (1000.0 / aspect.max(1e-6)) as f32],
            transforms: [IDENTITY; Slot::COUNT],
            pixels_per_point: 1.0,
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
    /// A faint trace of the toolpath behind [`Layer::Paths`], replaced only
    /// when the path changes (the playback window, issue 205).
    Trace,
    /// Toolpaths.
    Paths,
    /// The tool, which moves with the playback position.
    Tool,
    /// Selection and hover highlights.
    Overlay,
    /// The outlines of the hovered or selected held region or tab, kept
    /// apart from the selection highlights so the two are replaced
    /// separately.
    Focus,
}

impl Layer {
    /// Every layer, in drawing order.
    pub const ALL: [Self; 7] = [
        Self::Scene,
        Self::Stock,
        Self::Trace,
        Self::Paths,
        Self::Tool,
        Self::Overlay,
        Self::Focus,
    ];

    /// The layer's position in [`Layer::ALL`], for front ends that keep a
    /// record per layer.
    pub const fn index(self) -> usize {
        match self {
            Self::Scene => 0,
            Self::Stock => 1,
            Self::Trace => 2,
            Self::Paths => 3,
            Self::Tool => 4,
            Self::Overlay => 5,
            Self::Focus => 6,
        }
    }
}

/// What one layer holds on the GPU, for the benchmark and the tests of the
/// cost of a frame (NFR-03; issue #200).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LayerStats {
    /// Draw calls for its lines: one per look ([`crate::LineItem`]s that
    /// differ only in their points share one).
    pub line_draws: usize,
    /// Meshes uploaded.
    pub meshes: usize,
    /// GPU buffers: vertices, indices and uniforms.
    pub buffers: usize,
    /// Bytes in those buffers.
    pub bytes: u64,
}

/// One draw call's uniforms.
#[derive(Debug)]
struct Uniform {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    colour: Colour,
    /// A wide line's width; zero for everything else.
    width: f32,
    /// A line's dash pattern ([`crate::LineStyle::pattern`]).
    pattern: [f32; 4],
    /// A mesh's hatch.
    hatch: Option<Hatch>,
}

/// What a draw's uniforms hold besides the matrix and the eye.
#[derive(Debug, Clone, Copy, Default)]
struct Look {
    colour: Colour,
    width: f32,
    pattern: [f32; 4],
    hatch: Option<Hatch>,
}

impl Look {
    fn colour(colour: Colour) -> Self {
        Self {
            colour,
            ..Self::default()
        }
    }
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
    /// Only the outline is drawn.
    outline_only: bool,
    /// The outline edges, their count and their uniforms.
    outline: Option<(wgpu::Buffer, u32, Uniform)>,
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
    wide: wgpu::RenderPipeline,
    wide_on_top: wgpu::RenderPipeline,
    highlight: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    fill_hatch: wgpu::RenderPipeline,
    glass_hatch: wgpu::RenderPipeline,
    highlight_hatch: wgpu::RenderPipeline,
    outline: wgpu::RenderPipeline,
    layers: [GpuLayer; Layer::ALL.len()],
    mode: DisplayMode,
    /// Uniform buffers written by the last [`Renderer::prepare`].
    uniform_writes: usize,
    /// Whether each layer is drawn, by [`Layer::index`].
    shown: [bool; Layer::ALL.len()],
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
        // A line's point carries its distance along the line, for the dashes.
        let path_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32];
        let path_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &path_attributes,
        })];
        // A wide line reads the two ends of its segment, with their
        // distances, as one instance.
        let wide_attributes = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32, 2 => Float32x3, 3 => Float32
        ];
        let wide_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: 32,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wide_attributes,
        })];
        // An outline edge: its two ends, the normals of its two faces and its
        // distance along the outline.
        let outline_attributes = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3, 4 => Float32
        ];
        let outline_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: 52,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &outline_attributes,
        })];
        let make = |label: &str,
                    vertex: &str,
                    fragment: &str,
                    topology: wgpu::PrimitiveTopology,
                    compare: wgpu::CompareFunction,
                    write: bool,
                    bias: wgpu::DepthBiasState| {
            let buffers = match vertex {
                "vs_wide" => &wide_buffers,
                "vs_path" => &path_buffers,
                "vs_outline" => &outline_buffers,
                _ => &buffers,
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vertex),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers,
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
                "lines", "vs_path", "fs_path", line_list, LessEqual, true, none,
            ),
            lines_on_top: make(
                "guides", "vs_path", "fs_path", line_list, Always, false, none,
            ),
            wide: make(
                "wide lines",
                "vs_wide",
                "fs_path",
                triangles,
                LessEqual,
                true,
                none,
            ),
            wide_on_top: make(
                "wide guides",
                "vs_wide",
                "fs_path",
                triangles,
                Always,
                false,
                none,
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
            fill_hatch: make(
                "hatched fill",
                "vs_mesh",
                "fs_hatch",
                triangles,
                Less,
                true,
                push_back,
            ),
            highlight_hatch: make(
                "hatched highlight",
                "vs_mesh",
                "fs_hatch",
                triangles,
                LessEqual,
                false,
                pull_forward,
            ),
            glass_hatch: make(
                "hatched glass",
                "vs_mesh",
                "fs_hatch",
                triangles,
                Less,
                false,
                push_back,
            ),
            outline: make(
                "outline",
                "vs_outline",
                "fs_path",
                triangles,
                LessEqual,
                true,
                none,
            ),
            layout,
            layers: Default::default(),
            mode: DisplayMode::default(),
            uniform_writes: 0,
            shown: [true; Layer::ALL.len()],
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
                fill: self.uniform(
                    device,
                    Look {
                        hatch: m.hatch,
                        ..Look::colour(m.colour)
                    },
                ),
                edges: self.uniform(device, Look::colour(m.edge_colour)),
                outline_only: m.outline_only,
                outline: m.outline.and_then(|o| {
                    let edges = outline_edges(&m.positions, &m.indices);
                    let floats: Vec<f32> = edges.iter().flat_map(|e| e.floats()).collect();
                    let count = u32::try_from(edges.len()).ok().filter(|c| *c > 0)?;
                    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("outline edges"),
                        contents: &f32_bytes(&floats),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let look = Look {
                        width: o.width,
                        pattern: o.style.pattern(),
                        ..Look::colour(o.colour)
                    };
                    Some((buffer, count, self.uniform(device, look)))
                }),
            })
            .collect();
        // Lines that look alike are one buffer and one draw (issue #200).
        let lines = batch_lines(&scene.lines)
            .into_iter()
            .map(|b| {
                let floats: Vec<f32> = b.vertices.iter().flatten().copied().collect();
                GpuLines {
                    slot: b.look.slot,
                    vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("lines"),
                        contents: &f32_bytes(&floats),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                    count: u32::try_from(b.vertices.len() & !1).unwrap_or(!1),
                    on_top: b.look.on_top,
                    uniform: self.uniform(
                        device,
                        Look {
                            width: b.look.width,
                            pattern: b.look.style.pattern(),
                            ..Look::colour(b.look.colour)
                        },
                    ),
                }
            })
            .collect();
        GpuLayer { meshes, lines }
    }

    /// Shows or hides a layer. A hidden layer keeps what it uploaded and is
    /// left out of the frame, so showing it again uploads nothing (NFR-04:
    /// a path of a million points took 96 ms to upload; issue #200).
    pub fn set_layer_shown(&mut self, layer: Layer, shown: bool) {
        if let Some(s) = self.shown.get_mut(layer.index()) {
            *s = shown;
        }
    }

    /// Whether a layer is drawn.
    pub fn is_layer_shown(&self, layer: Layer) -> bool {
        self.shown.get(layer.index()).copied().unwrap_or(false)
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
        let per_mm = frame.pixels_per_mm();
        let writes = std::cell::Cell::new(0usize);
        #[allow(clippy::cast_possible_truncation)]
        let write = |u: &Uniform, slot: Slot| {
            let Some((matrix, eye)) = per_slot.get(slot.index()) else {
                return;
            };
            writes.set(writes.get() + 1);
            let (hatch, kind) = match u.hatch {
                Some(h) => (h.colour, if h.cross { 2.0 } else { 1.0 }),
                None => ([0.0; 4], 0.0),
            };
            let mut floats = Vec::with_capacity(40);
            floats.extend(matrix.iter().flatten());
            floats.extend(u.colour);
            floats.extend(eye);
            floats.push(w as f32);
            floats.extend([
                u.width,
                frame.viewport[0],
                frame.viewport[1],
                frame.pixels_per_point,
            ]);
            floats.extend(u.pattern);
            floats.extend(hatch);
            floats.extend([per_mm, kind, 0.0, 0.0]);
            queue.write_buffer(&u.buffer, 0, &f32_bytes(&floats));
        };
        for (layer, _) in self.layers.iter_mut().zip(self.shown).filter(|(_, s)| *s) {
            for mesh in &mut layer.meshes {
                write(&mesh.fill, mesh.slot);
                write(&mesh.edges, mesh.slot);
                if let Some((_, _, uniform)) = &mesh.outline {
                    write(uniform, mesh.slot);
                }
                if frame.mode == DisplayMode::Wireframe && mesh.wireframe.is_none() {
                    mesh.wireframe =
                        index_buffer(device, "wireframe", &wireframe_indices(&mesh.indices));
                }
            }
            for lines in &layer.lines {
                write(&lines.uniform, lines.slot);
            }
        }
        self.uniform_writes = writes.get();
    }

    /// Uniform buffers written by the last [`Renderer::prepare`]: one per
    /// mesh fill, mesh edges, outline and line draw.
    pub fn uniform_writes(&self) -> usize {
        self.uniform_writes
    }

    /// What `layer` holds on the GPU.
    pub fn layer_stats(&self, layer: Layer) -> LayerStats {
        let layer = self.layer(layer);
        let mut stats = LayerStats {
            line_draws: layer.lines.len(),
            meshes: layer.meshes.len(),
            ..LayerStats::default()
        };
        let mut add = |buffer: &wgpu::Buffer| {
            stats.buffers += 1;
            stats.bytes += buffer.size();
        };
        for mesh in &layer.meshes {
            add(&mesh.vertices);
            for (buffer, _) in [&mesh.triangles, &mesh.feature_edges, &mesh.wireframe]
                .into_iter()
                .flatten()
            {
                add(buffer);
            }
            add(&mesh.fill.buffer);
            add(&mesh.edges.buffer);
            if let Some((buffer, _, uniform)) = &mesh.outline {
                add(buffer);
                add(&uniform.buffer);
            }
        }
        for lines in &layer.lines {
            add(&lines.vertices);
            add(&lines.uniform.buffer);
        }
        stats
    }

    /// Draws the scene into `pass`, which must have the colour and depth
    /// formats the renderer was built for.
    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        let highlights = |l: &Layer| matches!(l, Layer::Overlay | Layer::Focus);
        // Opaque meshes of every layer but the highlights.
        for layer in Layer::ALL.iter().filter(|l| !highlights(l)) {
            for mesh in self
                .drawn(*layer)
                .meshes
                .iter()
                .filter(|m| !m.translucent && !m.outline_only)
            {
                self.paint_mesh(pass, mesh);
            }
        }
        // Translucent meshes over them, without hiding what is behind.
        for layer in Layer::ALL {
            for mesh in self
                .drawn(layer)
                .meshes
                .iter()
                .filter(|m| m.translucent && !m.outline_only)
            {
                if let Some((buffer, count)) = &mesh.triangles {
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_pipeline(if mesh.fill.hatch.is_some() {
                        &self.glass_hatch
                    } else {
                        &self.glass
                    });
                    pass.set_bind_group(0, &mesh.fill.bind_group, &[]);
                    pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*count, 0, 0..1);
                }
            }
        }
        // Highlights over the surfaces they cover.
        for layer in Layer::ALL.iter().filter(|l| highlights(l)) {
            for mesh in self
                .drawn(*layer)
                .meshes
                .iter()
                .filter(|m| !m.translucent && !m.outline_only)
            {
                if let Some((buffer, count)) = &mesh.triangles {
                    pass.set_pipeline(if mesh.fill.hatch.is_some() {
                        &self.highlight_hatch
                    } else {
                        &self.highlight
                    });
                    pass.set_bind_group(0, &mesh.fill.bind_group, &[]);
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*count, 0, 0..1);
                }
            }
        }
        // Outlines, tested against depth like the lines.
        for layer in Layer::ALL {
            for mesh in &self.drawn(layer).meshes {
                if let Some((buffer, count, uniform)) = &mesh.outline {
                    pass.set_pipeline(&self.outline);
                    pass.set_bind_group(0, &uniform.bind_group, &[]);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..6, 0..*count);
                }
            }
        }
        // Lines tested against depth first, then the guides drawn over everything.
        for on_top in [false, true] {
            for layer in Layer::ALL {
                for lines in self
                    .drawn(layer)
                    .lines
                    .iter()
                    .filter(|l| l.on_top == on_top)
                {
                    let wide = lines.uniform.width > 1.0;
                    pass.set_pipeline(match (wide, on_top) {
                        (false, false) => &self.lines,
                        (false, true) => &self.lines_on_top,
                        (true, false) => &self.wide,
                        (true, true) => &self.wide_on_top,
                    });
                    pass.set_bind_group(0, &lines.uniform.bind_group, &[]);
                    pass.set_vertex_buffer(0, lines.vertices.slice(..));
                    if wide {
                        // Six corners for each segment.
                        pass.draw(0..6, 0..lines.count / 2);
                    } else {
                        pass.draw(0..lines.count, 0..1);
                    }
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

    /// The layer as the frame draws it: empty while it is hidden.
    fn drawn(&self, layer: Layer) -> &GpuLayer {
        static EMPTY: GpuLayer = GpuLayer {
            meshes: Vec::new(),
            lines: Vec::new(),
        };
        if self.is_layer_shown(layer) {
            self.layer(layer)
        } else {
            &EMPTY
        }
    }

    fn paint_mesh(&self, pass: &mut wgpu::RenderPass<'_>, mesh: &GpuMesh) {
        pass.set_vertex_buffer(0, mesh.vertices.slice(..));
        match self.mode {
            DisplayMode::Shaded | DisplayMode::ShadedWithEdges => {
                if let Some((buffer, count)) = &mesh.triangles {
                    pass.set_pipeline(if mesh.fill.hatch.is_some() {
                        &self.fill_hatch
                    } else {
                        &self.fill
                    });
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

    fn uniform(&self, device: &wgpu::Device, look: Look) -> Uniform {
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
            colour: look.colour,
            width: look.width,
            pattern: look.pattern,
            hatch: look.hatch,
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

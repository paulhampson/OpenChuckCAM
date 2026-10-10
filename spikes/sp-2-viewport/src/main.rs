//! SP-2 viewport spike (work breakdown, Spikes). Throwaway.
//!
//! Question: does egui with `egui_wgpu` hold 30 fps on 1M triangles, with native graphics?
//! Contents: window, wgpu indexed mesh draw (with optional edge overlay), orbit/pan/zoom camera,
//! one polyline layer, and CPU BVH picking (parry3d-f64 TriMesh).
//!
//! Usage:
//!   sp2-viewport                     interactive (drag = orbit, right/shift-drag = pan, wheel = zoom, click = pick)
//!   sp2-viewport --bench [secs]      scripted camera path (orbit, pan, zoom), print frame-time stats, exit
//!   options: --tris N (default 1000000)  --wire (edge overlay)  --novsync  --out FILE (append one JSON line)

use std::time::{Duration, Instant};

use eframe::{egui, egui_wgpu, wgpu};
use egui_wgpu::wgpu::util::DeviceExt;
use parry3d_f64::math::Vector as PVec;
use parry3d_f64::query::{Ray, RayCast};
use parry3d_f64::shape::TriMesh;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

// ---------------------------------------------------------------- mesh generation

struct MeshData {
    pos_f64: Vec<[f64; 3]>,
    gpu_vertices: Vec<GpuVertex>, // position + normal
    triangles: Vec<[u32; 3]>,
    edge_indices: Vec<u32>, // line list over the grid edges
    polyline: Vec<[f32; 3]>,
    extent: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    pos: [f32; 3],
    normal: [f32; 3],
}

/// A bumpy rotary part about the X axis: radius varies with angle and length.
fn radius(theta: f64, x: f64) -> f64 {
    let end = ((x * 0.06).sin().abs()).min(1.0);
    25.0 + 2.5 * (7.0 * theta + 0.35 * x).sin() * end + 1.2 * (0.5 * x).sin()
}

fn build_mesh(target_tris: usize) -> MeshData {
    // n x n quads, two triangles each; the angular direction wraps.
    let n = ((target_tris as f64 / 2.0).sqrt().round() as usize).max(4);
    let (nu, nv) = (n, n); // nu around, nv along
    let length = 120.0;
    let mut pos_f64 = Vec::with_capacity(nu * (nv + 1));
    for j in 0..=nv {
        let x = length * (j as f64 / nv as f64);
        for i in 0..nu {
            let th = std::f64::consts::TAU * (i as f64 / nu as f64);
            let r = radius(th, x);
            pos_f64.push([x - length / 2.0, r * th.cos(), r * th.sin()]);
        }
    }
    let idx = |i: usize, j: usize| (j * nu + (i % nu)) as u32;
    let mut triangles = Vec::with_capacity(2 * nu * nv);
    for j in 0..nv {
        for i in 0..nu {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            triangles.push([a, b, c]);
            triangles.push([a, c, d]);
        }
    }
    // Smooth normals by area-weighted accumulation.
    let mut nrm = vec![[0.0f64; 3]; pos_f64.len()];
    for t in &triangles {
        let (p0, p1, p2) = (pos_f64[t[0] as usize], pos_f64[t[1] as usize], pos_f64[t[2] as usize]);
        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let c = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        for &v in t {
            for k in 0..3 {
                nrm[v as usize][k] += c[k];
            }
        }
    }
    let gpu_vertices = pos_f64
        .iter()
        .zip(&nrm)
        .map(|(p, n)| {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
            GpuVertex {
                pos: [p[0] as f32, p[1] as f32, p[2] as f32],
                normal: [(n[0] / l) as f32, (n[1] / l) as f32, (n[2] / l) as f32],
            }
        })
        .collect();
    // Edge overlay: every grid edge, including diagonals (worst case: full triangle wireframe).
    let mut edge_indices = Vec::with_capacity(6 * nu * nv);
    for j in 0..=nv {
        for i in 0..nu {
            edge_indices.extend([idx(i, j), idx(i + 1, j)]);
            if j < nv {
                edge_indices.extend([idx(i, j), idx(i, j + 1)]);
                edge_indices.extend([idx(i + 1, j), idx(i, j + 1)]);
            }
        }
    }
    // Polyline layer: a helical wrap path (the toolpath stand-in), 300k points, just above the surface.
    let pts = 300_000usize;
    let turns = 120.0;
    let polyline = (0..pts)
        .map(|k| {
            let s = k as f64 / (pts - 1) as f64;
            let x = length * s;
            let th = std::f64::consts::TAU * turns * s;
            let r = radius(th, x) + 0.15;
            [(x - length / 2.0) as f32, (r * th.cos()) as f32, (r * th.sin()) as f32]
        })
        .collect();
    MeshData { pos_f64, gpu_vertices, triangles, edge_indices, polyline, extent: 70.0 }
}

// ---------------------------------------------------------------- camera

#[derive(Clone, Copy)]
struct Camera {
    target: [f32; 3],
    yaw: f32,
    pitch: f32,
    dist: f32,
}

type M4 = [[f32; 4]; 4]; // column-major

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt().max(1e-20);
    [a[0] / l, a[1] / l, a[2] / l]
}
fn mul(a: M4, b: M4) -> M4 {
    let mut r = [[0.0; 4]; 4];
    for c in 0..4 {
        for row in 0..4 {
            r[c][row] = (0..4).map(|k| a[k][row] * b[c][k]).sum();
        }
    }
    r
}

const FOV_Y: f32 = 45.0 * std::f32::consts::PI / 180.0;

impl Camera {
    fn eye(&self) -> [f32; 3] {
        let (sy, cy, sp, cp) = (self.yaw.sin(), self.yaw.cos(), self.pitch.sin(), self.pitch.cos());
        [
            self.target[0] + self.dist * cp * cy,
            self.target[1] + self.dist * sp,
            self.target[2] + self.dist * cp * sy,
        ]
    }
    /// Right, up, forward basis (right-handed, Y up).
    fn basis(&self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let eye = self.eye();
        let f = norm(sub(self.target, eye));
        let r = norm(cross(f, [0.0, 1.0, 0.0]));
        let u = cross(r, f);
        (r, u, f)
    }
    fn view_proj(&self, aspect: f32) -> M4 {
        let eye = self.eye();
        let (r, u, f) = self.basis();
        let view: M4 = [
            [r[0], u[0], -f[0], 0.0],
            [r[1], u[1], -f[1], 0.0],
            [r[2], u[2], -f[2], 0.0],
            [-dot(r, eye), -dot(u, eye), dot(f, eye), 1.0],
        ];
        let (near, far) = ((self.dist * 0.02).max(0.05), self.dist * 4.0 + 400.0);
        let t = 1.0 / (FOV_Y / 2.0).tan();
        // Right-handed, depth range 0..1 (wgpu).
        let proj: M4 = [
            [t / aspect, 0.0, 0.0, 0.0],
            [0.0, t, 0.0, 0.0],
            [0.0, 0.0, far / (near - far), -1.0],
            [0.0, 0.0, near * far / (near - far), 0.0],
        ];
        mul(proj, view)
    }
    /// World-space ray through a normalised device point (x right, y up, both -1..1).
    fn ray(&self, ndc: [f32; 2], aspect: f32) -> ([f64; 3], [f64; 3]) {
        let (r, u, f) = self.basis();
        let th = (FOV_Y / 2.0).tan();
        let d = norm([
            f[0] + r[0] * ndc[0] * th * aspect + u[0] * ndc[1] * th,
            f[1] + r[1] * ndc[0] * th * aspect + u[1] * ndc[1] * th,
            f[2] + r[2] * ndc[0] * th * aspect + u[2] * ndc[1] * th,
        ]);
        let e = self.eye();
        ([e[0] as f64, e[1] as f64, e[2] as f64], [d[0] as f64, d[1] as f64, d[2] as f64])
    }
}

// ---------------------------------------------------------------- GPU resources

const SHADER: &str = r#"
struct U { view_proj: mat4x4<f32>, color: vec4<f32>, eye: vec4<f32> };
@group(0) @binding(0) var<uniform> u: U;

struct MeshOut { @builtin(position) clip: vec4<f32>, @location(0) n: vec3<f32>, @location(1) p: vec3<f32> };

@vertex fn vs_mesh(@location(0) pos: vec3<f32>, @location(1) n: vec3<f32>) -> MeshOut {
    var o: MeshOut;
    o.clip = u.view_proj * vec4<f32>(pos, 1.0);
    o.n = n;
    o.p = pos;
    return o;
}
@fragment fn fs_mesh(i: MeshOut) -> @location(0) vec4<f32> {
    let n = normalize(i.n);
    let v = normalize(u.eye.xyz - i.p);
    let ndl = abs(dot(n, v));            // headlight, two-sided
    let c = u.color.rgb * (0.25 + 0.75 * ndl);
    return vec4<f32>(c, 1.0);
}

@vertex fn vs_line(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    return u.view_proj * vec4<f32>(pos, 1.0);
}
@fragment fn fs_line() -> @location(0) vec4<f32> { return u.color; }
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    color: [f32; 4],
    eye: [f32; 4],
}

struct Layer {
    ubuf: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

struct GpuScene {
    mesh_pipe: wgpu::RenderPipeline,
    wire_pipe: wgpu::RenderPipeline,
    strip_pipe: wgpu::RenderPipeline,
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    index_count: u32,
    ebuf: wgpu::Buffer,
    edge_count: u32,
    pbuf: wgpu::Buffer,
    poly_count: u32,
    mesh_l: Layer,
    wire_l: Layer,
    poly_l: Layer,
}

fn init_gpu(rs: &egui_wgpu::RenderState, m: &MeshData) -> GpuScene {
    let device = &rs.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sp2"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("sp2-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("sp2-pl"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
    let line_attrs = wgpu::vertex_attr_array![0 => Float32x3];
    let make = |label: &str,
                vs: &str,
                fs: &str,
                buffers: &[Option<wgpu::VertexBufferLayout>],
                topo: wgpu::PrimitiveTopology,
                bias: wgpu::DepthBiasState,
                compare: wgpu::CompareFunction| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some(vs),
                compilation_options: Default::default(),
                buffers,
            },
            primitive: wgpu::PrimitiveState { topology: topo, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(compare),
                stencil: Default::default(),
                bias,
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[Some(rs.target_format.into())],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let mesh_buf = [Some(wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &mesh_attrs,
    })];
    let line_buf = [Some(wgpu::VertexBufferLayout {
        array_stride: 12,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &line_attrs,
    })];
    // Fill is pushed back slightly so the edge and toolpath lines win the depth test.
    let push_back = wgpu::DepthBiasState { constant: 2, slope_scale: 1.5, clamp: 0.0 };
    let mesh_pipe = make("mesh", "vs_mesh", "fs_mesh", &mesh_buf, wgpu::PrimitiveTopology::TriangleList, push_back, wgpu::CompareFunction::Less);
    // Edge overlay: the mesh's own vertex buffer, position only (stride 24, attribute 0), line list.
    let wire_pipe = make("wire", "vs_line", "fs_line", &mesh_buf_pos_only(), wgpu::PrimitiveTopology::LineList, Default::default(), wgpu::CompareFunction::LessEqual);
    let strip_pipe = make("strip", "vs_line", "fs_line", &line_buf, wgpu::PrimitiveTopology::LineStrip, Default::default(), wgpu::CompareFunction::LessEqual);

    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("verts"),
        contents: bytemuck::cast_slice(&m.gpu_vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("indices"),
        contents: bytemuck::cast_slice(&m.triangles),
        usage: wgpu::BufferUsages::INDEX,
    });
    let ebuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("edges"),
        contents: bytemuck::cast_slice(&m.edge_indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let pbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("poly"),
        contents: bytemuck::cast_slice(&m.polyline),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let layer = |label: &str| {
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() }],
        });
        Layer { ubuf, bind }
    };
    GpuScene {
        mesh_pipe,
        wire_pipe,
        strip_pipe,
        vbuf,
        ibuf,
        index_count: (m.triangles.len() * 3) as u32,
        ebuf,
        edge_count: m.edge_indices.len() as u32,
        pbuf,
        poly_count: m.polyline.len() as u32,
        mesh_l: layer("mesh-u"),
        wire_l: layer("wire-u"),
        poly_l: layer("poly-u"),
    }
}

fn mesh_buf_pos_only() -> [Option<wgpu::VertexBufferLayout<'static>>; 1] {
    static ATTR: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
    [Some(wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTR,
    })]
}

struct ViewCb {
    vp: M4,
    eye: [f32; 3],
    wire: bool,
    polyline: bool,
}

impl egui_wgpu::CallbackTrait for ViewCb {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _enc: &mut wgpu::CommandEncoder,
        res: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let s: &GpuScene = res.get().expect("scene");
        let eye = [self.eye[0], self.eye[1], self.eye[2], 1.0];
        let w = |l: &Layer, color: [f32; 4]| {
            queue.write_buffer(&l.ubuf, 0, bytemuck::bytes_of(&Uniforms { view_proj: self.vp, color, eye }));
        };
        w(&s.mesh_l, [0.62, 0.68, 0.78, 1.0]);
        w(&s.wire_l, [0.05, 0.05, 0.08, 1.0]);
        w(&s.poly_l, [1.0, 0.55, 0.1, 1.0]);
        Vec::new()
    }

    fn paint(&self, _i: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, res: &egui_wgpu::CallbackResources) {
        let s: &GpuScene = res.get().expect("scene");
        pass.set_pipeline(&s.mesh_pipe);
        pass.set_bind_group(0, &s.mesh_l.bind, &[]);
        pass.set_vertex_buffer(0, s.vbuf.slice(..));
        pass.set_index_buffer(s.ibuf.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..s.index_count, 0, 0..1);
        if self.wire {
            pass.set_pipeline(&s.wire_pipe);
            pass.set_bind_group(0, &s.wire_l.bind, &[]);
            pass.set_vertex_buffer(0, s.vbuf.slice(..));
            pass.set_index_buffer(s.ebuf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..s.edge_count, 0, 0..1);
        }
        if self.polyline {
            pass.set_pipeline(&s.strip_pipe);
            pass.set_bind_group(0, &s.poly_l.bind, &[]);
            pass.set_vertex_buffer(0, s.pbuf.slice(..));
            pass.draw(0..s.poly_count, 0..1);
        }
    }
}

// ---------------------------------------------------------------- app

struct Args {
    bench: Option<f32>,
    tris: usize,
    wire: bool,
    novsync: bool,
    out: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args { bench: None, tris: 1_000_000, wire: false, novsync: false, out: None };
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < v.len() {
        match v[i].as_str() {
            "--bench" => {
                let secs = v.get(i + 1).and_then(|s| s.parse().ok());
                if secs.is_some() {
                    i += 1;
                }
                a.bench = Some(secs.unwrap_or(60.0));
            }
            "--tris" => {
                a.tris = v[i + 1].parse().expect("--tris N");
                i += 1;
            }
            "--wire" => a.wire = true,
            "--novsync" => a.novsync = true,
            "--out" => {
                a.out = Some(v[i + 1].clone());
                i += 1;
            }
            other => panic!("unknown argument {other}"),
        }
        i += 1;
    }
    a
}

struct App {
    cam: Camera,
    trimesh: TriMesh,
    wire: bool,
    polyline: bool,
    n_tris: usize,
    n_poly: usize,
    n_edges: usize,
    adapter: String,
    last_frame: Option<Instant>,
    frame_ms: Vec<f32>,
    start: Instant,
    bench: Option<f32>,
    out: Option<String>,
    pick_us: Vec<f32>,
    pick_text: String,
    build_ms: (f64, f64),
    tris_cfg: usize,
    vsync_off: bool,
    extent: f32,
    reported: bool,
}

impl App {
    fn pick(&mut self, ndc: [f32; 2], aspect: f32) {
        let (o, d) = self.cam.ray(ndc, aspect);
        let t0 = Instant::now();
        let ray = Ray::new(PVec::new(o[0], o[1], o[2]), PVec::new(d[0], d[1], d[2]));
        let hit = self.trimesh.cast_local_ray_and_get_normal(&ray, 1.0e6, false);
        self.pick_us.push(t0.elapsed().as_secs_f32() * 1.0e6);
        self.pick_text = match hit {
            Some(h) => {
                let p = ray.point_at(h.time_of_impact);
                format!("hit at ({:.2}, {:.2}, {:.2}) mm", p.x, p.y, p.z)
            }
            None => "miss".into(),
        };
    }

    /// Scripted camera path: orbit for the first third, pan the second, zoom the last (verification plan 4.2).
    fn scripted(&mut self, t: f32, total: f32) {
        let ph = (t / total).clamp(0.0, 1.0) * 3.0;
        self.cam.target = [0.0, 0.0, 0.0];
        self.cam.pitch = 0.45;
        if ph < 1.0 {
            self.cam.yaw = ph * std::f32::consts::TAU * 2.0;
            self.cam.dist = 180.0;
        } else if ph < 2.0 {
            let p = ph - 1.0;
            self.cam.yaw = 0.6;
            self.cam.dist = 180.0;
            self.cam.target = [(p * std::f32::consts::TAU * 2.0).sin() * 40.0, (p * std::f32::consts::TAU * 3.0).sin() * 15.0, 0.0];
        } else {
            let p = ph - 2.0;
            self.cam.yaw = 0.6;
            self.cam.dist = 180.0 * (0.25f32).powf((p * std::f32::consts::TAU * 2.0).sin() * 0.5 + 0.5);
        }
    }

    fn report(&self, ctx: &egui::Context) {
        let warm = 30.min(self.frame_ms.len() / 4);
        let mut v: Vec<f32> = self.frame_ms[warm..].to_vec();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pct = |p: f32| v[(((v.len() - 1) as f32) * p).round() as usize];
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        let pick_max = self.pick_us.iter().cloned().fold(0.0, f32::max);
        let mut ps = self.pick_us.clone();
        ps.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pick_p95 = ps.get(((ps.len().max(1) - 1) as f32 * 0.95) as usize).copied().unwrap_or(0.0);
        let line = format!(
            "{{\"adapter\":\"{}\",\"tris\":{},\"edges\":{},\"polyline_pts\":{},\"wire\":{},\"vsync_off\":{},\"frames\":{},\"mean_ms\":{:.2},\"p50_ms\":{:.2},\"p95_ms\":{:.2},\"p99_ms\":{:.2},\"max_ms\":{:.2},\"fps_mean\":{:.1},\"pass_p95_le_33ms\":{},\"bvh_build_ms\":{:.1},\"mesh_build_ms\":{:.1},\"picks\":{},\"pick_p95_us\":{:.1},\"pick_max_us\":{:.1}}}",
            self.adapter, self.n_tris, self.n_edges, self.n_poly, self.wire, self.vsync_off, v.len(), mean,
            pct(0.5), pct(0.95), pct(0.99), pct(1.0), 1000.0 / mean, pct(0.95) <= 33.0,
            self.build_ms.1, self.build_ms.0, self.pick_us.len(), pick_p95, pick_max
        );
        println!("{line}");
        if let Some(path) = &self.out {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{line}");
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = Instant::now();
        if let Some(l) = self.last_frame {
            self.frame_ms.push((now - l).as_secs_f32() * 1000.0);
        }
        self.last_frame = Some(now);
        ctx.request_repaint(); // continuous rendering for the measurement

        let mut done = false;
        if let Some(secs) = self.bench {
            let t = self.start.elapsed().as_secs_f32();
            self.scripted(t, secs);
            done = t >= secs;
        }

        egui::Panel::top("top").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let n = self.frame_ms.len();
                let recent: f32 = self.frame_ms.iter().rev().take(60).sum::<f32>() / self.frame_ms.iter().rev().take(60).count().max(1) as f32;
                ui.label(format!("{} tris | {} | frame {:.1} ms ({:.0} fps) | frames {}", self.n_tris, self.adapter, recent, 1000.0 / recent.max(0.001), n));
                ui.checkbox(&mut self.wire, "edges");
                ui.checkbox(&mut self.polyline, "toolpath polyline");
                ui.label(format!("pick: {}", self.pick_text));
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
            let aspect = (rect.width() / rect.height()).max(0.01);
            if self.bench.is_none() {
                let d = resp.drag_delta();
                let pan = resp.dragged_by(egui::PointerButton::Secondary) || ui.input(|i| i.modifiers.shift);
                if pan {
                    let (r, u, _) = self.cam.basis();
                    let k = self.cam.dist * (FOV_Y / 2.0).tan() * 2.0 / rect.height();
                    for a in 0..3 {
                        self.cam.target[a] += (-r[a] * d.x + u[a] * d.y) * k;
                    }
                } else {
                    self.cam.yaw -= d.x * 0.01;
                    self.cam.pitch = (self.cam.pitch + d.y * 0.01).clamp(-1.5, 1.5);
                }
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                self.cam.dist = (self.cam.dist * (1.0 - scroll * 0.002)).clamp(2.0, 2000.0);
                if resp.clicked() {
                    if let Some(p) = resp.interact_pointer_pos() {
                        let nx = (p.x - rect.left()) / rect.width() * 2.0 - 1.0;
                        let ny = 1.0 - (p.y - rect.top()) / rect.height() * 2.0;
                        self.pick([nx, ny], aspect);
                    }
                }
            } else {
                // Exercise the picker during the benchmark: a sweep of rays across the view each frame.
                let k = (self.frame_ms.len() % 64) as f32 / 32.0 - 1.0;
                self.pick([k, 0.1 * k], aspect);
            }
            ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                rect,
                ViewCb { vp: self.cam.view_proj(aspect), eye: self.cam.eye(), wire: self.wire, polyline: self.polyline },
            ));
        });

        if done && !self.reported {
            self.reported = true;
            self.report(&ctx);
        }
    }
}

fn main() -> eframe::Result {
    let args = parse_args();
    let t0 = Instant::now();
    let mesh = build_mesh(args.tris);
    let mesh_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let t1 = Instant::now();
    let verts: Vec<PVec> = mesh.pos_f64.iter().map(|p| PVec::new(p[0], p[1], p[2])).collect();
    let trimesh = TriMesh::new(verts, mesh.triangles.clone()).expect("trimesh");
    let bvh_ms = t1.elapsed().as_secs_f64() * 1000.0;
    eprintln!("mesh: {} tris, {} verts, built in {:.0} ms; parry3d-f64 TriMesh+BVH in {:.0} ms", mesh.triangles.len(), mesh.pos_f64.len(), mesh_ms, bvh_ms);

    let mut wgpu_options = egui_wgpu::WgpuConfiguration::default();
    if args.novsync || args.bench.is_some() {
        wgpu_options.surface.present_mode = wgpu::PresentMode::AutoNoVsync;
    }
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1920.0, 1080.0]).with_title("SP-2 viewport spike"),
        depth_buffer: 32,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        ..Default::default()
    };
    let (n_tris, n_edges, n_poly, extent) = (mesh.triangles.len(), mesh.edge_indices.len() / 2, mesh.polyline.len(), mesh.extent);
    eframe::run_native(
        "sp2-viewport",
        native,
        Box::new(move |cc| {
            let rs = cc.wgpu_render_state.as_ref().expect("wgpu render state (is the wgpu backend active?)");
            let info = rs.adapter.get_info();
            let adapter = format!("{} ({:?}, {:?})", info.name, info.backend, info.device_type);
            eprintln!("adapter: {adapter}");
            let scene = init_gpu(rs, &mesh);
            rs.renderer.write().callback_resources.insert(scene);
            Ok(Box::new(App {
                cam: Camera { target: [0.0; 3], yaw: 0.6, pitch: 0.45, dist: 180.0 },
                trimesh,
                wire: args.wire,
                polyline: true,
                n_tris,
                n_poly,
                n_edges,
                adapter,
                last_frame: None,
                frame_ms: Vec::new(),
                start: Instant::now() + Duration::from_millis(0),
                bench: args.bench,
                out: args.out.clone(),
                pick_us: Vec::new(),
                pick_text: String::new(),
                build_ms: (mesh_ms, bvh_ms),
                tris_cfg: args.tris,
                vsync_off: args.novsync || args.bench.is_some(),
                extent,
                reported: false,
            }))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pick sanity: a ray fired down the -Z axis at x = 0 must hit the surface at the analytic radius.
    #[test]
    fn pick_hits_analytic_radius() {
        let m = build_mesh(200_000);
        let verts: Vec<PVec> = m.pos_f64.iter().map(|p| PVec::new(p[0], p[1], p[2])).collect();
        let tm = TriMesh::new(verts, m.triangles.clone()).unwrap();
        let ray = Ray::new(PVec::new(0.0, 0.0, 100.0), PVec::new(0.0, 0.0, -1.0));
        let t = tm.cast_local_ray(&ray, 1.0e6, false).expect("hit");
        let z = 100.0 - t;
        let expected = radius(std::f64::consts::FRAC_PI_2, 60.0); // x = 0 is the part's mid-length (60 mm along)
        assert!((z - expected).abs() < 0.05, "z {z} vs analytic {expected}");
    }
}

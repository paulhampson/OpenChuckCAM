// SPDX-License-Identifier: AGPL-3.0-or-later
//! The long-toolpath frame benchmark (issue #200; NFR-03, NFR-04; decision
//! record 0166). It draws a toolpath layer offscreen while the camera follows
//! the scripted orbit, pan and zoom of [`CameraPath`], and reports what the
//! path costs: points, draw calls, vertices, GPU memory, the upload, the CPU
//! work of each frame and the frame time on whatever adapter wgpu finds.
//!
//! The path is a stand-in for a blisk finishing path, built as the playback
//! panel builds its layer: one line per run of moves of one class (rapid,
//! link, entry, cut), cut and entry moves 2 pixels wide, rapids dotted.
//!
//! ```text
//! cargo run --release -p openchuck-viewport --example path_frames -- \
//!     [--blades 24] [--passes 200] [--cut-points 200] [--frames 120] \
//!     [--size 1366x768] [--merged] [--runs FILE]
//! ```
//!
//! `--runs FILE` draws a real project's path instead, as written by
//! `openchuck-app`'s `path_runs` example.
//!
//! `--merged` draws the same points as one line per class instead of one per
//! run, to separate the cost of the draw calls from that of the vertices.
//! The frame time on a software adapter (llvmpipe, WARP) is not the NFR-03
//! result, which is measured on the reference machines (verification plan
//! 4.2; OI-176).

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use openchuck_types::geometry::Point3;
use openchuck_viewport::bench::{CameraPath, FrameTimes};
use openchuck_viewport::wgpu;
use openchuck_viewport::{
    Camera, Colour, DisplayMode, Frame, Layer, LineStyle, Renderer, Scene, Slot,
};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// The playback panel's classes of move (`playback_panel::class_look`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Rapid,
    Link,
    Entry,
    Cut,
}

impl Class {
    const ALL: [Self; 4] = [Self::Rapid, Self::Link, Self::Entry, Self::Cut];

    fn look(self) -> (Colour, f32, LineStyle) {
        match self {
            Self::Rapid => ([0.60, 0.65, 0.71, 1.0], 1.0, LineStyle::Dotted),
            Self::Link => ([0.44, 0.83, 0.66, 1.0], 1.0, LineStyle::Solid),
            Self::Entry => ([0.91, 0.71, 1.0, 1.0], 2.0, LineStyle::Solid),
            Self::Cut => ([1.0, 0.71, 0.33, 1.0], 2.0, LineStyle::Solid),
        }
    }
}

struct Options {
    blades: usize,
    passes: usize,
    cut_points: usize,
    frames: usize,
    size: [u32; 2],
    merged: bool,
    runs: Option<String>,
}

fn options() -> Options {
    let mut o = Options {
        blades: 24,
        passes: 200,
        cut_points: 200,
        frames: 120,
        size: [1366, 768],
        merged: false,
        runs: None,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().expect("the option needs a value").clone();
        match flag.as_str() {
            "--blades" => o.blades = value().parse().expect("--blades N"),
            "--passes" => o.passes = value().parse().expect("--passes N"),
            "--cut-points" => o.cut_points = value().parse().expect("--cut-points N"),
            "--frames" => o.frames = value().parse().expect("--frames N"),
            "--size" => {
                let v = value();
                let (w, h) = v.split_once('x').expect("--size WxH");
                o.size = [w.parse().expect("width"), h.parse().expect("height")];
            }
            "--merged" => o.merged = true,
            "--runs" => o.runs = Some(value()),
            other => panic!("unknown option {other}"),
        }
    }
    o
}

/// A blisk finishing path in runs of one class: for each blade, passes from
/// the hub to the tip, each an entry, a cut along the twisted blade, an exit
/// and a link to the next pass; a rapid between blades. Points in the part's
/// frame, the rotary axis along X.
/// The runs of a file written by `path_runs`: `run CLASS COUNT` and then
/// `COUNT` lines of `X Y Z`.
fn file_runs(path: &str) -> Vec<(Class, Vec<Point3>)> {
    let text = std::fs::read_to_string(path).expect("the runs file reads");
    let mut runs = Vec::new();
    let mut lines = text.lines();
    while let Some(header) = lines.next() {
        let words: Vec<&str> = header.split_whitespace().collect();
        let (Some(&"run"), Some(class), Some(count)) = (words.first(), words.get(1), words.get(2))
        else {
            panic!("not a run header: {header}");
        };
        let class = match *class {
            "rapid" => Class::Rapid,
            "link" => Class::Link,
            "entry" => Class::Entry,
            _ => Class::Cut,
        };
        let count: usize = count.parse().expect("a point count");
        let points = (0..count)
            .map(|_| {
                let mut xyz = lines
                    .next()
                    .expect("a point")
                    .split_whitespace()
                    .map(|v| v.parse::<f64>().expect("a coordinate"));
                let mut next = || xyz.next().expect("three coordinates");
                Point3::new(next(), next(), next())
            })
            .collect();
        runs.push((class, points));
    }
    runs
}

fn blisk_runs(o: &Options) -> Vec<(Class, Vec<Point3>)> {
    let point = |x: f64, r: f64, angle: f64| Point3::new(x, r * angle.cos(), r * angle.sin());
    let mut runs = Vec::new();
    let (hub, tip, half_chord, twist) = (30.0, 60.0, 12.0, 0.04);
    for b in 0..o.blades {
        let base = std::f64::consts::TAU * b as f64 / o.blades as f64;
        for p in 0..o.passes {
            let r = hub + (tip - hub) * p as f64 / o.passes.max(2).saturating_sub(1) as f64;
            let angle = |x: f64| base + twist * x;
            let start = point(-half_chord, r, angle(-half_chord));
            let end = point(half_chord, r, angle(half_chord));
            let above = |q: Point3, by: f64| {
                let s = (r + by) / r;
                Point3::new(q.x, q.y * s, q.z * s)
            };
            runs.push((
                Class::Entry,
                vec![above(start, 2.0), above(start, 0.5), start],
            ));
            let cut = (0..o.cut_points)
                .map(|k| {
                    let x = -half_chord + 2.0 * half_chord * k as f64 / (o.cut_points - 1) as f64;
                    point(x, r, angle(x))
                })
                .collect();
            runs.push((Class::Cut, cut));
            runs.push((Class::Entry, vec![end, above(end, 0.5), above(end, 2.0)]));
            if p + 1 < o.passes {
                let next = r + (tip - hub) / o.passes as f64;
                let back = point(-half_chord, next, angle(-half_chord));
                runs.push((Class::Link, vec![above(end, 2.0), above(back, 2.0)]));
            }
        }
        let next = std::f64::consts::TAU * (b + 1) as f64 / o.blades as f64;
        runs.push((
            Class::Rapid,
            vec![
                point(half_chord, tip + 10.0, base),
                point(-half_chord, tip + 10.0, next),
            ],
        ));
    }
    runs
}

/// The path layer as the playback panel builds it (`paths_scene`): one line
/// per run, or with `merged` one per class.
fn paths_scene(runs: &[(Class, Vec<Point3>)], merged: bool) -> Scene {
    let mut scene = Scene::default();
    let add = |scene: &mut Scene, class: Class, points: &[Point3]| {
        let (colour, width, style) = class.look();
        scene.add_styled_polyline(points, colour, false, width, style);
        if let Some(added) = scene.lines.last_mut() {
            added.slot = Slot::Part;
        }
    };
    if merged {
        for class in Class::ALL {
            let mut joined = Scene::default();
            for (_, points) in runs.iter().filter(|(c, _)| *c == class) {
                add(&mut joined, class, points);
            }
            let segments = joined
                .lines
                .iter()
                .flat_map(|l| l.segments.iter().copied())
                .collect();
            if let Some(mut line) = joined.lines.pop() {
                line.segments = segments;
                scene.lines.push(line);
            }
        }
    } else {
        for (class, points) in runs {
            add(&mut scene, *class, points);
        }
    }
    scene
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = future.as_mut().poll(&mut context) {
            return v;
        }
        std::thread::yield_now();
    }
}

fn ms(d: Duration) -> String {
    format!("{:.2} ms", d.as_secs_f64() * 1000.0)
}

fn summary(name: &str, times: &FrameTimes) {
    if let Some(s) = times.summary() {
        println!(
            "{name:<22} mean {:>10}  p50 {:>10}  p95 {:>10}  max {:>10}",
            ms(s.mean),
            ms(s.p50),
            ms(s.p95),
            ms(s.max)
        );
    }
}

fn main() {
    let o = options();
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("no graphics adapter");
    let info = adapter.get_info();
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

    let runs = match &o.runs {
        Some(file) => file_runs(file),
        None => blisk_runs(&o),
    };
    let points: usize = runs.iter().map(|(_, p)| p.len()).sum();
    let scene = paths_scene(&runs, o.merged);
    let segments: usize = scene.lines.iter().map(|l| l.segments.len() / 2).sum();
    let wide_segments: usize = scene
        .lines
        .iter()
        .filter(|l| l.width > 1.0)
        .map(|l| l.segments.len() / 2)
        .sum();
    // Vertex shader runs: six corners per wide segment, two per hairline one.
    let vertex_runs = wide_segments * 6 + (segments - wide_segments) * 2;

    let [width, height] = o.size;
    let texture = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let target = texture(FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let depth = texture(DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

    let mut renderer = Renderer::new(&device, FORMAT, DEPTH, 1);
    let started = Instant::now();
    renderer.set_layer(&device, Layer::Paths, &scene);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let upload = started.elapsed();

    let aspect = f64::from(width) / f64::from(height);
    let mut camera = Camera::default();
    let (min, max) = scene.bounds().expect("the path has points");
    camera.fit(min, max, aspect);
    let path = CameraPath::new(camera, Duration::from_secs(60));

    let (mut prepare, mut encode, mut gpu, mut total) = (
        FrameTimes::default(),
        FrameTimes::default(),
        FrameTimes::default(),
        FrameTimes::default(),
    );
    let warm_up = 5;
    for i in 0..o.frames + warm_up {
        let at = path
            .duration()
            .mul_f64(i.saturating_sub(warm_up) as f64 / o.frames as f64);
        let mut frame = Frame::new(path.at(at), aspect, DisplayMode::ShadedWithEdges);
        frame.viewport = [width as f32, height as f32];
        let t0 = Instant::now();
        renderer.prepare(&device, &queue, &frame);
        let t1 = Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            renderer.paint(&mut pass);
        }
        let commands = encoder.finish();
        let t2 = Instant::now();
        queue.submit([commands]);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let t3 = Instant::now();
        if i >= warm_up {
            prepare.push(t1 - t0);
            encode.push(t2 - t1);
            gpu.push(t3 - t2);
            total.push(t3 - t0);
        }
    }

    println!("OpenChuckCAM long-toolpath frame benchmark (issue #200)");
    println!(
        "adapter:              {} ({:?}, {:?})",
        info.name, info.device_type, info.backend
    );
    println!(
        "target:               {width} x {height} pixels, {} frames of the 4.2 path",
        o.frames
    );
    match &o.runs {
        Some(file) => println!("path:                 {file}"),
        None => println!(
            "path:                 {} blades x {} passes x {} cut points",
            o.blades, o.passes, o.cut_points
        ),
    }
    println!("runs:                 {}", runs.len());
    println!("points:               {points}");
    let stats = renderer.layer_stats(Layer::Paths);
    println!(
        "line items:           {} ({})",
        scene.lines.len(),
        if o.merged {
            "one per class"
        } else {
            "one per run"
        }
    );
    println!("draw calls:           {} per frame", stats.line_draws);
    println!(
        "uniform writes:       {} per frame",
        renderer.uniform_writes()
    );
    println!("segments:             {segments} ({wide_segments} wide)");
    println!("vertex shader runs:   {vertex_runs} per frame");
    println!(
        "GPU memory:           {:.1} MB in {} buffers",
        stats.bytes as f64 / 1e6,
        stats.buffers
    );
    println!("upload (set_layer):   {}", ms(upload));
    summary("CPU prepare:", &prepare);
    summary("CPU encode (paint):", &encode);
    summary("GPU submit and wait:", &gpu);
    summary("frame total:", &total);
}

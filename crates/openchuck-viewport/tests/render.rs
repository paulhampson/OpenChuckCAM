// SPDX-License-Identifier: AGPL-3.0-or-later
//! Renders a scene offscreen and reads the pixels back (UI-02, NFR-02): the
//! software-rendered smoke test of verification plan section 4.2, where the
//! machine has a software adapter (lavapipe, WARP or llvmpipe). With no
//! adapter at all the test says so and passes; set
//! `OPENCHUCK_REQUIRE_ADAPTER=1` to make that a failure.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;
use openchuck_viewport::wgpu;
use openchuck_viewport::{Camera, DisplayMode, Frame, Projection, Renderer, Scene, StandardView};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// Runs a wgpu future to completion; native wgpu futures are ready at once
/// or after a device poll.
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

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        force_fallback_adapter: false,
        ..Default::default()
    }));
    let adapter = match adapter {
        Ok(a) => a,
        Err(e) => {
            assert!(
                std::env::var_os("OPENCHUCK_REQUIRE_ADAPTER").is_none(),
                "no graphics adapter: {e}"
            );
            eprintln!("skipped: no graphics adapter ({e})");
            return None;
        }
    };
    eprintln!("adapter: {:?}", adapter.get_info());
    Some(block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap())
}

/// A box from `min` to `max` with outward triangles.
fn block(min: [f64; 3], max: [f64; 3]) -> TriangleMesh {
    let v = |i: usize| {
        Point3::new(
            if i & 1 == 0 { min[0] } else { max[0] },
            if i & 2 == 0 { min[1] } else { max[1] },
            if i & 4 == 0 { min[2] } else { max[2] },
        )
    };
    TriangleMesh {
        vertices: (0..8).map(v).collect(),
        triangles: vec![
            [0, 2, 3],
            [0, 3, 1],
            [4, 5, 7],
            [4, 7, 6],
            [0, 1, 5],
            [0, 5, 4],
            [2, 6, 7],
            [2, 7, 3],
            [1, 3, 7],
            [1, 7, 5],
            [0, 4, 6],
            [0, 6, 2],
        ],
    }
}

/// Renders and returns RGBA rows, top row first.
fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    scene: &Scene,
    frame: &Frame,
) -> Vec<[u8; 4]> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let mut renderer = Renderer::new(device, FORMAT, DEPTH, 1);
    renderer.set_scene(device, scene);
    renderer.prepare(device, queue, frame);
    let (view, depth_view) = (
        target.create_view(&wgpu::TextureViewDescriptor::default()),
        depth.create_view(&wgpu::TextureViewDescriptor::default()),
    );
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(SIZE * SIZE * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
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
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 4),
                rows_per_image: Some(SIZE),
            },
        },
        wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let bytes = readback.slice(..).get_mapped_range().unwrap().to_vec();
    bytes
        .chunks_exact(4)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .collect()
}

fn frame(view: StandardView, projection: Projection, mode: DisplayMode, scene: &Scene) -> Frame {
    let mut camera = Camera {
        projection,
        ..Camera::default()
    };
    let (min, max) = scene.bounds().unwrap();
    camera.fit(min, max, 1.0);
    camera.set_view(view);
    Frame {
        camera,
        aspect: 1.0,
        mode,
    }
}

/// Red and blue pixels on the left and right halves.
fn halves(pixels: &[[u8; 4]]) -> ([usize; 2], [usize; 2]) {
    let mut red = [0; 2];
    let mut blue = [0; 2];
    for (i, p) in pixels.iter().enumerate() {
        let half = usize::from((i as u32 % SIZE) >= SIZE / 2);
        if p[0] > 60 && p[1] < 30 && p[2] < 30 {
            red[half] += 1;
        }
        if p[2] > 60 && p[0] < 30 && p[1] < 30 {
            blue[half] += 1;
        }
    }
    (red, blue)
}

#[test]
fn req_ui_02_req_nfr_02_scene_renders_with_x_to_the_right_in_the_front_view() {
    let Some((device, queue)) = device() else {
        return;
    };
    // Red block at +X, blue at -X: the front view shows red on the right.
    let mut scene = Scene::default();
    scene.add_mesh(&block([5.0, -5.0, -5.0], [15.0, 5.0, 5.0]), &[], RED, BLACK);
    scene.add_mesh(
        &block([-15.0, -5.0, -5.0], [-5.0, 5.0, 5.0]),
        &[],
        BLUE,
        BLACK,
    );
    for projection in [Projection::Perspective, Projection::Orthographic] {
        let f = frame(StandardView::Front, projection, DisplayMode::Shaded, &scene);
        let (red, blue) = halves(&render(&device, &queue, &scene, &f));
        assert!(red[1] > 50 && red[0] == 0, "{projection:?}: red {red:?}");
        assert!(
            blue[0] > 50 && blue[1] == 0,
            "{projection:?}: blue {blue:?}"
        );
        // The back view mirrors it.
        let f = frame(StandardView::Back, projection, DisplayMode::Shaded, &scene);
        let (red, blue) = halves(&render(&device, &queue, &scene, &f));
        assert!(red[0] > 50 && red[1] == 0, "{projection:?}: red {red:?}");
        assert!(
            blue[1] > 50 && blue[0] == 0,
            "{projection:?}: blue {blue:?}"
        );
    }
}

#[test]
fn req_ui_02_display_modes_draw_surfaces_edges_and_wireframe() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mesh = block([-10.0; 3], [10.0; 3]);
    let edges = box_edges();
    let mut scene = Scene::default();
    scene.add_mesh(&mesh, &edges, RED, BLACK);
    let count = |mode| {
        let f = frame(
            StandardView::Isometric,
            Projection::Orthographic,
            mode,
            &scene,
        );
        let pixels = render(&device, &queue, &scene, &f);
        let red = pixels.iter().filter(|p| p[0] > 60 && p[1] < 30).count();
        let dark = pixels
            .iter()
            .filter(|p| p[0] < 30 && p[1] < 30 && p[2] < 30)
            .count();
        (red, dark)
    };
    let (shaded_red, shaded_dark) = count(DisplayMode::Shaded);
    let (edged_red, edged_dark) = count(DisplayMode::ShadedWithEdges);
    let (wire_red, wire_dark) = count(DisplayMode::Wireframe);
    assert!(
        shaded_red > 500 && shaded_dark == 0,
        "{shaded_red} {shaded_dark}"
    );
    assert!(
        edged_dark > 20 && edged_red < shaded_red,
        "{edged_red} {edged_dark}"
    );
    assert!(
        wire_red == 0 && wire_dark > edged_dark,
        "{wire_red} {wire_dark}"
    );
}

#[test]
fn req_ui_03_guides_draw_over_the_part() {
    let Some((device, queue)) = device() else {
        return;
    };
    // A guide line along X through the middle of a block: drawn on top it
    // shows; tested against depth it hides inside the block.
    let mut scene = Scene::default();
    scene.add_mesh(&block([-10.0; 3], [10.0; 3]), &[], RED, BLACK);
    let axis = [Point3::new(-20.0, 0.0, 0.0), Point3::new(20.0, 0.0, 0.0)];
    let blue_in_middle = |on_top: bool| {
        let mut s = scene.clone();
        s.add_polyline(&axis, BLUE, on_top);
        let f = frame(
            StandardView::Front,
            Projection::Orthographic,
            DisplayMode::Shaded,
            &s,
        );
        let pixels = render(&device, &queue, &s, &f);
        (0..SIZE)
            .flat_map(|y| (SIZE / 2 - 4..SIZE / 2 + 4).map(move |x| (y * SIZE + x) as usize))
            .filter(|i| pixels[*i][2] > 200 && pixels[*i][0] < 30)
            .count()
    };
    assert!(blue_in_middle(true) > 0);
    assert_eq!(blue_in_middle(false), 0);
}

/// The block's twelve edges, by vertex index.
fn box_edges() -> Vec<[u32; 2]> {
    let mut edges = Vec::new();
    for i in 0u32..8 {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                edges.push([i, i | bit]);
            }
        }
    }
    edges
}

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
use openchuck_viewport::camera::{IDENTITY, multiply, place, rigid_inverse, rotation_x};
use openchuck_viewport::wgpu;
use openchuck_viewport::{
    Camera, DisplayMode, Frame, Layer, Projection, Renderer, Scene, Slot, StandardView,
};

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
    render_over(device, queue, scene, None, frame)
}

/// Renders `scene` with an overlay drawn over it.
fn render_over(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    scene: &Scene,
    overlay: Option<&Scene>,
    frame: &Frame,
) -> Vec<[u8; 4]> {
    render_layers(device, queue, scene, overlay, &[], frame)
}

/// Renders `scene`, an overlay and more layers, each in its own slot of the
/// renderer.
fn render_layers(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    scene: &Scene,
    overlay: Option<&Scene>,
    layers: &[(Layer, &Scene)],
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
    if let Some(overlay) = overlay {
        renderer.set_overlay(device, overlay);
    }
    for (layer, extra) in layers {
        renderer.set_layer(device, *layer, extra);
    }
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
    Frame::new(camera, 1.0, mode)
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

/// The triangles `which` of `mesh` as a mesh of their own, on the same
/// vertices.
fn some(mesh: &TriangleMesh, which: &[usize]) -> TriangleMesh {
    TriangleMesh {
        vertices: mesh.vertices.clone(),
        triangles: which.iter().map(|&t| mesh.triangles[t]).collect(),
    }
}

#[test]
fn req_ui_04_an_overlay_wins_over_the_surface_it_covers_and_not_over_what_hides_it() {
    let Some((device, queue)) = device() else {
        return;
    };
    let block = block([-10.0; 3], [10.0; 3]);
    let mut scene = Scene::default();
    scene.add_mesh(&block, &[], RED, BLACK);
    let f = frame(
        StandardView::Front,
        Projection::Orthographic,
        DisplayMode::Shaded,
        &scene,
    );
    let (red, blue) = halves(&render(&device, &queue, &scene, &f));
    assert!(red[0] + red[1] > 500 && blue == [0, 0]);

    // The front face (triangles 4 and 5, at y = -10) painted blue: it covers
    // the red face it lies on, with no speckle where the two are coplanar.
    let mut front = Scene::default();
    front.add_mesh(&some(&block, &[4, 5]), &[], BLUE, BLACK);
    let (red, blue) = halves(&render_over(&device, &queue, &scene, Some(&front), &f));
    assert!(blue[0] + blue[1] > 500, "blue {blue:?}");
    assert_eq!(red, [0, 0], "red left {red:?}");

    // The back face (triangles 6 and 7, at y = +10) is hidden by the block.
    let mut back = Scene::default();
    back.add_mesh(&some(&block, &[6, 7]), &[], BLUE, BLACK);
    let (red, blue) = halves(&render_over(&device, &queue, &scene, Some(&back), &f));
    assert_eq!(blue, [0, 0], "blue shows through {blue:?}");
    assert!(red[0] + red[1] > 500);
}

#[test]
fn req_sim_02_an_item_in_a_slot_is_drawn_where_the_slots_transform_puts_it() {
    let Some((device, queue)) = device() else {
        return;
    };
    // A red block in the part slot and a blue one in the tool slot, both
    // built around the origin. Fixed, they overlap in the middle.
    let mut part = Scene::default();
    part.add_mesh(&block([-5.0; 3], [5.0; 3]), &[], RED, BLACK);
    part.meshes[0].slot = Slot::Part;
    let mut tool = Scene::default();
    tool.add_mesh(&block([-5.0; 3], [5.0; 3]), &[], BLUE, BLACK);
    tool.meshes[0].slot = Slot::Tool;
    // Fit to where the blocks will be, 25 mm either side of the origin.
    let mut wide = Scene::default();
    wide.add_mesh(
        &block([-40.0, -5.0, -5.0], [40.0, 5.0, 5.0]),
        &[],
        BLACK,
        BLACK,
    );
    let mut f = frame(
        StandardView::Front,
        Projection::Orthographic,
        DisplayMode::Shaded,
        &wide,
    );
    f.transforms[Slot::Part.index()] = place([-25.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    f.transforms[Slot::Tool.index()] = place([25.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let pixels = render_layers(&device, &queue, &part, None, &[(Layer::Tool, &tool)], &f);
    let (red, blue) = halves(&pixels);
    // Front view: X to the right. The part moved to -X (left), the tool to +X.
    assert!(red[0] > 20 && red[1] == 0, "red {red:?}");
    assert!(blue[1] > 20 && blue[0] == 0, "blue {blue:?}");
}

#[test]
fn req_sim_02_a_turned_item_shades_by_its_own_normals() {
    let Some((device, queue)) = device() else {
        return;
    };
    // A block turned half a turn about X looks the same as the block itself,
    // lit the same way, because the eye is carried into its frame: the
    // pixels match exactly.
    let mut scene = Scene::default();
    scene.add_mesh(&block([-10.0; 3], [10.0; 3]), &[], RED, BLACK);
    let still = frame(
        StandardView::Isometric,
        Projection::Perspective,
        DisplayMode::Shaded,
        &scene,
    );
    let before = render(&device, &queue, &scene, &still);
    let mut turned_scene = scene.clone();
    turned_scene.meshes[0].slot = Slot::Part;
    let mut turned = still;
    turned.transforms[Slot::Part.index()] = rotation_x(std::f64::consts::PI);
    let after = render(&device, &queue, &turned_scene, &turned);
    // Rotating the block half a turn about X maps it onto itself, but the
    // picture is only the same if the shading uses the item's own frame.
    // Mirror image across the camera is not expected: compare the red area.
    let area = |p: &[[u8; 4]]| p.iter().filter(|c| c[0] > 40 && c[1] < 10).count();
    assert!(area(&before) > 300);
    assert_eq!(area(&before), area(&after));
}

#[test]
fn req_sim_05_a_translucent_mesh_blends_over_what_is_behind_it() {
    let Some((device, queue)) = device() else {
        return;
    };
    // A red opaque block with a larger blue translucent one around it. The
    // opaque block shows through; the glass is drawn after it whichever is
    // listed first.
    let mut part = Scene::default();
    part.add_mesh(&block([-5.0; 3], [5.0; 3]), &[], RED, BLACK);
    let mut glass = Scene::default();
    glass.add_mesh(
        &block([-10.0; 3], [10.0; 3]),
        &[],
        [0.0, 0.0, 1.0, 0.4],
        BLACK,
    );
    glass.meshes[0].translucent = true;
    let f = frame(
        StandardView::Front,
        Projection::Orthographic,
        DisplayMode::Shaded,
        &glass,
    );
    let pixels = render_layers(&device, &queue, &glass, None, &[(Layer::Stock, &part)], &f);
    let centre = pixels[(SIZE as usize / 2) * SIZE as usize + SIZE as usize / 2];
    // Red shows through, tinted blue; the glass alone would be blue-white.
    assert!(centre[0] > 100 && centre[2] > 50, "{centre:?}");
    let edge = pixels[(SIZE as usize / 2) * SIZE as usize + SIZE as usize / 4];
    assert!(edge[2] > edge[0], "glass only: {edge:?}");
}

#[test]
fn req_sim_02_matrix_helpers_are_rigid_and_invert() {
    let m = multiply(&place([3.0, -2.0, 7.0], [0.0, 1.0, 1.0]), &rotation_x(0.7));
    let product = multiply(&m, &rigid_inverse(&m));
    for (i, column) in product.iter().enumerate() {
        for (j, value) in column.iter().enumerate() {
            let want = if i == j { 1.0 } else { 0.0 };
            assert!((value - want).abs() < 1e-12, "{product:?}");
        }
    }
    // place() takes +Z onto the axis and the origin to the point.
    let p = place([1.0, 2.0, 3.0], [0.0, 1.0, 0.0]);
    let axis = [p[2][0], p[2][1], p[2][2]];
    assert!((axis[1] - 1.0).abs() < 1e-12 && axis[0].abs() < 1e-12);
    assert!((p[3][0] - 1.0).abs() + (p[3][1] - 2.0).abs() + (p[3][2] - 3.0).abs() < 1e-12);
    assert!((IDENTITY[0][0] - 1.0).abs() < 1e-12);
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

/// Blue pixels of `pixels`.
fn blue_pixels(pixels: &[[u8; 4]]) -> usize {
    pixels
        .iter()
        .filter(|p| p[2] > 200 && p[0] < 60 && p[1] < 60)
        .count()
}

#[test]
fn req_ui_04_req_nfr_15_a_wide_line_is_as_wide_as_asked_whatever_the_zoom() {
    let Some((device, queue)) = device() else {
        return;
    };
    // A line along X through the middle of the view, drawn as a hairline and
    // then 6 pixels wide; the view is 64 pixels, and so is the viewport.
    let line = [Point3::new(-20.0, 0.0, 0.0), Point3::new(20.0, 0.0, 0.0)];
    let draw = |width: f32, zoom: f64| {
        let mut scene = Scene::default();
        scene.add_wide_polyline(&line, BLUE, true, width);
        let mut f = frame(
            StandardView::Front,
            Projection::Orthographic,
            DisplayMode::Shaded,
            &scene,
        );
        f.viewport = [SIZE as f32, SIZE as f32];
        f.camera.zoom(zoom);
        let pixels = render(&device, &queue, &scene, &f);
        // The rows the line covers, and how many pixels it paints.
        let rows = (0..SIZE)
            .filter(|y| (0..SIZE).any(|x| blue_pixels(&[pixels[(y * SIZE + x) as usize]]) == 1))
            .count();
        (rows, blue_pixels(&pixels))
    };
    let (hair_rows, hair) = draw(0.0, 1.0);
    let (wide_rows, wide) = draw(6.0, 1.0);
    assert!(
        hair_rows <= 2 && hair > 10,
        "hairline {hair_rows} rows {hair}"
    );
    assert!(
        (5..=8).contains(&wide_rows) && wide > 4 * hair,
        "wide {wide_rows} rows {wide}"
    );
    // Zooming in does not make it wider.
    let (zoomed_rows, _) = draw(6.0, 0.5);
    assert!(
        (5..=8).contains(&zoomed_rows),
        "zoomed {zoomed_rows} rows, not {wide_rows}"
    );
}

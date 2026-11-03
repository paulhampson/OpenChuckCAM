// SPDX-License-Identifier: AGPL-3.0-or-later
//! SIMUI checks on actual stock geometry, including offscreen pixels.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::float_cmp
)]
use super::*;
use crate::viewport::{ViewStep, Viewport, cut_part_scene};
use eframe::egui;
use openchuck_viewport::{
    Camera, DisplayMode, Frame, Layer, Projection, Renderer, Scene, StandardView, wgpu,
};
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};
const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

fn surface_fixture(name: &str) -> (Rig, Arc<openchuck_app::stockplay::StockSurface>) {
    let mut rig = simulated(name);
    let deadline = Instant::now() + Duration::from_secs(120);
    while rig.shell.viewport.stock_surface().is_none() {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let surface = Arc::clone(rig.shell.viewport.stock_surface().unwrap());
    (rig, surface)
}

fn stock_frame(scene: &Scene, aspect: f64) -> Frame {
    let mut camera = Camera {
        projection: Projection::Orthographic,
        ..Camera::default()
    };
    let (min, max) = scene.bounds().unwrap();
    camera.fit(min, max, aspect);
    camera.set_view(StandardView::Front);
    Frame::new(camera, aspect, DisplayMode::Shaded)
}

#[test]
fn req_nfr_04_cut_stock_rendering_cost_is_measured_at_full_hd_on_the_available_adapter() {
    let Some((device, queue)) = device() else {
        return;
    };
    let (_rig, surface) = surface_fixture("simui-stock-cost");
    let mut renderer = Renderer::new(&device, FORMAT, DEPTH, 1);
    let texture = |format| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    };
    let target = texture(FORMAT);
    let depth = texture(DEPTH);
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let depth = depth.create_view(&wgpu::TextureViewDescriptor::default());
    for (mapped, solid) in [(false, false), (false, true), (true, false), (true, true)] {
        let bands = surface.bands.as_ref().filter(|_| mapped);
        assert!(!mapped || bands.is_some());
        let scene = cut_part_scene(&surface, bands, solid);
        let frame = stock_frame(&scene, 1920.0 / 1080.0);
        renderer.set_layer(&device, Layer::Stock, &scene);
        renderer.prepare(&device, &queue, &frame);
        let mut times = Vec::new();
        for i in 0..12 {
            let started = Instant::now();
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
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
                        view: &depth,
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
            queue.submit([encoder.finish()]);
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            if i >= 2 {
                times.push(started.elapsed().as_secs_f64() * 1000.0);
            }
        }
        times.sort_by(f64::total_cmp);
        eprintln!(
            "SIMUI 1920×1080, mapped={mapped}, solid={solid}, triangles={}, warmed submit+GPU-wait ms: p50={:.3}, p95={:.3}",
            scene.triangle_count(),
            times[5],
            times[9]
        );
    }
}

#[test]
fn req_sim_05_req_nfr_15_cut_surface_pixels_preserve_occlusion_and_distinguishable_deviation_levels()
 {
    let Some((device, queue)) = device() else {
        return;
    };
    let (_rig, surface) = surface_fixture("simui-gpu-stock");
    let mut renderer = Renderer::new(&device, FORMAT, DEPTH, 1);
    let plain = cut_part_scene(&surface, None, true);
    let frame = stock_frame(&plain, 1.0);
    let render = |renderer: &mut Renderer, stock: &Scene, behind: &Scene| {
        renderer.set_scene(&device, behind);
        renderer.set_layer(&device, Layer::Stock, stock);
        renderer.prepare(&device, &queue, &frame);
        paint_and_read(&device, &queue, renderer)
    };
    let mut inner = surface.mesh.clone();
    for p in &mut inner.vertices {
        p.x *= 0.4;
        p.y *= 0.4;
        p.z *= 0.4;
    }
    let mut model = Scene::default();
    model.add_mesh(&inner, &[], [1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0]);
    let solid_plain = render(&mut renderer, &plain, &Scene::default());
    assert_eq!(
        render(&mut renderer, &plain, &model),
        solid_plain,
        "opaque stock occludes the model inside it"
    );
    let glass = cut_part_scene(&surface, None, false);
    assert_ne!(
        render(&mut renderer, &glass, &model),
        render(&mut renderer, &glass, &Scene::default()),
        "translucent stock lets the model show through"
    );
    let mut path = Scene::default();
    let line: Vec<_> = inner.vertices.iter().copied().take(2).collect();
    path.add_polyline(&line, [0.0, 1.0, 0.0, 1.0], false);
    assert_eq!(
        render(&mut renderer, &plain, &path),
        solid_plain,
        "opaque stock occludes internal toolpaths too"
    );
    let mut colours = Vec::new();
    for solid in [true, false] {
        let mut row = Vec::new();
        for value in [-0.4, 0.0, 0.2, 0.7, 1.4] {
            let bands = openchuck_app::pathview::deviation_bands(
                &vec![Some((value, false)); surface.mesh.triangles.len()],
                0.1,
            );
            let scene = cut_part_scene(&surface, Some(&bands), solid);
            let pixels = render(&mut renderer, &scene, &Scene::default());
            if !solid && value == 0.7 {
                assert_ne!(
                    pixels,
                    render(&mut renderer, &scene, &model),
                    "mapped colours remain translucent to the internal model"
                );
            }
            let centre = pixels[(SIZE as usize / 2) * SIZE as usize + SIZE as usize / 2];
            row.push([
                f32::from(centre[0]) / 255.0,
                f32::from(centre[1]) / 255.0,
                f32::from(centre[2]) / 255.0,
                1.0,
            ]);
        }
        use crate::roles::tests::measure::{contrast, difference, lightness, worst_difference};
        let background = [44.0 / 255.0, 49.0 / 255.0, 57.0 / 255.0, 1.0];
        for c in &row {
            assert!(
                contrast(*c, background) >= 3.0,
                "solid={solid}, contrast={}",
                contrast(*c, background)
            );
        }
        for pair in row.windows(2) {
            assert!(
                difference(pair[0], pair[1]) >= 20.0,
                "solid={solid}, difference={}",
                difference(pair[0], pair[1])
            );
            assert!(
                worst_difference(pair[0], pair[1]) >= 16.0,
                "solid={solid}, colour-vision difference={}",
                worst_difference(pair[0], pair[1])
            );
        }
        assert!(lightness(row[2]) > lightness(row[3]) && lightness(row[3]) > lightness(row[4]));
        colours.push(row);
    }
    eprintln!("actual rendered band colours: {colours:?}");
}

#[test]
fn req_sim_11_req_sim_05_surface_tooltips_pick_and_pin_the_visible_stock_in_both_modes() {
    let (mut rig, surface) = surface_fixture("simui-stock-probe");
    let ctx = egui::Context::default();
    let mut viewport = Viewport::new(true);
    viewport.set_scene(rig.shell.viewport.view().clone(), false);
    viewport.set_step(Some(ViewStep::Simulate));
    viewport.set_stock_surface(Some(Arc::clone(&surface)));
    viewport.set_bands(Some((
        surface.model.unwrap(),
        surface.bands.clone().unwrap(),
    )));
    viewport.set_view(StandardView::Front);
    viewport.fit();
    let frame = |viewport: &mut Viewport, time, events| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 480.0),
            )),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| viewport.show(ui));
        out.textures_delta.clear();
        fn shows_value(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(t) => t.galley.text().starts_with("Final result · "),
                egui::Shape::Vec(shapes) => shapes.iter().any(shows_value),
                _ => false,
            }
        }
        out.shapes.iter().any(|s| shows_value(&s.shape))
    };
    for (i, solid) in [false, true].into_iter().enumerate() {
        viewport.set_cut_part_solid(solid);
        frame(&mut viewport, i as f64 * 2.0, Vec::new());
        let at = viewport.last_rect.center();
        frame(
            &mut viewport,
            i as f64 * 2.0 + 0.1,
            vec![Event::PointerMoved(at)],
        );
        frame(
            &mut viewport,
            i as f64 * 2.0 + 0.5,
            vec![Event::PointerMoved(at)],
        );
        let triangle = viewport
            .probe_hover
            .expect("the visible cut stock is probed");
        let tip = viewport.probe_text(triangle).unwrap();
        assert!(tip.starts_with("Final result · ") && tip.ends_with("(nearest target sample)"));
        for pressed in [true, false] {
            frame(
                &mut viewport,
                i as f64 * 2.0 + 0.6,
                vec![Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }],
            );
        }
        let (pinned, point) = viewport.probe_pinned.unwrap();
        assert_eq!(pinned, triangle);
        let (origin, direction) = viewport.camera.ray(
            [0.0, 0.0],
            f64::from(viewport.last_rect.width() / viewport.last_rect.height()),
        );
        let expected = surface.pick(origin, direction).unwrap().unwrap();
        assert!(
            (point[0] - expected.point.x).abs() < 1e-6
                && (point[1] - expected.point.y).abs() < 1e-6
                && (point[2] - expected.point.z).abs() < 1e-6,
            "the pinned point lies on the displayed stock, not the target"
        );
    }
    viewport.set_view(StandardView::Back);
    assert!(
        !frame(&mut viewport, 8.0, vec![Event::PointerGone]),
        "rotating behind the stock hides the far-side pin"
    );
    viewport.set_view(StandardView::Front);
    assert!(
        frame(&mut viewport, 8.5, vec![Event::PointerGone]),
        "the retained pin returns when its surface is visible"
    );
    // Put the visible model in front of the same retained stock. Neither a
    // hover nor a previously pinned value may read through that surface.
    let app = rig.shell.app.as_mut().unwrap();
    let model = app.project().models()[0].id;
    app.execute(Command::SetModelTransform {
        model,
        transform: vec![
            openchuck_app::model::TransformStep::CentreOnRotaryAxis,
            openchuck_app::model::TransformStep::Translate {
                x: 0.0,
                y: -20.0,
                z: 0.0,
            },
        ],
    })
    .unwrap();
    viewport.set_scene(app.start_scene(None).wait().unwrap(), false);
    viewport.set_simulate_show(crate::viewport::SimulateShow {
        model: true,
        cut_part: true,
        toolpath: false,
    });
    for (i, solid) in [false, true].into_iter().enumerate() {
        viewport.set_cut_part_solid(solid);
        let at = viewport.last_rect.center();
        frame(
            &mut viewport,
            10.0 + i as f64,
            vec![Event::PointerMoved(at)],
        );
        let shows = frame(
            &mut viewport,
            10.5 + i as f64,
            vec![Event::PointerMoved(at)],
        );
        assert!(
            viewport.probe_hover.is_none(),
            "the model occludes the stock probe in either mode"
        );
        assert!(
            !shows,
            "a pinned stock value is hidden behind the visible model too"
        );
    }
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

fn paint_and_read(device: &wgpu::Device, queue: &wgpu::Queue, renderer: &Renderer) -> Vec<[u8; 4]> {
    let texture = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
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
    let target = texture(
        FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let depth = texture(DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT);
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
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 44.0 / 255.0,
                        g: 49.0 / 255.0,
                        b: 57.0 / 255.0,
                        a: 1.0,
                    }),
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

#[test]
fn req_sim_11_req_sim_05_a_coloured_part_is_probed_through_the_plain_cut_part() {
    // Decision 0492 item 4: with the part coloured, the pointer reads the
    // part's triangle; the cut part drawn plain in front of it is not probed.
    let (rig, surface) = surface_fixture("simui-part-probe");
    let ctx = egui::Context::default();
    let mut viewport = Viewport::new(true);
    let view = rig.shell.viewport.view().clone();
    let model = view.meshes[0].model.unwrap();
    viewport.set_scene(view, false);
    viewport.set_step(Some(ViewStep::Simulate));
    viewport.set_stock_surface(Some(Arc::clone(&surface)));
    viewport.set_bands(Some((
        surface.model.unwrap(),
        surface.bands.clone().unwrap(),
    )));
    viewport.set_simulate_show(crate::viewport::SimulateShow {
        model: true,
        cut_part: true,
        toolpath: false,
    });
    let values: Vec<Option<(f64, bool)>> = (0..12).map(|_| Some((-1.02, false))).collect();
    viewport.set_part_bands(Some((
        model,
        openchuck_app::pathview::deviation_bands(&values, 0.1),
    )));
    viewport.set_view(StandardView::Front);
    viewport.fit();
    let frame = |viewport: &mut Viewport, time: f64, events| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 480.0),
            )),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| viewport.show(ui));
        out.textures_delta.clear();
    };
    frame(&mut viewport, 0.0, Vec::new());
    let at = viewport.last_rect.center();
    frame(&mut viewport, 0.1, vec![Event::PointerMoved(at)]);
    frame(&mut viewport, 0.5, vec![Event::PointerMoved(at)]);
    let triangle = viewport.probe_hover.expect("the part is probed");
    assert!(
        triangle < 12,
        "the part's own triangle, not the cut surface's"
    );
    assert_eq!(
        viewport.probe_text(triangle).as_deref(),
        Some("Final result \u{b7} Overcut 1.020 mm")
    );
}

#[test]
fn req_sim_11_req_sim_05_a_pinned_part_value_hides_behind_nearer_part_faces() {
    // Decision 0492 item 5 with decision 0238 item 4: a value pinned on the
    // coloured part is read through the plain cut part, never through the
    // part itself. Turned to the far side, the nearer faces hide it; turned
    // back, it shows again (review of #472).
    let (rig, surface) = surface_fixture("simui-part-pin-hidden");
    let ctx = egui::Context::default();
    let mut viewport = Viewport::new(true);
    let view = rig.shell.viewport.view().clone();
    let model = view.meshes[0].model.unwrap();
    viewport.set_scene(view, false);
    viewport.set_step(Some(ViewStep::Simulate));
    viewport.set_stock_surface(Some(Arc::clone(&surface)));
    viewport.set_bands(Some((
        surface.model.unwrap(),
        surface.bands.clone().unwrap(),
    )));
    viewport.set_simulate_show(crate::viewport::SimulateShow {
        model: true,
        cut_part: true,
        toolpath: false,
    });
    let values: Vec<Option<(f64, bool)>> = (0..12).map(|_| Some((-1.02, false))).collect();
    viewport.set_part_bands(Some((
        model,
        openchuck_app::pathview::deviation_bands(&values, 0.1),
    )));
    viewport.set_view(StandardView::Front);
    viewport.fit();
    let frame = |viewport: &mut Viewport, time: f64, events| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 480.0),
            )),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| viewport.show(ui));
        out.textures_delta.clear();
        fn shows_value(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(t) => t.galley.text().starts_with("Final result · "),
                egui::Shape::Vec(shapes) => shapes.iter().any(shows_value),
                _ => false,
            }
        }
        out.shapes.iter().any(|s| shows_value(&s.shape))
    };
    frame(&mut viewport, 0.0, Vec::new());
    let at = viewport.last_rect.center();
    for pressed in [true, false] {
        frame(
            &mut viewport,
            0.1,
            vec![Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    let (triangle, _) = viewport
        .probe_pinned
        .expect("a value is pinned on the part");
    assert!(triangle < 12, "pinned on the part's own triangle");
    assert!(
        frame(&mut viewport, 1.0, vec![Event::PointerGone]),
        "the pinned value shows while its face is in view"
    );
    viewport.set_view(StandardView::Back);
    assert!(
        !frame(&mut viewport, 2.0, vec![Event::PointerGone]),
        "turned to the far side, the part's nearer faces hide the pinned value"
    );
    viewport.set_view(StandardView::Front);
    assert!(
        frame(&mut viewport, 3.0, vec![Event::PointerGone]),
        "the kept value shows again when its face is in view"
    );
}

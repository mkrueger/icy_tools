use super::*;
use eframe::egui;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub struct Fixture(pub PathBuf);

impl Fixture {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!("icy-view-egui-{}-{}", std::process::id(), fastrand::u64(..)));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn config() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| icy_view::init_config_dir(false, Some(std::env::temp_dir().join(format!("icy-view-egui-config-{}", std::process::id())))));
}

pub fn wait_browser(browser: &mut browser::Browser, context: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while browser.loading {
        browser.poll(context);
        assert!(Instant::now() < deadline, "browser timed out");
        std::thread::yield_now();
    }
    assert!(browser.error.is_none(), "{:?}", browser.error);
}

pub fn wait_preview(preview: &mut preview::Preview, context: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while preview.loading {
        preview.poll(context);
        assert!(Instant::now() < deadline, "preview timed out: {}", preview.file);
        std::thread::yield_now();
    }
}

#[test]
fn browser_preserves_selection_across_history_and_enters_archives() {
    use std::io::Write;
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("art.ans"), b"ANSI").unwrap();
    let mut archive = zip::ZipWriter::new(std::fs::File::create(fixture.0.join("pack.zip")).unwrap());
    archive.start_file("sub/inner.ans", zip::write::SimpleFileOptions::default()).unwrap();
    archive.write_all(b"INNER").unwrap();
    archive.finish().unwrap();
    let context = egui::Context::default();
    let mut browser = browser::Browser::new(fixture.0.clone(), Default::default()).unwrap();
    browser.refresh(&context);
    wait_browser(&mut browser, &context);
    let art = browser.items.iter().position(|item| item.get_label() == "art.ans").unwrap();
    browser.select(art, &context);
    let archive = browser.items.iter().position(|item| item.get_label() == "pack.zip").unwrap();
    browser.enter(archive, &context);
    wait_browser(&mut browser, &context);
    assert_eq!(browser.items[0].get_label(), "sub");
    browser.enter(0, &context);
    wait_browser(&mut browser, &context);
    assert_eq!(browser.items[0].get_label(), "inner.ans");
    browser.up(&context);
    wait_browser(&mut browser, &context);
    browser.history(false, &context);
    wait_browser(&mut browser, &context);
    assert_eq!(browser.items[browser.selected.unwrap()].get_label(), "art.ans");
    browser.history(true, &context);
    wait_browser(&mut browser, &context);
    assert_eq!(browser.items[0].get_label(), "sub");
}

#[test]
fn list_double_click_enters_containers_without_invalidating_remaining_rows() {
    use std::io::Write;

    let fixture = Fixture::new();
    std::fs::create_dir(fixture.0.join("folder")).unwrap();
    std::fs::write(fixture.0.join("folder/inner.ans"), b"INNER").unwrap();
    std::fs::write(fixture.0.join("tail.ans"), b"TAIL").unwrap();
    let mut archive = zip::ZipWriter::new(std::fs::File::create(fixture.0.join("pack.zip")).unwrap());
    archive.start_file("inner.ans", zip::write::SimpleFileOptions::default()).unwrap();
    archive.write_all(b"INNER").unwrap();
    archive.finish().unwrap();

    for size in [egui::vec2(1100.0, 760.0), egui::vec2(360.0, 640.0)] {
        for container in ["folder", "pack.zip"] {
            let context = egui::Context::default();
            icy_engine_gui::egui::appearance::apply(&context);
            let mut viewer = app::Viewer::new(fixture.0.clone(), Default::default(), &context).unwrap();
            wait_browser(&mut viewer.browser, &context);
            let mut time = 0.0;
            let mut frame = |viewer: &mut app::Viewer, events| {
                time += 0.05;
                context.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        time: Some(time),
                        events,
                        ..Default::default()
                    },
                    |context| viewer.show(context),
                )
            };
            for _ in 0..3 {
                frame(&mut viewer, vec![]);
            }
            let output = frame(&mut viewer, vec![]);
            let label_bounds = |label: &str| {
                output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == label => {
                            let bounds = text.galley.rect.translate(text.pos.to_vec2());
                            shape.clip_rect.contains_rect(bounds).then_some(bounds)
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("missing visible row: {label}"))
            };
            let row = label_bounds(container);
            assert!(label_bounds("tail.ans").top() > row.bottom());
            let position = row.center();
            let original_path = viewer.browser.location.point.path.clone();
            for click in 0..2 {
                for pressed in [true, false] {
                    frame(
                        &mut viewer,
                        vec![
                            egui::Event::PointerMoved(position),
                            egui::Event::PointerButton {
                                pos: position,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                }
                if click == 0 {
                    assert_eq!(viewer.browser.location.point.path, original_path);
                    assert_eq!(viewer.browser.items[viewer.browser.selected.unwrap()].get_label(), container);
                }
            }
            assert_ne!(viewer.browser.location.point.path, original_path);
            assert_eq!(viewer.browser.back.len(), 1);
            wait_browser(&mut viewer.browser, &context);
            assert_eq!(viewer.browser.items.len(), 1);
            assert_eq!(viewer.browser.items[0].get_label(), "inner.ans");
            frame(&mut viewer, vec![]);
        }
    }
}

#[test]
fn preview_loads_text_images_and_reports_corrupt_formats() {
    let context = egui::Context::default();
    let mut preview = preview::Preview::new(&context).unwrap();
    preview.load("art.ans".into(), b"\x1b[31mHELLO\r\n".to_vec(), false, &context);
    wait_preview(&mut preview, &context);
    assert!(preview.error.is_none());
    assert_eq!(preview.screen.terminal.screen.lock().char_at((0, 0).into()).ch, 'H');
    preview.load("icon.png".into(), include_bytes!("../../../build/linux/128x128.png").to_vec(), false, &context);
    wait_preview(&mut preview, &context);
    assert!(preview.image.is_some());
    assert_eq!(preview.image_pixels.as_ref().unwrap().dimensions(), (128, 128));
    preview.load("invalid.icy".into(), b"invalid".to_vec(), false, &context);
    wait_preview(&mut preview, &context);
    assert!(preview.error.is_some());
}

#[test]
fn shortcuts_use_exact_modifiers_and_all_viewer_commands_resolve() {
    let commands = icy_view::commands::create_icy_view_commands();
    for id in dialogs::COMMANDS {
        let command = commands.get(id).unwrap();
        for hotkey in command.active_hotkeys() {
            assert!(icy_engine_gui::egui::shortcuts::key(hotkey.key).is_some(), "{id}: {:?}", hotkey.key);
        }
        let label = text(&command.fluent_action_key());
        assert!(!label.contains("No localization"), "{id}: {label}");
    }
    let context = egui::Context::default();
    let mut matched = true;
    let _ = context.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::F3,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::SHIFT,
            }],
            ..Default::default()
        },
        |context| {
            matched = icy_engine_gui::egui::shortcuts::consume(context, commands.get("playback.baud_rate").unwrap());
            assert!(icy_engine_gui::egui::shortcuts::consume(
                context,
                commands.get("playback.baud_rate_back").unwrap()
            ));
        },
    );
    assert!(!matched);
}

struct Gpu {
    device: eframe::wgpu::Device,
    queue: eframe::wgpu::Queue,
    renderer: eframe::egui_wgpu::Renderer,
    context: egui::Context,
    time: f64,
    labels: std::collections::HashMap<String, egui::Rect>,
    copied: Vec<String>,
}

impl Gpu {
    async fn new() -> Self {
        use eframe::{egui_wgpu, wgpu};
        let adapter = wgpu::Instance::default().request_adapter(&Default::default()).await.unwrap();
        let (device, queue) = adapter.request_device(&Default::default()).await.unwrap();
        let mut renderer = egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, Default::default());
        renderer
            .callback_resources
            .insert(icy_engine_gui::TerminalShaderRenderer::new(&device, wgpu::TextureFormat::Rgba8Unorm));
        Self {
            device,
            queue,
            renderer,
            context: egui::Context::default(),
            time: 0.0,
            labels: Default::default(),
            copied: Vec::new(),
        }
    }

    fn capture(&mut self, app: &mut app::Viewer, size: [u32; 2], scale: f32, events: Vec<egui::Event>, name: &str) -> Vec<u8> {
        use eframe::{egui_wgpu, wgpu};
        self.time += 0.1;
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0] as f32 / scale, size[1] as f32 / scale),
            )),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(scale);
        let output = self.context.run(input, |context| app.show(context));
        for command in &output.platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = command {
                self.copied.push(text.clone());
            }
        }
        self.labels.clear();
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape {
                let bounds = text.galley.rect.translate(text.pos.to_vec2());
                if shape.clip_rect.contains_rect(bounds) && self.context.content_rect().contains_rect(bounds) {
                    self.labels.insert(text.galley.text().to_string(), bounds);
                }
            }
        }
        let jobs = self.context.tessellate(output.shapes, output.pixels_per_point);
        for (id, delta) in &output.textures_delta.set {
            self.renderer.update_texture(&self.device, &self.queue, *id, delta);
        }
        let descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: scale,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewer capture"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let stride = (size[0] * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(stride * size[1]),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let commands = self.renderer.update_buffers(&self.device, &self.queue, &mut encoder, &jobs, &descriptor);
        {
            let view = texture.create_view(&Default::default());
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });
            self.renderer.render(&mut pass.forget_lifetime(), &jobs, &descriptor);
        }
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: None,
                },
            },
            texture.size(),
        );
        self.queue.submit(commands.into_iter().chain([encoder.finish()]));
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receiver.recv().unwrap().unwrap();
        let pixels: Vec<_> = buffer
            .slice(..)
            .get_mapped_range()
            .chunks(stride as usize)
            .flat_map(|row| row[..size[0] as usize * 4].iter().copied())
            .collect();
        buffer.unmap();
        for id in output.textures_delta.free {
            self.renderer.free_texture(&id);
        }
        if let Some(directory) = std::env::var_os("ICY_EGUI_SCREENSHOTS") {
            std::fs::create_dir_all(&directory).unwrap();
            image::save_buffer(
                PathBuf::from(directory).join(format!("{name}.png")),
                &pixels,
                size[0],
                size[1],
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
        pixels
    }

    fn click(&mut self, app: &mut app::Viewer, size: [u32; 2], scale: f32, label: &str) {
        let position = self
            .labels
            .get(label)
            .unwrap_or_else(|| panic!("missing visible control {label}: {:?}", self.labels.keys()))
            .center();
        for pressed in [true, false] {
            self.capture(
                app,
                size,
                scale,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                "click",
            );
        }
    }
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_file_navigation_selection_copy_and_large_image_tiles() {
    config();
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("first.ans"), b"HELLO WORLD\r\nSECOND LINE").unwrap();
    std::fs::write(fixture.0.join("second.ans"), b"NEXT").unwrap();
    let mut gpu = futures::executor::block_on(Gpu::new());
    icy_engine_gui::egui::appearance::apply(&gpu.context);
    let options = icy_view::Options {
        auto_scroll_enabled: false,
        ..Default::default()
    };
    let mut app = app::Viewer::new(fixture.0.clone(), options, &gpu.context).unwrap();
    wait_browser(&mut app.browser, &gpu.context);
    gpu.capture(&mut app, [1100, 760], 1.0, vec![], "workflow-warmup");
    let key = |key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    gpu.capture(&mut app, [1100, 760], 1.0, vec![key(egui::Key::ArrowDown)], "workflow-selected");
    assert_eq!(app.browser.items[app.browser.selected.unwrap()].get_label(), "first.ans");
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.browser.preview_loading || app.preview.loading {
        gpu.capture(&mut app, [1100, 760], 1.0, vec![], "workflow-loading");
        assert!(Instant::now() < deadline);
    }
    app.options.monitor_settings.scaling_mode = icy_engine_gui::ScalingMode::Manual(1.0);
    gpu.capture(&mut app, [1100, 760], 1.0, vec![], "selection-warmup");
    gpu.capture(&mut app, [1100, 760], 1.0, vec![], "selection-before");
    let info = app.preview.screen.terminal.render_info.read().clone();
    let origin = egui::pos2(
        info.bounds_x + info.viewport_x + info.font_width * info.display_scale * 0.5,
        info.bounds_y + info.viewport_y + info.font_height * info.display_scale * 0.5,
    );
    let end = origin + egui::vec2(info.font_width * info.display_scale * 4.0, 0.0);
    gpu.capture(
        &mut app,
        [1100, 760],
        1.0,
        vec![
            egui::Event::PointerMoved(origin),
            egui::Event::PointerButton {
                pos: origin,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        "selection-press",
    );
    gpu.capture(&mut app, [1100, 760], 1.0, vec![egui::Event::PointerMoved(end)], "selection-drag");
    gpu.capture(
        &mut app,
        [1100, 760],
        1.0,
        vec![
            egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Copy,
        ],
        "selection-copy",
    );
    assert_eq!(gpu.copied.last().map(String::as_str), Some("HELLO"));
    gpu.capture(&mut app, [1100, 760], 1.0, vec![key(egui::Key::ArrowDown)], "workflow-next");
    assert_eq!(app.browser.items[app.browser.selected.unwrap()].get_label(), "second.ans");
    while app.browser.preview_loading || app.preview.loading {
        gpu.capture(&mut app, [1100, 760], 1.0, vec![], "workflow-next-loading");
        assert!(Instant::now() < deadline);
    }
    let image = image::RgbaImage::from_fn(64, 20000, |column, row| {
        image::Rgba([if column < 32 { 250 } else { 20 }, (row % 256) as u8, 100, 255])
    });
    let mut data = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image).write_to(&mut data, image::ImageFormat::Png).unwrap();
    app.preview.load("tall.png".into(), data.into_inner(), false, &gpu.context);
    wait_preview(&mut app.preview, &gpu.context);
    assert_eq!(app.preview.image_pixels.as_ref().unwrap().height(), 20000);
    gpu.capture(&mut app, [1100, 760], 1.0, vec![], "tall-top");
    app.preview.screen.scroll_to = Some(egui::vec2(0.0, f32::MAX));
    let pixels = gpu.capture(&mut app, [1100, 760], 1.0, vec![], "tall-bottom");
    assert!(app.preview.screen.offset.y > 19000.0);
    assert!(
        pixels.chunks_exact(4).filter(|pixel| pixel[0] == 250 && pixel[2] == 100).count() > 10000,
        "tall image lost its bottom tiles"
    );
    app.options.view_mode = icy_view::ViewMode::Tiles;
    for _ in 0..6 {
        gpu.capture(&mut app, [1100, 760], 1.0, vec![], "tiles");
    }
    assert!(gpu.labels.contains_key("first.ans") && gpu.labels.contains_key("second.ans"));
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_original_masonry_and_sauce_list_navigation() {
    use icy_engine::{AttributedChar, FileFormat, SaveOptions, TextAttribute, TextBuffer};
    config();
    let fixture = Fixture::new();
    for (name, columns, rows) in [("a-80.xb", 80, 80), ("b-160.xb", 160, 14), ("c-240.xb", 240, 18)] {
        let mut buffer = TextBuffer::new((columns, rows));
        for row in 0..rows {
            for column in 0..columns {
                buffer.layers[0].set_char(
                    (column, row),
                    AttributedChar::new('\u{00db}', TextAttribute::from_color(((column / 8 + row / 8) % 15 + 1) as u8, 0)),
                );
            }
        }
        let options = SaveOptions {
            sauce: Some(icy_sauce::MetaData {
                title: "COLOR STUDY".into(),
                author: "Test Artist".into(),
                group: "TEST GROUP".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        std::fs::write(fixture.0.join(name), FileFormat::XBin.to_bytes(&buffer, &options).unwrap()).unwrap();
    }
    image::RgbaImage::from_fn(320, 2600, |column, row| {
        image::Rgba([if column < 160 { 250 } else { 20 }, (row % 256) as u8, 100, 255])
    })
    .save(fixture.0.join("d-tall.png"))
    .unwrap();
    let mut gpu = futures::executor::block_on(Gpu::new());
    icy_engine_gui::egui::appearance::apply(&gpu.context);
    let mut app = app::Viewer::new(
        fixture.0.clone(),
        icy_view::Options {
            view_mode: icy_view::ViewMode::Tiles,
            auto_scroll_enabled: false,
            ..Default::default()
        },
        &gpu.context,
    )
    .unwrap();
    wait_browser(&mut app.browser, &gpu.context);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        gpu.capture(&mut app, [1400, 1000], 1.0, vec![], "masonry-loading");
        let items = &app.tiles.layout.as_ref().unwrap().items;
        if items[0].height > 600.0 && items[1].width == 680.0 && items[2].width == 1028.0 && items[3].height > 2500.0 {
            break;
        }
        assert!(Instant::now() < deadline, "thumbnail dimensions not loaded: {items:?}");
    }
    let pixels = gpu.capture(&mut app, [1400, 1000], 1.0, vec![], "masonry-desktop");
    assert!(
        pixels
            .chunks_exact(4)
            .filter(|pixel| pixel[0].abs_diff(pixel[1]) > 60 || pixel[1].abs_diff(pixel[2]) > 60)
            .count()
            > 150000
    );
    let key = |key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    for (size, scale, name) in [([360, 640], 1.0, "masonry-narrow"), ([1600, 1200], 2.0, "masonry-hidpi")] {
        for _ in 0..2 {
            gpu.capture(&mut app, size, scale, vec![], name);
        }
        let items = &app.tiles.layout.as_ref().unwrap().items;
        for (index, item) in items.iter().enumerate() {
            let bounds = egui::Rect::from_min_size(egui::pos2(item.x, item.y), egui::vec2(item.width, item.height));
            assert!(bounds.right() <= size[0] as f32 / scale);
            for other in &items[index + 1..] {
                assert!(!bounds.intersects(egui::Rect::from_min_size(egui::pos2(other.x, other.y), egui::vec2(other.width, other.height))));
            }
        }
    }
    gpu.capture(&mut app, [1400, 1000], 1.0, vec![key(egui::Key::End)], "masonry-end");
    let last = app.browser.selected.unwrap();
    assert_eq!(app.browser.items[last].get_label(), "d-tall.png");
    for _ in 0..8 {
        gpu.capture(&mut app, [1400, 1000], 1.0, vec![], "masonry-end");
    }
    let selected_tile = app.tiles.layout.as_ref().unwrap().items.iter().find(|tile| tile.index == last).unwrap();
    assert!(app.tiles.offset > 0.0 && selected_tile.y >= app.tiles.offset - 1.0 && selected_tile.y < app.tiles.offset + app.tiles.viewport_height);
    gpu.capture(
        &mut app,
        [1400, 1000],
        1.0,
        vec![
            egui::Event::PointerMoved(egui::pos2(500.0, 500.0)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -4000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        "masonry-scrolling",
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.tiles.offset + app.tiles.viewport_height < app.tiles.layout.as_ref().unwrap().content_height - 2.0 {
        gpu.capture(&mut app, [1400, 1000], 1.0, vec![], "masonry-scrolling");
        assert!(Instant::now() < deadline, "tile grid did not scroll to the image bottom");
    }
    let pixels = gpu.capture(&mut app, [1400, 1000], 1.0, vec![], "masonry-bottom");
    assert!(pixels.chunks_exact(4).filter(|pixel| pixel[0] == 250 && pixel[2] == 100).count() > 10000);
    app.options.view_mode = icy_view::ViewMode::List;
    for _ in 0..2 {
        gpu.capture(&mut app, [1400, 1000], 1.0, vec![], "list-compact");
    }
    gpu.click(&mut app, [1400, 1000], 1.0, "SAUCE");
    assert!(app.options.sauce_mode);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !gpu.labels.contains_key("TEST GROUP") {
        gpu.capture(&mut app, [1400, 1000], 1.0, vec![], "list-sauce");
        assert!(Instant::now() < deadline, "missing SAUCE columns: {:?}", gpu.labels.keys());
    }
    assert!(gpu.labels["COLOR STUDY"].right() < gpu.labels["Test Artist"].left());
    assert!(gpu.labels["Test Artist"].right() < gpu.labels["TEST GROUP"].left());
    gpu.click(&mut app, [1400, 1000], 1.0, &text("header-name"));
    assert_eq!(app.browser.sort, icy_view::sort_order::SortOrder::NameDesc);
    assert_eq!(app.browser.items[app.browser.selected.unwrap()].get_label(), "d-tall.png");
    for (keycode, expected) in [
        (egui::Key::End, "a-80.xb"),
        (egui::Key::Home, "d-tall.png"),
        (egui::Key::PageDown, "a-80.xb"),
        (egui::Key::PageUp, "d-tall.png"),
    ] {
        gpu.capture(&mut app, [1400, 1000], 1.0, vec![key(keycode)], "list-navigation");
        assert_eq!(app.browser.items[app.browser.selected.unwrap()].get_label(), expected);
    }
    gpu.context.set_visuals(egui::Visuals::light());
    for _ in 0..2 {
        gpu.capture(&mut app, [360, 640], 1.0, vec![], "list-sauce-narrow");
    }
    assert!(gpu.labels.contains_key("SAUCE") && gpu.labels.contains_key("d-tall.png"));
}

#[test]
fn tile_toolbar_overlays_the_grid_and_returns_through_the_hover_zone() {
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.0.join("folder")).unwrap();
    std::fs::write(fixture.0.join("folder/inner.ans"), b"INNER").unwrap();
    let context = egui::Context::default();
    icy_engine_gui::egui::appearance::apply(&context);
    let options = icy_view::Options {
        view_mode: icy_view::ViewMode::Tiles,
        ..Default::default()
    };
    let mut viewer = app::Viewer::new(fixture.0.join("folder"), options, &context).unwrap();
    wait_browser(&mut viewer.browser, &context);
    let size = egui::vec2(1100.0, 760.0);
    let mut time = 0.0;
    let mut frame = |viewer: &mut app::Viewer, events| {
        time += 0.05;
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                time: Some(time),
                events,
                ..Default::default()
            },
            |context| viewer.show(context),
        );
    };
    for _ in 0..3 {
        frame(&mut viewer, vec![]);
    }
    assert!(viewer.tile_toolbar.visible, "the toolbar starts visible");
    let bar = viewer.tile_toolbar.rect;
    assert!(bar.width() > 0.0 && bar.width() < size.x * 0.5, "the toolbar only covers a corner: {bar:?}");
    assert!(bar.top() > 0.0 && bar.top() < size.y * 0.5, "the toolbar sits at the top of the tiles: {bar:?}");

    let position = bar.min + egui::vec2(20.0, bar.height() / 2.0);
    let original = viewer.browser.location.point.path.clone();
    for pressed in [true, false] {
        frame(
            &mut viewer,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_ne!(viewer.browser.location.point.path, original, "the overlay up button must work");
    wait_browser(&mut viewer.browser, &context);

    viewer.tile_toolbar.visible = false;
    frame(&mut viewer, vec![egui::Event::PointerMoved(bar.center() + egui::vec2(0.0, size.y * 0.5))]);
    assert!(!viewer.tile_toolbar.visible, "pointing at the tiles keeps the toolbar hidden");
    frame(&mut viewer, vec![egui::Event::PointerMoved(bar.min + egui::Vec2::splat(4.0))]);
    assert!(viewer.tile_toolbar.visible, "the top left corner brings the toolbar back");
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_status_bar_sauce_info_and_shuffle_overlay() {
    use icy_engine::{AttributedChar, FileFormat, SaveOptions, TextAttribute, TextBuffer};
    config();
    let fixture = Fixture::new();
    for (name, title) in [("a.xb", "FIRST ART"), ("b.xb", "SECOND ART")] {
        let mut buffer = TextBuffer::new((80, 25));
        for row in 0..25 {
            for column in 0..80 {
                buffer.layers[0].set_char((column, row), AttributedChar::new('\u{00b1}', TextAttribute::from_color(7, 0)));
            }
        }
        let options = SaveOptions {
            sauce: Some(icy_sauce::MetaData {
                title: title.into(),
                author: "Test Artist".into(),
                group: "TEST GROUP".into(),
                comments: vec!["FIRST COMMENT LINE".into(), "SECOND COMMENT LINE".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        std::fs::write(fixture.0.join(name), FileFormat::XBin.to_bytes(&buffer, &options).unwrap()).unwrap();
    }
    let mut gpu = futures::executor::block_on(Gpu::new());
    icy_engine_gui::egui::appearance::apply(&gpu.context);
    let options = icy_view::Options {
        auto_scroll_enabled: false,
        ..Default::default()
    };
    let mut app = app::Viewer::new(fixture.0.clone(), options, &gpu.context).unwrap();
    wait_browser(&mut app.browser, &gpu.context);
    let size = [1100, 760];
    gpu.capture(&mut app, size, 1.0, vec![], "status-warmup");
    gpu.capture(
        &mut app,
        size,
        1.0,
        vec![egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        "status-selected",
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.browser.preview_loading || app.preview.loading {
        gpu.capture(&mut app, size, 1.0, vec![], "status-loading");
        assert!(Instant::now() < deadline, "preview timed out");
    }
    gpu.capture(&mut app, size, 1.0, vec![], "status-sauce");
    let summary = gpu
        .labels
        .keys()
        .find(|label| label.starts_with("FIRST ART"))
        .cloned()
        .unwrap_or_else(|| panic!("status bar misses the SAUCE summary: {:?}", gpu.labels.keys()));
    for part in ["Test Artist", "TEST GROUP", "80\u{00d7}25"] {
        assert!(summary.contains(part), "status bar misses {part}: {summary}");
    }
    assert!(!summary.contains("0000"), "an empty SAUCE date must stay hidden: {summary}");
    gpu.click(&mut app, size, 1.0, &summary);
    assert_eq!(app.dialogs.mode, Some(dialogs::Mode::Sauce), "clicking the SAUCE summary must open the dialog");
    for _ in 0..2 {
        gpu.capture(&mut app, size, 1.0, vec![], "sauce-dialog");
    }
    for label in ["FIRST ART", "Test Artist  •  TEST GROUP", "80 × 25"] {
        assert!(gpu.labels.contains_key(label), "SAUCE dialog misses {label}: {:?}", gpu.labels.keys());
    }
    assert!(
        gpu.labels
            .keys()
            .any(|label| label.contains("FIRST COMMENT LINE") && label.contains("SECOND COMMENT LINE")),
        "SAUCE dialog misses the comments: {:?}",
        gpu.labels.keys()
    );
    gpu.click(&mut app, size, 1.0, &text("sauce-btn-raw"));
    for _ in 0..2 {
        gpu.capture(&mut app, size, 1.0, vec![], "sauce-dialog-raw");
    }
    assert!(
        gpu.labels.contains_key(&text("sauce-section-technical")) && gpu.labels.contains_key(&text("sauce-btn-formatted")),
        "raw SAUCE view: {:?}",
        gpu.labels.keys()
    );
    gpu.click(&mut app, size, 1.0, &text("sauce-btn-formatted"));
    app.dialogs.mode = None;

    app.shuffle_start(&gpu.context);
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.browser.preview_loading || app.preview.loading {
        gpu.capture(&mut app, size, 1.0, vec![], "shuffle-loading");
        assert!(Instant::now() < deadline, "shuffle preview timed out");
    }
    let playing = app.browser.items[app.browser.selected.unwrap()].get_label();
    gpu.capture(&mut app, size, 1.0, vec![], "shuffle-overlay");
    let title = if playing == "a.xb" { "FIRST ART" } else { "SECOND ART" };
    for label in [title, "by Test Artist", "TEST GROUP"] {
        assert!(gpu.labels.contains_key(label), "shuffle overlay misses {label}: {:?}", gpu.labels.keys());
    }
    // The slideshow shows the artwork alone.
    assert!(!gpu.labels.contains_key("a.xb") && !gpu.labels.contains_key("b.xb"));
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_viewer_and_dialogs_fit_desktop_narrow_short_and_hidpi() {
    config();
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("welcome.xb"), include_bytes!("../../../data/welcome.xb")).unwrap();
    let mut gpu = futures::executor::block_on(Gpu::new());
    icy_engine_gui::egui::appearance::apply(&gpu.context);
    let mut app = app::Viewer::new(fixture.0.clone(), Default::default(), &gpu.context).unwrap();
    wait_browser(&mut app.browser, &gpu.context);
    gpu.capture(&mut app, [1100, 760], 1.0, vec![], "desktop-warmup");
    let pixels = gpu.capture(&mut app, [1100, 760], 1.0, vec![], "desktop");
    let info = app.preview.screen.terminal.render_info.read().clone();
    let mut colors = std::collections::HashSet::new();
    for row in info.bounds_y as usize..(info.bounds_y + info.bounds_height) as usize {
        for column in info.bounds_x as usize..(info.bounds_x + info.bounds_width) as usize {
            let offset = (row * 1100 + column) * 4;
            colors.insert(&pixels[offset..offset + 3]);
        }
    }
    assert!(colors.len() > 8, "blank terminal preview: {} colors", colors.len());
    app.preview
        .load("welcome.xb".into(), include_bytes!("../../../data/welcome.xb").to_vec(), false, &gpu.context);
    wait_preview(&mut app.preview, &gpu.context);
    for (size, scale, name) in [
        ([1100, 760], 1.0, "desktop"),
        ([360, 640], 1.0, "narrow"),
        ([1600, 1200], 2.0, "hidpi"),
        ([360, 240], 1.0, "short"),
    ] {
        for dark in [true, false] {
            gpu.context.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
            for mode in [
                dialogs::Mode::Settings,
                dialogs::Mode::About,
                dialogs::Mode::Help,
                dialogs::Mode::Sauce,
                dialogs::Mode::Export,
            ] {
                app.dialogs.open(mode, &app.options, &app.preview);
                gpu.capture(&mut app, size, scale, vec![], "warmup");
                gpu.capture(&mut app, size, scale, vec![], &format!("{name}-{dark}-{}", mode as u8));
                let action = text(match mode {
                    dialogs::Mode::Settings => "dialog-ok-button",
                    dialogs::Mode::Export => "egui-save",
                    _ => "dialog-close-button",
                });
                assert!(gpu.labels.contains_key(&action), "{name}, mode {}: hidden {action}", mode as u8);
                if mode == dialogs::Mode::Export {
                    assert!(!gpu.labels.contains_key(&text("cmd-file-export-action")), "{name}: redundant export heading");
                    assert!(!gpu.labels.contains_key("×"), "{name}: export dialog has a close glyph");
                    if name == "narrow" {
                        gpu.capture(
                            &mut app,
                            size,
                            scale,
                            vec![
                                egui::Event::PointerMoved(egui::pos2(180.0, 300.0)),
                                egui::Event::MouseWheel {
                                    unit: egui::MouseWheelUnit::Point,
                                    delta: egui::vec2(0.0, 4000.0),
                                    modifiers: egui::Modifiers::NONE,
                                },
                            ],
                            "export-scroll-top",
                        );
                        for _ in 0..5 {
                            gpu.capture(&mut app, size, scale, vec![], "export-scroll-top-settle");
                        }
                        let path = *gpu
                            .labels
                            .get(&text("settings-paths-export-path"))
                            .unwrap_or_else(|| panic!("missing export path {dark}: {:?}", gpu.labels.keys()));
                        let filename = *gpu
                            .labels
                            .get(&text("header-name"))
                            .unwrap_or_else(|| panic!("missing filename {dark}: {:?}", gpu.labels.keys()));
                        let footer = gpu.labels[&action];
                        assert!(filename.top() > path.top(), "{name}: filename must follow export path");
                        assert!(
                            filename.top() - path.bottom() < 100.0,
                            "{name}: gap between path and filename: {path:?} -> {filename:?}"
                        );
                        assert!(
                            filename.bottom() < footer.top(),
                            "{name}: filename must be visible above footer: {filename:?} / {footer:?}"
                        );
                        gpu.capture(
                            &mut app,
                            size,
                            scale,
                            vec![
                                egui::Event::PointerMoved(filename.center()),
                                egui::Event::MouseWheel {
                                    unit: egui::MouseWheelUnit::Point,
                                    delta: egui::vec2(0.0, -400.0),
                                    modifiers: egui::Modifiers::NONE,
                                },
                            ],
                            "export-scroll",
                        );
                        for _ in 0..5 {
                            gpu.capture(&mut app, size, scale, vec![], "export-scroll-settle");
                        }
                        assert!(
                            gpu.labels.contains_key(&text("egui-normalize-spaces")),
                            "{name}: export options must scroll into view"
                        );
                        assert_eq!(gpu.labels[&action], footer, "{name}: scrolling must not move the footer");
                    }
                }
                if mode == dialogs::Mode::Settings {
                    let before = gpu.labels[&action];
                    for tab in ["settings-commands-category", "settings-paths-category", "settings-monitor-category"] {
                        gpu.click(&mut app, size, scale, &text(tab));
                        gpu.capture(&mut app, size, scale, vec![], &format!("{name}-{dark}-{tab}"));
                        let after = gpu.labels[&action];
                        assert!(
                            (after.center() - before.center()).length() <= 1.5 && after.size() == before.size(),
                            "dialog jumps between tabs: {name}: {before:?} -> {after:?}"
                        );
                    }
                }
                app.dialogs.mode = None;
            }
        }
    }
}

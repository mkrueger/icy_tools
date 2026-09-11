use super::*;
use icy_mail::reader::{NavigateDirection, Pane, ViewMode};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn loaded(context: &egui::Context) -> (packet_tests::TempDir, app::MailApp) {
    let (dir, package) = packet_tests::load();
    let mut mail = app::MailApp::new(context);
    mail.reader.set_package(Arc::new(package));
    wait(&mut mail, context);
    (dir, mail)
}

fn wait(mail: &mut app::MailApp, context: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        mail.poll(context);
        if mail.loading.is_none() && !mail.body_loading {
            break;
        }
        assert!(Instant::now() < deadline, "mail worker timed out");
        std::thread::yield_now();
    }
    assert!(mail.error.is_none(), "{:?}", mail.error);
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn pointer(position: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(position),
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

fn frame(context: &egui::Context, mail: &mut app::MailApp, size: egui::Vec2, events: Vec<egui::Event>) -> egui::FullOutput {
    let time = context.input(|input| input.time) + 0.05;
    let modifiers = events
        .iter()
        .find_map(|event| match event {
            egui::Event::Key { modifiers, .. } => Some(*modifiers),
            _ => None,
        })
        .unwrap_or_default();
    context.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(time),
            events,
            modifiers,
            ..Default::default()
        },
        |context| mail.show(context),
    )
}

fn label(output: &egui::FullOutput, label: &str) -> egui::Rect {
    output
        .shapes
        .iter()
        .rev()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let bounds = text.galley.rect.translate(text.pos.to_vec2());
                shape.clip_rect.contains_rect(bounds).then_some(bounds)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing visible label: {label}"))
}

#[test]
fn file_loading_populates_an_initially_empty_reader_and_reports_errors() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (dir, _package) = packet_tests::load();
    let path = dir.path().join("TEST.QWK");
    let mut mail = app::MailApp::new(&context);
    frame(&context, &mut mail, egui::vec2(1100.0, 760.0), vec![]);
    mail.open(path.clone(), &context);
    assert!(mail.loading.is_some());
    wait(&mut mail, &context);
    assert_eq!(mail.reader.messages.len(), 4);
    assert_eq!(mail.reader.selected_message, Some(0));
    assert_eq!(mail.path.as_ref(), Some(&path));
    let original = mail.reader.package.clone().unwrap();
    mail.open(dir.path().join("missing.qwk"), &context);
    let deadline = Instant::now() + Duration::from_secs(5);
    while mail.loading.is_some() {
        mail.poll(&context);
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(mail.error.as_ref().unwrap().contains("missing.qwk"));
    assert!(Arc::ptr_eq(&original, mail.reader.package.as_ref().unwrap()));
    if let Some(directory) = std::env::var_os("ICY_EGUI_SCREENSHOTS") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::copy(path, PathBuf::from(directory).join("example.qwk")).unwrap();
    }
}

#[test]
fn new_window_shortcut_uses_exact_modifiers_and_registers_native_viewport() {
    let context = egui::Context::default();
    context.set_embed_viewports(false);
    appearance::apply(&context);
    let mut mail = app::MailApp::new(&context);
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![]);
    let output = frame(
        &context,
        &mut mail,
        size,
        vec![key(egui::Key::N, egui::Modifiers::COMMAND | egui::Modifiers::ALT | egui::Modifiers::SHIFT)],
    );
    assert_eq!(output.viewport_output.len(), 1);
    let output = frame(
        &context,
        &mut mail,
        size,
        vec![key(egui::Key::N, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT)],
    );
    assert_eq!(output.viewport_output.len(), 2);
    let child = output.viewport_output.iter().find(|(id, _)| **id != egui::ViewportId::ROOT).unwrap().0;
    assert!(output.viewport_output[child].viewport_ui_cb.is_some());
    let output = frame(&context, &mut mail, size, vec![key(egui::Key::W, egui::Modifiers::COMMAND)]);
    assert!(mail.closed);
    assert!(output.viewport_output[child].commands.contains(&egui::ViewportCommand::Close));
}

#[test]
fn stale_package_and_body_results_cannot_replace_current_mail() {
    let context = egui::Context::default();
    let (_dir, mut mail) = loaded(&context);
    mail.loader.package_generation = 5;
    mail.loader.body_generation = 9;
    let original = mail.reader.package.clone().unwrap();
    mail.loader
        .sender
        .send(loading::Event::Package(4, PathBuf::from("stale.qwk"), Err("stale failure".into())))
        .unwrap();
    mail.loader
        .sender
        .send(loading::Event::Body(
            8,
            icy_mail::reader::render_body(b"STALE").map_err(|error| error.to_string()),
        ))
        .unwrap();
    mail.poll(&context);
    assert!(mail.error.is_none());
    assert!(Arc::ptr_eq(&original, mail.reader.package.as_ref().unwrap()));
    assert_eq!(mail.screen.terminal.screen.lock().char_at((0, 0).into()).ch, 'l');
    mail.loader
        .sender
        .send(loading::Event::Package(5, PathBuf::from("bad.qwk"), Err("invalid archive".into())))
        .unwrap();
    mail.poll(&context);
    assert!(mail.error.as_ref().unwrap().contains("invalid archive"));
    assert!(Arc::ptr_eq(&original, mail.reader.package.as_ref().unwrap()));
}

#[test]
fn selection_change_and_empty_filter_drop_old_body_results() {
    let context = egui::Context::default();
    let (_dir, mut mail) = loaded(&context);
    mail.reader.navigate(Pane::Messages, NavigateDirection::Last);
    mail.poll(&context);
    let generation = mail.loader.body_generation;
    mail.reader.filter = "NO MATCH".into();
    mail.reader.rebuild_messages();
    mail.poll(&context);
    assert!(mail.reader.selected_message.is_none());
    mail.loader
        .sender
        .send(loading::Event::Body(
            generation,
            icy_mail::reader::render_body(b"STALE").map_err(|error| error.to_string()),
        ))
        .unwrap();
    mail.poll(&context);
    assert!(!mail.body_loading);
    assert_eq!(mail.screen.terminal.screen.lock().char_at((0, 0).into()).ch, ' ');
}

#[test]
fn tables_click_sort_filter_and_preserve_keyboard_focus() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let size = egui::vec2(1100.0, 760.0);
    for _ in 0..3 {
        frame(&context, &mut mail, size, vec![]);
    }
    let output = frame(&context, &mut mail, size, vec![]);
    let retro = label(&output, "Retro").center();
    for pressed in [true, false] {
        frame(&context, &mut mail, size, pointer(retro, pressed));
    }
    assert_eq!(mail.reader.selected_conference, Some(2));
    assert_eq!(mail.reader.messages.len(), 2);
    assert_eq!(mail.focus, Pane::Conferences);
    frame(&context, &mut mail, size, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    assert_eq!(mail.focus, Pane::Messages);
    frame(&context, &mut mail, size, vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)]);
    assert_eq!(mail.reader.selected_message, Some(3));
    frame(&context, &mut mail, size, vec![key(egui::Key::T, egui::Modifiers::COMMAND)]);
    assert_eq!(mail.reader.view_mode, ViewMode::Threads);
    frame(&context, &mut mail, size, vec![key(egui::Key::F, egui::Modifiers::COMMAND)]);
    frame(&context, &mut mail, size, vec![egui::Event::Text("CAROL".into())]);
    assert_eq!(mail.reader.selected_message, Some(2));
    frame(&context, &mut mail, size, vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)]);
    assert_eq!(mail.reader.selected_message, Some(2));
    frame(&context, &mut mail, size, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(mail.reader.filter.is_empty());
    frame(&context, &mut mail, size, vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
    assert_eq!(mail.focus, Pane::Content);
    frame(&context, &mut mail, size, vec![key(egui::Key::Tab, egui::Modifiers::SHIFT)]);
    assert_eq!(mail.focus, Pane::Messages);
}

#[test]
fn modal_blocks_navigation_and_close_key_does_not_leak() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    mail.error = Some("Invalid package".into());
    frame(
        &context,
        &mut mail,
        egui::vec2(360.0, 240.0),
        vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
    );
    assert_eq!(mail.reader.selected_message, Some(0));
    frame(
        &context,
        &mut mail,
        egui::vec2(360.0, 240.0),
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(mail.error.is_none());
    assert_eq!(mail.reader.selected_message, Some(0));
}

#[test]
fn responsive_toolbar_has_one_filter_and_no_clipped_modes() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    for size in [
        egui::vec2(1100.0, 760.0),
        egui::vec2(600.0, 500.0),
        egui::vec2(360.0, 240.0),
        egui::vec2(1100.0, 760.0),
    ] {
        for _ in 0..3 {
            frame(&context, &mut mail, size, vec![]);
        }
        let output = frame(&context, &mut mail, size, vec![]);
        for text in ["List", "Threads"] {
            assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(label(&output, text)));
        }
        let filters = output
            .shapes
            .iter()
            .filter(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Filter author or subject"))
            .count();
        assert_eq!(filters, 1, "{size:?}: exactly one filter");
    }
}

#[test]
fn virtualized_table_renders_only_visible_rows_and_reveals_end() {
    let context = egui::Context::default();
    appearance::apply(&context);
    let (_dir, mut mail) = loaded(&context);
    let package = Arc::make_mut(mail.reader.package.as_mut().unwrap());
    let info = package.infos[0].clone();
    let descriptor = package.descriptors[0].clone();
    for index in 4..20_000 {
        let mut next = info.clone();
        next.index = index;
        next.number = index as u32 + 100;
        next.subject = format!("Message {index:05}");
        next.date = package.infos[3].date;
        package.infos.push(next);
        package.descriptors.push(descriptor.clone());
    }
    mail.reader.rebuild_conferences();
    mail.reader.rebuild_messages();
    let size = egui::vec2(1100.0, 760.0);
    frame(&context, &mut mail, size, vec![]);
    frame(&context, &mut mail, size, vec![key(egui::Key::End, egui::Modifiers::NONE)]);
    let output = frame(&context, &mut mail, size, vec![]);
    assert_eq!(mail.reader.selected_message, Some(19_999));
    label(&output, "Message 19999");
    let drawn = output
        .shapes
        .iter()
        .filter(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().starts_with("Message ")))
        .count();
    assert!(drawn < 40, "only visible rows should be painted: {drawn}");
}

struct Gpu {
    context: egui::Context,
    device: eframe::wgpu::Device,
    queue: eframe::wgpu::Queue,
    renderer: egui_wgpu::Renderer,
}

impl Gpu {
    async fn new() -> Self {
        use eframe::wgpu;
        let adapter = wgpu::Instance::default().request_adapter(&Default::default()).await.unwrap();
        let (device, queue) = adapter.request_device(&Default::default()).await.unwrap();
        let mut renderer = egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, Default::default());
        renderer
            .callback_resources
            .insert(TerminalShaderRenderer::new(&device, wgpu::TextureFormat::Rgba8Unorm));
        let context = egui::Context::default();
        appearance::apply(&context);
        Self {
            context,
            device,
            queue,
            renderer,
        }
    }

    fn capture(&mut self, mail: &mut app::MailApp, size: [u32; 2], scale: f32, events: Vec<egui::Event>, name: &str) -> (Vec<u8>, egui::FullOutput) {
        use eframe::wgpu;
        let time = self.context.input(|input| input.time) + 0.05;
        let modifiers = events
            .iter()
            .find_map(|event| match event {
                egui::Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0] as f32 / scale, size[1] as f32 / scale),
            )),
            time: Some(time),
            events,
            modifiers,
            ..Default::default()
        };
        input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(scale);
        let output = self.context.run(input, |context| mail.show(context));
        let jobs = self.context.tessellate(output.shapes.clone(), output.pixels_per_point);
        for (id, delta) in &output.textures_delta.set {
            self.renderer.update_texture(&self.device, &self.queue, *id, delta);
        }
        let descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: scale,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mail capture"),
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
        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
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
        (pixels, output)
    }
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_mail_layout_themes_narrow_and_hidpi() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut gpu = runtime.block_on(Gpu::new());
    let (_dir, mut mail) = loaded(&gpu.context);
    for (size, scale, theme, name) in [
        ([1100, 760], 1.0, egui::Theme::Dark, "desktop-dark"),
        ([1100, 760], 1.0, egui::Theme::Light, "desktop-light"),
        ([360, 640], 1.0, egui::Theme::Dark, "narrow"),
        ([360, 240], 1.0, egui::Theme::Dark, "short"),
        ([1600, 1200], 2.0, egui::Theme::Light, "hidpi"),
    ] {
        gpu.context.set_theme(theme);
        for pane in [Pane::Conferences, Pane::Messages, Pane::Content] {
            mail.focus = pane;
            for _ in 0..3 {
                gpu.capture(&mut mail, size, scale, vec![], "warmup");
            }
            let (pixels, output) = gpu.capture(&mut mail, size, scale, vec![], &format!("{name}-{pane:?}"));
            label(&output, "List");
            label(&output, "Threads");
            if pane == Pane::Content {
                let rect = mail.content_rect;
                assert!(rect.is_positive());
                assert!(
                    gpu.context.content_rect().contains_rect(rect),
                    "{name}: content {rect:?} outside {:?}",
                    gpu.context.content_rect()
                );
                let mut colors = std::collections::HashSet::new();
                for row in ((rect.top() * scale) as usize)..((rect.bottom() * scale) as usize).min(size[1] as usize) {
                    for column in ((rect.left() * scale) as usize)..((rect.right() * scale) as usize).min(size[0] as usize) {
                        let pos = (row * size[0] as usize + column) * 4;
                        colors.insert(pixels[pos..pos + 3].to_vec());
                    }
                }
                assert!(colors.len() > 2, "nonblank terminal in {name}: {} colors", colors.len());
            }
        }
    }
}

#[test]
#[ignore = "requires a working wgpu adapter"]
fn gpu_long_message_scroll_and_selection_copy() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut gpu = runtime.block_on(Gpu::new());
    let (_dir, mut mail) = loaded(&gpu.context);
    let body = format!("\x1b[31mHELLO WORLD\x1b[0m\n{}THE END", "more text\n".repeat(200));
    mail.screen = icy_engine_gui::egui::screen::ScreenView::new(icy_mail::reader::render_body(body.as_bytes()).unwrap());
    mail.focus = Pane::Content;
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "long-warmup");
    }
    assert!(mail.screen.max_offset.y > 1000.0);
    let (start, end) = {
        let info = mail.screen.terminal.render_info.read();
        let start = egui::pos2(
            info.bounds_x + info.viewport_x + info.font_width * info.display_scale * 0.1,
            info.bounds_y + info.viewport_y + info.font_height * info.display_scale * 0.5,
        );
        (start, start + egui::vec2(info.font_width * info.display_scale * 4.8, 0.0))
    };
    gpu.capture(&mut mail, [1100, 760], 1.0, pointer(start, true), "selection-start");
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![egui::Event::PointerMoved(end)], "selection-drag");
    gpu.capture(&mut mail, [1100, 760], 1.0, pointer(end, false), "selection-end");
    let (_, copied) = gpu.capture(&mut mail, [1100, 760], 1.0, vec![egui::Event::Copy], "selection-copy");
    assert!(
        copied
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::CopyText(text) if text == "HELLO")),
        "{:?}",
        copied.platform_output.commands
    );
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![key(egui::Key::End, egui::Modifiers::NONE)], "long-end");
    assert!(mail.screen.offset.y > 1000.0);
    gpu.capture(&mut mail, [1100, 760], 1.0, vec![key(egui::Key::Home, egui::Modifiers::NONE)], "long-home");
    assert_eq!(mail.screen.offset.y, 0.0);
    let pointer = mail.content_rect.center();
    gpu.capture(
        &mut mail,
        [1100, 760],
        1.0,
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -300.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        "long-wheel",
    );
    for _ in 0..3 {
        gpu.capture(&mut mail, [1100, 760], 1.0, vec![], "long-wheel-settled");
    }
    assert!(mail.screen.offset.y > 0.0);
}

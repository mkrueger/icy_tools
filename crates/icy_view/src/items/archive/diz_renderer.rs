use crate::ui::thumbnail_view::RgbaData;

/// Render FILE_ID.DIZ content to a thumbnail
/// This is similar to render_ansi_thumbnail but simplified for DIZ files
pub fn render_diz_to_thumbnail(data: &[u8]) -> Option<RgbaData> {
    use icy_engine::{BufferType, RenderOptions, ScreenMode};
    use icy_net::telnet::TerminalEmulation;

    // Strip SAUCE from data
    let stripped_data = icy_sauce::strip_sauce(data, icy_sauce::StripMode::All).to_vec();

    // Create screen with ANSI defaults
    let mode = ScreenMode::Vga(80, 25);
    let emulation = TerminalEmulation::Ansi;
    let (mut screen, mut parser) = mode.create_screen(emulation, None);

    // Prepare data
    let (file_data, is_unicode) = crate::ui::preview::prepare_parser_data(stripped_data, "diz");

    if is_unicode {
        if let Some(editable) = screen.as_editable() {
            *editable.buffer_type_mut() = BufferType::Unicode;
        }
    }

    // Parse the DIZ content
    let mut command_queue = std::collections::VecDeque::new();
    {
        let mut sink = icy_engine_gui::util::QueueingSink::new(&mut command_queue);
        parser.parse(&file_data, &mut sink);
    }

    // Filter out sound and delay commands
    command_queue.retain(|cmd| !cmd.is_sound() && !cmd.is_delay());

    // Process commands
    if let Some(editable) = screen.as_editable() {
        editable.terminal_state_mut().is_terminal_buffer = false;
        let mut screen_sink = icy_engine::ScreenSink::new(editable);
        while let Some(cmd) = command_queue.pop_front() {
            cmd.process_screen_command(&mut screen_sink);

            // Safety limit
            if screen_sink.screen().height() >= 100 {
                break;
            }
        }
    }

    let width = screen.width();
    let height = screen.height();

    if width == 0 || height == 0 {
        return None;
    }

    // Render to RGBA
    let rect = icy_engine::Selection::from(icy_engine::Rectangle::from(0, 0, width, height));
    let opts = RenderOptions {
        rect,
        blink_on: true,
        selection: None,
        selection_fg: None,
        selection_bg: None,
        override_scan_lines: Some(false),
    };

    let (size, rgba) = screen.render_to_rgba(&opts);

    if size.width <= 0 || size.height <= 0 || rgba.is_empty() {
        return None;
    }

    let orig_width = size.width as u32;
    let orig_height = size.height as u32;

    // Scale to thumbnail size
    use crate::ui::thumbnail_view::{THUMBNAIL_MAX_HEIGHT, THUMBNAIL_RENDER_WIDTH};

    let scale = (THUMBNAIL_RENDER_WIDTH as f32 / orig_width as f32)
        .min(THUMBNAIL_MAX_HEIGHT as f32 / orig_height as f32)
        .min(1.0);

    let new_width = ((orig_width as f32 * scale) as u32).max(1);
    let new_height = ((orig_height as f32 * scale) as u32).max(1);

    if new_width == orig_width && new_height == orig_height {
        Some(RgbaData::new(rgba, orig_width, orig_height))
    } else {
        // Scale using image crate
        match image::RgbaImage::from_raw(orig_width, orig_height, rgba) {
            Some(img) => {
                let resized = image::imageops::resize(&img, new_width, new_height, image::imageops::FilterType::Triangle);
                Some(RgbaData::new(resized.into_raw(), new_width, new_height))
            }
            None => None,
        }
    }
}

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use icy_engine::{
    AttributedChar, BufferType, FORMATS, LoadData, Rectangle, RenderOptions, Screen, ScreenMode, TextAttribute, TextBuffer, TextPane, TextScreen,
    formats::FileFormat,
};
use icy_net::telnet::TerminalEmulation;
use icy_sauce::SauceRecord;
use log::{debug, error, warn};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::Item;
use crate::ui::preview::prepare_parser_data;

use super::thumbnail::{RgbaData, THUMBNAIL_MAX_HEIGHT, THUMBNAIL_RENDER_WIDTH, ThumbnailResult, ThumbnailState, get_width_multiplier};

/// Maximum characters per line for label tag
const TAG_MAX_CHARS_PER_LINE: usize = 20;
/// Maximum lines for label tag
const TAG_MAX_LINES: usize = 3;

/// Request to load a thumbnail
pub struct ThumbnailRequest {
    /// The item to render thumbnail for
    pub item: Arc<dyn Item>,
    /// Priority (lower = higher priority, visible items should be 0)
    pub priority: u32,
}

/// Thumbnail loader that uses Tokio for async task management
pub struct ThumbnailLoader {
    /// Sender for results
    result_tx: mpsc::UnboundedSender<ThumbnailResult>,
    /// Current cancellation token
    cancel_token: CancellationToken,
    /// Tokio runtime handle
    runtime: Arc<tokio::runtime::Runtime>,
}

impl ThumbnailLoader {
    /// Spawn a new thumbnail loader
    /// Returns the loader and the result receiver
    pub fn spawn() -> (Self, mpsc::UnboundedReceiver<ThumbnailResult>) {
        let (result_tx, result_rx) = mpsc::unbounded_channel();

        // Create a multi-threaded Tokio runtime for thumbnail loading
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .thread_name("thumbnail-loader")
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime for thumbnail loader");

        (
            Self {
                result_tx,
                cancel_token: CancellationToken::new(),
                runtime: Arc::new(runtime),
            },
            result_rx,
        )
    }

    /// Queue a thumbnail for loading (async task)
    pub fn load(&self, request: ThumbnailRequest) {
        let result_tx = self.result_tx.clone();
        let cancel_token = self.cancel_token.child_token();

        let item = request.item;
        // Use full_path for matching if available, otherwise fall back to file_path
        let path = item.get_full_path().unwrap_or_else(|| item.get_file_path());

        debug!("[ThumbnailLoader] Spawning task for: {:?} (gen={})", path, 0);
        // Spawn async task
        self.runtime.spawn(async move {
            // Check cancellation
            if cancel_token.is_cancelled() {
                debug!("[ThumbnailLoader] Task cancelled before start: {:?}", path);
                return;
            }

            let label = item.get_label();

            // Step 1: Async I/O - get thumbnail preview or read data
            let render_input = get_render_input(&*item, &cancel_token).await;

            if cancel_token.is_cancelled() {
                debug!("[ThumbnailLoader] Task cancelled after I/O: {:?}", path);
                return;
            }

            // Step 2: CPU-bound rendering in spawn_blocking
            let path_clone = path.clone();
            let label_clone = label.clone();
            let cancel_clone = cancel_token.clone();

            let result = tokio::task::spawn_blocking(move || {
                match render_input {
                    RenderInput::PreRendered(thumbnail_image) => {
                        // Already have thumbnail image (e.g., folder placeholder, 16colors thumbnail)
                        let label_tag = render_label_tag(&label_clone, 1);
                        Some(ThumbnailResult {
                            path: path_clone,
                            state: ThumbnailState::Ready { rgba: thumbnail_image },
                            sauce_info: None,
                            width_multiplier: 1,
                            label_tag,
                        })
                    }
                    RenderInput::FileData(data) => {
                        // Need to render from file data
                        let result = render_thumbnail(&path_clone, &data, &label_clone, &cancel_clone)?;
                        Some(result)
                    }
                    RenderInput::Error(msg) => {
                        let label_tag = render_label_tag(&label_clone, 1);
                        Some(ThumbnailResult {
                            path: path_clone,
                            state: ThumbnailState::Error(msg),
                            sauce_info: None,
                            width_multiplier: 1,
                            label_tag,
                        })
                    }
                    RenderInput::Cancelled => None,
                }
            })
            .await;

            // Check cancellation after rendering
            if cancel_token.is_cancelled() {
                debug!("[ThumbnailLoader] Discarding cancelled result: {:?}", path);
                return;
            }

            let thumbnail_result = match result {
                Ok(Some(result)) => {
                    debug!("[ThumbnailLoader] Completed: {:?}", path);
                    result
                }
                Ok(None) => {
                    debug!("[ThumbnailLoader] Cancelled during render: {:?}", path);
                    return;
                }
                Err(e) => {
                    error!("[ThumbnailLoader] Task panicked for {:?}: {:?}", path, e);
                    ThumbnailResult {
                        path: path.clone(),
                        state: ThumbnailState::Error(format!("Render panic: {:?}", e)),
                        sauce_info: None,
                        width_multiplier: 1,
                        label_tag: render_label_tag(&label, 1),
                    }
                }
            };

            if let Err(e) = result_tx.send(thumbnail_result) {
                warn!("[ThumbnailLoader] Failed to send result: {}", e);
            }
        });
    }

    /// Clear all pending requests (cancellation is handled by set_generation)
    pub fn clear_pending(&self) {
        // Note: Don't cancel here - set_generation() handles cancellation
        // and creates a new token. Cancelling here would break the new token.
    }

    /// Set the current generation (cancels all older generations)
    pub fn cancel_loading(&mut self) {
        // Cancel all existing tasks
        self.cancel_token.cancel();
        // Create new cancellation token for new generation
        self.cancel_token = CancellationToken::new();
    }

    /// Get a handle to the runtime for spawning tasks
    pub fn runtime(&self) -> Arc<tokio::runtime::Runtime> {
        self.runtime.clone()
    }
}

/// Input for the rendering step - either pre-rendered data or raw file data
enum RenderInput {
    /// Already rendered thumbnail image (e.g., folder placeholder, API thumbnail)
    PreRendered(RgbaData),
    /// Raw file data that needs to be rendered
    FileData(Vec<u8>),
    /// Error occurred during I/O
    Error(String),
    /// Operation was cancelled
    Cancelled,
}

/// Async function to get render input (I/O phase)
/// Uses tokio::select! to cancel I/O operations when the token is cancelled
async fn get_render_input(item: &dyn Item, cancel_token: &CancellationToken) -> RenderInput {
    // Check for cancellation
    if cancel_token.is_cancelled() {
        return RenderInput::Cancelled;
    }

    // Priority #1: Check if the item provides its own thumbnail preview
    // Use select! to allow cancellation during the async operation
    let preview_result = tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            return RenderInput::Cancelled;
        }
        result = item.get_thumbnail_preview(cancel_token) => result
    };

    if let Some(thumbnail_image) = preview_result {
        return RenderInput::PreRendered(thumbnail_image);
    }

    // Check for cancellation before reading data
    if cancel_token.is_cancelled() {
        return RenderInput::Cancelled;
    }

    // Read data from the item with cancellation support
    let data_result = tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            return RenderInput::Cancelled;
        }
        result = item.read_data() => result
    };

    match data_result {
        Some(data) => RenderInput::FileData(data),
        None => RenderInput::Error("Failed to read file data".to_string()),
    }
}

/// Render a thumbnail for the given file
/// Returns None if cancelled
fn render_thumbnail(path: &PathBuf, data: &[u8], label: &str, cancel_token: &CancellationToken) -> Option<ThumbnailResult> {
    let ext = path.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).unwrap_or_default();

    // Check if it's an image file
    if is_image_file(&ext) {
        return render_image_thumbnail(path, data, label, cancel_token);
    }

    // Try to render as ANSI/terminal content
    render_ansi_thumbnail(path, data, &ext, label, cancel_token)
}

/// Check if the extension is a supported image format
fn is_image_file(ext: &str) -> bool {
    matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico")
}

/// Render an image file as thumbnail
/// Returns None if cancelled
fn render_image_thumbnail(path: &PathBuf, data: &[u8], label: &str, cancel_token: &CancellationToken) -> Option<ThumbnailResult> {
    if cancel_token.is_cancelled() {
        return None;
    }

    match ::image::load_from_memory(data) {
        Ok(img) => {
            if cancel_token.is_cancelled() {
                return None;
            }

            let (orig_width, orig_height) = (img.width(), img.height());

            // Scale down if needed
            let scale = (THUMBNAIL_RENDER_WIDTH as f32 / orig_width as f32)
                .min(THUMBNAIL_MAX_HEIGHT as f32 / orig_height as f32)
                .min(1.0);

            let new_width = ((orig_width as f32 * scale) as u32).max(1);
            let new_height = ((orig_height as f32 * scale) as u32).max(1);

            let resized = if scale < 1.0 {
                img.resize(new_width, new_height, ::image::imageops::FilterType::Triangle)
            } else {
                img
            };

            if cancel_token.is_cancelled() {
                return None;
            }

            let rgba: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = resized.to_rgba8();
            let rgba_data = RgbaData::new(rgba.into_raw(), new_width, new_height);

            // Generate label tag as separate texture (1x width for images)
            let label_tag = render_label_tag(label, 1);

            // Check for animated GIF
            // TODO: Handle animated GIFs with multiple frames

            Some(ThumbnailResult {
                path: path.clone(),
                state: ThumbnailState::Ready { rgba: rgba_data },
                sauce_info: None,
                width_multiplier: 1,
                label_tag,
            })
        }
        Err(e) => Some(ThumbnailResult {
            path: path.clone(),
            state: ThumbnailState::Error(e.to_string()),
            sauce_info: None,
            width_multiplier: 1,
            label_tag: render_label_tag(label, 1),
        }),
    }
}

/// Render ANSI/terminal content as thumbnail
/// Returns None if cancelled
fn render_ansi_thumbnail(path: &PathBuf, data: &[u8], ext: &str, label: &str, cancel_token: &CancellationToken) -> Option<ThumbnailResult> {
    if cancel_token.is_cancelled() {
        return None;
    }

    // Extract SAUCE first
    let sauce_opt = SauceRecord::from_bytes(data).ok().flatten();

    // Strip SAUCE from data
    let stripped_data = icy_sauce::strip_sauce(data, icy_sauce::StripMode::All).to_vec();

    debug!("[ThumbnailLoader] Rendering ANSI {:?} ({} bytes, ext={})", path, stripped_data.len(), ext);

    // Check cancellation before heavy processing
    if cancel_token.is_cancelled() {
        return None;
    }

    // Try format-based loading
    if let Some(result) = render_with_format(path, &stripped_data, ext, sauce_opt.as_ref(), label, cancel_token) {
        return Some(result);
    }

    // Check cancellation
    if cancel_token.is_cancelled() {
        return None;
    }

    // Try parser-based using FileFormat
    if let Some(format) = FileFormat::from_extension(ext) {
        if format.uses_parser() {
            return render_with_parser(path, &stripped_data, ext, sauce_opt, label, cancel_token);
        }
    }

    // Check if SAUCE indicates this is a character-based file (ANSI, ASCII, etc.)
    if let Some(ref sauce) = sauce_opt {
        if let Some(icy_sauce::Capabilities::Character(_)) = sauce.capabilities() {
            // SAUCE says this is a character file - try ANSI parser
            return render_with_parser(path, &stripped_data, "ans", sauce_opt, label, cancel_token);
        }
    }

    // No format or parser found - return a placeholder
    Some(render_placeholder_thumbnail(path, sauce_opt, label))
}

/// Render using a parser (for ANSI, etc.)
/// Returns None if cancelled
fn render_with_parser(
    path: &PathBuf,
    data: &[u8],
    ext: &str,
    sauce: Option<SauceRecord>,
    label: &str,
    cancel_token: &CancellationToken,
) -> Option<ThumbnailResult> {
    if cancel_token.is_cancelled() {
        return None;
    }

    // Get screen mode and emulation from FileFormat, fallback to ANSI
    let (mode, emulation) = if let Some(format) = FileFormat::from_extension(ext) {
        (format.screen_mode(), format.terminal_emulation().unwrap_or(TerminalEmulation::Ansi))
    } else {
        (ScreenMode::Vga(80, 25), TerminalEmulation::Ansi)
    };
    let (mut screen, mut parser) = mode.create_screen(emulation, None);

    // Prepare data (strips BOM and truncates at 0x1A)
    let (file_data, is_unicode) = prepare_parser_data(data.to_vec(), ext);

    if is_unicode {
        if let Some(editable) = screen.as_editable() {
            *editable.buffer_type_mut() = BufferType::Unicode;
        }
    }

    // Apply SAUCE width if available
    if let Some(sauce) = &sauce {
        if let Some(editable) = screen.as_editable() {
            editable.apply_sauce(sauce);
        }
    }

    // Process queued commands - this will grow the screen as needed
    if let Some(editable) = screen.as_editable() {
        // Check cancellation before parsing (parsing can be slow for large files)
        if cancel_token.is_cancelled() {
            return None;
        }

        editable.terminal_state_mut().is_terminal_buffer = false;
        let mut screen_sink: icy_engine::ScreenSink<'_> = icy_engine::ScreenSink::new(editable);
        parser.parse(&file_data, &mut screen_sink);
    }

    // Check cancellation before rendering
    if cancel_token.is_cancelled() {
        return None;
    }

    // Render to RGBA - use the actual screen buffer type
    let use_unicode = if let Some(editable) = screen.as_editable() {
        editable.buffer_type() == BufferType::Unicode
    } else {
        false
    };
    debug!(
        "[ThumbnailLoader] {} parsed screen size: {}x{} (unicode={})",
        path.display(),
        screen.get_width(),
        screen.get_height(),
        use_unicode
    );
    render_screen_to_thumbnail(path, &*screen, use_unicode, sauce, label, cancel_token)
}

/// Render using a format loader (for XBin, etc.)
/// Returns None if format not found, Some(result) if rendered (or cancelled during render)
fn render_with_format(
    path: &PathBuf,
    data: &[u8],
    ext: &str,
    sauce: Option<&SauceRecord>,
    label: &str,
    cancel_token: &CancellationToken,
) -> Option<ThumbnailResult> {
    if cancel_token.is_cancelled() {
        return None;
    }

    let start = Instant::now();

    // Find matching format
    let format_idx = FORMATS.iter().enumerate().find_map(|(i, format)| {
        if format.get_file_extension().eq_ignore_ascii_case(ext) {
            return Some(i);
        }
        for alt_ext in format.get_alt_extensions() {
            if alt_ext == ext {
                return Some(i);
            }
        }
        None
    })?;

    if cancel_token.is_cancelled() {
        return None;
    }

    // Use max_height limit for thumbnail loading
    let load_data = LoadData::new(sauce.cloned(), None, None).with_max_height(icy_engine::limits::MAX_BUFFER_HEIGHT);
    match FORMATS[format_idx].load_buffer(path, data, Some(load_data)) {
        Ok(buffer) => {
            if cancel_token.is_cancelled() {
                return None;
            }

            // Determine if this is a unicode buffer
            let is_unicode = buffer.buffer_type == BufferType::Unicode;

            let _width = buffer.get_width();
            let height = buffer.get_height();

            // Debug output
            let _buffer_type = if is_unicode { "Unicode" } else { "CP437" };
            let elapsed = start.elapsed().as_millis();
            debug!(
                "[Thumbnail] {:?} | Format | {:?} | height={} | {}ms",
                path.file_name().unwrap_or_default(),
                buffer.buffer_type,
                height,
                elapsed
            );

            // Use the buffer directly as Screen - no need to
            let screen = TextScreen { buffer, ..Default::default() };
            render_screen_to_thumbnail(path, &screen, is_unicode, sauce.cloned(), label, cancel_token)
        }
        Err(_) => None,
    }
}

/// Render a Screen to a thumbnail image
/// Uses render_unicode_to_rgba for Unicode screens, render_to_rgba for others
/// Returns None if cancelled
fn render_screen_to_thumbnail(
    path: &PathBuf,
    screen: &dyn Screen,
    is_unicode: bool,
    sauce: Option<SauceRecord>,
    label: &str,
    cancel_token: &CancellationToken,
) -> Option<ThumbnailResult> {
    if cancel_token.is_cancelled() {
        return None;
    }

    let width = screen.get_width();
    let height = screen.get_height();

    if width == 0 || height == 0 {
        return Some(ThumbnailResult {
            path: path.clone(),
            state: ThumbnailState::Error("Empty buffer".to_string()),
            sauce_info: sauce,
            width_multiplier: 1,
            label_tag: render_label_tag(label, 1),
        });
    }

    // Calculate width multiplier based on character columns
    let width_multiplier = get_width_multiplier(width);

    // Check if content has blinking
    let has_blinking = screen_has_blinking(screen);

    // Check cancellation before expensive rendering
    if cancel_token.is_cancelled() {
        return None;
    }

    // Render based on buffer type
    let (size_on, rgba_on, size_off, rgba_off) = if is_unicode {
        // Use unicode renderer for Unicode screens
        use icy_engine_gui::{RenderUnicodeOptions, render_unicode_to_rgba};

        let glyph_cache = Arc::new(Mutex::new(None));

        let opts_on = RenderUnicodeOptions {
            selection: None,
            selection_fg: None,
            selection_bg: None,
            blink_on: true,
            font_px_size: None,
            glyph_cache: glyph_cache.clone(),
        };
        let (size_on, rgba_on) = render_unicode_to_rgba(screen, &opts_on);

        // Check cancellation after first render
        if cancel_token.is_cancelled() {
            return None;
        }

        let (size_off, rgba_off) = if has_blinking {
            let opts_off = RenderUnicodeOptions {
                selection: None,
                selection_fg: None,
                selection_bg: None,
                blink_on: false,
                font_px_size: None,
                glyph_cache,
            };
            render_unicode_to_rgba(screen, &opts_off)
        } else {
            (size_on, Vec::new())
        };

        (size_on, rgba_on, size_off, rgba_off)
    } else {
        // Use native screen renderer for non-Unicode screens
        let rect = icy_engine::Selection::from(icy_engine::Rectangle::from(0, 0, width, height));

        let opts_on = RenderOptions {
            rect: rect.clone(),
            blink_on: true,
            selection: None,
            selection_fg: None,
            selection_bg: None,
            override_scan_lines: Some(false),
        };
        let (size_on, rgba_on) = screen.render_to_rgba(&opts_on);

        // Check cancellation after first render
        if cancel_token.is_cancelled() {
            return None;
        }

        let (size_off, rgba_off) = if has_blinking {
            let opts_off = RenderOptions {
                rect,
                blink_on: false,
                selection: None,
                selection_fg: None,
                selection_bg: None,
                override_scan_lines: Some(false),
            };
            screen.render_to_rgba(&opts_off)
        } else {
            (size_on, Vec::new())
        };

        (size_on, rgba_on, size_off, rgba_off)
    };

    let orig_width = size_on.width as u32;
    let orig_height = size_on.height as u32;

    if orig_width == 0 || orig_height == 0 {
        return Some(ThumbnailResult {
            path: path.clone(),
            state: ThumbnailState::Error("Empty rendered buffer".to_string()),
            sauce_info: sauce,
            width_multiplier: 1,
            label_tag: render_label_tag(label, 1),
        });
    }

    // Check cancellation before scaling
    if cancel_token.is_cancelled() {
        return None;
    }

    // Calculate target width based on multiplier
    // Each multiplier step is THUMBNAIL_RENDER_WIDTH (640px for 80 columns)
    let target_width = THUMBNAIL_RENDER_WIDTH * width_multiplier;

    // Scale to fit target width, respecting max height
    // Note: We always scale to target_width (allow upscaling) for consistent layout
    let scale_for_width = target_width as f32 / orig_width as f32;
    let scale_for_height = THUMBNAIL_MAX_HEIGHT as f32 / orig_height as f32;
    let scale = scale_for_width.min(scale_for_height);

    let new_width = ((orig_width as f32 * scale) as u32).max(1);
    let new_height = ((orig_height as f32 * scale) as u32).max(1);

    let rgba_on = scale_rgba_data(&rgba_on, orig_width, orig_height, new_width, new_height);

    // Generate label tag as separate texture
    // Tag width is based on width_multiplier (1x, 2x, or 3x tile width)
    let label_tag = render_label_tag(label, width_multiplier);

    if has_blinking && !rgba_off.is_empty() {
        let rgba_off = scale_rgba_data(&rgba_off, size_off.width as u32, size_off.height as u32, new_width, new_height);

        Some(ThumbnailResult {
            path: path.clone(),
            state: ThumbnailState::Animated {
                frames: vec![rgba_on, rgba_off],
                current_frame: 0,
            },
            sauce_info: sauce,
            width_multiplier,
            label_tag,
        })
    } else {
        Some(ThumbnailResult {
            path: path.clone(),
            state: ThumbnailState::Ready { rgba: rgba_on },
            sauce_info: sauce,
            width_multiplier,
            label_tag,
        })
    }
}

/// Render a placeholder thumbnail for unsupported file types
/// Creates an 80x25 sized placeholder image
fn render_placeholder_thumbnail(path: &PathBuf, sauce: Option<SauceRecord>, label: &str) -> ThumbnailResult {
    // Create an empty 80x25 screen using ANSI defaults
    let mode = ScreenMode::Vga(80, 25);
    let emulation = TerminalEmulation::Ansi;
    let (screen, _parser) = mode.create_screen(emulation, None);

    // Render the empty screen to get proper dimensions (80x25 with font)
    let width = screen.get_width();
    let height = screen.get_height();
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
    let orig_width = size.width as u32;
    let orig_height = size.height as u32;

    let label_tag = render_label_tag(label, 1);

    if orig_width == 0 || orig_height == 0 {
        // Fallback to a simple gray placeholder
        let placeholder_width = 640u32; // 80 chars * 8 pixels
        let placeholder_height = 400u32; // 25 chars * 16 pixels
        let placeholder = vec![64u8; (placeholder_width * placeholder_height * 4) as usize];

        return ThumbnailResult {
            path: path.clone(),
            state: ThumbnailState::Ready {
                rgba: RgbaData::new(placeholder, placeholder_width, placeholder_height),
            },
            sauce_info: sauce,
            width_multiplier: 1,
            label_tag,
        };
    }

    ThumbnailResult {
        path: path.clone(),
        state: ThumbnailState::Ready {
            rgba: RgbaData::new(rgba, orig_width, orig_height),
        },
        sauce_info: sauce,
        width_multiplier: 1,
        label_tag,
    }
}

/// Check if the screen has any blinking content
fn screen_has_blinking(screen: &dyn Screen) -> bool {
    let width = screen.get_width();
    let height = screen.get_height();

    for y in 0..height {
        for x in 0..width {
            let ch = screen.get_char((x, y).into());
            if ch.attribute.is_blinking() {
                return true;
            }
        }
    }
    false
}

/// Scale RGBA data and return as RgbaData
fn scale_rgba_data(rgba: &[u8], orig_width: u32, orig_height: u32, new_width: u32, new_height: u32) -> RgbaData {
    if new_width == orig_width && new_height == orig_height {
        // No scaling needed
        RgbaData::new(rgba.to_vec(), orig_width, orig_height)
    } else {
        // Scale using image crate
        match ::image::RgbaImage::from_raw(orig_width, orig_height, rgba.to_vec()) {
            Some(img) => {
                let resized = ::image::imageops::resize(&img, new_width, new_height, ::image::imageops::FilterType::Triangle);
                RgbaData::new(resized.into_raw(), new_width, new_height)
            }
            None => {
                error!("[ThumbnailLoader] Invalid RGBA data: {}x{} with {} bytes", orig_width, orig_height, rgba.len());
                // Return a placeholder
                let placeholder = vec![128u8; (new_width * new_height * 4) as usize];
                RgbaData::new(placeholder, new_width, new_height)
            }
        }
    }
}

/// Render a DOS-style label tag using the IBM BitFont
/// Returns an RgbaData with the rendered tag, or None if rendering fails
/// width_multiplier: 1 for normal tiles, 2 for 2x width, 3 for 3x width
pub fn render_label_tag(label: &str, width_multiplier: u32) -> Option<RgbaData> {
    // Calculate max chars per line based on width multiplier
    // 1x = 20 chars, 2x = 40 chars, 3x = 60 chars
    let max_chars = TAG_MAX_CHARS_PER_LINE * width_multiplier as usize;

    // Wrap the label into lines
    let lines = wrap_label(label, max_chars, TAG_MAX_LINES);
    if lines.is_empty() {
        return None;
    }

    // Find the maximum line length for buffer width
    let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as i32;
    let num_lines = lines.len() as i32;

    // Create a TextBuffer with exact dimensions
    let mut buffer = TextBuffer::new((max_line_len, num_lines));

    // Set up the inverted DOS attribute: black foreground (0), light gray background (7)
    let attr = TextAttribute::new(0, 7); // fg=0 (black), bg=7 (light gray)

    // Fill the buffer with the filename text
    for (y, line) in lines.iter().enumerate() {
        // Center the line within the buffer width
        let line_len = line.chars().count();
        let padding = (max_line_len as usize - line_len) / 2;

        // Fill the entire line with spaces first (for background color)
        for x in 0..max_line_len {
            buffer.layers[0].set_char((x, y as i32), AttributedChar::new(' ', attr));
        }

        // Write the text centered
        for (i, ch) in line.chars().enumerate() {
            let x = padding + i;
            if x < max_line_len as usize {
                buffer.layers[0].set_char((x as i32, y as i32), AttributedChar::new(ch, attr));
            }
        }
    }

    // Render the buffer to RGBA
    let rect = Rectangle::from(0, 0, max_line_len, num_lines);
    let opts = RenderOptions {
        rect: rect.into(),
        blink_on: true,
        selection: None,
        selection_fg: None,
        selection_bg: None,
        override_scan_lines: Some(false),
    };

    let (size, rgba) = buffer.render_to_rgba(&opts, false);

    if size.width <= 0 || size.height <= 0 || rgba.is_empty() {
        return None;
    }

    let tag_width = size.width as u32;
    let tag_height = size.height as u32;

    Some(RgbaData::new(rgba, tag_width, tag_height))
}

/// Wrap a label into multiple lines
fn wrap_label(label: &str, max_chars_per_line: usize, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut remaining = label;

    while !remaining.is_empty() && lines.len() < max_lines {
        if remaining.chars().count() <= max_chars_per_line {
            // Fits on one line
            lines.push(remaining.to_string());
            break;
        } else if lines.len() == max_lines - 1 {
            // Last allowed line - need to ellipsize with "..."
            let truncated: String = remaining.chars().take(max_chars_per_line - 3).collect();
            lines.push(format!("{}...", truncated));
            break;
        } else {
            // Need to wrap
            let line: String = remaining.chars().take(max_chars_per_line).collect();
            lines.push(line);
            remaining = &remaining[remaining.chars().take(max_chars_per_line).map(|c| c.len_utf8()).sum::<usize>()..];
        }
    }

    lines
}

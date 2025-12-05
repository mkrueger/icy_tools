//! Shader-based file list rendering
//!
//! Provides GPU-accelerated rendering for the file list using pre-rendered
//! icon and text textures for maximum performance.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use ab_glyph::Font;
use iced::Rectangle;
use iced::mouse;
use iced::widget::shader;
use icy_engine::FileFormat;
use icy_engine_gui::ui::FileIcon;
use once_cell::sync::Lazy;

use super::ITEM_HEIGHT;

// ============================================================================
// Global GPU Cache
// ============================================================================

/// Global cache for GPU item resources that can be cleared externally
static GPU_CACHE_VERSION: Lazy<Arc<RwLock<u64>>> = Lazy::new(|| Arc::new(RwLock::new(0)));

/// Increment the GPU cache version, causing all cached textures to be invalidated
pub fn invalidate_gpu_cache() {
    if let Ok(mut version) = GPU_CACHE_VERSION.write() {
        *version = version.wrapping_add(1);
    }
}

/// Get the current GPU cache version
fn get_gpu_cache_version() -> u64 {
    GPU_CACHE_VERSION.read().map(|v| *v).unwrap_or(0)
}

// ============================================================================
// Constants
// ============================================================================

/// Base icon size in pixels (before scaling)
pub const ICON_SIZE: u32 = 18;

/// Padding before icon
pub const ICON_PADDING: f32 = 8.0;

/// Gap between icon and text
pub const ICON_TEXT_GAP: f32 = 6.0;

/// Text start position
pub const TEXT_START_X: f32 = ICON_PADDING + ICON_SIZE as f32 + ICON_TEXT_GAP;

/// Get the current scale factor for high-DPI rendering
fn get_render_scale() -> f32 {
    icy_engine_gui::get_scale_factor().max(1.0)
}

// ============================================================================
// Icon Rendering
// ============================================================================

/// Render a FileIcon SVG to RGBA pixels at high resolution
/// If `tint_color` is provided, the icon will be rendered with that color (monochrome tinting)
pub fn render_icon_to_rgba(icon: FileIcon, base_size: u32, tint_color: Option<[u8; 4]>) -> Option<Vec<u8>> {
    use resvg::tiny_skia::{Pixmap, Transform};
    use resvg::usvg::{Options, Tree};

    let scale = get_render_scale();
    let size = ((base_size as f32) * scale).ceil() as u32;

    let svg_data = get_icon_svg_data(icon)?;
    let tree = Tree::from_data(svg_data, &Options::default()).ok()?;
    let mut pixmap = Pixmap::new(size, size)?;

    let svg_size = tree.size();
    let icon_scale = (size as f32 / svg_size.width()).min(size as f32 / svg_size.height());
    let offset_x = (size as f32 - svg_size.width() * icon_scale) / 2.0;
    let offset_y = (size as f32 - svg_size.height() * icon_scale) / 2.0;

    let transform = Transform::from_scale(icon_scale, icon_scale).post_translate(offset_x, offset_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let mut data = pixmap.take();

    // Apply tint color if specified - replace RGB while preserving alpha
    if let Some(color) = tint_color {
        for pixel in data.chunks_exact_mut(4) {
            if pixel[3] > 0 {
                // Preserve alpha, replace RGB with tint color
                pixel[0] = color[0];
                pixel[1] = color[1];
                pixel[2] = color[2];
                // Keep original alpha: pixel[3] unchanged
            }
        }
    }

    Some(data)
}

/// Get SVG data for a FileIcon
fn get_icon_svg_data(icon: FileIcon) -> Option<&'static [u8]> {
    Some(match icon {
        FileIcon::Ansi => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_ansi.svg"),
        FileIcon::Text => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_text.svg"),
        FileIcon::Binary => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_binary.svg"),
        FileIcon::Terminal => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_terminal.svg"),
        FileIcon::Retro => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_terminal.svg"),
        FileIcon::Graphics => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_graphics.svg"),
        FileIcon::Game => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_game.svg"),
        FileIcon::Native => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_native.svg"),
        FileIcon::Image => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_image.svg"),
        FileIcon::Music => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_music.svg"),
        FileIcon::Movie => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_movie.svg"),
        FileIcon::Archive => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/folder_zip.svg"),
        FileIcon::Folder => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_folder.svg"),
        FileIcon::FolderOpen => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/folder_open.svg"),
        FileIcon::FolderParent => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/folder_parent.svg"),
        FileIcon::FolderData => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/folder_data.svg"),
        FileIcon::Unknown => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_generic.svg"),
    })
}

// ============================================================================
// Text Rendering
// ============================================================================

/// Embedded VGA font for text rendering
static FONT_DATA: &[u8] = include_bytes!("../../../../icy_engine_gui/fonts/1985-ibm-pc-vga/PxPlus_IBM_VGA8.ttf");

/// Render text to RGBA using the embedded VGA font at high resolution
/// If text exceeds max_width, it will be truncated with an ellipsis
pub fn render_text_to_rgba(text: &str, color: [u8; 4], base_max_width: u32) -> (Vec<u8>, u32, u32) {
    use ab_glyph::{FontRef, PxScale, ScaleFont};

    let font = match FontRef::try_from_slice(FONT_DATA) {
        Ok(f) => f,
        Err(_) => return (Vec::new(), 0, 0),
    };

    let scale_factor = get_render_scale();
    let scaled_icon_size = ((ICON_SIZE as f32) * scale_factor).ceil();
    let max_width = ((base_max_width as f32) * scale_factor).ceil() as u32;

    // Font size to match icon height
    let font_size = scaled_icon_size;
    let scale = PxScale::from(font_size);
    let scaled_font = font.as_scaled(scale);

    // Calculate ellipsis width
    let ellipsis = '…';
    let ellipsis_width = scaled_font.h_advance(scaled_font.glyph_id(ellipsis));

    // Calculate total text width and determine if truncation is needed
    let mut total_width: f32 = 0.0;
    let mut char_widths: Vec<f32> = Vec::new();
    for c in text.chars() {
        let glyph_id = scaled_font.glyph_id(c);
        let w = scaled_font.h_advance(glyph_id);
        char_widths.push(w);
        total_width += w;
    }

    let needs_ellipsis = total_width > max_width as f32;
    let effective_max_width = if needs_ellipsis {
        max_width as f32 - ellipsis_width
    } else {
        max_width as f32
    };

    // Determine how many characters to render
    let mut rendered_width: f32 = 0.0;
    let mut chars_to_render = 0;
    for w in &char_widths {
        if rendered_width + w > effective_max_width {
            break;
        }
        rendered_width += w;
        chars_to_render += 1;
    }

    // Build the text to render
    let text_to_render: String = if needs_ellipsis {
        format!("{}…", text.chars().take(chars_to_render).collect::<String>())
    } else {
        text.to_string()
    };

    // Recalculate width for rendered text
    let mut text_width: f32 = 0.0;
    for c in text_to_render.chars() {
        let glyph_id = scaled_font.glyph_id(c);
        text_width += scaled_font.h_advance(glyph_id);
    }

    let text_width = (text_width.ceil() as u32).min(max_width);
    if text_width == 0 {
        return (Vec::new(), 0, 0);
    }

    // Use icon size as text buffer height
    let height = scaled_icon_size.ceil() as u32;
    let mut rgba = vec![0u8; (text_width * height * 4) as usize];

    // Baseline for vertical centering
    let ascent = scaled_font.ascent();
    let descent = scaled_font.descent();
    let line_height = ascent - descent;
    let baseline = (height as f32 - line_height) / 2.0 + ascent;

    let mut x_offset = 0.0f32;

    for c in text_to_render.chars() {
        if x_offset >= max_width as f32 {
            break;
        }

        let glyph_id = scaled_font.glyph_id(c);
        let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(x_offset, baseline));

        if let Some(outlined) = scaled_font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|px, py, coverage| {
                let x = (bounds.min.x as i32 + px as i32) as u32;
                let y = (bounds.min.y as i32 + py as i32) as u32;

                if x < text_width && y < height {
                    let idx = ((y * text_width + x) * 4) as usize;
                    let alpha = (coverage * 255.0) as u8;

                    if alpha > rgba[idx + 3] {
                        rgba[idx] = color[0];
                        rgba[idx + 1] = color[1];
                        rgba[idx + 2] = color[2];
                        rgba[idx + 3] = alpha;
                    }
                }
            });
        }

        x_offset += scaled_font.h_advance(glyph_id);
    }

    (rgba, text_width, height)
}

// ============================================================================
// List Item Rendering
// ============================================================================

/// Render a complete list item (icon + text) to RGBA at scaled resolution
/// If filter is non-empty, the matching portion of the label will be highlighted
pub fn render_list_item(icon: FileIcon, label: &str, is_folder: bool, width: u32, theme_colors: &FileListThemeColors, filter: &str) -> (Vec<u8>, u32, u32) {
    let scale_factor = get_render_scale();
    let height = ITEM_HEIGHT as u32;

    // Scaled dimensions for high-DPI rendering
    let scaled_width = ((width as f32) * scale_factor).ceil() as u32;
    let scaled_height = ((height as f32) * scale_factor).ceil() as u32;
    let scaled_icon_size = ((ICON_SIZE as f32) * scale_factor).ceil() as u32;
    let scaled_icon_padding = ((ICON_PADDING) * scale_factor).ceil() as u32;
    let scaled_text_start_x = ((TEXT_START_X) * scale_factor).ceil() as u32;

    let mut rgba = vec![0u8; (scaled_width * scaled_height * 4) as usize];

    // Text color based on folder status, from theme
    let text_color: [u8; 4] = if is_folder {
        theme_colors.folder_color
    } else {
        if let Some(format) = FileFormat::from_path(&PathBuf::from(&label)) {
            if format.is_image() {
                theme_colors.image_color
            } else if format.is_supported() {
                theme_colors.supported_color
            } else {
                theme_colors.text_color
            }
        } else {
            theme_colors.text_color
        }
    };

    // Render icon at scaled size with theme color tinting
    if let Some(icon_rgba) = render_icon_to_rgba(icon, ICON_SIZE, Some(theme_colors.icon_color)) {
        let icon_x = scaled_icon_padding;
        let icon_y = (scaled_height.saturating_sub(scaled_icon_size)) / 2;

        for y in 0..scaled_icon_size {
            for x in 0..scaled_icon_size {
                let src_idx = ((y * scaled_icon_size + x) * 4) as usize;
                let dst_x = icon_x + x;
                let dst_y = icon_y + y;

                if dst_x < scaled_width && src_idx + 3 < icon_rgba.len() {
                    let dst_idx = ((dst_y * scaled_width + dst_x) * 4) as usize;
                    if dst_idx + 3 < rgba.len() {
                        // Alpha blend
                        let alpha = icon_rgba[src_idx + 3] as f32 / 255.0;
                        rgba[dst_idx] = (icon_rgba[src_idx] as f32 * alpha) as u8;
                        rgba[dst_idx + 1] = (icon_rgba[src_idx + 1] as f32 * alpha) as u8;
                        rgba[dst_idx + 2] = (icon_rgba[src_idx + 2] as f32 * alpha) as u8;
                        rgba[dst_idx + 3] = icon_rgba[src_idx + 3];
                    }
                }
            }
        }
    }

    // Render text with fade-out effect - use highlight version if filter is active
    let text_x = scaled_text_start_x;
    let text_max_width = width.saturating_sub(TEXT_START_X as u32 + 8);

    let (text_rgba, text_width, text_height) = if filter.is_empty() {
        render_text_with_fade(label, text_color, text_max_width)
    } else {
        render_text_with_fade_and_highlight(label, text_color, theme_colors.highlight_color, filter, text_max_width)
    };

    if text_width > 0 && !text_rgba.is_empty() {
        // Center text vertically
        let text_y = (scaled_height.saturating_sub(text_height)) / 2;

        for y in 0..text_height {
            for x in 0..text_width {
                let src_idx = ((y * text_width + x) * 4) as usize;
                let dst_x = text_x + x;
                let dst_y = text_y + y;

                if dst_x < scaled_width && src_idx + 3 < text_rgba.len() {
                    let dst_idx = ((dst_y * scaled_width + dst_x) * 4) as usize;
                    if dst_idx + 3 < rgba.len() {
                        let alpha = text_rgba[src_idx + 3];
                        if alpha > rgba[dst_idx + 3] {
                            rgba[dst_idx] = text_rgba[src_idx];
                            rgba[dst_idx + 1] = text_rgba[src_idx + 1];
                            rgba[dst_idx + 2] = text_rgba[src_idx + 2];
                            rgba[dst_idx + 3] = alpha;
                        }
                    }
                }
            }
        }
    }

    // Return scaled dimensions
    (rgba, scaled_width, scaled_height)
}

/// Column widths for SAUCE mode (matching header row in file_list_view.rs)
/// SAUCE fields: Title=35 chars, Author=20 chars, Group=20 chars
/// Using 8px per char for monospace font at smaller size
/// Name width matches normal list width (286px)
pub const SAUCE_NAME_WIDTH: u32 = 286;
pub const SAUCE_TITLE_WIDTH: u32 = 280; // 35 chars * 8px
pub const SAUCE_AUTHOR_WIDTH: u32 = 160; // 20 chars * 8px
pub const SAUCE_GROUP_WIDTH: u32 = 160; // 20 chars * 8px

/// Font size for SAUCE columns (smaller than icon size)
const SAUCE_FONT_SIZE: f32 = 14.0;

/// Render text without ellipsis at a specific font size
fn render_text_at_size(text: &str, color: [u8; 4], font_size: f32) -> (Vec<u8>, u32, u32) {
    use ab_glyph::{FontRef, PxScale, ScaleFont};

    let font = match FontRef::try_from_slice(FONT_DATA) {
        Ok(f) => f,
        Err(_) => return (Vec::new(), 0, 0),
    };

    let scale_factor = get_render_scale();
    let scaled_font_size = font_size * scale_factor;
    let scale = PxScale::from(scaled_font_size);
    let scaled_font = font.as_scaled(scale);

    // Calculate total text width
    let mut text_width: f32 = 0.0;
    for c in text.chars() {
        let glyph_id = scaled_font.glyph_id(c);
        text_width += scaled_font.h_advance(glyph_id);
    }

    let text_width = text_width.ceil() as u32;
    if text_width == 0 {
        return (Vec::new(), 0, 0);
    }

    let height = scaled_font_size.ceil() as u32;
    let mut rgba = vec![0u8; (text_width * height * 4) as usize];

    // Baseline for vertical centering
    let ascent = scaled_font.ascent();
    let descent = scaled_font.descent();
    let line_height = ascent - descent;
    let baseline = (height as f32 - line_height) / 2.0 + ascent;

    let mut x_offset = 0.0f32;

    for c in text.chars() {
        let glyph_id = scaled_font.glyph_id(c);
        let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(x_offset, baseline));

        if let Some(outlined) = scaled_font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|px, py, coverage| {
                let x = (bounds.min.x as i32 + px as i32) as u32;
                let y = (bounds.min.y as i32 + py as i32) as u32;

                if x < text_width && y < height {
                    let idx = ((y * text_width + x) * 4) as usize;
                    let alpha = (coverage * 255.0) as u8;

                    if alpha > rgba[idx + 3] {
                        rgba[idx] = color[0];
                        rgba[idx + 1] = color[1];
                        rgba[idx + 2] = color[2];
                        rgba[idx + 3] = alpha;
                    }
                }
            });
        }

        x_offset += scaled_font.h_advance(glyph_id);
    }

    (rgba, text_width, height)
}

/// Render text with fade-out effect when truncated
/// The last few characters fade to transparent if text exceeds max_width
fn render_text_with_fade(text: &str, color: [u8; 4], base_max_width: u32) -> (Vec<u8>, u32, u32) {
    use ab_glyph::{FontRef, PxScale, ScaleFont};

    let font = match FontRef::try_from_slice(FONT_DATA) {
        Ok(f) => f,
        Err(_) => return (Vec::new(), 0, 0),
    };

    let scale_factor = get_render_scale();
    let scaled_icon_size = ((ICON_SIZE as f32) * scale_factor).ceil();
    let max_width = ((base_max_width as f32) * scale_factor).ceil() as u32;

    // Font size to match icon height
    let font_size = scaled_icon_size;
    let scale = PxScale::from(font_size);
    let scaled_font = font.as_scaled(scale);

    // Calculate total text width
    let mut total_width: f32 = 0.0;
    let mut char_widths: Vec<f32> = Vec::new();
    for c in text.chars() {
        let glyph_id = scaled_font.glyph_id(c);
        let w = scaled_font.h_advance(glyph_id);
        char_widths.push(w);
        total_width += w;
    }

    let needs_fade = total_width > max_width as f32;
    let fade_width = if needs_fade { 30.0 * scale_factor } else { 0.0 }; // 30px fade zone

    // Determine how many characters fit
    let mut rendered_width: f32 = 0.0;
    let mut chars_to_render = 0;
    for w in &char_widths {
        if rendered_width + w > max_width as f32 {
            break;
        }
        rendered_width += w;
        chars_to_render += 1;
    }

    let text_to_render: String = text.chars().take(chars_to_render).collect();
    let actual_width = rendered_width.ceil() as u32;

    if actual_width == 0 {
        return (Vec::new(), 0, 0);
    }

    let height = scaled_icon_size.ceil() as u32;
    let mut rgba = vec![0u8; (actual_width * height * 4) as usize];

    // Baseline for vertical centering
    let ascent = scaled_font.ascent();
    let descent = scaled_font.descent();
    let line_height = ascent - descent;
    let baseline = (height as f32 - line_height) / 2.0 + ascent;

    let mut x_offset = 0.0f32;

    for c in text_to_render.chars() {
        let glyph_id = scaled_font.glyph_id(c);
        let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(x_offset, baseline));

        if let Some(outlined) = scaled_font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|px, py, coverage| {
                let x = (bounds.min.x as i32 + px as i32) as u32;
                let y = (bounds.min.y as i32 + py as i32) as u32;

                if x < actual_width && y < height {
                    let idx = ((y * actual_width + x) * 4) as usize;
                    let mut alpha = (coverage * 255.0) as u8;

                    // Apply fade effect near the end if needed
                    if needs_fade && x as f32 > (actual_width as f32 - fade_width) {
                        let fade_start = actual_width as f32 - fade_width;
                        let fade_progress = (x as f32 - fade_start) / fade_width;
                        let fade_factor = 1.0 - fade_progress.clamp(0.0, 1.0);
                        alpha = (alpha as f32 * fade_factor) as u8;
                    }

                    if alpha > rgba[idx + 3] {
                        rgba[idx] = color[0];
                        rgba[idx + 1] = color[1];
                        rgba[idx + 2] = color[2];
                        rgba[idx + 3] = alpha;
                    }
                }
            });
        }

        x_offset += scaled_font.h_advance(glyph_id);
    }

    (rgba, actual_width, height)
}

/// Render text with fade-out and highlight for filter matching
fn render_text_with_fade_and_highlight(text: &str, base_color: [u8; 4], highlight_color: [u8; 4], filter: &str, base_max_width: u32) -> (Vec<u8>, u32, u32) {
    use ab_glyph::{FontRef, PxScale, ScaleFont};

    let font = match FontRef::try_from_slice(FONT_DATA) {
        Ok(f) => f,
        Err(_) => return (Vec::new(), 0, 0),
    };

    let scale_factor = get_render_scale();
    let scaled_icon_size = ((ICON_SIZE as f32) * scale_factor).ceil();
    let max_width = ((base_max_width as f32) * scale_factor).ceil() as u32;

    let font_size = scaled_icon_size;
    let scale = PxScale::from(font_size);
    let scaled_font = font.as_scaled(scale);

    // Calculate total text width
    let mut total_width: f32 = 0.0;
    let mut char_widths: Vec<f32> = Vec::new();
    for c in text.chars() {
        let glyph_id = scaled_font.glyph_id(c);
        let w = scaled_font.h_advance(glyph_id);
        char_widths.push(w);
        total_width += w;
    }

    let needs_fade = total_width > max_width as f32;
    let fade_width = if needs_fade { 30.0 * scale_factor } else { 0.0 };

    // Find match position
    let text_lower = text.to_lowercase();
    let filter_lower = filter.to_lowercase();
    let match_start = text_lower.find(&filter_lower);
    let filter_char_count = filter.chars().count();

    // Determine how many characters fit
    let mut rendered_width: f32 = 0.0;
    let mut chars_to_render = 0;
    for w in &char_widths {
        if rendered_width + w > max_width as f32 {
            break;
        }
        rendered_width += w;
        chars_to_render += 1;
    }

    let text_to_render: String = text.chars().take(chars_to_render).collect();
    let actual_width = rendered_width.ceil() as u32;

    if actual_width == 0 {
        return (Vec::new(), 0, 0);
    }

    let height = scaled_icon_size.ceil() as u32;
    let mut rgba = vec![0u8; (actual_width * height * 4) as usize];

    let ascent = scaled_font.ascent();
    let descent = scaled_font.descent();
    let line_height = ascent - descent;
    let baseline = (height as f32 - line_height) / 2.0 + ascent;

    let mut x_offset = 0.0f32;
    let mut char_index = 0usize;

    for c in text_to_render.chars() {
        let is_highlighted = match match_start {
            Some(start) => char_index >= start && char_index < start + filter_char_count,
            None => false,
        };

        let color = if is_highlighted { highlight_color } else { base_color };

        let glyph_id = scaled_font.glyph_id(c);
        let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(x_offset, baseline));

        if let Some(outlined) = scaled_font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|px, py, coverage| {
                let x = (bounds.min.x as i32 + px as i32) as u32;
                let y = (bounds.min.y as i32 + py as i32) as u32;

                if x < actual_width && y < height {
                    let idx = ((y * actual_width + x) * 4) as usize;
                    let mut alpha = (coverage * 255.0) as u8;

                    // Apply fade effect near the end if needed
                    if needs_fade && x as f32 > (actual_width as f32 - fade_width) {
                        let fade_start = actual_width as f32 - fade_width;
                        let fade_progress = (x as f32 - fade_start) / fade_width;
                        let fade_factor = 1.0 - fade_progress.clamp(0.0, 1.0);
                        alpha = (alpha as f32 * fade_factor) as u8;
                    }

                    if alpha > rgba[idx + 3] {
                        rgba[idx] = color[0];
                        rgba[idx + 1] = color[1];
                        rgba[idx + 2] = color[2];
                        rgba[idx + 3] = alpha;
                    }
                }
            });
        }

        x_offset += scaled_font.h_advance(glyph_id);
        char_index += 1;
    }

    (rgba, actual_width, height)
}

/// Render a complete list item with SAUCE info (icon + name + title + author + group) to RGBA at scaled resolution
/// If filter is non-empty, the matching portion of the label will be highlighted
pub fn render_list_item_with_sauce(
    icon: FileIcon,
    label: &str,
    is_folder: bool,
    width: u32,
    theme_colors: &FileListThemeColors,
    filter: &str,
    sauce_title: Option<&str>,
    sauce_author: Option<&str>,
    sauce_group: Option<&str>,
) -> (Vec<u8>, u32, u32) {
    let scale_factor = get_render_scale();
    let height = super::ITEM_HEIGHT as u32;

    // Scaled dimensions for high-DPI rendering
    let scaled_width = ((width as f32) * scale_factor).ceil() as u32;
    let scaled_height = ((height as f32) * scale_factor).ceil() as u32;
    let scaled_icon_size = ((ICON_SIZE as f32) * scale_factor).ceil() as u32;
    let scaled_icon_padding = ((ICON_PADDING) * scale_factor).ceil() as u32;
    let scaled_text_start_x = ((TEXT_START_X) * scale_factor).ceil() as u32;

    let mut rgba = vec![0u8; (scaled_width * scaled_height * 4) as usize];

    // Text color based on folder status, from theme
    let text_color: [u8; 4] = if is_folder {
        theme_colors.folder_color
    } else {
        if let Some(format) = FileFormat::from_path(&PathBuf::from(&label)) {
            if format.is_image() {
                theme_colors.image_color
            } else if format.is_supported() {
                theme_colors.supported_color
            } else {
                theme_colors.text_color
            }
        } else {
            theme_colors.text_color
        }
    };

    // Render icon at scaled size with theme color tinting
    if let Some(icon_rgba) = render_icon_to_rgba(icon, ICON_SIZE, Some(theme_colors.icon_color)) {
        let icon_x = scaled_icon_padding;
        let icon_y = (scaled_height.saturating_sub(scaled_icon_size)) / 2;

        for y in 0..scaled_icon_size {
            for x in 0..scaled_icon_size {
                let src_idx = ((y * scaled_icon_size + x) * 4) as usize;
                let dst_x = icon_x + x;
                let dst_y = icon_y + y;

                if dst_x < scaled_width && src_idx + 3 < icon_rgba.len() {
                    let dst_idx = ((dst_y * scaled_width + dst_x) * 4) as usize;
                    if dst_idx + 3 < rgba.len() {
                        // Alpha blend
                        let alpha = icon_rgba[src_idx + 3] as f32 / 255.0;
                        rgba[dst_idx] = (icon_rgba[src_idx] as f32 * alpha) as u8;
                        rgba[dst_idx + 1] = (icon_rgba[src_idx + 1] as f32 * alpha) as u8;
                        rgba[dst_idx + 2] = (icon_rgba[src_idx + 2] as f32 * alpha) as u8;
                        rgba[dst_idx + 3] = icon_rgba[src_idx + 3];
                    }
                }
            }
        }
    }

    // Column layout:
    // | Icon + Name (SAUCE_NAME_WIDTH) | Title (SAUCE_TITLE_WIDTH) | Author (SAUCE_AUTHOR_WIDTH) | Group (SAUCE_GROUP_WIDTH) |

    // Helper to render SAUCE text at a specific column (smaller font, no ellipsis)
    let render_sauce_column = |rgba: &mut Vec<u8>, text: &str, x_start: u32, color: [u8; 4]| {
        let scaled_x_start = ((x_start as f32) * scale_factor).ceil() as u32;

        // Use smaller font for SAUCE columns, no truncation
        let (text_rgba, text_width, text_height) = render_text_at_size(text, color, SAUCE_FONT_SIZE);

        if text_width > 0 && !text_rgba.is_empty() {
            let text_y = (scaled_height.saturating_sub(text_height)) / 2;

            for y in 0..text_height {
                for x in 0..text_width {
                    let src_idx = ((y * text_width + x) * 4) as usize;
                    let dst_x = scaled_x_start + x;
                    let dst_y = text_y + y;

                    if dst_x < scaled_width && src_idx + 3 < text_rgba.len() {
                        let dst_idx = ((dst_y * scaled_width + dst_x) * 4) as usize;
                        if dst_idx + 3 < rgba.len() {
                            let alpha = text_rgba[src_idx + 3];
                            if alpha > rgba[dst_idx + 3] {
                                rgba[dst_idx] = text_rgba[src_idx];
                                rgba[dst_idx + 1] = text_rgba[src_idx + 1];
                                rgba[dst_idx + 2] = text_rgba[src_idx + 2];
                                rgba[dst_idx + 3] = alpha;
                            }
                        }
                    }
                }
            }
        }
    };

    // Render name column with fade-out effect (with icon offset already accounted for)
    let name_max_width = SAUCE_NAME_WIDTH.saturating_sub(TEXT_START_X as u32 + 4);
    {
        // Render name with filter highlight and fade-out
        let (text_rgba, text_width, text_height) = if filter.is_empty() {
            render_text_with_fade(label, text_color, name_max_width)
        } else {
            render_text_with_fade_and_highlight(label, text_color, theme_colors.highlight_color, filter, name_max_width)
        };

        if text_width > 0 && !text_rgba.is_empty() {
            let text_x = scaled_text_start_x;
            let text_y = (scaled_height.saturating_sub(text_height)) / 2;

            for y in 0..text_height {
                for x in 0..text_width {
                    let src_idx = ((y * text_width + x) * 4) as usize;
                    let dst_x = text_x + x;
                    let dst_y = text_y + y;

                    if dst_x < scaled_width && src_idx + 3 < text_rgba.len() {
                        let dst_idx = ((dst_y * scaled_width + dst_x) * 4) as usize;
                        if dst_idx + 3 < rgba.len() {
                            let alpha = text_rgba[src_idx + 3];
                            if alpha > rgba[dst_idx + 3] {
                                rgba[dst_idx] = text_rgba[src_idx];
                                rgba[dst_idx + 1] = text_rgba[src_idx + 1];
                                rgba[dst_idx + 2] = text_rgba[src_idx + 2];
                                rgba[dst_idx + 3] = alpha;
                            }
                        }
                    }
                }
            }
        }
    }

    // Render SAUCE columns (Title, Author, Group) - no ellipsis, smaller font, colored like status bar
    if let Some(title) = sauce_title {
        if !title.is_empty() {
            render_sauce_column(&mut rgba, title, SAUCE_NAME_WIDTH, theme_colors.sauce_title_color);
        }
    }

    if let Some(author) = sauce_author {
        if !author.is_empty() {
            render_sauce_column(&mut rgba, author, SAUCE_NAME_WIDTH + SAUCE_TITLE_WIDTH, theme_colors.sauce_author_color);
        }
    }

    if let Some(group) = sauce_group {
        if !group.is_empty() {
            render_sauce_column(
                &mut rgba,
                group,
                SAUCE_NAME_WIDTH + SAUCE_TITLE_WIDTH + SAUCE_AUTHOR_WIDTH,
                theme_colors.sauce_group_color,
            );
        }
    }

    // Return scaled dimensions
    (rgba, scaled_width, scaled_height)
}

// ============================================================================
// Shader Uniforms
// ============================================================================

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ListItemUniforms {
    item_width: f32,
    item_height: f32,
    is_selected: f32,
    is_hovered: f32,
    // Theme colors (RGBA as f32)
    bg_color: [f32; 4],
    bg_selected: [f32; 4],
    bg_hovered: [f32; 4],
    fg_selected: [f32; 4],
}

/// Theme colors for the file list shader
#[derive(Clone, Copy, Debug)]
pub struct FileListThemeColors {
    pub bg_color: [f32; 4],
    pub bg_selected: [f32; 4],
    pub bg_hovered: [f32; 4],
    pub fg_selected: [f32; 4],
    pub text_color: [u8; 4],
    pub image_color: [u8; 4],
    pub supported_color: [u8; 4],
    pub folder_color: [u8; 4],
    pub highlight_color: [u8; 4],
    pub icon_color: [u8; 4],
    // SAUCE field colors (matching status bar)
    pub sauce_title_color: [u8; 4],
    pub sauce_author_color: [u8; 4],
    pub sauce_group_color: [u8; 4],
}

impl Default for FileListThemeColors {
    fn default() -> Self {
        // Dark theme defaults
        Self {
            bg_color: [0.12, 0.12, 0.12, 1.0],
            bg_selected: [0.2, 0.4, 0.6, 1.0],
            bg_hovered: [0.16, 0.18, 0.22, 1.0],
            fg_selected: [1.0, 1.0, 1.0, 1.0], // White text on selection
            text_color: [230, 230, 230, 255],
            image_color: [0xFF, 0x55, 0xFF, 255],     // Light green
            supported_color: [0x55, 0xFF, 0x55, 255], // Light blue
            folder_color: [0x55, 0x55, 255, 255],     // Light blue (DOS-like)
            highlight_color: [255, 220, 100, 255],    // Yellow highlight
            icon_color: [230, 230, 230, 255],         // Same as text for dark theme
            // SAUCE colors (dark theme - matching status bar)
            sauce_title_color: [230, 230, 153, 255],  // Yellow (0.9, 0.9, 0.6)
            sauce_author_color: [153, 230, 153, 255], // Green (0.6, 0.9, 0.6)
            sauce_group_color: [153, 204, 230, 255],  // Blue (0.6, 0.8, 0.9)
        }
    }
}

impl FileListThemeColors {
    /// Create colors from an iced Theme
    pub fn from_theme(theme: &iced::Theme) -> Self {
        let palette = theme.extended_palette();
        let is_dark = palette.is_dark;

        // Get base text color from theme
        let base_color = palette.background.base.text;
        let icon_color = [(base_color.r * 255.0) as u8, (base_color.g * 255.0) as u8, (base_color.b * 255.0) as u8, 255];

        // Get selection foreground color (text color on primary/selection background)
        let fg_selected = color_to_array(palette.primary.base.text);

        if is_dark {
            Self {
                bg_color: color_to_array(palette.background.base.color),
                bg_selected: color_to_array(palette.primary.base.color),
                bg_hovered: color_to_array(palette.background.strong.color),
                fg_selected,
                text_color: [230, 230, 230, 255],
                image_color: [0xFF, 0x55, 0xFF, 255],     // Light Magenta
                supported_color: [0x55, 0xFF, 0x55, 255], // Light Green
                folder_color: [0x55, 0x55, 255, 255],     // Light blue (DOS-like)
                highlight_color: [255, 220, 100, 255],    // Yellow highlight
                icon_color,
                // SAUCE colors (matching status bar)
                sauce_title_color: [230, 230, 153, 255],  // Yellow (0.9, 0.9, 0.6)
                sauce_author_color: [153, 230, 153, 255], // Green (0.6, 0.9, 0.6)
                sauce_group_color: [153, 204, 230, 255],  // Blue (0.6, 0.8, 0.9)
            }
        } else {
            Self {
                bg_color: color_to_array(palette.background.base.color),
                bg_selected: color_to_array(palette.primary.base.color),
                bg_hovered: color_to_array(palette.background.strong.color),
                fg_selected,
                text_color: [40, 40, 40, 255],
                image_color: [0xAA, 0x00, 0xAA, 255],     // Magenta
                supported_color: [0x00, 0xAA, 0x00, 255], // Green
                folder_color: [0x55, 0x55, 255, 255],     // Light blue (DOS-like)
                highlight_color: [200, 150, 0, 255],      // Darker yellow for light theme
                icon_color,
                // SAUCE colors (matching status bar)
                sauce_title_color: [153, 128, 0, 255], // Dark yellow (0.6, 0.5, 0.0)
                sauce_author_color: [0, 128, 0, 255],  // Dark green (0.0, 0.5, 0.0)
                sauce_group_color: [0, 102, 153, 255], // Dark blue (0.0, 0.4, 0.6)
            }
        }
    }
}

fn color_to_array(color: iced::Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

// ============================================================================
// List Item Data
// ============================================================================

/// Data for a single rendered list item
#[derive(Clone)]
pub struct ListItemRenderData {
    /// Unique ID
    pub id: u64,
    /// Pre-rendered RGBA content (icon + text)
    pub rgba_data: Arc<Vec<u8>>,
    /// Width of the rendered content
    pub width: u32,
    /// Height of the rendered content
    pub height: u32,
    /// Is this item selected?
    pub is_selected: bool,
    /// Is this item hovered?
    pub is_hovered: bool,
    /// Y position in the list (for rendering)
    pub y_position: f32,
}

// ============================================================================
// Shader Primitive
// ============================================================================

/// Shader primitive for rendering the file list
#[derive(Clone)]
pub struct FileListShaderPrimitive {
    pub items: Vec<ListItemRenderData>,
    pub scroll_y: f32,
    pub viewport_height: f32,
    pub viewport_width: f32,
    pub theme_colors: FileListThemeColors,
}

impl std::fmt::Debug for FileListShaderPrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileListShaderPrimitive")
            .field("items_count", &self.items.len())
            .field("scroll_y", &self.scroll_y)
            .finish()
    }
}

// ============================================================================
// GPU Resources
// ============================================================================

struct ItemGpuResources {
    #[allow(dead_code)]
    texture: iced::wgpu::Texture,
    #[allow(dead_code)]
    texture_view: iced::wgpu::TextureView,
    bind_group: iced::wgpu::BindGroup,
    uniform_buffer: iced::wgpu::Buffer,
    size: (u32, u32),
}

/// Shader pipeline for file list rendering
pub struct FileListShaderPipeline {
    pipeline: iced::wgpu::RenderPipeline,
    bind_group_layout: iced::wgpu::BindGroupLayout,
    sampler: iced::wgpu::Sampler,
    items: HashMap<u64, ItemGpuResources>,
    /// Tracked cache version - when this differs from global, clear items
    cache_version: u64,
}

impl shader::Pipeline for FileListShaderPipeline {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader_source = include_str!("file_list.wgsl");

        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("File List Shader"),
            source: iced::wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("File List Bind Group Layout"),
            entries: &[
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: iced::wgpu::TextureViewDimension::D2,
                        sample_type: iced::wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: iced::wgpu::ShaderStages::FRAGMENT,
                    ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                iced::wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: iced::wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: iced::wgpu::BindingType::Buffer {
                        ty: iced::wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
            label: Some("File List Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("File List Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: iced::wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(iced::wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(iced::wgpu::ColorTargetState {
                    format,
                    blend: Some(iced::wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: iced::wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: iced::wgpu::PrimitiveState {
                topology: iced::wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: iced::wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor {
            label: Some("File List Sampler"),
            address_mode_u: iced::wgpu::AddressMode::ClampToEdge,
            address_mode_v: iced::wgpu::AddressMode::ClampToEdge,
            mag_filter: iced::wgpu::FilterMode::Linear,
            min_filter: iced::wgpu::FilterMode::Linear,
            ..Default::default()
        });

        FileListShaderPipeline {
            pipeline,
            bind_group_layout,
            sampler,
            items: HashMap::new(),
            cache_version: get_gpu_cache_version(),
        }
    }
}

impl shader::Primitive for FileListShaderPrimitive {
    type Pipeline = FileListShaderPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &iced::wgpu::Device,
        queue: &iced::wgpu::Queue,
        _bounds: &Rectangle,
        viewport: &iced::advanced::graphics::Viewport,
    ) {
        icy_engine_gui::set_scale_factor(viewport.scale_factor() as f32);

        // Check if cache was invalidated externally
        let current_version = get_gpu_cache_version();
        if pipeline.cache_version != current_version {
            pipeline.items.clear();
            pipeline.cache_version = current_version;
        }

        for item in &self.items {
            if item.width == 0 || item.height == 0 || item.rgba_data.is_empty() {
                continue;
            }

            let expected_size = (4 * item.width * item.height) as usize;
            if item.rgba_data.len() != expected_size {
                continue;
            }

            let exists = pipeline.items.get(&item.id).map(|r| r.size == (item.width, item.height)).unwrap_or(false);

            if !exists {
                let texture = device.create_texture(&iced::wgpu::TextureDescriptor {
                    label: Some(&format!("List Item Texture {}", item.id)),
                    size: iced::wgpu::Extent3d {
                        width: item.width,
                        height: item.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: iced::wgpu::TextureDimension::D2,
                    format: iced::wgpu::TextureFormat::Rgba8Unorm,
                    usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                queue.write_texture(
                    iced::wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: iced::wgpu::Origin3d::ZERO,
                        aspect: iced::wgpu::TextureAspect::All,
                    },
                    &item.rgba_data,
                    iced::wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * item.width),
                        rows_per_image: Some(item.height),
                    },
                    iced::wgpu::Extent3d {
                        width: item.width,
                        height: item.height,
                        depth_or_array_layers: 1,
                    },
                );

                let texture_view = texture.create_view(&iced::wgpu::TextureViewDescriptor::default());

                let uniform_buffer = device.create_buffer(&iced::wgpu::BufferDescriptor {
                    label: Some(&format!("List Item Uniforms {}", item.id)),
                    size: std::mem::size_of::<ListItemUniforms>() as u64,
                    usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });

                let bind_group = device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
                    label: Some(&format!("List Item BindGroup {}", item.id)),
                    layout: &pipeline.bind_group_layout,
                    entries: &[
                        iced::wgpu::BindGroupEntry {
                            binding: 0,
                            resource: iced::wgpu::BindingResource::TextureView(&texture_view),
                        },
                        iced::wgpu::BindGroupEntry {
                            binding: 1,
                            resource: iced::wgpu::BindingResource::Sampler(&pipeline.sampler),
                        },
                        iced::wgpu::BindGroupEntry {
                            binding: 2,
                            resource: uniform_buffer.as_entire_binding(),
                        },
                    ],
                });

                pipeline.items.insert(
                    item.id,
                    ItemGpuResources {
                        texture,
                        texture_view,
                        bind_group,
                        uniform_buffer,
                        size: (item.width, item.height),
                    },
                );
            }

            // Update uniforms with theme colors
            if let Some(resources) = pipeline.items.get(&item.id) {
                let uniforms = ListItemUniforms {
                    item_width: item.width as f32,
                    item_height: item.height as f32,
                    is_selected: if item.is_selected { 1.0 } else { 0.0 },
                    is_hovered: if item.is_hovered { 1.0 } else { 0.0 },
                    bg_color: self.theme_colors.bg_color,
                    bg_selected: self.theme_colors.bg_selected,
                    bg_hovered: self.theme_colors.bg_hovered,
                    fg_selected: self.theme_colors.fg_selected,
                };

                queue.write_buffer(&resources.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
            }
        }

        // Cleanup unused items
        let active_ids: std::collections::HashSet<u64> = self.items.iter().map(|i| i.id).collect();
        pipeline.items.retain(|id, _| active_ids.contains(id));
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut iced::wgpu::CommandEncoder, target: &iced::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let scale_factor = icy_engine_gui::get_scale_factor();

        let widget_left = clip_bounds.x as f32;
        let widget_top = clip_bounds.y as f32;
        let widget_right = (clip_bounds.x + clip_bounds.width) as f32;
        let widget_bottom = (clip_bounds.y + clip_bounds.height) as f32;

        for item in &self.items {
            let item_y = item.y_position - self.scroll_y;

            // Skip items outside viewport
            if item_y + ITEM_HEIGHT < 0.0 || item_y > self.viewport_height {
                continue;
            }

            if let Some(resources) = pipeline.items.get(&item.id) {
                let item_x = widget_left;
                let item_y_screen = widget_top + item_y * scale_factor;
                let item_w = self.viewport_width * scale_factor;
                let item_h = ITEM_HEIGHT * scale_factor;

                // Clip to widget bounds
                let clipped_left = item_x.max(widget_left);
                let clipped_top = item_y_screen.max(widget_top);
                let clipped_right = (item_x + item_w).min(widget_right);
                let clipped_bottom = (item_y_screen + item_h).min(widget_bottom);

                if clipped_left >= clipped_right || clipped_top >= clipped_bottom {
                    continue;
                }

                let mut render_pass = encoder.begin_render_pass(&iced::wgpu::RenderPassDescriptor {
                    label: Some(&format!("List Item Render {}", item.id)),
                    color_attachments: &[Some(iced::wgpu::RenderPassColorAttachment {
                        view: target,
                        resolve_target: None,
                        ops: iced::wgpu::Operations {
                            load: iced::wgpu::LoadOp::Load,
                            store: iced::wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_scissor_rect(
                    clipped_left as u32,
                    clipped_top as u32,
                    (clipped_right - clipped_left) as u32,
                    (clipped_bottom - clipped_top) as u32,
                );

                render_pass.set_viewport(item_x, item_y_screen, item_w, item_h, 0.0, 1.0);
                render_pass.set_pipeline(&pipeline.pipeline);
                render_pass.set_bind_group(0, &resources.bind_group, &[]);
                render_pass.draw(0..6, 0..1);
            }
        }
    }
}

// ============================================================================
// Shader Program
// ============================================================================

/// State for the file list shader program
#[derive(Debug, Default)]
pub struct FileListShaderState {
    pub hovered_index: Option<usize>,
}

/// Shader program for rendering the file list
pub struct FileListShaderProgram {
    pub items: Vec<ListItemRenderData>,
    pub scroll_y: f32,
    pub content_height: f32,
    pub selected_index: Option<usize>,
    pub viewport_width: f32,
    pub theme_colors: FileListThemeColors,
}

impl FileListShaderProgram {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            scroll_y: 0.0,
            content_height: 0.0,
            selected_index: None,
            viewport_width: 300.0,
            theme_colors: FileListThemeColors::default(),
        }
    }
}

impl Default for FileListShaderProgram {
    fn default() -> Self {
        Self::new()
    }
}

impl<Message> shader::Program<Message> for FileListShaderProgram
where
    Message: Clone + 'static,
{
    type State = FileListShaderState;
    type Primitive = FileListShaderPrimitive;

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        let items: Vec<ListItemRenderData> = self
            .items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let mut item = item.clone();
                item.is_hovered = state.hovered_index == Some(idx);
                item.is_selected = self.selected_index == Some(idx);
                item
            })
            .collect();

        FileListShaderPrimitive {
            items,
            scroll_y: self.scroll_y,
            viewport_height: bounds.height,
            viewport_width: bounds.width,
            theme_colors: self.theme_colors,
        }
    }

    fn update(&self, state: &mut Self::State, _event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::Action<Message>> {
        if let Some(cursor_pos) = cursor.position_in(bounds) {
            let item_y = cursor_pos.y + self.scroll_y;
            let index = (item_y / ITEM_HEIGHT) as usize;

            if index < self.items.len() {
                state.hovered_index = Some(index);
            } else {
                state.hovered_index = None;
            }
        } else {
            state.hovered_index = None;
        }

        None
    }

    fn mouse_interaction(&self, state: &Self::State, _bounds: Rectangle, _cursor: mouse::Cursor) -> mouse::Interaction {
        if state.hovered_index.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

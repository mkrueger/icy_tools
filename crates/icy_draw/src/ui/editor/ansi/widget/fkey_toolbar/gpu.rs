//! GPU-accelerated F-Key Toolbar Component
//!
//! Renders F1-F12 function key slots with characters from the current font.
//! Uses WGSL shader for background (drop shadow, borders, hover highlights)
//! and glyph atlas shader for text rendering (labels, chars, arrows).

use std::num::NonZeroU64;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use icy_engine::{BitFont, Palette};
use icy_engine_gui::theme::main_area_background;
use icy_ui::wgpu::util::DeviceExt;
use icy_ui::{
    mouse::{self, Cursor},
    widget::{self, canvas, container, shader},
    Color, Element, Length, Point, Rectangle, Theme,
};

use super::layout::{
    FKeyLayout, HoverState, ARROW_SIZE, BORDER_WIDTH, CORNER_RADIUS, LABEL_HEIGHT, LABEL_WIDTH, LEFT_PADDING, NAV_GAP, NAV_NEXT_SHIFT_X, NAV_NUM_SHIFT_X,
    NAV_SIZE, NO_HOVER, SET_NUM_ICON_GAP, SHADOW_PADDING, SLOT_CHAR_HEIGHT, SLOT_SPACING, SLOT_WIDTH,
};
use crate::ui::FKeySets;

fn align_up(value: u64, alignment: u64) -> u64 {
    if alignment == 0 {
        return value;
    }
    ((value + alignment - 1) / alignment) * alignment
}

// ═══════════════════════════════════════════════════════════════════════════
// Messages
// ═══════════════════════════════════════════════════════════════════════════

/// Messages from the F-key toolbar (same as Canvas version for drop-in replacement)
#[derive(Clone, Debug)]
pub enum FKeyToolbarMessage {
    /// Click on F-key slot to type character
    TypeFKey(usize),
    /// Click on F-key label to open character selector popup
    OpenCharSelector(usize),
    /// Navigate to previous F-key set
    PrevSet,
    /// Navigate to next F-key set
    NextSet,
}

// ═══════════════════════════════════════════════════════════════════════════
// GPU Shader Types
// ═══════════════════════════════════════════════════════════════════════════

// Flag for fkey background instance (must match shader)
const FLAG_FKEY_BG: u32 = 16;

// ═══════════════════════════════════════════════════════════════════════════
// Glyph Atlas Types (shared)
// ═══════════════════════════════════════════════════════════════════════════

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct QuadVertex {
    unit_pos: [f32; 2],
    unit_uv: [f32; 2],
}

unsafe impl bytemuck::Pod for QuadVertex {}
unsafe impl bytemuck::Zeroable for QuadVertex {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GlyphInstance {
    pos: [f32; 2],
    size: [f32; 2],
    fg: [f32; 4],
    bg: [f32; 4],
    glyph: u32,
    flags: u32,
    _pad: [u32; 2],
}

unsafe impl bytemuck::Pod for GlyphInstance {}
unsafe impl bytemuck::Zeroable for GlyphInstance {}

pub(crate) fn font_key(font: &BitFont) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    font.name().hash(&mut hasher);
    let size = font.size();
    size.width.hash(&mut hasher);
    size.height.hash(&mut hasher);
    // This is intentionally a heuristic key; enough to avoid constant re-uploads.
    font.is_default().hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn build_glyph_atlas_rgba(font: &BitFont) -> (u32, u32, Vec<u8>) {
    let size = font.size();
    let gw = size.width.max(1) as u32;
    let gh = size.height.max(1) as u32;
    let atlas_w = gw * 16;
    let atlas_h = gh * 16;
    let mut rgba = vec![0u8; (atlas_w * atlas_h * 4) as usize];

    for code in 0u32..256u32 {
        // Fonts in the wild differ:
        // - some label glyphs by 0..255 "codepoint" slots (CP/ANSI index)
        // - others label glyphs by Unicode (e.g. box-drawing U+250C)
        // We build the atlas by CP437 index, using the slot character.
        let slot_ch = char::from_u32(code).unwrap_or(' ');
        let col = (code % 16) as u32;
        let row = (code / 16) as u32;
        let base_x = col * gw;
        let base_y = row * gh;

        let glyph = font.glyph(slot_ch);
        for y in 0..gh as usize {
            let dst_y = base_y as usize + y;
            if dst_y >= atlas_h as usize {
                continue;
            }
            for x in 0..gw as usize {
                let dst_x = base_x as usize + x;
                if dst_x >= atlas_w as usize {
                    continue;
                }
                let on = glyph.get_pixel(x, y);
                let idx = ((dst_y * atlas_w as usize + dst_x) * 4) as usize;
                rgba[idx + 0] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = if on { 255 } else { 0 };
            }
        }
    }

    (atlas_w, atlas_h, rgba)
}

// ═══════════════════════════════════════════════════════════════════════════
// One-Pass Combined Renderer (Background + Glyphs)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
struct FKeyOnePassUniforms {
    clip_size: [f32; 2],
    atlas_size: [f32; 2],
    glyph_size: [f32; 2],
    _pad0: [f32; 2],
    num_slots: u32,
    hovered_slot: u32,
    hover_type: u32,
    _pad1: u32,
    corner_radius: f32,
    // WGSL `vec3<f32>` has 16-byte alignment/size in uniform layout.
    // Add explicit padding after `corner_radius` to align the next field.
    _pad_corner: [f32; 3],
    // Represents WGSL `_pad2: vec3<f32>` (takes 16 bytes in uniform layout).
    _pad2: [f32; 4],
    bg_color: [f32; 4],
    content_start_x: f32,
    slot_width: f32,
    slot_spacing: f32,
    label_width: f32,
}

unsafe impl bytemuck::Pod for FKeyOnePassUniforms {}
unsafe impl bytemuck::Zeroable for FKeyOnePassUniforms {}

#[derive(Clone)]
struct FKeyOnePassProgram {
    fkeys: FKeySets,
    font: Option<BitFont>,
    palette: Palette,
    fg_color: u32,
    bg_color: u32,
    bg_color_themed: Color,
    hovered_slot: Arc<AtomicU32>,
    hover_type: Arc<AtomicU32>,
    nav_label_space_bits: Arc<AtomicU32>,
    render_bitfont_labels: bool,
}

impl shader::Program<FKeyToolbarMessage> for FKeyOnePassProgram {
    type State = ();
    type Primitive = FKeyOnePassPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: Cursor, _bounds: Rectangle) -> Self::Primitive {
        FKeyOnePassPrimitive {
            fkeys: self.fkeys.clone(),
            font: self.font.clone(),
            palette: self.palette.clone(),
            fg_color: self.fg_color,
            bg_color: self.bg_color,
            bg_color_themed: self.bg_color_themed,
            hovered_slot: self.hovered_slot.clone(),
            hover_type: self.hover_type.clone(),
            render_bitfont_labels: self.render_bitfont_labels,
            viewport_x: Arc::new(AtomicU32::new(0)),
            viewport_y: Arc::new(AtomicU32::new(0)),
            viewport_w: Arc::new(AtomicU32::new(0)),
            viewport_h: Arc::new(AtomicU32::new(0)),
            uniform_offset_bytes: Arc::new(AtomicU32::new(0)),
            instance_offset_bytes: Arc::new(AtomicU32::new(0)),
            instance_count: Arc::new(AtomicU32::new(0)),
        }
    }

    fn update(&self, _state: &mut Self::State, event: &icy_ui::Event, bounds: Rectangle, cursor: Cursor) -> Option<icy_ui::widget::Action<FKeyToolbarMessage>> {
        let nav_label_space = f32::from_bits(self.nav_label_space_bits.load(Ordering::Relaxed)).max(0.0);
        // Handle mouse movement for hover state
        if let Some(pos) = cursor.position_in(bounds) {
            let (slot, hover_type) = compute_hover_state(pos, bounds, nav_label_space);
            self.hovered_slot.store(slot, Ordering::Relaxed);
            self.hover_type.store(hover_type, Ordering::Relaxed);
        } else {
            self.hovered_slot.store(NO_HOVER, Ordering::Relaxed);
            self.hover_type.store(0, Ordering::Relaxed);
        }

        // Handle mouse clicks
        if let icy_ui::Event::Mouse(mouse::Event::ButtonPressed {
            button: mouse::Button::Left, ..
        }) = event
        {
            if let Some(pos) = cursor.position_in(bounds) {
                let (slot, hover_type) = compute_hover_state(pos, bounds, nav_label_space);

                if slot != NO_HOVER {
                    let is_on_char = hover_type == 1;
                    if is_on_char {
                        return Some(icy_ui::widget::Action::publish(FKeyToolbarMessage::TypeFKey(slot as usize)));
                    } else {
                        return Some(icy_ui::widget::Action::publish(FKeyToolbarMessage::OpenCharSelector(slot as usize)));
                    }
                }

                if hover_type == 2 {
                    return Some(icy_ui::widget::Action::publish(FKeyToolbarMessage::PrevSet));
                }
                if hover_type == 3 {
                    return Some(icy_ui::widget::Action::publish(FKeyToolbarMessage::NextSet));
                }
            }
        }

        None
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: Cursor) -> mouse::Interaction {
        if let Some(pos) = cursor.position_in(bounds) {
            let nav_label_space = f32::from_bits(self.nav_label_space_bits.load(Ordering::Relaxed)).max(0.0);
            let (slot, hover_type) = compute_hover_state(pos, bounds, nav_label_space);
            if slot != NO_HOVER || hover_type == 2 || hover_type == 3 {
                return mouse::Interaction::Pointer;
            }
        }
        mouse::Interaction::default()
    }
}

#[derive(Clone, Debug)]
struct FKeyOnePassPrimitive {
    fkeys: FKeySets,
    font: Option<BitFont>,
    palette: Palette,
    fg_color: u32,
    bg_color: u32,
    bg_color_themed: Color,
    hovered_slot: Arc<AtomicU32>,
    hover_type: Arc<AtomicU32>,
    render_bitfont_labels: bool,
    viewport_x: Arc<AtomicU32>,
    viewport_y: Arc<AtomicU32>,
    viewport_w: Arc<AtomicU32>,
    viewport_h: Arc<AtomicU32>,
    uniform_offset_bytes: Arc<AtomicU32>,
    instance_offset_bytes: Arc<AtomicU32>,
    instance_count: Arc<AtomicU32>,
}

impl shader::Primitive for FKeyOnePassPrimitive {
    type Pipeline = FKeyOnePassRenderer;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &icy_ui::wgpu::Device,
        queue: &icy_ui::wgpu::Queue,
        bounds: &Rectangle,
        viewport: &icy_ui::advanced::graphics::Viewport,
    ) {
        let scale = viewport.scale_factor();
        let origin_x = (bounds.x * scale).round().max(0.0);
        let origin_y = (bounds.y * scale).round().max(0.0);
        let size_w = (bounds.width * scale).round().max(1.0);
        let size_h = (bounds.height * scale).round().max(1.0);

        self.viewport_x.store(origin_x as u32, Ordering::Relaxed);
        self.viewport_y.store(origin_y as u32, Ordering::Relaxed);
        self.viewport_w.store(size_w as u32, Ordering::Relaxed);
        self.viewport_h.store(size_h as u32, Ordering::Relaxed);

        let hovered_slot = self.hovered_slot.load(Ordering::Relaxed);
        let hover_type = self.hover_type.load(Ordering::Relaxed);

        let (glyph_w, glyph_h) = if let Some(font) = &self.font {
            let key = font_key(font);
            if pipeline.atlas_key != Some(key) {
                let (aw, ah, rgba) = build_glyph_atlas_rgba(font);
                pipeline.update_atlas(device, queue, key, aw, ah, &rgba);
            }
            let s = font.size();
            (s.width.max(1) as f32, s.height.max(1) as f32)
        } else {
            self.instance_count.store(0, Ordering::Relaxed);
            return;
        };

        let content_start_x = ((SHADOW_PADDING + BORDER_WIDTH + LEFT_PADDING) * scale).floor();

        let uniforms = FKeyOnePassUniforms {
            clip_size: [size_w, size_h],
            atlas_size: [pipeline.atlas_w as f32, pipeline.atlas_h as f32],
            glyph_size: [glyph_w, glyph_h],
            _pad0: [0.0, 0.0],
            num_slots: 12,
            hovered_slot,
            hover_type,
            _pad1: 0,
            corner_radius: CORNER_RADIUS * scale,
            _pad_corner: [0.0, 0.0, 0.0],
            _pad2: [0.0, 0.0, 0.0, 0.0],
            bg_color: [self.bg_color_themed.r, self.bg_color_themed.g, self.bg_color_themed.b, self.bg_color_themed.a],
            content_start_x,
            slot_width: SLOT_WIDTH * scale,
            slot_spacing: SLOT_SPACING * scale,
            label_width: LABEL_WIDTH * scale,
        };

        let uniform_slot = pipeline.next_uniform.fetch_add(1, Ordering::Relaxed) % pipeline.uniform_capacity;
        let uniform_offset = (uniform_slot as u64) * pipeline.uniform_stride;
        self.uniform_offset_bytes.store(uniform_offset as u32, Ordering::Relaxed);
        queue.write_buffer(&pipeline.uniform_buffer, uniform_offset, bytemuck::bytes_of(&uniforms));

        // Build instances - first background, then glyphs
        let mut instances: Vec<GlyphInstance> = Vec::with_capacity(128);

        // Background instance covers the entire widget
        instances.push(GlyphInstance {
            pos: [0.0, 0.0],
            size: [size_w, size_h],
            fg: [0.0, 0.0, 0.0, 0.0],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_FKEY_BG,
            _pad: [0, 0],
        });

        // Build glyph instances (same logic as FKeyGlyphPrimitive)
        let (fg_r, fg_g, fg_b) = self.palette.rgb(self.fg_color);
        let (bg_r, bg_g, bg_b) = self.palette.rgb(self.bg_color);
        let fg = Color::from_rgb8(fg_r, fg_g, fg_b);
        let bg = Color::from_rgb8(bg_r, bg_g, bg_b);

        let hovered = HoverState::from_uniforms(hovered_slot, hover_type);
        let set_idx = self.fkeys.current_set();

        const FLAG_BG_ONLY: u32 = 2;
        const FLAG_LINEAR_SAMPLE: u32 = 32;

        let control_height_px = ((bounds.height - SHADOW_PADDING * 2.0) * scale).round().max(1.0);
        let font_height = glyph_h.max(1.0);
        let font_width = glyph_w.max(1.0);

        let target_slot_char_h_px = (SLOT_CHAR_HEIGHT * scale).round().max(1.0);
        let slot_char_magnify = (target_slot_char_h_px / font_height).floor().max(1.0);
        let target_label_h_px = LABEL_HEIGHT * scale;
        // Use non-integer magnification for labels (linear sampling in shader)
        let label_magnify = (target_label_h_px / font_height).max(1.0);

        let slot_char_w = (font_width * slot_char_magnify).round().max(1.0);
        let slot_char_h = (font_height * slot_char_magnify).round().max(1.0);
        let label_render_w = (font_width * label_magnify).round().max(1.0);
        let label_render_h = (font_height * label_magnify).round().max(1.0);
        let label_char_w = label_render_w;

        let slot_char_y = (SHADOW_PADDING * scale).floor() + ((control_height_px - slot_char_h) / 2.0).floor();
        let label_y = (SHADOW_PADDING * scale).floor() + ((control_height_px - label_render_h) / 2.0).floor();

        let mut digit_offset: [f32; 10] = [0.0; 10];
        let mut digit_offset_set: [bool; 10] = [false; 10];

        let mut glyph_y_offset = |digit: u32, font_opt: &Option<BitFont>| -> f32 {
            let d = digit as usize;
            if d < 10 && digit_offset_set[d] {
                return digit_offset[d];
            }
            let ch = char::from_digit(digit, 10).unwrap_or('0');
            let Some(font) = font_opt else {
                return 0.0;
            };
            let glyph = font.glyph(ch);

            let char_height = label_render_h;
            let pixel_h = label_magnify;

            let mut min_row: Option<usize> = None;
            let mut max_row: Option<usize> = None;
            let bitmap_pixels = glyph.to_bitmap_pixels();
            for (row_idx, row) in bitmap_pixels.iter().enumerate() {
                if row.iter().any(|&p| p) {
                    min_row = Some(min_row.map_or(row_idx, |m| m.min(row_idx)));
                    max_row = Some(max_row.map_or(row_idx, |m| m.max(row_idx)));
                }
            }

            let off = if let (Some(min_row), Some(max_row)) = (min_row, max_row) {
                let used_height = ((max_row - min_row + 1) as f32) * pixel_h;
                let desired_top = ((char_height - used_height) / 2.0).floor();
                let current_top = (min_row as f32) * pixel_h;
                (desired_top - current_top).floor()
            } else {
                0.0
            };

            if d < 10 {
                digit_offset[d] = off;
                digit_offset_set[d] = true;
            }
            off
        };

        // Draw each slot
        // Label consists of 2 digits; add a small gap between label and char preview
        let label_to_char_gap = (4.0 * scale).floor();
        let label_total_width = 2.0 * label_char_w + label_to_char_gap;

        for slot in 0..12usize {
            let slot_x = (content_start_x + slot as f32 * ((SLOT_WIDTH + SLOT_SPACING) * scale)).floor();
            let label_x = (slot_x + 2.0 * scale).floor();
            let char_x = (slot_x + label_total_width).floor();

            let is_label_hovered = matches!(hovered, HoverState::Slot(s, false) if s == slot);
            let is_char_hovered = matches!(hovered, HoverState::Slot(s, true) if s == slot);

            let label_color = if is_label_hovered {
                Color::from_rgba(0.85, 0.85, 0.88, 1.0)
            } else {
                Color::from_rgba(0.55, 0.55, 0.58, 1.0)
            };
            let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

            let num = slot + 1;
            let (d1, d2) = if num < 10 {
                (0u32, num as u32)
            } else if num == 10 {
                (1u32, 0u32)
            } else if num == 11 {
                (1u32, 1u32)
            } else {
                (1u32, 2u32)
            };

            if self.render_bitfont_labels {
                for (i, &d) in [d1, d2].iter().enumerate() {
                    let glyph = char::from_digit(d, 10).unwrap_or('0') as u32;
                    let y_off = glyph_y_offset(d, &self.font);
                    let x = label_x + i as f32 * label_char_w;
                    let y = label_y + y_off;

                    let label_w = label_render_w.floor().max(1.0);
                    let label_h = label_render_h.floor().max(1.0);

                    instances.push(GlyphInstance {
                        pos: [(x + 1.0).floor(), (y + 1.0).floor()],
                        size: [label_w, label_h],
                        fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
                        bg: [0.0, 0.0, 0.0, 0.0],
                        glyph,
                        flags: FLAG_LINEAR_SAMPLE,
                        _pad: [0, 0],
                    });

                    instances.push(GlyphInstance {
                        pos: [x.floor(), y.floor()],
                        size: [label_w, label_h],
                        fg: [label_color.r, label_color.g, label_color.b, label_color.a],
                        bg: [0.0, 0.0, 0.0, 0.0],
                        glyph,
                        flags: FLAG_LINEAR_SAMPLE,
                        _pad: [0, 0],
                    });
                }
            }

            instances.push(GlyphInstance {
                pos: [char_x.floor(), slot_char_y.floor()],
                size: [slot_char_w.floor(), slot_char_h.floor()],
                fg: [0.0, 0.0, 0.0, 0.0],
                bg: [bg.r, bg.g, bg.b, 1.0],
                glyph: 0,
                flags: FLAG_BG_ONLY,
                _pad: [0, 0],
            });

            let code = self.fkeys.code_at(set_idx, slot);
            let glyph = (code as u32) & 0xFF;
            let char_fg = if is_char_hovered {
                Color::from_rgb((fg.r * 1.3).min(1.0), (fg.g * 1.3).min(1.0), (fg.b * 1.3).min(1.0))
            } else {
                fg
            };

            instances.push(GlyphInstance {
                pos: [char_x.floor(), slot_char_y.floor()],
                size: [slot_char_w.floor(), slot_char_h.floor()],
                fg: [char_fg.r, char_fg.g, char_fg.b, 1.0],
                bg: [0.0, 0.0, 0.0, 0.0],
                glyph,
                flags: 0,
                _pad: [0, 0],
            });
        }

        // Navigation arrows and set number
        let slot_width_px = (SLOT_WIDTH * scale).round().max(1.0);
        let slot_spacing_px = (SLOT_SPACING * scale).round().max(0.0);
        let nav_gap_px = (NAV_GAP * scale).round().max(0.0);
        let nav_size_px = (NAV_SIZE * scale).round().max(1.0);
        let arrow_size_px = (ARROW_SIZE * scale).round().max(1.0);
        let set_num_icon_gap_px = (SET_NUM_ICON_GAP * scale).round().max(0.0);
        let nav_num_shift_x_px = (NAV_NUM_SHIFT_X * scale).round();
        let nav_next_shift_x_px = (NAV_NEXT_SHIFT_X * scale).round();

        let slots_width_px = 12.0 * slot_width_px + 11.0 * slot_spacing_px;
        let nav_x = (content_start_x + slots_width_px + nav_gap_px).floor();
        let set_num = set_idx + 1;
        let num_str = set_num.to_string();
        let num_width = num_str.len() as f32 * label_char_w;

        let icon_side_gap = (nav_size_px - arrow_size_px) / 2.0;
        let num_padding: f32 = (set_num_icon_gap_px - icon_side_gap).max(0.0);
        let num_field_width = 2.0 * label_char_w;
        let label_space = num_field_width + 2.0 * num_padding;

        let next_x = nav_x + nav_size_px + label_space + nav_next_shift_x_px;
        let num_x = nav_x + nav_size_px + num_padding + (num_field_width - num_width) / 2.0 + nav_num_shift_x_px;

        let label_color = Color::from_rgba(0.55, 0.55, 0.58, 1.0);
        let label_hover_color = Color::from_rgba(0.85, 0.85, 0.88, 1.0);
        let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

        let is_prev_hovered = matches!(hovered, HoverState::NavPrev);
        let is_next_hovered = matches!(hovered, HoverState::NavNext);

        const FLAG_ARROW_LEFT: u32 = 4;
        const FLAG_ARROW_RIGHT: u32 = 8;

        let arrow_w = arrow_size_px.floor().max(1.0);
        let arrow_h = arrow_size_px.floor().max(1.0);
        let arrow_y = (SHADOW_PADDING * scale).floor() + ((control_height_px - arrow_size_px) / 2.0).floor();

        let left_arrow_x = nav_x + ((nav_size_px - arrow_size_px) / 2.0).floor();
        let left_arrow_color = if is_prev_hovered { label_hover_color } else { label_color };

        instances.push(GlyphInstance {
            pos: [(left_arrow_x + 1.0).floor(), (arrow_y + 1.0).floor()],
            size: [arrow_w, arrow_h],
            fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_LEFT,
            _pad: [0, 0],
        });
        instances.push(GlyphInstance {
            pos: [left_arrow_x.floor(), arrow_y.floor()],
            size: [arrow_w, arrow_h],
            fg: [left_arrow_color.r, left_arrow_color.g, left_arrow_color.b, 1.0],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_LEFT,
            _pad: [0, 0],
        });

        let right_arrow_x = next_x + ((nav_size_px - arrow_size_px) / 2.0).floor();
        let right_arrow_color = if is_next_hovered { label_hover_color } else { label_color };

        instances.push(GlyphInstance {
            pos: [(right_arrow_x + 1.0).floor(), (arrow_y + 1.0).floor()],
            size: [arrow_w, arrow_h],
            fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_RIGHT,
            _pad: [0, 0],
        });
        instances.push(GlyphInstance {
            pos: [right_arrow_x.floor(), arrow_y.floor()],
            size: [arrow_w, arrow_h],
            fg: [right_arrow_color.r, right_arrow_color.g, right_arrow_color.b, 1.0],
            bg: [0.0, 0.0, 0.0, 0.0],
            glyph: 0,
            flags: FLAG_ARROW_RIGHT,
            _pad: [0, 0],
        });

        if self.render_bitfont_labels {
            for (i, ch) in num_str.chars().enumerate() {
                let digit = ch.to_digit(10).unwrap_or(0);
                let y_off = glyph_y_offset(digit, &self.font);
                let x = num_x + i as f32 * label_char_w;
                let y = label_y + y_off;
                let glyph = ch as u32;

                let label_w = label_render_w.floor().max(1.0);
                let label_h = label_render_h.floor().max(1.0);

                instances.push(GlyphInstance {
                    pos: [(x + 1.0).floor(), (y + 1.0).floor()],
                    size: [label_w, label_h],
                    fg: [shadow_color.r, shadow_color.g, shadow_color.b, shadow_color.a],
                    bg: [0.0, 0.0, 0.0, 0.0],
                    glyph,
                    flags: 0,
                    _pad: [0, 0],
                });
                instances.push(GlyphInstance {
                    pos: [x.floor(), y.floor()],
                    size: [label_w, label_h],
                    fg: [label_color.r, label_color.g, label_color.b, label_color.a],
                    bg: [0.0, 0.0, 0.0, 0.0],
                    glyph,
                    flags: 0,
                    _pad: [0, 0],
                });
            }
        }

        // Upload instances
        let slot = pipeline.next_instance_slot.fetch_add(1, Ordering::Relaxed) % pipeline.instance_slots;
        let instance_offset = (slot as u64) * pipeline.instance_slot_stride;
        self.instance_offset_bytes.store(instance_offset as u32, Ordering::Relaxed);

        let max_instances = pipeline.instance_capacity_per_primitive as usize;
        let count = instances.len().min(max_instances);
        self.instance_count.store(count as u32, Ordering::Relaxed);
        if count > 0 {
            let bytes = bytemuck::cast_slice(&instances[..count]);
            queue.write_buffer(&pipeline.instance_buffer, instance_offset, bytes);
        }
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut icy_ui::wgpu::CommandEncoder, target: &icy_ui::wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        let instance_count = self.instance_count.load(Ordering::Relaxed);
        if instance_count == 0 {
            return;
        }

        let mut pass = encoder.begin_render_pass(&icy_ui::wgpu::RenderPassDescriptor {
            label: Some("FKey OnePass Render Pass"),
            color_attachments: &[Some(icy_ui::wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: icy_ui::wgpu::Operations {
                    load: icy_ui::wgpu::LoadOp::Load,
                    store: icy_ui::wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if clip_bounds.width > 0 && clip_bounds.height > 0 {
            pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);

            let vx = self.viewport_x.load(Ordering::Relaxed) as f32;
            let vy = self.viewport_y.load(Ordering::Relaxed) as f32;
            let vw = self.viewport_w.load(Ordering::Relaxed).max(1) as f32;
            let vh = self.viewport_h.load(Ordering::Relaxed).max(1) as f32;
            pass.set_viewport(vx, vy, vw, vh, 0.0, 1.0);

            pass.set_pipeline(&pipeline.pipeline);
            let uniform_offset = self.uniform_offset_bytes.load(Ordering::Relaxed);
            pass.set_bind_group(0, &pipeline.bind_group, &[uniform_offset]);
            pass.set_vertex_buffer(0, pipeline.quad_vertex_buffer.slice(..));
            let instance_offset = self.instance_offset_bytes.load(Ordering::Relaxed) as u64;
            let instance_bytes = (instance_count as u64) * pipeline.instance_stride;
            pass.set_vertex_buffer(1, pipeline.instance_buffer.slice(instance_offset..(instance_offset + instance_bytes)));
            pass.draw(0..6, 0..instance_count);
        }
    }
}

pub struct FKeyOnePassRenderer {
    pipeline: icy_ui::wgpu::RenderPipeline,
    bind_group: icy_ui::wgpu::BindGroup,
    uniform_buffer: icy_ui::wgpu::Buffer,
    uniform_stride: u64,
    uniform_capacity: u32,
    next_uniform: AtomicU32,
    quad_vertex_buffer: icy_ui::wgpu::Buffer,
    instance_buffer: icy_ui::wgpu::Buffer,
    instance_stride: u64,
    instance_capacity_per_primitive: u32,
    instance_slots: u32,
    instance_slot_stride: u64,
    next_instance_slot: AtomicU32,

    atlas_texture: icy_ui::wgpu::Texture,
    atlas_view: icy_ui::wgpu::TextureView,
    atlas_sampler: icy_ui::wgpu::Sampler,
    atlas_key: Option<u64>,
    atlas_w: u32,
    atlas_h: u32,
}

impl FKeyOnePassRenderer {
    fn update_atlas(&mut self, device: &icy_ui::wgpu::Device, queue: &icy_ui::wgpu::Queue, key: u64, w: u32, h: u32, rgba: &[u8]) {
        if self.atlas_w != w || self.atlas_h != h {
            self.atlas_texture = device.create_texture(&icy_ui::wgpu::TextureDescriptor {
                label: Some("FKey OnePass Glyph Atlas"),
                size: icy_ui::wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: icy_ui::wgpu::TextureDimension::D2,
                format: icy_ui::wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: icy_ui::wgpu::TextureUsages::TEXTURE_BINDING | icy_ui::wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.atlas_view = self.atlas_texture.create_view(&icy_ui::wgpu::TextureViewDescriptor::default());
            self.atlas_w = w;
            self.atlas_h = h;

            let bind_group_layout = self.pipeline.get_bind_group_layout(0);
            self.bind_group = device.create_bind_group(&icy_ui::wgpu::BindGroupDescriptor {
                label: Some("FKey OnePass Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    icy_ui::wgpu::BindGroupEntry {
                        binding: 0,
                        resource: icy_ui::wgpu::BindingResource::Buffer(icy_ui::wgpu::BufferBinding {
                            buffer: &self.uniform_buffer,
                            offset: 0,
                            size: NonZeroU64::new(std::mem::size_of::<FKeyOnePassUniforms>() as u64),
                        }),
                    },
                    icy_ui::wgpu::BindGroupEntry {
                        binding: 1,
                        resource: icy_ui::wgpu::BindingResource::TextureView(&self.atlas_view),
                    },
                    icy_ui::wgpu::BindGroupEntry {
                        binding: 2,
                        resource: icy_ui::wgpu::BindingResource::Sampler(&self.atlas_sampler),
                    },
                ],
            });
        }

        queue.write_texture(
            icy_ui::wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: icy_ui::wgpu::Origin3d::ZERO,
                aspect: icy_ui::wgpu::TextureAspect::All,
            },
            rgba,
            icy_ui::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            icy_ui::wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        self.atlas_key = Some(key);
    }
}

impl shader::Pipeline for FKeyOnePassRenderer {
    fn new(device: &icy_ui::wgpu::Device, queue: &icy_ui::wgpu::Queue, format: icy_ui::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(icy_ui::wgpu::ShaderModuleDescriptor {
            label: Some("FKey OnePass Shader"),
            source: icy_ui::wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_size = std::mem::size_of::<FKeyOnePassUniforms>() as u64;
        debug_assert_eq!(uniform_size, 112);
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let uniform_stride = align_up(uniform_size, alignment);
        let uniform_capacity: u32 = 1024;
        let uniform_buffer_size = uniform_stride * (uniform_capacity as u64);

        let uniform_buffer = device.create_buffer(&icy_ui::wgpu::BufferDescriptor {
            label: Some("FKey OnePass Uniforms (Dynamic)"),
            size: uniform_buffer_size,
            usage: icy_ui::wgpu::BufferUsages::UNIFORM | icy_ui::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let atlas_texture = device.create_texture(&icy_ui::wgpu::TextureDescriptor {
            label: Some("FKey OnePass Glyph Atlas (init)"),
            size: icy_ui::wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: icy_ui::wgpu::TextureDimension::D2,
            format: icy_ui::wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: icy_ui::wgpu::TextureUsages::TEXTURE_BINDING | icy_ui::wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            icy_ui::wgpu::TexelCopyTextureInfo {
                texture: &atlas_texture,
                mip_level: 0,
                origin: icy_ui::wgpu::Origin3d::ZERO,
                aspect: icy_ui::wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 0],
            icy_ui::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            icy_ui::wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let atlas_view = atlas_texture.create_view(&icy_ui::wgpu::TextureViewDescriptor::default());

        let atlas_sampler = device.create_sampler(&icy_ui::wgpu::SamplerDescriptor {
            label: Some("FKey OnePass Atlas Sampler"),
            mag_filter: icy_ui::wgpu::FilterMode::Nearest,
            min_filter: icy_ui::wgpu::FilterMode::Nearest,
            mipmap_filter: icy_ui::wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let quad: [QuadVertex; 6] = [
            QuadVertex {
                unit_pos: [0.0, 0.0],
                unit_uv: [0.0, 0.0],
            },
            QuadVertex {
                unit_pos: [1.0, 0.0],
                unit_uv: [1.0, 0.0],
            },
            QuadVertex {
                unit_pos: [0.0, 1.0],
                unit_uv: [0.0, 1.0],
            },
            QuadVertex {
                unit_pos: [0.0, 1.0],
                unit_uv: [0.0, 1.0],
            },
            QuadVertex {
                unit_pos: [1.0, 0.0],
                unit_uv: [1.0, 0.0],
            },
            QuadVertex {
                unit_pos: [1.0, 1.0],
                unit_uv: [1.0, 1.0],
            },
        ];
        let quad_vertex_buffer = device.create_buffer_init(&icy_ui::wgpu::util::BufferInitDescriptor {
            label: Some("FKey OnePass Quad"),
            contents: bytemuck::cast_slice(&quad),
            usage: icy_ui::wgpu::BufferUsages::VERTEX,
        });

        let instance_stride = std::mem::size_of::<GlyphInstance>() as u64;
        let instance_capacity_per_primitive: u32 = 512;
        let instance_slots: u32 = 256;
        let instance_slot_stride = instance_stride * (instance_capacity_per_primitive as u64);
        let instance_buffer_size = instance_slot_stride * (instance_slots as u64);

        let instance_buffer = device.create_buffer(&icy_ui::wgpu::BufferDescriptor {
            label: Some("FKey OnePass Instances (Ring)"),
            size: instance_buffer_size,
            usage: icy_ui::wgpu::BufferUsages::VERTEX | icy_ui::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&icy_ui::wgpu::BindGroupLayoutDescriptor {
            label: Some("FKey OnePass Bind Group Layout"),
            entries: &[
                icy_ui::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: icy_ui::wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: icy_ui::wgpu::BindingType::Buffer {
                        ty: icy_ui::wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(uniform_size),
                    },
                    count: None,
                },
                icy_ui::wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: icy_ui::wgpu::ShaderStages::FRAGMENT,
                    ty: icy_ui::wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: icy_ui::wgpu::TextureViewDimension::D2,
                        sample_type: icy_ui::wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                icy_ui::wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: icy_ui::wgpu::ShaderStages::FRAGMENT,
                    ty: icy_ui::wgpu::BindingType::Sampler(icy_ui::wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&icy_ui::wgpu::BindGroupDescriptor {
            label: Some("FKey OnePass Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                icy_ui::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: icy_ui::wgpu::BindingResource::Buffer(icy_ui::wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: NonZeroU64::new(uniform_size),
                    }),
                },
                icy_ui::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: icy_ui::wgpu::BindingResource::TextureView(&atlas_view),
                },
                icy_ui::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: icy_ui::wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&icy_ui::wgpu::PipelineLayoutDescriptor {
            label: Some("FKey OnePass Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_buffers = [
            icy_ui::wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<QuadVertex>() as u64,
                step_mode: icy_ui::wgpu::VertexStepMode::Vertex,
                attributes: &[
                    icy_ui::wgpu::VertexAttribute {
                        format: icy_ui::wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    icy_ui::wgpu::VertexAttribute {
                        format: icy_ui::wgpu::VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 1,
                    },
                ],
            },
            icy_ui::wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                step_mode: icy_ui::wgpu::VertexStepMode::Instance,
                attributes: &[
                    icy_ui::wgpu::VertexAttribute {
                        format: icy_ui::wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 2,
                    },
                    icy_ui::wgpu::VertexAttribute {
                        format: icy_ui::wgpu::VertexFormat::Float32x2,
                        offset: 8,
                        shader_location: 3,
                    },
                    icy_ui::wgpu::VertexAttribute {
                        format: icy_ui::wgpu::VertexFormat::Float32x4,
                        offset: 16,
                        shader_location: 4,
                    },
                    icy_ui::wgpu::VertexAttribute {
                        format: icy_ui::wgpu::VertexFormat::Float32x4,
                        offset: 32,
                        shader_location: 5,
                    },
                    icy_ui::wgpu::VertexAttribute {
                        format: icy_ui::wgpu::VertexFormat::Uint32,
                        offset: 48,
                        shader_location: 6,
                    },
                    icy_ui::wgpu::VertexAttribute {
                        format: icy_ui::wgpu::VertexFormat::Uint32,
                        offset: 52,
                        shader_location: 7,
                    },
                ],
            },
        ];

        let pipeline = device.create_render_pipeline(&icy_ui::wgpu::RenderPipelineDescriptor {
            label: Some("FKey OnePass Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: icy_ui::wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(icy_ui::wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(icy_ui::wgpu::ColorTargetState {
                    format,
                    blend: Some(icy_ui::wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: icy_ui::wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: icy_ui::wgpu::PrimitiveState {
                topology: icy_ui::wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: icy_ui::wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            uniform_stride,
            uniform_capacity,
            next_uniform: AtomicU32::new(0),
            quad_vertex_buffer,
            instance_buffer,
            instance_stride,
            instance_capacity_per_primitive,
            instance_slots,
            instance_slot_stride,
            next_instance_slot: AtomicU32::new(0),
            atlas_texture,
            atlas_view,
            atlas_sampler,
            atlas_key: None,
            atlas_w: 1,
            atlas_h: 1,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════

/// Compute hover state from cursor position using FKeyLayout.
///
/// The `nav_label_space` parameter allows overriding the computed label space
/// (used when we know the exact font dimensions). Pass 0.0 to use default.
fn compute_hover_state(pos: Point, _bounds: Rectangle, nav_label_space: f32) -> (u32, u32) {
    // Use default font for hit testing (the exact font doesn't matter much for hit areas)
    let mut layout = FKeyLayout::default_font();

    // Override nav_label_space if provided
    if nav_label_space > 0.0 {
        layout.nav_label_space = nav_label_space;
        // Recalculate next_nav_x with the new label space
        layout.next_nav_x = layout.nav_x + NAV_SIZE + nav_label_space + NAV_NEXT_SHIFT_X;
    }

    layout.hit_test_uniforms(pos)
}

// ═══════════════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════════════

/// GPU-accelerated F-Key Toolbar with shader background
pub struct ShaderFKeyToolbar {
    hovered_slot: Arc<AtomicU32>,
    hover_type: Arc<AtomicU32>,
    nav_label_space_bits: Arc<AtomicU32>,
}

impl Default for ShaderFKeyToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaderFKeyToolbar {
    pub fn new() -> Self {
        Self {
            hovered_slot: Arc::new(AtomicU32::new(NO_HOVER)),
            hover_type: Arc::new(AtomicU32::new(0)),
            nav_label_space_bits: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Clear the render cache
    pub fn clear_cache(&mut self) {
        // Cache is owned per-view
    }

    /// Render the toolbar
    pub fn view(
        &self,
        fkeys: FKeySets,
        font: Option<BitFont>,
        palette: Palette,
        fg_color: u32,
        bg_color: u32,
        theme: &Theme,
    ) -> Element<'_, FKeyToolbarMessage> {
        // Use centralized layout calculations
        let (font_w, font_h) = font
            .as_ref()
            .map(|f| (f.size().width.max(1) as f32, f.size().height.max(1) as f32))
            .unwrap_or((8.0, 16.0));
        let layout = FKeyLayout::new(font_w, font_h);

        // Share the exact label-space with hit-testing.
        self.nav_label_space_bits.store(layout.nav_label_space.to_bits(), Ordering::Relaxed);

        let bg_color_themed = main_area_background(theme);

        let current_set = fkeys.current_set();

        // Create one-pass shader (background + glyphs combined)
        let shader_onepass: Element<'_, FKeyToolbarMessage> = widget::shader(FKeyOnePassProgram {
            fkeys,
            font,
            palette,
            fg_color,
            bg_color,
            bg_color_themed,
            hovered_slot: self.hovered_slot.clone(),
            hover_type: self.hover_type.clone(),
            nav_label_space_bits: self.nav_label_space_bits.clone(),
            // Labels are drawn via TTF overlay for higher resolution.
            render_bitfont_labels: false,
        })
        .width(Length::Fixed(layout.total_width))
        .height(Length::Fixed(layout.total_height))
        .into();

        let overlay: Element<'_, FKeyToolbarMessage> = canvas(FKeyTtfLabelOverlay {
            layout: layout.clone(),
            current_set,
            hovered_slot: self.hovered_slot.clone(),
            hover_type: self.hover_type.clone(),
        })
        .width(Length::Fixed(layout.total_width))
        .height(Length::Fixed(layout.total_height))
        .into();

        let stacked: Element<'_, FKeyToolbarMessage> = icy_ui::widget::stack![shader_onepass, overlay]
            .width(Length::Fixed(layout.total_width))
            .height(Length::Fixed(layout.total_height))
            .into();

        // Wrap in container to center horizontally in parent
        container(stacked)
            .width(Length::Fill)
            .height(Length::Fixed(layout.total_height))
            .center_x(Length::Fill)
            .into()
    }
}

#[derive(Clone, Debug)]
struct FKeyTtfLabelOverlay {
    layout: FKeyLayout,
    current_set: usize,
    hovered_slot: Arc<AtomicU32>,
    hover_type: Arc<AtomicU32>,
}

impl canvas::Program<FKeyToolbarMessage> for FKeyTtfLabelOverlay {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &icy_ui::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: Cursor) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Keep font sizes in one place so it is easy to tune.
        // (Logical pixels; DPI scaling is handled by the renderer.)
        let slot_label_font_size: f32 = 18.0;
        let set_label_font_size: f32 = 18.0;

        let hovered_raw = self.hovered_slot.load(Ordering::Relaxed);
        let hover_type = self.hover_type.load(Ordering::Relaxed);
        let hovered = HoverState::from_uniforms(hovered_raw, hover_type);

        let label_color = Color::from_rgba(0.55, 0.55, 0.58, 1.0);
        let label_hover_color = Color::from_rgba(0.85, 0.85, 0.88, 1.0);
        let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

        // Slot number labels ("01".."12")
        for slot in 0..12usize {
            let slot_x = self.layout.slot_x(slot);
            let is_label_hovered = matches!(hovered, HoverState::Slot(s, false) if s == slot);
            let c = if is_label_hovered { label_hover_color } else { label_color };

            let text = format!("{:02}", slot + 1);
            // Match the shader's label start, but center only across the digit area.
            // The shader adds n extra gap between the digits and the char preview; that
            // gap must NOT affeact the centering of the digits themselves.
            let x = slot_x - 1.0;
            let y = SHADOW_PADDING;
            let w = (LABEL_WIDTH * 2.0).min(SLOT_WIDTH);
            let h = self.layout.control_height;

            let center = icy_ui::Point::new(x + w / 2.0, y + h / 2.0);

            // Shadow
            frame.fill_text(canvas::Text {
                content: text.clone(),
                position: icy_ui::Point::new(center.x + 1.0, center.y + 1.0),
                color: shadow_color,
                size: slot_label_font_size.into(),
                font: icy_ui::Font::default(),
                align_x: icy_ui::alignment::Horizontal::Center.into(),
                align_y: icy_ui::alignment::Vertical::Center.into(),
                ..Default::default()
            });

            // Main
            frame.fill_text(canvas::Text {
                content: text,
                position: center,
                color: c,
                size: slot_label_font_size.into(),
                font: icy_ui::Font::default(),
                align_x: icy_ui::alignment::Horizontal::Center.into(),
                align_y: icy_ui::alignment::Vertical::Center.into(),
                ..Default::default()
            });
        }

        // Set number label between arrows
        let nav_prev = self.layout.nav_prev_rect();
        // Important: `next_nav_x` includes `NAV_NEXT_SHIFT_X` (a visual tweak for the next button).
        // The set label should be centered in the reserved label space, not in the distance between
        // the shifted button rectangles.
        let label_rect = Rectangle {
            x: self.layout.nav_x + NAV_SIZE,
            y: nav_prev.y,
            width: self.layout.nav_label_space.max(1.0),
            height: nav_prev.height,
        };

        let set_text = (self.current_set + 1).to_string();
        let center = icy_ui::Point::new(label_rect.x + label_rect.width / 2.0, label_rect.y + label_rect.height / 2.0);

        frame.fill_text(canvas::Text {
            content: set_text.clone(),
            position: icy_ui::Point::new(center.x + 1.0, center.y + 1.0),
            color: shadow_color,
            size: set_label_font_size.into(),
            font: icy_ui::Font::default(),
            align_x: icy_ui::alignment::Horizontal::Center.into(),
            align_y: icy_ui::alignment::Vertical::Center.into(),
            ..Default::default()
        });

        frame.fill_text(canvas::Text {
            content: set_text,
            position: center,
            color: label_color,
            size: set_label_font_size.into(),
            font: icy_ui::Font::default(),
            align_x: icy_ui::alignment::Horizontal::Center.into(),
            align_y: icy_ui::alignment::Vertical::Center.into(),
            ..Default::default()
        });

        vec![frame.into_geometry()]
    }
}

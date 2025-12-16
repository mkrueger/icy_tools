//! Shared GPU Glyph Renderer
//!
//! Provides a reusable GPU-accelerated glyph rendering system based on a
//! 16x16 CP437 glyph atlas. Used by FKey-Toolbar and SegmentedControl for
//! crisp, pixel-perfect character rendering.

use codepages::tables::CP437_TO_UNICODE;
use icy_engine::BitFont;

// ═══════════════════════════════════════════════════════════════════════════
// Instance Structures
// ═══════════════════════════════════════════════════════════════════════════

/// Quad vertex for instanced glyph rendering
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadVertex {
    pub unit_pos: [f32; 2],
    pub unit_uv: [f32; 2],
}

unsafe impl bytemuck::Pod for QuadVertex {}
unsafe impl bytemuck::Zeroable for QuadVertex {}

/// Per-instance data for a single glyph
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GlyphInstance {
    /// Position in clip-space pixels (top-left)
    pub pos: [f32; 2],
    /// Size in pixels (width, height)
    pub size: [f32; 2],
    /// Foreground color (RGBA)
    pub fg: [f32; 4],
    /// Background color (RGBA)
    pub bg: [f32; 4],
    /// Glyph index (0-255 for CP437)
    pub glyph: u32,
    /// Flags: bit 1 = draw bg, bit 2 = bg only, bit 3 = left arrow, bit 4 = right arrow
    pub flags: u32,
    pub _pad: [u32; 2],
}

unsafe impl bytemuck::Pod for GlyphInstance {}
unsafe impl bytemuck::Zeroable for GlyphInstance {}

// Flag constants
pub const FLAG_DRAW_BG: u32 = 1;

// ═══════════════════════════════════════════════════════════════════════════
// Atlas Generation
// ═══════════════════════════════════════════════════════════════════════════

/// Generate a unique key for the given font for atlas caching
pub fn font_key(font: &BitFont) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    font.name().hash(&mut hasher);
    let size = font.size();
    size.width.hash(&mut hasher);
    size.height.hash(&mut hasher);
    font.is_default().hash(&mut hasher);
    hasher.finish()
}

/// Build a 16x16 glyph atlas texture (256 glyphs) from a BitFont.
/// Returns (atlas_width, atlas_height, rgba_data).
pub fn build_glyph_atlas_rgba(font: &BitFont) -> (u32, u32, Vec<u8>) {
    let size = font.size();
    let gw = size.width.max(1) as u32;
    let gh = size.height.max(1) as u32;
    let atlas_w = gw * 16;
    let atlas_h = gh * 16;
    let mut rgba = vec![0u8; (atlas_w * atlas_h * 4) as usize];

    for code in 0u32..256u32 {
        // Try both CP437 slot and Unicode lookup
        let slot_ch = char::from_u32(code).unwrap_or(' ');
        let unicode_ch = CP437_TO_UNICODE.get(code as usize).copied().unwrap_or(' ');
        let col = (code % 16) as u32;
        let row = (code / 16) as u32;
        let base_x = col * gw;
        let base_y = row * gh;

        if let Some(glyph) = font.glyph(slot_ch).or_else(|| font.glyph(unicode_ch)) {
            for y in 0..gh as usize {
                let dst_y = base_y as usize + y;
                if dst_y >= atlas_h as usize {
                    continue;
                }
                let src_row = glyph.bitmap.pixels.get(y);
                for x in 0..gw as usize {
                    let dst_x = base_x as usize + x;
                    if dst_x >= atlas_w as usize {
                        continue;
                    }
                    let on = src_row.and_then(|r| r.get(x)).copied().unwrap_or(false);
                    let idx = ((dst_y * atlas_w as usize + dst_x) * 4) as usize;
                    rgba[idx] = 255;
                    rgba[idx + 1] = 255;
                    rgba[idx + 2] = 255;
                    rgba[idx + 3] = if on { 255 } else { 0 };
                }
            }
        }
    }

    (atlas_w, atlas_h, rgba)
}

/// Convert a Unicode char to CP437 index for atlas lookup
pub fn cp437_index(ch: char) -> u32 {
    if (ch as u32) <= 0xFF {
        return ch as u32;
    }
    CP437_TO_UNICODE.iter().position(|&c| c == ch).map(|idx| idx as u32).unwrap_or(b'?' as u32)
}

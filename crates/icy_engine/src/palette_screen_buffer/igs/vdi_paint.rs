use icy_parser_core::{DrawingMode, LineKind, ParameterBounds, PatternType, PolymarkerKind, TextEffects, TextRotation};

mod blitting;
mod circle;
mod fill;
mod line;
mod poly;
mod rect;
mod text;

pub use blitting::BlitSurface;

use crate::palette_screen_buffer::igs::TerminalResolution;
use crate::{EditableScreen, Position};

pub struct VdiPaint {
    terminal_resolution: TerminalResolution,

    pub draw_to_position: Position,

    pub polymarker_color: u8,
    pub line_color: u8,
    pub fill_color: u8,
    pub text_color: u8,

    pub text_effects: TextEffects,
    pub text_size: i32,
    pub text_rotation: TextRotation,

    pub polymarker_type: PolymarkerKind,
    pub line_kind: LineKind,
    pub drawing_mode: DrawingMode,
    pub polymarker_size: i32,
    pub line_thickness: i32,
    line_user_mask: u16,

    fill_pattern_type: PatternType,
    pub fill_draw_border: bool,

    // Screen memory for blit operations
    pub blit_buffer: BlitSurface,

    pub user_patterns: [Vec<u16>; 8],
    pub scaling_mode: icy_parser_core::GraphicsScalingMode,

    pub random_bounds: ParameterBounds
}

unsafe impl Send for VdiPaint {}

unsafe impl Sync for VdiPaint {}

impl Default for VdiPaint {
    fn default() -> Self {
        VdiPaint::new(TerminalResolution::Low)
    }
}

impl VdiPaint {
    pub fn new(terminal_resolution: TerminalResolution) -> Self {
        let default_color = terminal_resolution.default_fg_color();
        Self {
            terminal_resolution,
            polymarker_color: default_color,
            line_color: default_color,
            fill_color: default_color,
            text_color: default_color,
            draw_to_position: Position::new(0, 0),
            text_effects: TextEffects::NORMAL,
            text_size: 9,
            text_rotation: TextRotation::Degrees0,
            polymarker_type: PolymarkerKind::Point,
            line_kind: LineKind::Solid,
            drawing_mode: DrawingMode::Replace,
            polymarker_size: 1,
            line_thickness: 1,
            line_user_mask: 0b1010_1010_1010_1010,

            fill_pattern_type: PatternType::Solid,
            fill_draw_border: false,
            user_patterns: [vec![0], vec![0], vec![0], vec![0], vec![0], vec![0], vec![0], vec![0]],
            blit_buffer: BlitSurface::new(0, 0),
            scaling_mode: icy_parser_core::GraphicsScalingMode::Normal,
            random_bounds: ParameterBounds::default(),
        }
    }

    pub fn scroll(&mut self, buf: &mut dyn EditableScreen, amount: i32) {
        if amount == 0 {
            return;
        }
        let res = buf.get_resolution();
        if amount < 0 {
            buf.screen_mut().splice(0..0, vec![1; res.width as usize * amount.abs() as usize]);
            buf.screen_mut().truncate(res.width as usize * res.height as usize);
        } else {
            buf.screen_mut().splice(0..res.width as usize * amount.abs() as usize, vec![]);
            buf.screen_mut().extend(vec![1; res.width as usize * amount.abs() as usize]);
        }
    }

    pub fn set_resolution(&mut self, res: TerminalResolution) {
        self.terminal_resolution = res;
    }

    pub fn get_terminal_resolution(&self) -> TerminalResolution {
        self.terminal_resolution
    }

    pub fn init_resolution(&mut self, buf: &mut dyn EditableScreen) {
        buf.clear_screen();
        // TODO?
    }

    pub fn reset_attributes(&mut self) {
        let default_color = self.terminal_resolution.default_fg_color();
        
        // Reset polymarker attributes (vsm_*)
        self.polymarker_type = PolymarkerKind::Point;
        self.polymarker_color = default_color;
        self.drawing_mode = DrawingMode::Replace;  // vswr_mode
        self.polymarker_size = 1;

        // Reset line attributes (vsl_*)
        self.line_kind = LineKind::Solid;
        self.line_color = default_color;
        self.line_thickness = 1;
        // Note: line endpoints (vsl_ends) are not stored as separate attributes

        // Reset fill attributes (vsf_*)
        self.fill_pattern_type = PatternType::Solid;
        self.fill_color = default_color;
        // fill_style would be fill_pattern_type

        // Reset text attributes (vst_*)
        self.text_color = default_color;
        self.text_rotation = TextRotation::Degrees0;
        self.text_effects = TextEffects::NORMAL;  // vst_effects(0)
        self.text_size = 9;  // Default text height

        // Reset drawing position
        self.draw_to_position = Position::new(0, 0);

        // Reset other attributes
        self.fill_draw_border = false;
        self.line_user_mask = 0b1010_1010_1010_1010;
    }

    pub fn set_pixel(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, color: u8) {
        let res = buf.get_resolution();
        if x < 0 || y < 0 || x >= res.width || y >= res.height {
            return;
        }
        let offset = (y * res.width + x) as usize;
        buf.screen_mut()[offset] = color;
    }

    fn set_pixel_with_mode(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32, color: u8, is_non_transparent: bool) {
        let res = buf.get_resolution();
        if x < 0 || y < 0 || x >= res.width || y >= res.height {
            return;
        }
        let offset = (y * res.width + x) as usize;

        // Apply drawing mode to fill operations
        let final_color = match self.drawing_mode {
            DrawingMode::Replace => color,
            DrawingMode::Transparent => {
                // In transparent mode, only draw if color is non-zero
                if is_non_transparent {
                    color
                } else {
                    return; // Don't draw transparent pixels
                }
            }
            DrawingMode::Xor => {
                let existing = buf.screen()[offset];
                color ^ existing
            }
            DrawingMode::ReverseTransparent => {
                // In reverse transparent mode, only draw if color is zero
                if color == 0 {
                    color
                } else {
                    return;
                }
            }
        };

        buf.screen_mut()[offset] = final_color;
    }

    pub fn get_pixel(&mut self, buf: &dyn EditableScreen, x: i32, y: i32) -> u8 {
        let offset = (y * buf.get_resolution().width + x) as usize;
        buf.screen()[offset]
    }

    fn fill_pixel(&mut self, buf: &mut dyn EditableScreen, x: i32, y: i32) {
        let res = buf.get_resolution();
        let px = x;
        // In IGS medium/high Auflösungen sind die Pattern immer 16‑Pixel breit
        // und werden unabhängig von der aktuellen X‑Position wiederholt.
        // Damit das Brick‑Muster in CARD.ig sichtbar wird, verwenden wir
        // x modulo 16 statt der absoluten X‑Koordinate.
        if px < 0 || px >= res.width {
            return;
        }

        let fill_pattern = self.fill_pattern_type.fill_pattern(&self.user_patterns);
        let w = fill_pattern[(y as usize) % fill_pattern.len()];
        let mask = w & (0x8000 >> (px as usize % 16)) != 0;

        // Extract color from pattern: 1-bits use fill_color, 0-bits use background (0)
        let color = if mask { self.fill_color } else { 0 };

        // Apply drawing mode via set_pixel_with_mode
        self.set_pixel_with_mode(buf, x, y, color, mask);
    }
    
    
    pub fn set_fill_pattern(&mut self, pattern_type: PatternType) {
        self.fill_pattern_type = pattern_type;
    }
}

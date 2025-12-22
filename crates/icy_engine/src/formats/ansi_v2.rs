//! ANSI exporter (v2)
//!
//! This module provides a compatibility-level based ANSI saver.

use std::collections::HashMap;

use codepages::tables::UNICODE_TO_CP437;
use serde::{Deserialize, Serialize};

use crate::{
    ANSI_FONTS, AttributedChar, BitFont, Color, DOS_DEFAULT_PALETTE, Rectangle, Result, Tag, TagPlacement, TextBuffer, TextPane, XTERM_256_PALETTE,
    analyze_font_usage,
};

use super::{ControlCharHandling, ScreenPreperation};

/// Settings for SIXEL image encoding.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SixelSettings {
    /// Maximum number of colors in the palette (2-256).
    /// Fewer colors = smaller SIXEL output but less accurate colors.
    pub max_colors: u16,

    /// Floyd-Steinberg error diffusion strength (0.0-1.0).
    /// - 0.875: Default, best for photographs with smooth gradients
    /// - 0.5: Reduced dithering, less noise, good for graphics
    /// - 0.0: No dithering, sharp edges but may show color banding
    pub diffusion: f32,

    /// Use K-means clustering instead of Wu's quantizer.
    /// K-means is slower but may be more accurate for some images.
    pub use_kmeans: bool,
}

impl Default for SixelSettings {
    fn default() -> Self {
        Self {
            max_colors: 256,
            diffusion: 0.875, // FloydSteinberg::DEFAULT_ERROR_DIFFUSION
            use_kmeans: false,
        }
    }
}

impl SixelSettings {
    /// Convert to icy_sixel::EncodeOptions
    pub fn to_encode_options(&self) -> icy_sixel::EncodeOptions {
        icy_sixel::EncodeOptions {
            max_colors: self.max_colors,
            diffusion: self.diffusion,
            quantize_method: if self.use_kmeans {
                icy_sixel::QuantizeMethod::kmeans()
            } else {
                icy_sixel::QuantizeMethod::Wu
            },
        }
    }
}

const COLOR_OFFSETS: [u8; 8] = [0, 4, 2, 6, 1, 5, 3, 7];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnsiCompatibilityLevel {
    /// Strictest output targeting DOS `ANSI.SYS`.
    AnsiSys,
    /// DEC VT100-ish baseline (still 7-bit/8-bit text, 16 colors).
    Vt100,
    /// IcyTerm/SyncTerm class terminals (256 colors / truecolor / REP / sixel).
    IcyTerm,
    /// Modern UTF-8 terminal (truecolor / UTF-8 output / sixel).
    Utf8Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorSaveRestore {
    None,
    Dec,
}

impl AnsiCompatibilityLevel {
    fn supports_utf8(self) -> bool {
        matches!(self, Self::Utf8Terminal)
    }

    fn supports_256_colors(self) -> bool {
        matches!(self, Self::IcyTerm | Self::Utf8Terminal)
    }

    fn supports_truecolor(self) -> bool {
        matches!(self, Self::IcyTerm | Self::Utf8Terminal)
    }

    fn supports_sixel(self) -> bool {
        matches!(self, Self::IcyTerm | Self::Utf8Terminal)
    }

    fn supports_cuf(self) -> bool {
        matches!(self, Self::Vt100 | Self::IcyTerm | Self::Utf8Terminal)
    }

    fn supports_rep(self) -> bool {
        matches!(self, Self::IcyTerm | Self::Utf8Terminal)
    }

    fn cursor_save_restore(self) -> CursorSaveRestore {
        match self {
            Self::AnsiSys => CursorSaveRestore::None,
            Self::Vt100 | Self::IcyTerm | Self::Utf8Terminal => CursorSaveRestore::Dec,
        }
    }

    fn supports_font_pages(self) -> bool {
        matches!(self, Self::IcyTerm | Self::Utf8Terminal)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AnsiSaveOptions {
    pub format_type: i32,

    pub screen_preparation: ScreenPreperation,
    pub modern_terminal_output: bool,

    /// Optional override for the ANSI v2 exporter compatibility level.
    /// When `None`, the exporter will choose a reasonable default based on other options.
    pub level: Option<AnsiCompatibilityLevel>,

    /// Optional SAUCE metadata to append.
    #[serde(skip)]
    pub save_sauce: Option<super::save_options::SauceMetaData>,

    /// When set, the output will be compressed (subject to `level` capabilities).
    pub compress: bool,

    /// When set, the output may contain cursor forward sequences (CSI Ps C).
    pub use_cursor_forward: bool,
    /// When set, the output may contain repeat sequences (CSI Ps b).
    pub use_repeat_sequences: bool,

    /// When set, the output will contain the full line length.
    /// This is useful for files that are meant to be displayed on a unix terminal where the bg color may not be 100% black.
    pub preserve_line_length: bool,

    /// When set, the output will be cropped to this length.
    pub output_line_length: Option<usize>,

    /// When set the ansi engine will generate a gotoxy sequence at each line start
    /// making the file work on longer terminals.
    pub longer_terminal_output: bool,

    /// When set output ignores fg color changes in whitespaces
    /// and bg color changes in blocks.
    pub lossles_output: bool,

    /// When set the output will use extended color codes if they apply.
    pub use_extended_colors: bool,

    /// When set all whitespaces will be converted to spaces.
    pub normalize_whitespaces: bool,

    /// Changes control char output behavior
    pub control_char_handling: ControlCharHandling,

    /// Optional lines to skip when `longer_terminal_output` is enabled.
    #[serde(skip)]
    pub skip_lines: Option<Vec<usize>>,

    #[serde(skip)]
    pub alt_rgb: bool,

    #[serde(skip)]
    pub always_use_rgb: bool,

    /// When set, skip generating the thumbnail image (for formats that embed one).
    /// This is useful for autosave where rendering is expensive.
    #[serde(skip)]
    pub skip_thumbnail: bool,

    /// Settings for SIXEL image encoding.
    pub sixel_settings: SixelSettings,
}

impl Default for AnsiSaveOptions {
    fn default() -> Self {
        Self {
            format_type: 0,
            screen_preparation: ScreenPreperation::None,
            modern_terminal_output: false,
            level: None,
            save_sauce: None,
            compress: false,
            use_cursor_forward: true,
            use_repeat_sequences: false,
            preserve_line_length: false,
            output_line_length: None,
            longer_terminal_output: false,
            control_char_handling: ControlCharHandling::Ignore,
            skip_lines: None,
            lossles_output: false,
            use_extended_colors: true,
            normalize_whitespaces: true,
            alt_rgb: false,
            always_use_rgb: false,
            skip_thumbnail: false,
            sixel_settings: SixelSettings::default(),
        }
    }
}

impl AnsiSaveOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn effective_level(&self) -> AnsiCompatibilityLevel {
        if let Some(level) = self.level {
            return level;
        }

        if self.always_use_rgb {
            return AnsiCompatibilityLevel::IcyTerm;
        }

        if self.modern_terminal_output {
            return AnsiCompatibilityLevel::Utf8Terminal;
        }

        if self.use_extended_colors {
            return AnsiCompatibilityLevel::IcyTerm;
        }

        AnsiCompatibilityLevel::Vt100
    }

    /// Create from the new SaveOptions structure.
    pub fn from_save_options(options: &super::SaveOptions) -> Self {
        let ansi_opts = options.ansi_options();

        // Map AnsiCompatibilityLevel from new to old
        let level = Some(match ansi_opts.level {
            super::save_options::AnsiCompatibilityLevel::AnsiSys => AnsiCompatibilityLevel::AnsiSys,
            super::save_options::AnsiCompatibilityLevel::Vt100 => AnsiCompatibilityLevel::Vt100,
            super::save_options::AnsiCompatibilityLevel::IcyTerm => AnsiCompatibilityLevel::IcyTerm,
            super::save_options::AnsiCompatibilityLevel::Utf8Terminal => AnsiCompatibilityLevel::Utf8Terminal,
        });

        let modern_terminal_output = ansi_opts.level.supports_utf8();
        let use_extended_colors = ansi_opts.level.supports_256_colors();

        let (preserve_line_length, output_line_length) = match ansi_opts.line_length {
            super::save_options::LineLength::Default => (false, None),
            super::save_options::LineLength::Minimum(len) => (true, Some(len as usize)),
            super::save_options::LineLength::Maximum(len) => (false, Some(len as usize)),
        };

        let longer_terminal_output = matches!(ansi_opts.line_break, super::save_options::LineBreakBehavior::GotoXY);

        Self {
            format_type: 0,
            screen_preparation: ansi_opts.screen_prep,
            modern_terminal_output,
            level,
            save_sauce: options.sauce.clone(),
            compress: true, // ANSI uses compression sequences when available
            use_cursor_forward: ansi_opts.level.supports_cursor_forward(),
            use_repeat_sequences: ansi_opts.level.supports_repeat(),
            preserve_line_length,
            output_line_length,
            longer_terminal_output,
            lossles_output: !options.preprocess.optimize_colors,
            use_extended_colors,
            normalize_whitespaces: options.preprocess.normalize_whitespaces,
            control_char_handling: ansi_opts.control_char_handling,
            skip_lines: if ansi_opts.skip_lines.is_empty() {
                None
            } else {
                Some(ansi_opts.skip_lines.clone())
            },
            alt_rgb: false,
            always_use_rgb: false,
            skip_thumbnail: false,
            sixel_settings: SixelSettings {
                max_colors: ansi_opts.sixel.max_colors,
                diffusion: ansi_opts.sixel.diffusion,
                use_kmeans: ansi_opts.sixel.use_kmeans,
            },
        }
    }
}

fn uses_ice_colors(buf: &TextBuffer) -> bool {
    if buf.ice_mode == crate::IceMode::Ice {
        return true;
    }

    // try search ice colors
    for layer in &buf.layers {
        for y in 0..layer.height() {
            for x in 0..layer.width() {
                let ch = layer.char_at((x, y).into());
                let bg = ch.attribute.background();
                if bg >= 8 && bg < 16 {
                    return true;
                }
            }
        }
    }
    false
}

/// Save a `TextBuffer` to ANSI bytes using the new v2 compatibility-level API.
pub fn save_ansi_v2(buf: &TextBuffer, options: &super::SaveOptions) -> Result<Vec<u8>> {
    let ansi_options = AnsiSaveOptions::from_save_options(options);
    save_ansi_v2_internal(buf, &ansi_options)
}

/// Internal save function using legacy AnsiSaveOptions.
fn save_ansi_v2_internal(buf: &TextBuffer, options: &AnsiSaveOptions) -> Result<Vec<u8>> {
    let mut result: Vec<u8> = Vec::new();

    let mut generator = StringGeneratorV2::new(options.clone());
    generator.use_ice_colors = uses_ice_colors(buf);
    generator.tags = buf.tags.clone();

    generator.screen_prep();
    let state = generator.generate(buf, buf);
    generator.screen_end(buf, state);

    if generator.level.supports_sixel() {
        generator.add_sixels(buf);
    }

    result.extend(generator.data());

    if let Some(meta) = &options.save_sauce {
        use super::save_options::SauceBuilder;
        let sauce = buf.build_character_sauce(meta, icy_sauce::CharacterFormat::Ansi);
        sauce.write(&mut result)?;
    }

    Ok(result)
}

#[derive(Debug)]
struct CharCell {
    ch: char,
    sgr: Vec<u8>,
    extra_esc: Vec<u8>,
    font_page: usize,
    cur_state: AnsiState,
}

#[derive(Debug, Clone)]
struct AnsiState {
    is_bold: bool,
    is_blink: bool,
    is_faint: bool,
    is_italic: bool,
    is_underlined: bool,
    is_double_underlined: bool,
    is_crossed_out: bool,
    is_concealed: bool,

    fg_idx: u32,
    fg: Color,

    bg_idx: u32,
    bg: Color,
}

struct StringGeneratorV2 {
    output: Vec<u8>,
    options: AnsiSaveOptions,
    level: AnsiCompatibilityLevel,
    last_line_break: usize,
    max_output_line_length: usize,
    extended_color_hash: HashMap<(u8, u8, u8), u8>,

    pub line_offsets: Vec<usize>,
    pub tags: Vec<Tag>,
    use_ice_colors: bool,
}

impl StringGeneratorV2 {
    fn new(options: AnsiSaveOptions) -> Self {
        let level = options.effective_level();
        let mut output = Vec::new();

        if level.supports_utf8() {
            // write UTF-8 BOM as unicode indicator.
            output.extend([0xEF, 0xBB, 0xBF]);
        }

        let mut extended_color_hash = HashMap::new();
        if level.supports_256_colors() {
            for (i, (_, col)) in XTERM_256_PALETTE.iter().enumerate() {
                extended_color_hash.insert(col.rgb(), i as u8);
            }
        }

        let max_output_line_length = match level.cursor_save_restore() {
            CursorSaveRestore::None => usize::MAX,
            CursorSaveRestore::Dec => options.output_line_length.unwrap_or(usize::MAX),
        };

        Self {
            output,
            options,
            level,
            last_line_break: 0,
            max_output_line_length,
            extended_color_hash,
            line_offsets: Vec::new(),
            tags: Vec::new(),
            use_ice_colors: false,
        }
    }

    fn data(&self) -> &[u8] {
        &self.output
    }

    fn screen_prep(&mut self) {
        if self.use_ice_colors {
            self.push_bytes(b"\x1b[?33h");
        }

        match self.options.screen_preparation {
            ScreenPreperation::None => {}
            ScreenPreperation::ClearScreen => self.push_bytes(b"\x1b[2J"),
            ScreenPreperation::Home => self.push_bytes(b"\x1b[1;1H"),
        }
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        self.output.extend_from_slice(bytes);
    }

    fn cursor_save(&mut self) {
        match self.level.cursor_save_restore() {
            CursorSaveRestore::None => {}
            CursorSaveRestore::Dec => self.push_bytes(b"\x1b7"),
        }
    }

    fn cursor_restore(&mut self) {
        match self.level.cursor_save_restore() {
            CursorSaveRestore::None => {}
            CursorSaveRestore::Dec => self.push_bytes(b"\x1b8"),
        }
    }

    fn color(&self, buf: &TextBuffer, ch: AttributedChar, mut state: AnsiState) -> (AnsiState, Vec<u8>, Vec<u8>) {
        let attr = ch.attribute;
        let mut sgr: Vec<u8> = Vec::new();
        let mut extra_esc: Vec<u8> = Vec::new();

        let is_blank_cell = ch.ch == '\0' || ch.ch == ' ';

        let fg_is_ext = attr.is_foreground_ext();
        let bg_is_ext = attr.is_background_ext();

        let (cur_fore_rgb, cur_fore_color) = if attr.is_foreground_rgb() {
            let rgb = attr.foreground_rgb();
            (rgb, crate::Color::new(rgb.0, rgb.1, rgb.2))
        } else if fg_is_ext {
            let idx = attr.foreground_ext() as usize;
            let col = &XTERM_256_PALETTE[idx].1;
            (col.rgb(), col.clone())
        } else {
            let fg = attr.foreground();
            let cur_fore_color = buf.palette.color(fg);
            (cur_fore_color.rgb(), cur_fore_color)
        };

        let bg_value = if self.use_ice_colors && attr.is_blinking() && !attr.is_background_rgb() && !bg_is_ext {
            attr.background() + 8
        } else {
            attr.background()
        };

        let (cur_back_rgb, cur_back_color) = if attr.is_background_rgb() {
            let rgb = attr.background_rgb();
            (rgb, crate::Color::new(rgb.0, rgb.1, rgb.2))
        } else if bg_is_ext {
            let idx = attr.background_ext() as usize;
            let col = &XTERM_256_PALETTE[idx].1;
            (col.rgb(), col.clone())
        } else {
            let cur_back_color = buf.palette.color(bg_value);
            (cur_back_color.rgb(), cur_back_color)
        };

        let fore_idx: Option<usize> = DOS_DEFAULT_PALETTE.iter().position(|c| c.rgb() == cur_fore_rgb);
        let mut back_idx: Option<usize> = DOS_DEFAULT_PALETTE.iter().position(|c| c.rgb() == cur_back_rgb);

        // DOS bright foreground colors (8..15) are typically represented via bold + base color.
        let (fore_base_idx, fore_needs_bold) = match fore_idx {
            Some(idx) if (8..16).contains(&idx) => (Some(idx - 8), true),
            Some(idx) => (Some(idx), false),
            None => (None, false),
        };

        // Foreground-only style bits are irrelevant for blank cells and keeping
        // them stable greatly reduces output size (avoids SGR resets between
        // bold glyphs and non-bold spaces).
        let is_bold: bool = if is_blank_cell { state.is_bold } else { attr.is_bold() || fore_needs_bold };
        let mut is_blink = attr.is_blinking();
        let is_faint = if is_blank_cell { state.is_faint } else { attr.is_faint() };
        let is_italic = if is_blank_cell { state.is_italic } else { attr.is_italic() };
        let is_underlined = if is_blank_cell { state.is_underlined } else { attr.is_underlined() };
        let is_double_underlined = if is_blank_cell {
            state.is_double_underlined
        } else {
            attr.is_double_underlined()
        };
        let is_crossed_out = if is_blank_cell { state.is_crossed_out } else { attr.is_crossed_out() };
        let is_concealed: bool = if is_blank_cell { state.is_concealed } else { attr.is_concealed() };

        match buf.ice_mode {
            crate::IceMode::Unlimited => {
                if let Some(idx) = back_idx {
                    if idx > 7 {
                        back_idx = None;
                    }
                }
            }
            crate::IceMode::Blink => {
                if let Some(idx) = back_idx {
                    if (8..16).contains(&idx) {
                        back_idx = None;
                    }
                }
            }
            crate::IceMode::Ice => {
                if let Some(idx) = back_idx {
                    if idx < 8 {
                        is_blink = is_blink | attr.is_blinking();
                    } else if (8..16).contains(&idx) {
                        is_blink = true;
                        back_idx = Some(idx - 8);
                    }
                }
            }
        }

        let need_reset = (!is_bold && state.is_bold)
            || (!is_blink && state.is_blink)
            || (!is_italic && state.is_italic)
            || (!is_faint && state.is_faint)
            || (!is_underlined && state.is_underlined)
            || (!is_double_underlined && state.is_double_underlined)
            || (!is_crossed_out && state.is_crossed_out)
            || (!is_concealed && state.is_concealed);

        if need_reset {
            sgr.push(0);
            state.is_bold = false;
            state.is_blink = false;
            state.is_italic = false;
            state.is_faint = false;
            state.is_underlined = false;
            state.is_double_underlined = false;
            state.is_crossed_out = false;
            state.is_concealed = false;

            state.fg_idx = 7;
            state.fg = DOS_DEFAULT_PALETTE[7].clone();
            state.bg_idx = 0;
            state.bg = DOS_DEFAULT_PALETTE[0].clone();
        }

        if is_bold && !state.is_bold {
            sgr.push(1);
            if state.fg_idx < 8 {
                state.fg_idx += 8;
                state.fg = DOS_DEFAULT_PALETTE[state.fg_idx as usize].clone();
            }
            state.is_bold = true;
        }
        if is_faint && !state.is_faint {
            sgr.push(2);
            state.is_faint = true;
        }
        if is_italic && !state.is_italic {
            sgr.push(3);
            state.is_italic = true;
        }
        if is_underlined && !state.is_underlined {
            sgr.push(4);
            state.is_underlined = true;
        }

        if is_blink && !state.is_blink {
            sgr.push(5);
            state.is_blink = true;
        }

        if is_concealed && !state.is_concealed {
            sgr.push(8);
            state.is_concealed = true;
        }

        if is_crossed_out && !state.is_crossed_out {
            sgr.push(9);
            state.is_crossed_out = true;
        }

        if is_double_underlined && !state.is_double_underlined {
            sgr.push(21);
            state.is_double_underlined = true;
        }

        // Foreground
        // Only skip foreground changes for truly blank cells (space/NUL).
        if cur_fore_rgb != state.fg.rgb() && !(ch.ch == '\0' || ch.ch == ' ') {
            if fg_is_ext && self.level.supports_256_colors() {
                sgr.extend([38, 5, attr.foreground_ext()]);
                state.fg_idx = attr.foreground_ext() as u32;
            } else if let Some(base) = fore_base_idx {
                sgr.push(COLOR_OFFSETS[base] + 30);
                state.fg_idx = fore_idx.unwrap_or(base) as u32;
            } else if self.level.supports_256_colors() {
                if let Some(ext_color) = self.extended_color_hash.get(&cur_fore_rgb) {
                    sgr.extend([38, 5, *ext_color]);
                    state.fg_idx = *ext_color as u32;
                } else if self.level.supports_truecolor() {
                    if self.level == AnsiCompatibilityLevel::IcyTerm {
                        extra_esc.extend_from_slice(format!("\x1b[1;{};{};{}t", cur_fore_rgb.0, cur_fore_rgb.1, cur_fore_rgb.2).as_bytes());
                        state.fg_idx = u32::MAX;
                    } else {
                        sgr.extend([38, 2, cur_fore_rgb.0, cur_fore_rgb.1, cur_fore_rgb.2]);
                        state.fg_idx = u32::MAX;
                    }
                } else {
                    // Best effort: fall back to 16-color mapping.
                    sgr.push(37);
                    state.fg_idx = 7;
                }
            } else if self.level.supports_truecolor() {
                if self.level == AnsiCompatibilityLevel::IcyTerm {
                    extra_esc.extend_from_slice(format!("\x1b[1;{};{};{}t", cur_fore_rgb.0, cur_fore_rgb.1, cur_fore_rgb.2).as_bytes());
                    state.fg_idx = u32::MAX;
                } else {
                    sgr.extend([38, 2, cur_fore_rgb.0, cur_fore_rgb.1, cur_fore_rgb.2]);
                    state.fg_idx = u32::MAX;
                }
            } else {
                // Best effort: fall back to 16-color mapping.
                sgr.push(37);
                state.fg_idx = 7;
            }
            state.fg = cur_fore_color;
        }

        // Background
        if cur_back_rgb != state.bg.rgb() {
            if bg_is_ext && self.level.supports_256_colors() {
                sgr.extend([48, 5, attr.background_ext()]);
                state.bg_idx = attr.background_ext() as u32;
            } else if let Some(bg_idx) = back_idx {
                let skip_base_bg_emit_due_to_ice =
                    matches!(buf.ice_mode, crate::IceMode::Ice) && is_blink && bg_idx == 0 && state.bg_idx == 0 && !attr.is_background_rgb() && !bg_is_ext;

                if !skip_base_bg_emit_due_to_ice {
                    sgr.push(COLOR_OFFSETS[bg_idx] + 40);
                }
                state.bg_idx = bg_idx as u32;
            } else if self.level.supports_256_colors() {
                if let Some(ext_color) = self.extended_color_hash.get(&cur_back_rgb) {
                    sgr.extend([48, 5, *ext_color]);
                    state.bg_idx = *ext_color as u32;
                } else if self.level.supports_truecolor() {
                    if self.level == AnsiCompatibilityLevel::IcyTerm {
                        extra_esc.extend_from_slice(format!("\x1b[0;{};{};{}t", cur_back_rgb.0, cur_back_rgb.1, cur_back_rgb.2).as_bytes());
                        state.bg_idx = u32::MAX;
                    } else {
                        sgr.extend([48, 2, cur_back_rgb.0, cur_back_rgb.1, cur_back_rgb.2]);
                        state.bg_idx = u32::MAX;
                    }
                } else {
                    // Best effort: fall back to black background.
                    sgr.push(40);
                    state.bg_idx = 0;
                }
            } else if self.level.supports_truecolor() {
                if self.level == AnsiCompatibilityLevel::IcyTerm {
                    extra_esc.extend_from_slice(format!("\x1b[0;{};{};{}t", cur_back_rgb.0, cur_back_rgb.1, cur_back_rgb.2).as_bytes());
                    state.bg_idx = u32::MAX;
                } else {
                    sgr.extend([48, 2, cur_back_rgb.0, cur_back_rgb.1, cur_back_rgb.2]);
                    state.bg_idx = u32::MAX;
                }
            } else {
                // Best effort: fall back to black background.
                sgr.push(40);
                state.bg_idx = 0;
            }

            state.bg = cur_back_color;
        }

        (state, sgr, extra_esc)
    }

    fn generate_ansi_font_map(buf: &TextBuffer) -> HashMap<u8, u8> {
        let mut font_map = HashMap::new();

        let mut ansi_fonts = Vec::new();
        for i in 0..ANSI_FONTS as u8 {
            ansi_fonts.push(BitFont::from_ansi_font_page(i, buf.font_dimensions().height as u8).unwrap());
        }
        for (page, font) in buf.font_iter() {
            let mut to_page = *page;
            for (i, ansi_font) in ansi_fonts.iter().enumerate() {
                if *ansi_font == font {
                    to_page = i as u8;
                    break;
                }
            }
            font_map.insert(*page, to_page);
        }

        font_map
    }

    fn generate_cells<T: TextPane>(&self, buf: &TextBuffer, layer: &T, area: Rectangle, font_map: &HashMap<u8, u8>) -> (AnsiState, Vec<Vec<CharCell>>) {
        let mut result: Vec<Vec<CharCell>> = Vec::new();

        let mut state = AnsiState {
            is_bold: false,
            is_blink: false,
            is_italic: false,
            is_faint: false,
            is_underlined: false,
            is_double_underlined: false,
            is_crossed_out: false,
            is_concealed: false,
            fg_idx: 7,
            fg: DOS_DEFAULT_PALETTE[7].clone(),
            bg: DOS_DEFAULT_PALETTE[0].clone(),
            bg_idx: 0,
        };

        for y in area.y_range() {
            let mut line = Vec::new();

            if self.options.longer_terminal_output {
                if let Some(skip_lines) = &self.options.skip_lines {
                    if skip_lines.contains(&(y as usize)) {
                        result.push(line);
                        continue;
                    }
                }
            }

            let mut len = if self.options.compress && !self.options.preserve_line_length {
                let mut last = area.width() - 1;
                let last_attr = layer.char_at((last, y).into()).attribute;
                if last_attr.background() == 0 {
                    while last > area.left() {
                        let c = layer.char_at((last, y).into());
                        if c.ch != ' ' && c.ch != 0xFF as char && c.ch != 0 as char {
                            break;
                        }
                        if c.attribute != last_attr {
                            break;
                        }
                        last -= 1;
                    }
                }
                let last = last + 1;
                if last >= area.width() - 1 { area.width() } else { last }
            } else {
                area.width()
            };

            for t in self.tags.iter() {
                if t.is_enabled && t.tag_placement == TagPlacement::InText && t.position.y == y as i32 {
                    len = len.max(t.position.x + t.len() as i32);
                }
            }

            let mut x = 0;
            while x < len {
                let mut found_tag = false;
                for t in self.tags.iter() {
                    if t.is_enabled && t.tag_placement == TagPlacement::InText && t.position.y == y as i32 && t.position.x == x as i32 {
                        for ch in t.replacement_value.chars() {
                            line.push(CharCell {
                                ch,
                                sgr: Vec::new(),
                                extra_esc: Vec::new(),
                                font_page: 0,
                                cur_state: state.clone(),
                            });
                        }
                        x += (t.len() as i32).max(1);
                        found_tag = true;
                        break;
                    }
                }
                if found_tag {
                    continue;
                }

                let ch = layer.char_at((x, y).into());
                if ch.is_visible() {
                    let (new_state, sgr, extra_esc) = self.color(buf, ch, state);
                    state = new_state;
                    line.push(CharCell {
                        ch: ch.ch,
                        sgr,
                        extra_esc,
                        font_page: *font_map.get(&ch.font_page()).unwrap_or(&0) as usize,
                        cur_state: state.clone(),
                    });
                } else {
                    line.push(CharCell {
                        ch: ' ',
                        sgr: Vec::new(),
                        extra_esc: Vec::new(),
                        font_page: *font_map.get(&ch.font_page()).unwrap_or(&0) as usize,
                        cur_state: state.clone(),
                    });
                }
                x += 1;
            }

            // In UTF-8 mode we keep the state across lines; the legacy exporter
            // resets this for some modes, but doing so changes output size.
            result.push(line);
        }

        (state, result)
    }

    fn generate<T: TextPane>(&mut self, buf: &TextBuffer, layer: &T) -> AnsiState {
        let mut result = Vec::new();

        // Embed fonts only for terminals that support it.
        if self.level.supports_font_pages() {
            let used_fonts = analyze_font_usage(buf);
            for font_slot in used_fonts {
                if font_slot >= 100 {
                    if let Some(font) = buf.font(font_slot) {
                        result.extend_from_slice(font.encode_as_ansi(font_slot as usize).as_bytes());
                    }
                }
            }
        }

        let font_map = StringGeneratorV2::generate_ansi_font_map(buf);
        let mut area = layer.rectangle();
        let line_count = layer.line_count();
        if line_count > 0 {
            area.size.height = line_count.min(area.size.height);
        }

        let (state, cells) = self.generate_cells(buf, layer, area, &font_map);
        let mut cur_font_page = 0;

        let mut effective_line_lengths: Vec<usize> = Vec::with_capacity(cells.len());
        let full_width = layer.width().max(0) as usize;
        for line in &cells {
            if self.options.preserve_line_length {
                effective_line_lengths.push(line.len());
                continue;
            }

            let mut len = line.len();
            while len > 0 {
                let cell = &line[len - 1];
                let is_default_state = cell.cur_state.fg_idx == 7
                    && cell.cur_state.bg_idx == 0
                    && !cell.cur_state.is_bold
                    && !cell.cur_state.is_blink
                    && !cell.cur_state.is_faint
                    && !cell.cur_state.is_italic
                    && !cell.cur_state.is_underlined
                    && !cell.cur_state.is_double_underlined
                    && !cell.cur_state.is_crossed_out
                    && !cell.cur_state.is_concealed;

                if cell.ch == ' ' && cell.sgr.is_empty() && cell.extra_esc.is_empty() && cell.font_page == 0 && is_default_state {
                    len -= 1;
                } else {
                    break;
                }
            }
            effective_line_lengths.push(len);
        }

        let mut is_first_output_line = true;

        for (y, line) in cells.iter().enumerate() {
            let mut x = 0;
            let mut printed_last_column = false;

            if !self.output.is_empty() {
                self.line_offsets.push(self.output.len());
            }

            if self.options.longer_terminal_output {
                if let Some(skip_lines) = &self.options.skip_lines {
                    if skip_lines.contains(&y) {
                        continue;
                    }
                }
                if is_first_output_line {
                    is_first_output_line = false;
                    result.extend_from_slice(b"\x1b[0m");
                }
                result.extend_from_slice(b"\x1b[");
                result.extend_from_slice((y + 1).to_string().as_bytes());
                result.push(b'H');
                self.push_result(&mut result);
            }

            let len = *effective_line_lengths.get(y).unwrap_or(&line.len());
            while x < len {
                let cell = &line[x];

                if self.level.supports_font_pages() && cur_font_page != cell.font_page {
                    cur_font_page = cell.font_page;
                    result.extend_from_slice(b"\x1b[0;");
                    result.extend_from_slice(cur_font_page.to_string().as_bytes());
                    result.extend_from_slice(b" D");
                    self.push_result(&mut result);
                }

                if !cell.sgr.is_empty() {
                    result.extend_from_slice(b"\x1b[");
                    for i in 0..cell.sgr.len() - 1 {
                        result.extend_from_slice(cell.sgr[i].to_string().as_bytes());
                        result.push(b';');
                    }
                    result.extend_from_slice(cell.sgr.last().unwrap().to_string().as_bytes());
                    result.push(b'm');
                    self.push_result(&mut result);
                }

                if !cell.extra_esc.is_empty() {
                    result.extend_from_slice(&cell.extra_esc);
                    self.push_result(&mut result);
                }

                let cell_char = if self.level.supports_utf8() {
                    if cell.ch == '\0' {
                        vec![b' ']
                    } else {
                        let uni_ch = buf.buffer_type.convert_to_unicode(cell.ch);
                        uni_ch.to_string().as_bytes().to_vec()
                    }
                } else {
                    let mut ch = cell.ch;
                    if ch == '\0' {
                        ch = ' ';
                    }
                    if buf.buffer_type == crate::BufferType::Unicode {
                        if let Some(tch) = UNICODE_TO_CP437.get(&ch) {
                            ch = *tch as char;
                        }
                    }

                    if Self::CONTROL_CHARS.contains(ch) {
                        match self.options.control_char_handling {
                            ControlCharHandling::Ignore => vec![ch as u8],
                            ControlCharHandling::IcyTerm => vec![b'\x1B', ch as u8],
                            ControlCharHandling::FilterOut => vec![b'.'],
                        }
                    } else {
                        vec![ch as u8]
                    }
                };

                // Compression
                if self.options.compress {
                    let mut rle = x + 1;
                    while rle < len {
                        if line[rle].ch != line[x].ch || !line[rle].sgr.is_empty() || line[rle].font_page != line[x].font_page {
                            break;
                        }
                        rle += 1;
                    }
                    rle -= 1;
                    rle -= x;

                    if self.options.use_cursor_forward
                        && self.level.supports_cuf()
                        && line[x].ch == ' '
                        && line[x].cur_state.bg_idx == 0
                        && !line[x].cur_state.is_blink
                    {
                        let fmt = format!("\x1B[{}C", rle + 1);
                        let output = fmt.as_bytes();
                        if output.len() <= rle {
                            self.push_result(&mut result);
                            result.extend_from_slice(output);
                            self.push_result(&mut result);
                            x += rle + 1;
                            continue;
                        }
                    }

                    if self.options.use_repeat_sequences && self.level.supports_rep() {
                        let fmt = format!("\x1B[{rle}b");
                        let output = fmt.as_bytes();
                        if output.len() <= rle {
                            self.push_result(&mut result);
                            result.extend_from_slice(&cell_char);
                            result.extend_from_slice(output);
                            self.push_result(&mut result);

                            if full_width > 0 && x + rle >= full_width.saturating_sub(1) {
                                printed_last_column = true;
                            }

                            x += rle + 1;
                            continue;
                        }
                    }
                }

                result.extend_from_slice(&cell_char);
                self.push_result(&mut result);

                if full_width > 0 && x == full_width.saturating_sub(1) {
                    printed_last_column = true;
                }

                x += 1;
            }

            if !self.options.longer_terminal_output {
                // Deterministic playback: always emit CRLF between rows.
                // Relying on terminal autowrap differs across emulators and also
                // makes roundtrip-parse comparisons flaky.
                if y + 1 < cells.len() {
                    let is_full_width = full_width > 0 && len == full_width;
                    let can_rely_on_autowrap = is_full_width && printed_last_column;

                    // If we printed the last column, many parsers/emulators will already
                    // advance to the next line due to autowrap. Emitting an explicit CRLF
                    // in that case can cause a double line-advance.
                    let emit_crlf = !can_rely_on_autowrap;

                    if emit_crlf {
                        if self.level.supports_utf8() {
                            result.extend_from_slice(b"\x1b[0m");
                        }
                        result.push(13);
                        result.push(10);
                    }
                }
                self.push_result(&mut result);
                self.last_line_break = self.output.len();
            }
        }

        // Flush any remaining buffered bytes (e.g. embedded fonts when the
        // visible area is empty).
        self.push_result(&mut result);

        state
    }

    const CONTROL_CHARS: &'static str = "\x1b\x07\x08\x09\x0C\x7F\r\n";

    fn screen_end(&mut self, buf: &TextBuffer, mut state: AnsiState) {
        let mut end_tags = 0;
        for tag in buf.tags.iter() {
            if tag.is_enabled && tag.tag_placement == crate::TagPlacement::WithGotoXY {
                let (new_state, sgr, extra_esc) = self.color(buf, AttributedChar::new('#', tag.attribute), state);
                state = new_state;

                if !extra_esc.is_empty() {
                    self.output.extend_from_slice(&extra_esc);
                }

                if !sgr.is_empty() {
                    self.output.extend_from_slice(b"\x1b[");
                    for i in 0..sgr.len() - 1 {
                        self.output.extend_from_slice(sgr[i].to_string().as_bytes());
                        self.output.push(b';');
                    }
                    self.output.extend_from_slice(sgr.last().unwrap().to_string().as_bytes());
                    self.output.push(b'm');
                }

                if end_tags == 0 {
                    self.cursor_save();
                }
                end_tags += 1;
                self.output
                    .extend_from_slice(format!("\x1b[{};{}H", tag.position.y + 1, tag.position.x + 1).as_bytes());
                self.output.extend_from_slice(tag.replacement_value.as_bytes());
            }
        }

        if end_tags > 0 {
            self.cursor_restore();
        }

        if self.use_ice_colors {
            self.output.extend_from_slice(b"\x1b[?33l");
        }
    }

    fn add_sixels(&mut self, buf: &TextBuffer) {
        let encode_options = self.options.sixel_settings.to_encode_options();
        for layer in &buf.layers {
            for sixel in &layer.sixels {
                match icy_sixel::sixel_encode(&sixel.picture_data, sixel.width() as usize, sixel.height() as usize, &encode_options) {
                    Err(err) => log::error!("{err}"),
                    Ok(data) => {
                        let p = layer.offset() + sixel.position;
                        self.output.extend(format!("\x1b[{};{}H", p.y + 1, p.x + 1).as_bytes());
                        self.output.extend(data.as_bytes());
                    }
                }
            }
        }
    }

    fn push_result(&mut self, result: &mut Vec<u8>) {
        if self.output.len() + result.len() - self.last_line_break > self.max_output_line_length {
            // Only safe when cursor save/restore is available.
            if self.max_output_line_length != usize::MAX {
                self.cursor_save();
                self.output.push(13);
                self.output.push(10);
                self.last_line_break = self.output.len();
                self.cursor_restore();
            }
        }
        self.output.append(result);
        result.clear();
    }
}

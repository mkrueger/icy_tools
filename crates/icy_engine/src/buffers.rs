use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Alignment;
use std::{
    cmp::max,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use i18n_embed_fl::fl;
use icy_sauce::prelude::*;

use crate::ansi::MusicOption;
use crate::ansi::sound::AnsiMusic;
use crate::paint::HalfBlock;
use crate::{
    Color, EngineResult, FORMATS, Glyph, Layer, Line, LoadData, LoadingError, OutputFormat, Position, Rectangle, Sixel, TerminalState, TextAttribute, TextPane,
    UnicodeConverter, attribute, parsers,
};

use super::{AttributedChar, BitFont, Palette, SaveOptions, Size};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BufferType {
    Unicode,
    CP437,
    Petscii,
    Atascii,
    Viewdata,
}

impl BufferType {
    pub fn from_byte(b: u8) -> Self {
        match b {
            // 0 => BufferType::Unicode,
            1 => BufferType::CP437,
            2 => BufferType::Petscii,
            3 => BufferType::Atascii,
            4 => BufferType::Viewdata,
            _ => BufferType::Unicode,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            BufferType::Unicode => 0,
            BufferType::CP437 => 1,
            BufferType::Petscii => 2,
            BufferType::Atascii => 3,
            BufferType::Viewdata => 4,
        }
    }

    pub fn get_selection_colors(&self) -> (Color, Color) {
        match self {
            // CP437 and Unicode use VGA-style magenta on gray selection
            BufferType::CP437 | BufferType::Unicode => (
                Color::new(0xAA, 0x00, 0xAA), // Magenta foreground
                Color::new(0xAA, 0xAA, 0xAA), // Gray background
            ),
            // Petscii uses Commodore VIC colors
            BufferType::Petscii => (
                Color::new(0x37, 0x39, 0xC4), // VIC blue foreground
                Color::new(0xB0, 0x3F, 0xB6), // VIC purple background
            ),
            // Atascii uses Atari ANTIC colors
            BufferType::Atascii => (
                Color::new(0x09, 0x51, 0x83), // ANTIC blue foreground
                Color::new(0xFF, 0xFF, 0xFF), // White background
            ),
            // Viewdata uses black on white like Videotex/Mode7
            BufferType::Viewdata => (
                Color::new(0x00, 0x00, 0x00), // Black foreground
                Color::new(0xFF, 0xFF, 0xFF), // White background
            ),
        }
    }

    pub fn convert_to_unicode(&self, ch: char) -> char {
        match self {
            BufferType::Unicode => ch, // Already Unicode, no conversion needed

            BufferType::CP437 => {
                // Use the CP437 converter for DOS/ANSI characters
                parsers::ascii::CP437Converter::default().convert_to_unicode(ch)
            }

            BufferType::Petscii => {
                // Use the PETSCII converter for Commodore characters
                parsers::petscii::CharConverter::default().convert_to_unicode(ch)
            }

            BufferType::Atascii => {
                // Use the ATASCII converter for Atari characters
                parsers::atascii::CharConverter::default().convert_to_unicode(ch)
            }

            BufferType::Viewdata => {
                // Viewdata/Teletext uses mostly ASCII with some special graphics
                // For now, pass through ASCII chars and replace control chars with space
                if ch.is_ascii_graphic() || ch == ' ' {
                    ch
                } else {
                    ' ' // Replace non-printable with space for Viewdata
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IceMode {
    Unlimited,
    Blink,
    Ice,
}

impl IceMode {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0 => IceMode::Unlimited,
            1 => IceMode::Blink,
            _ => IceMode::Ice,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            IceMode::Unlimited => 0,
            IceMode::Blink => 1,
            IceMode::Ice => 2,
        }
    }
    pub fn has_blink(self) -> bool {
        !matches!(self, IceMode::Ice)
    }

    pub fn has_high_bg_colors(self) -> bool {
        !matches!(self, IceMode::Blink)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaletteMode {
    RGB,
    Fixed16,
    /// Extended font mode in XB + Blink limits to 8 colors
    Free8,
    Free16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TagPlacement {
    InText,
    WithGotoXY,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TagRole {
    Displaycode,
    Hyperlink,
}

#[derive(Clone, Debug)]
pub struct Tag {
    pub is_enabled: bool,
    pub preview: String,
    pub replacement_value: String,

    pub position: Position,
    pub length: usize,
    pub alignment: Alignment,
    pub tag_placement: TagPlacement,
    pub tag_role: TagRole,

    pub attribute: TextAttribute,
}

impl Tag {
    pub fn contains(&self, cur: Position) -> bool {
        self.position.y == cur.y && self.position.x <= cur.x && cur.x < self.position.x + self.len() as i32
    }

    pub fn len(&self) -> usize {
        if self.length == 0 {
            return self.preview.len();
        }
        self.length
    }

    fn get_char_at(&self, x: i32) -> AttributedChar {
        let ch = match self.alignment {
            Alignment::Left => self.preview.chars().nth(x as usize).unwrap_or(' '),
            Alignment::Right => self.preview.chars().nth(self.len() as usize - x as usize - 1).unwrap_or(' '),
            Alignment::Center => {
                let half = self.len() as usize / 2;
                if (x as usize) < half {
                    self.preview.chars().nth(x as usize).unwrap_or(' ')
                } else {
                    self.preview.chars().nth(self.len() as usize - x as usize - 1).unwrap_or(' ')
                }
            }
        };
        if self.tag_role == TagRole::Displaycode {
            AttributedChar::new(ch, self.attribute)
        } else {
            let mut attr = self.attribute;
            attr.set_is_underlined(true);
            AttributedChar::new(ch, attr)
        }
    }
}

impl PaletteMode {
    pub fn from_byte(b: u8) -> Self {
        match b {
            // 0 => PaletteMode::RGB,
            1 => PaletteMode::Fixed16,
            2 => PaletteMode::Free8,
            3 => PaletteMode::Free16,
            _ => PaletteMode::RGB,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            PaletteMode::RGB => 0,
            PaletteMode::Fixed16 => 1,
            PaletteMode::Free8 => 2,
            PaletteMode::Free16 => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FontMode {
    /// Multiple fonts in the same document are possible without limit.
    Unlimited,
    /// Single font only sauce fonts apply
    Sauce,
    /// Single font all fonts are possible
    Single,
    /// Used to limit the the font pages
    /// For example 2 fonts for XB enhanced font mode.
    FixedSize,
}

impl FontMode {
    pub fn from_byte(b: u8) -> Self {
        match b {
            //  0 => FontMode::Unlimited,
            1 => FontMode::Sauce,
            2 => FontMode::Single,
            3 => FontMode::FixedSize,
            _ => FontMode::Unlimited,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            FontMode::Unlimited => 0,
            FontMode::Sauce => 1,
            FontMode::Single => 2,
            FontMode::FixedSize => 3,
        }
    }

    pub fn has_high_fg_colors(self) -> bool {
        !matches!(self, FontMode::FixedSize)
    }
}

pub struct Buffer {
    original_size: Size,
    size: Size,
    pub file_name: Option<PathBuf>,

    pub terminal_state: TerminalState,

    pub buffer_type: BufferType,
    pub ice_mode: IceMode,
    pub palette_mode: PaletteMode,
    pub font_mode: FontMode,

    sauce_data: icy_sauce::MetaData,

    pub palette: Palette,

    /// the layer the overlay is displayed upon (there could be layers above the overlay layer)
    overlay_index: usize,
    overlay_layer: Option<Layer>,

    font_table: HashMap<usize, BitFont>,
    is_font_table_dirty: bool,
    pub layers: Vec<Layer>,

    pub sixel_threads: VecDeque<std::thread::JoinHandle<EngineResult<Sixel>>>, // pub undo_stack: Vec<Box<dyn UndoOperation>>,
    // pub redo_stack: Vec<Box<dyn UndoOperation>>,
    use_letter_spacing: bool,
    use_aspect_ratio: bool,

    pub show_tags: bool,
    pub tags: Vec<Tag>,
    pub ansi_music: Vec<AnsiMusic>,

    /// Scrollback buffer storing lines that scrolled off the top
    pub scrollback_lines: VecDeque<Line>,

    /// Maximum number of lines to keep in scrollback (0 = unlimited)
    pub max_scrollback_lines: usize,
}

impl std::fmt::Debug for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("file_name", &self.file_name)
            .field("width", &self.get_width())
            .field("height", &self.get_height())
            .field("custom_palette", &self.palette)
            .field("layers", &self.layers)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut str = String::new();
        let p = parsers::ascii::CP437Converter::default();

        for y in 0..self.get_height() {
            str.extend(format!("{y:3}: ").chars());
            for x in 0..self.get_width() {
                let ch = self.get_char((x, y).into());
                str.push(p.convert_to_unicode(ch.ch));
            }
            str.push('\n');
        }
        write!(f, "{str}")
    }
}

impl Buffer {
    pub fn scan_buffer_features(&self) -> BufferFeatures {
        let mut result = BufferFeatures::default();
        for layer in &self.layers {
            if !layer.sixels.is_empty() {
                result.use_sixels = true;
            }
            if !layer.hyperlinks.is_empty() {
                result.has_links = true;
            }
            for y in 0..layer.get_height() {
                for x in 0..layer.get_width() {
                    let ch = layer.get_char((x, y).into());

                    if ch.attribute.get_foreground() != 7 || ch.attribute.get_background() != 0 {
                        result.use_colors = true;
                    }

                    result.use_blink |= ch.attribute.is_blinking();
                    result.use_extended_attributes |= ch.attribute.is_crossed_out()
                        || ch.attribute.is_underlined()
                        || ch.attribute.is_concealed()
                        || ch.attribute.is_crossed_out()
                        || ch.attribute.is_double_height()
                        || ch.attribute.is_double_underlined()
                        || ch.attribute.is_overlined();
                }
            }
        }
        result.font_count = analyze_font_usage(self).len();
        result.use_extended_colors = self.palette.len() > 16;

        result
    }

    pub fn get_sauce_meta(&self) -> &icy_sauce::MetaData {
        &self.sauce_data
    }
    pub fn set_sauce_meta(&mut self, sauce: icy_sauce::MetaData) {
        self.sauce_data = sauce;
    }

    pub(crate) fn load_sauce(&mut self, sauce: icy_sauce::SauceRecord) {
        match sauce.capabilities() {
            Some(Capabilities::Character(CharacterCapabilities {
                columns,
                lines,
                font_opt,
                ice_colors,
                aspect_ratio,
                letter_spacing,
                ..
            }))
            | Some(Capabilities::Binary(BinaryCapabilities {
                columns,
                lines,
                font_opt,
                ice_colors,
                aspect_ratio,
                letter_spacing,
                ..
            })) => {
                // check limits, some files have wrong sauce data, even if 0 is specified
                // some files specify the pixel size there and don't have line breaks in the file
                let size = Size::new(columns.clamp(1, 1000) as i32, lines as i32);
                self.set_size(size);
                self.terminal_state.set_size(size);

                if !self.layers.is_empty() {
                    self.layers[0].set_size(size);
                }

                if let Some(font) = &font_opt {
                    if let Ok(font) = BitFont::from_sauce_name(&font.to_string()) {
                        self.set_font(0, font);
                    }
                }
                if ice_colors {
                    self.ice_mode = IceMode::Ice;
                }
                self.use_aspect_ratio = aspect_ratio == AspectRatio::LegacyDevice;
                self.use_letter_spacing = letter_spacing == LetterSpacing::NinePixel;
            }
            _ => {}
        }

        self.is_font_table_dirty = true;
        self.sauce_data = sauce.metadata();
    }

    /// Clones the buffer (without sixel threads)
    pub fn flat_clone(&self, deep_layers: bool) -> Buffer {
        let mut frame = Buffer::new(self.get_size());
        frame.file_name = self.file_name.clone();
        frame.terminal_state = self.terminal_state.clone();
        frame.buffer_type = self.buffer_type;
        frame.ice_mode = self.ice_mode;
        frame.palette_mode = self.palette_mode;
        frame.font_mode = self.font_mode;
        frame.terminal_state.is_terminal_buffer = self.terminal_state.is_terminal_buffer;
        frame.terminal_state = self.terminal_state.clone();
        frame.palette = self.palette.clone();
        frame.sauce_data = self.sauce_data.clone();

        if deep_layers {
            frame.layers = Vec::new();
            for l in &self.layers {
                frame.layers.push(l.clone());
            }
        } else {
            for y in 0..self.get_height() {
                for x in 0..self.get_width() {
                    let ch = self.get_char((x, y).into());
                    frame.layers[0].set_char((x, y), ch);
                }
            }
        }

        frame.clear_font_table();
        for f in self.font_iter() {
            frame.set_font(*f.0, f.1.clone());
        }
        frame.tags = self.tags.clone();
        frame.show_tags = self.show_tags;
        frame
    }

    pub(crate) fn write_sauce_info(&self, data_type: SauceDataType, content_type: CharacterFormat, result: &mut Vec<u8>) -> anyhow::Result<()> {
        let mut builder = self.get_sauce_meta().to_builder()?.file_size(result.len() as u32).data_type(data_type);

        match data_type {
            SauceDataType::Character => {
                let mut caps = CharacterCapabilities::new(content_type);
                caps.columns = self.get_width() as u16;
                caps.lines = self.get_height() as u16;
                caps.ice_colors = self.ice_mode == IceMode::Ice;
                caps.letter_spacing = if self.use_letter_spacing {
                    LetterSpacing::NinePixel
                } else {
                    LetterSpacing::Legacy
                };
                caps.aspect_ratio = if self.use_aspect_ratio {
                    AspectRatio::LegacyDevice
                } else {
                    AspectRatio::Legacy
                };
                builder = builder.capabilities(Capabilities::Character(caps))?;
            }
            SauceDataType::BinaryText => {
                builder = builder
                    .capabilities(Capabilities::Binary(BinaryCapabilities::binary_text(self.get_width() as u16).unwrap()))
                    .unwrap();
            }
            SauceDataType::XBin => {
                builder = builder
                    .capabilities(Capabilities::Binary(
                        BinaryCapabilities::xbin(self.get_width() as u16, self.get_height() as u16).unwrap(),
                    ))
                    .unwrap();
            }
            _ => {}
        }

        builder.build().write(result)?;

        Ok(())
    }

    pub(crate) fn has_sauce(&self) -> bool {
        !self.get_sauce_meta().is_empty()
    }

    fn merge_layer_char(&self, found_char: &mut AttributedChar, cur_layer: &Layer, pos: Position) {
        let cur_char = cur_layer.get_char(pos);
        match cur_layer.properties.mode {
            crate::Mode::Normal => {
                let underlying_char = *found_char;
                if cur_char.is_visible() {
                    *found_char = cur_char;
                }

                if found_char.attribute.foreground_color == TextAttribute::TRANSPARENT_COLOR
                    || found_char.attribute.background_color == TextAttribute::TRANSPARENT_COLOR
                {
                    *found_char = self.make_solid_color(*found_char, underlying_char);
                }

                if !cur_layer.properties.has_alpha_channel {
                    found_char.attribute.attr &= !attribute::INVISIBLE;
                    if found_char.attribute.background_color == TextAttribute::TRANSPARENT_COLOR {
                        found_char.attribute.background_color = 0;
                    }
                    if found_char.attribute.foreground_color == TextAttribute::TRANSPARENT_COLOR {
                        found_char.attribute.foreground_color = 0;
                    }
                }
            }
            crate::Mode::Chars => {
                if !cur_char.is_transparent() {
                    found_char.ch = cur_char.ch;
                    found_char.set_font_page(cur_char.get_font_page());
                }
            }
            crate::Mode::Attributes => {
                if cur_char.is_visible() {
                    found_char.attribute = cur_char.attribute;
                }
            }
        }
    }

    pub fn push_to_scrollback(&mut self, line: Line) {
        self.scrollback_lines.push_back(line);

        // Trim oldest lines if we exceed the limit
        if self.max_scrollback_lines > 0 {
            while self.scrollback_lines.len() > self.max_scrollback_lines {
                self.scrollback_lines.pop_front();
            }
        }
    }

    pub fn clear_scrollback(&mut self) {
        self.scrollback_lines.clear();
    }

    pub fn get_max_scrollback_offset(&self) -> usize {
        self.scrollback_lines.len()
    }

    pub fn scrollback_position(&self) -> usize {
        // TODO
        0
    }

    pub fn set_scroll_position(&self, _line: usize) {
        // TODO
    }
}

pub fn analyze_font_usage(buf: &Buffer) -> Vec<usize> {
    let mut hash_set = HashSet::new();
    for y in 0..buf.get_height() {
        for x in 0..buf.get_width() {
            let ch = buf.get_char((x, y).into());
            hash_set.insert(ch.get_font_page());
        }
    }
    let mut v: Vec<usize> = hash_set.into_iter().collect();
    v.sort_unstable();
    v
}

#[derive(Default)]
pub struct BufferFeatures {
    pub use_sixels: bool,
    pub has_links: bool,
    pub font_count: usize,
    pub use_extended_colors: bool,
    pub use_colors: bool,
    pub use_blink: bool,
    pub use_extended_attributes: bool,
}

/// Options for rendering a buffer to RGBA format
#[derive(Default)]
pub struct RenderOptions {
    /// The rectangle area to render from the buffer
    pub rect: crate::Selection,

    /// Whether blinking characters should be shown (true) or hidden (false)
    pub blink_on: bool,

    /// Optional selection to highlight with custom colors
    /// If Some, cells within this selection will be rendered with selection colors
    pub selection: Option<crate::Selection>,

    /// Custom foreground color for selected text (requires selection to be Some)
    /// If None and selection is Some, colors will be inverted
    pub selection_fg: Option<Color>,

    /// Custom background color for selected text (requires selection to be Some)  
    /// If None and selection is Some, colors will be inverted
    pub selection_bg: Option<Color>,
}

impl From<Rectangle> for RenderOptions {
    fn from(value: Rectangle) -> Self {
        Self {
            rect: value.into(),
            blink_on: true,
            ..Default::default()
        }
    }
}

impl Buffer {
    pub fn new(size: impl Into<Size>) -> Self {
        let mut font_table = HashMap::new();
        font_table.insert(0, BitFont::default());
        let size = size.into();
        Buffer {
            file_name: None,
            original_size: size,
            size,
            terminal_state: TerminalState::from(size),
            sauce_data: icy_sauce::MetaData::default(),

            buffer_type: BufferType::CP437,
            ice_mode: IceMode::Unlimited,
            palette_mode: PaletteMode::Fixed16,
            font_mode: FontMode::Sauce,

            palette: Palette::dos_default(),

            font_table,
            is_font_table_dirty: false,
            overlay_index: 0,
            overlay_layer: None,
            layers: vec![Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-background-name"), size)],
            sixel_threads: VecDeque::new(), // file_name_changed: Box::new(|| {}),
            use_letter_spacing: false,
            use_aspect_ratio: false,
            show_tags: true,
            tags: Vec::new(),
            ansi_music: Vec::new(),

            scrollback_lines: VecDeque::new(),
            max_scrollback_lines: 10000, // Reasonable default
        }
    }

    /// Returns the update sixel threads of this [`Buffer`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn update_sixel_threads(&mut self) -> EngineResult<bool> {
        let mut updated_sixel = false;
        while let Some(handle) = self.sixel_threads.front() {
            if !handle.is_finished() {
                return Ok(false);
            }
            let Some(handle) = self.sixel_threads.pop_front() else {
                continue;
            };
            let Ok(result) = handle.join() else {
                continue;
            };

            let sixel = result?;

            updated_sixel = true;

            let font_dims = self.get_font_dimensions();
            let screen_rect = sixel.get_screen_rect(font_dims);

            let vec = &mut self.layers[0].sixels;
            let mut sixel_count = vec.len();
            // remove old sixel that are shadowed by the new one
            let mut i = 0;
            while i < sixel_count {
                let old_rect = vec[i].get_screen_rect(font_dims);
                if screen_rect.contains_rect(&old_rect) {
                    vec.remove(i);
                    sixel_count -= 1;
                } else {
                    i += 1;
                }
            }
            vec.push(sixel);
        }
        Ok(updated_sixel)
    }

    pub fn clear_font_table(&mut self) {
        self.font_table.clear();
        self.is_font_table_dirty = true;
    }

    pub fn has_fonts(&self) -> bool {
        !self.font_table.is_empty()
    }

    pub fn has_font(&self, id: usize) -> bool {
        self.font_table.contains_key(&id)
    }

    pub fn is_font_table_updated(&self) -> bool {
        self.is_font_table_dirty
    }

    pub fn set_font_table_is_updated(&mut self) {
        self.is_font_table_dirty = false;
    }

    pub fn search_font_by_name(&self, name: impl Into<String>) -> Option<usize> {
        let name = name.into();
        for (i, font) in &self.font_table {
            if font.name == name {
                return Some(*i);
            }
        }
        None
    }

    pub fn font_iter(&self) -> impl Iterator<Item = (&usize, &BitFont)> {
        self.font_table.iter()
    }

    pub fn font_iter_mut(&mut self) -> impl Iterator<Item = (&usize, &mut BitFont)> {
        self.font_table.iter_mut()
    }

    pub fn get_font(&self, font_number: usize) -> Option<&BitFont> {
        self.font_table.get(&font_number)
    }

    pub fn set_font(&mut self, font_number: usize, font: BitFont) {
        self.font_table.insert(font_number, font);
        self.is_font_table_dirty = true;
    }

    pub fn remove_font(&mut self, font_number: usize) -> Option<BitFont> {
        self.font_table.remove(&font_number)
    }

    pub fn font_count(&self) -> usize {
        self.font_table.len()
    }

    pub fn get_font_table(&self) -> HashMap<usize, BitFont> {
        self.font_table.clone()
    }

    pub fn set_font_table(&mut self, font_table: HashMap<usize, BitFont>) {
        self.font_table = font_table;
    }

    pub fn append_font(&mut self, font: BitFont) -> usize {
        let mut i = 0;
        while self.font_table.contains_key(&i) {
            i += 1;
        }
        self.font_table.insert(i, font);
        i
    }

    pub fn get_real_buffer_width(&self) -> i32 {
        let mut w = 0;
        for layer in &self.layers {
            for line in &layer.lines {
                w = max(w, line.get_line_length());
            }
        }
        w
    }

    pub fn reset_terminal(&mut self) {
        if self.terminal_state.is_terminal_buffer {
            let fixed = self.terminal_state.fixed_size;
            self.terminal_state.reset_terminal(self.original_size);
            self.size = self.original_size;
            self.terminal_state.fixed_size = fixed;
        } else {
            self.terminal_state.reset_terminal(self.size);
        }
        self.terminal_state.cleared_screen = true;
    }

    /// Sets the buffer size of this [`Buffer`].
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn set_size(&mut self, size: impl Into<Size>) {
        let size = size.into();
        self.size = size;
    }

    pub fn set_default_size(&mut self, size: impl Into<Size>) {
        let size = size.into();
        self.original_size = size;
    }

    pub fn set_width(&mut self, width: i32) {
        self.size.width = width;
    }

    pub fn set_height(&mut self, height: i32) {
        self.size.height = height;
    }

    /// Returns the clear of this [`Buffer`].
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn stop_sixel_threads(&mut self) {
        self.sixel_threads.clear();
    }

    /// terminal buffers have a viewport on the bottom of the buffer
    /// this function gives back the first visible line.
    #[must_use]
    pub fn get_first_visible_line(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            max(0, self.size.height.saturating_sub(self.terminal_state.get_height()))
        } else {
            0
        }
    }

    pub fn get_last_visible_line(&self) -> i32 {
        self.get_first_visible_line() + self.get_height()
    }

    pub fn get_first_editable_line(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((start, _)) = self.terminal_state.get_margins_top_bottom() {
                return self.get_first_visible_line() + start;
            }
        }
        self.get_first_visible_line()
    }

    pub fn get_first_editable_column(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((start, _)) = self.terminal_state.get_margins_left_right() {
                return start;
            }
        }
        0
    }

    pub fn get_last_editable_column(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((_, end)) = self.terminal_state.get_margins_left_right() {
                return end;
            }
        }
        self.get_width().saturating_sub(1)
    }

    #[must_use]
    pub fn get_last_editable_line(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((_, end)) = self.terminal_state.get_margins_top_bottom() {
                self.get_first_visible_line() + end
            } else {
                (self.get_first_visible_line() + self.get_height()).saturating_sub(1)
            }
        } else {
            max(self.layers[0].lines.len() as i32, self.get_height().saturating_sub(1))
        }
    }

    #[must_use]
    pub fn create(size: impl Into<Size>) -> Self {
        let size = size.into();
        let mut res = Buffer::new(size);
        res.layers[0].lines.resize(size.height as usize, crate::Line::create(size.width));

        res
    }

    pub fn get_overlay_layer(&mut self, index: usize) -> &mut Layer {
        if self.overlay_layer.is_none() {
            self.overlay_index = index;
            let mut l = Layer::new("Overlay", self.get_size());
            l.properties.has_alpha_channel = true;
            self.overlay_layer = Some(l);
        }
        self.overlay_layer.as_mut().unwrap()
    }

    pub fn remove_overlay(&mut self) -> Option<Layer> {
        self.overlay_layer.take()
    }

    #[must_use]
    pub fn get_glyph(&self, ch: &AttributedChar) -> Option<&Glyph> {
        if let Some(ext) = &self.get_font(ch.get_font_page()) {
            return ext.get_glyph(ch.ch);
        }
        None
    }

    #[must_use]
    pub fn get_font_dimensions(&self) -> Size {
        if let Some(font) = self.get_font(0) { font.size } else { Size::new(8, 16) }
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn load_buffer(file_name: &Path, skip_errors: bool, ansi_music: Option<MusicOption>) -> EngineResult<Buffer> {
        let mut f = match File::open(file_name) {
            Ok(f) => f,
            Err(err) => {
                return Err(LoadingError::OpenFileError(format!("{err}")).into());
            }
        };
        let mut bytes = Vec::new();
        if let Err(err) = f.read_to_end(&mut bytes) {
            return Err(LoadingError::ReadFileError(format!("{err}")).into());
        }

        Buffer::from_bytes(file_name, skip_errors, &bytes, ansi_music, None)
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn to_bytes(&mut self, extension: &str, options: &SaveOptions) -> EngineResult<Vec<u8>> {
        let extension = extension.to_ascii_lowercase();
        for fmt in &*crate::FORMATS {
            if fmt.get_file_extension() == extension || fmt.get_alt_extensions().contains(&extension) {
                let tags_enabled = self.show_tags;
                self.show_tags = false;
                let res = if options.lossles_output {
                    fmt.to_bytes(self, options)
                } else {
                    let optimizer = crate::ColorOptimizer::new(self, options);
                    fmt.to_bytes(&mut optimizer.optimize(self), options)
                };
                self.show_tags = tags_enabled;
                return res;
            }
        }
        Err(anyhow::anyhow!("Unknown format"))
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn from_bytes(
        file_name: &Path,
        _skip_errors: bool,
        bytes: &[u8],
        ansi_music: Option<MusicOption>,
        default_terminal_width: Option<usize>,
    ) -> EngineResult<Buffer> {
        let ext = file_name.extension().unwrap_or_default().to_string_lossy();
        let mut len = bytes.len();
        let sauce_data = match SauceRecord::from_bytes(bytes) {
            Ok(Some(sauce)) => {
                len -= sauce.record_len() - 1;
                Some(sauce)
            }
            Ok(None) => None,
            Err(err) => {
                log::error!("Error reading sauce data: {}", err);
                None
            }
        };

        let ext = ext.to_ascii_lowercase();
        for fmt in &*FORMATS {
            if fmt.get_file_extension() == ext || fmt.get_alt_extensions().contains(&ext) {
                return fmt.load_buffer(file_name, &bytes[..len], Some(LoadData::new(sauce_data, ansi_music, default_terminal_width)));
            }
        }

        crate::Ansi::default().load_buffer(file_name, &bytes[..len], Some(LoadData::new(sauce_data, ansi_music, default_terminal_width)))
    }

    pub fn to_screenx(&self, x: i32) -> f64 {
        let font_dimensions = self.get_font_dimensions();
        x as f64 * font_dimensions.width as f64
    }

    pub fn to_screeny(&self, y: i32) -> f64 {
        let font_dimensions = self.get_font_dimensions();
        y as f64 * font_dimensions.height as f64
    }

    pub fn use_letter_spacing(&self) -> bool {
        self.use_letter_spacing
    }

    pub fn set_use_letter_spacing(&mut self, use_letter_spacing: bool) {
        self.use_letter_spacing = use_letter_spacing;
        self.is_font_table_dirty = true;
    }

    pub fn use_aspect_ratio(&self) -> bool {
        self.use_aspect_ratio
    }

    pub fn set_use_aspect_ratio(&mut self, use_aspect_ratio: bool) {
        self.use_aspect_ratio = use_aspect_ratio;
    }

    pub fn make_solid_color(&self, mut transparent_char: AttributedChar, underlying_char: AttributedChar) -> AttributedChar {
        let half_block = HalfBlock::from_char(underlying_char, Position::default());

        match transparent_char.ch {
            crate::paint::HALF_BLOCK_TOP => {
                if transparent_char.attribute.foreground_color == TextAttribute::TRANSPARENT_COLOR {
                    transparent_char.attribute.foreground_color = half_block.upper_block_color;
                }
                if transparent_char.attribute.background_color == TextAttribute::TRANSPARENT_COLOR {
                    transparent_char.attribute.background_color = half_block.lower_block_color;
                }
            }
            crate::paint::HALF_BLOCK_BOTTOM => {
                if transparent_char.attribute.background_color == TextAttribute::TRANSPARENT_COLOR {
                    transparent_char.attribute.background_color = half_block.upper_block_color;
                }
                if transparent_char.attribute.foreground_color == TextAttribute::TRANSPARENT_COLOR {
                    transparent_char.attribute.foreground_color = half_block.lower_block_color;
                }
            }
            _ => {
                if transparent_char.attribute.foreground_color == TextAttribute::TRANSPARENT_COLOR {
                    transparent_char.attribute.foreground_color = half_block.lower_block_color;
                }
                if transparent_char.attribute.background_color == TextAttribute::TRANSPARENT_COLOR {
                    transparent_char.attribute.background_color = half_block.lower_block_color;
                }
            }
        }
        transparent_char
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer::new((80, 25))
    }
}

impl TextPane for Buffer {
    fn get_width(&self) -> i32 {
        self.size.width
    }

    fn get_height(&self) -> i32 {
        self.size.height
    }

    fn get_line_count(&self) -> i32 {
        if let Some(len) = self.layers.iter().map(|l| l.lines.len()).max() {
            len as i32
        } else {
            self.size.height
        }
    }

    fn get_char(&self, pos: Position) -> AttributedChar {
        let pos = pos.into();

        if self.show_tags {
            for tag in &self.tags {
                if tag.is_enabled && tag.contains(pos) {
                    return tag.get_char_at(pos.x - tag.position.x);
                }
            }
        }
        let mut found_char = AttributedChar::invisible();
        for i in 0..self.layers.len() {
            let cur_layer = &self.layers[i];
            if cur_layer.properties.is_visible {
                let pos: Position = pos - cur_layer.get_offset();
                if pos.x >= 0 && pos.y >= 0 && pos.x < cur_layer.get_width() && pos.y < cur_layer.get_height() {
                    self.merge_layer_char(&mut found_char, cur_layer, pos);
                }
            }

            if self.overlay_index == i {
                if let Some(overlay) = &self.overlay_layer {
                    self.merge_layer_char(&mut found_char, overlay, pos);
                }
            }
        }

        found_char
    }

    fn get_line_length(&self, line: i32) -> i32 {
        let mut length = 0;
        let mut pos = Position::new(0, line);
        let mut last_char = AttributedChar::invisible();
        for x in 0..self.get_width() {
            pos.x = x;
            let ch = self.get_char(pos);
            if x > 0 && ch.is_transparent() {
                let bg = last_char.attribute.get_background();
                if bg != TextAttribute::TRANSPARENT_COLOR && bg > 0 {
                    length = x + 1;
                }
            } else if !ch.is_transparent() {
                length = x + 1;
            }
            last_char = ch;
        }

        // Check if any tags extend the line length
        for tag in &self.tags {
            if tag.is_enabled && tag.position.y == line {
                let tag_end = tag.position.x + tag.len() as i32;
                length = length.max(tag_end);
            }
        }

        length
    }

    fn get_size(&self) -> Size {
        self.size
    }

    fn get_rectangle(&self) -> Rectangle {
        Rectangle::from_min_size((0, 0), (self.get_width(), self.get_height()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{AttributedChar, Buffer, Layer, SaveOptions, Size, TextAttribute, TextPane};

    #[test]
    fn test_respect_sauce_width() {
        let mut buf = Buffer::default();
        buf.set_width(10);
        for x in 0..buf.get_width() {
            buf.layers[0].set_char((x, 0), AttributedChar::new('1', TextAttribute::default()));
            buf.layers[0].set_char((x, 1), AttributedChar::new('2', TextAttribute::default()));
            buf.layers[0].set_char((x, 2), AttributedChar::new('3', TextAttribute::default()));
        }

        let mut opt = SaveOptions::new();
        opt.save_sauce = true;
        let ansi_bytes = buf.to_bytes("ans", &opt).unwrap();

        let loaded_buf = Buffer::from_bytes(&std::path::PathBuf::from("test.ans"), false, &ansi_bytes, None, None).unwrap();
        assert_eq!(10, loaded_buf.get_width());
        assert_eq!(10, loaded_buf.layers[0].get_width());
    }

    #[test]
    fn test_layer_offset() {
        let mut buf: Buffer = Buffer::default();

        let mut new_layer = Layer::new("1", Size::new(10, 10));
        new_layer.properties.has_alpha_channel = true;
        new_layer.set_offset((2, 2));
        new_layer.set_char((5, 5), AttributedChar::new('a', TextAttribute::default()));
        buf.layers.push(new_layer);

        assert_eq!('a', buf.get_char((7, 7).into()).ch);
    }

    #[test]
    fn test_layer_negative_offset() {
        let mut buf: Buffer = Buffer::default();

        let mut new_layer = Layer::new("1", Size::new(10, 10));
        new_layer.properties.has_alpha_channel = true;
        new_layer.set_offset((-2, -2));
        new_layer.set_char((5, 5), AttributedChar::new('a', TextAttribute::default()));
        buf.layers.push(new_layer);

        let mut new_layer = Layer::new("2", Size::new(10, 10));
        new_layer.properties.has_alpha_channel = true;
        new_layer.set_offset((2, 2));
        new_layer.set_char((5, 5), AttributedChar::new('b', TextAttribute::default()));
        buf.layers.push(new_layer);

        assert_eq!('a', buf.get_char((3, 3).into()).ch);
        assert_eq!('b', buf.get_char((7, 7).into()).ch);
    }
}

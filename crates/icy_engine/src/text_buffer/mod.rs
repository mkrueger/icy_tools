use std::collections::{HashMap, HashSet};
use std::fmt::Alignment;
use std::{
    cmp::max,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use icy_parser_core::MusicOption;
use icy_sauce::prelude::*;

pub mod buffers_rendering;

pub mod line;
pub use line::*;

pub mod text_screen;
pub use text_screen::*;

pub mod layer;
pub use layer::*;

mod buffer_type;
pub use buffer_type::*;

use crate::{
    Color, EngineResult, FORMATS, HalfBlock, LoadData, LoadingError, OutputFormat, Position, Rectangle, TerminalState, TextAttribute, TextPane, attribute,
};

use super::{AttributedChar, BitFont, Palette, SaveOptions, Size};

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

pub struct TextBuffer {
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

    // pub redo_stack: Vec<Box<dyn UndoOperation>>,
    use_letter_spacing: bool,
    use_aspect_ratio: bool,

    pub show_tags: bool,
    pub tags: Vec<Tag>,
    //    pub ansi_music: Vec<AnsiMusic>,
    /// Scrollback buffer storing lines that scrolled off the top

    /// Maximum number of lines to keep in scrollback (0 = unlimited)
    pub max_scrollback_lines: usize,

    /// Dirty flag: set when buffer content changes, cleared when rendered
    buffer_dirty: std::sync::atomic::AtomicBool,

    /// Generation counter: incremented on every buffer change for cache invalidation
    buffer_version: std::sync::atomic::AtomicU64,
}

impl std::fmt::Debug for TextBuffer {
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

impl std::fmt::Display for TextBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut str = String::new();

        for y in 0..self.get_height() {
            str.extend(format!("{y:3}: ").chars());
            for x in 0..self.get_width() {
                let ch = self.get_char((x, y).into());
                str.push(self.buffer_type.convert_to_unicode(ch.ch));
            }
            str.push('\n');
        }
        write!(f, "{str}")
    }
}

impl Clone for TextBuffer {
    fn clone(&self) -> Self {
        Self {
            original_size: self.original_size,
            size: self.size,
            file_name: self.file_name.clone(),
            terminal_state: self.terminal_state.clone(),
            buffer_type: self.buffer_type,
            ice_mode: self.ice_mode,
            palette_mode: self.palette_mode,
            font_mode: self.font_mode,
            sauce_data: self.sauce_data.clone(),
            palette: self.palette.clone(),
            overlay_index: self.overlay_index,
            overlay_layer: self.overlay_layer.clone(),
            font_table: self.font_table.clone(),
            is_font_table_dirty: self.is_font_table_dirty,
            layers: self.layers.clone(),
            use_letter_spacing: self.use_letter_spacing,
            use_aspect_ratio: self.use_aspect_ratio,
            show_tags: self.show_tags,
            tags: self.tags.clone(),
            max_scrollback_lines: self.max_scrollback_lines,
            buffer_dirty: std::sync::atomic::AtomicBool::new(self.buffer_dirty.load(std::sync::atomic::Ordering::Relaxed)),
            buffer_version: std::sync::atomic::AtomicU64::new(self.buffer_version.load(std::sync::atomic::Ordering::Relaxed)),
        }
    }
}

impl TextBuffer {
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
                let size: Size = Size::new(columns.clamp(1, 1000) as i32, lines as i32);
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
    pub fn flat_clone(&self, deep_layers: bool) -> TextBuffer {
        let mut frame = TextBuffer::new(self.get_size());
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
}

pub fn analyze_font_usage(buf: &TextBuffer) -> Vec<usize> {
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

    pub override_scan_lines: Option<bool>,
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

impl TextBuffer {
    pub fn new(size: impl Into<Size>) -> Self {
        let mut font_table = HashMap::new();
        font_table.insert(0, BitFont::default());
        let size = size.into();
        TextBuffer {
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
            layers: vec![Layer::new("Background", size)],
            use_letter_spacing: false,
            use_aspect_ratio: false,
            show_tags: true,
            tags: Vec::new(),
            //            ansi_music: Vec::new(),
            max_scrollback_lines: 10000, // Reasonable default

            buffer_dirty: std::sync::atomic::AtomicBool::new(true),
            buffer_version: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Mark the buffer as dirty (content changed). This increments the version counter.
    /// Should be called by any method that modifies buffer content.
    pub fn mark_dirty(&self) {
        self.buffer_dirty.store(true, std::sync::atomic::Ordering::Release);
        self.buffer_version.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Check if the buffer is dirty (needs re-rendering)
    pub fn is_dirty(&self) -> bool {
        self.buffer_dirty.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Clear the dirty flag (called after rendering)
    pub fn clear_dirty(&self) {
        self.buffer_dirty.store(false, std::sync::atomic::Ordering::Release);
    }

    /// Get the current buffer version (increments on each modification)
    /// Used for cache invalidation
    pub fn get_version(&self) -> u64 {
        self.buffer_version.load(std::sync::atomic::Ordering::Relaxed)
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
            if font.name() == name {
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
            self.terminal_state.reset_terminal(self.original_size);
            self.size = self.original_size;
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
        let mut res = TextBuffer::new(size);
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
    pub fn get_glyph(&self, ch: &AttributedChar) -> Option<libyaff::GlyphDefinition> {
        if let Some(ext) = &self.get_font(ch.get_font_page()) {
            return ext.get_glyph(ch.ch);
        }
        None
    }

    #[must_use]
    pub fn get_font_dimensions(&self) -> Size {
        if let Some(font) = self.get_font(0) { font.size() } else { Size::new(8, 16) }
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
    pub fn load_buffer(file_name: &Path, skip_errors: bool, ansi_music: Option<MusicOption>) -> EngineResult<TextBuffer> {
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

        TextBuffer::from_bytes(file_name, skip_errors, &bytes, ansi_music, None)
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
    ) -> EngineResult<TextBuffer> {
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

impl Default for TextBuffer {
    fn default() -> Self {
        TextBuffer::new((80, 25))
    }
}

impl TextPane for TextBuffer {
    fn get_width(&self) -> i32 {
        self.size.width
    }

    fn get_height(&self) -> i32 {
        self.size.height
    }

    fn get_line_count(&self) -> i32 {
        if let Some(len) = self.layers.iter().map(|l| l.get_line_count()).max() {
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

use std::cmp::max;
use std::collections::HashMap;
use std::fmt::Alignment;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

pub mod buffers_rendering;

pub mod line;
pub use line::*;

pub mod text_screen;
pub use text_screen::*;

pub mod layer;
pub use layer::*;

mod buffer_type;
pub use buffer_type::*;

use crate::{attribute, Color, HalfBlock, Position, Rectangle, TerminalState, TextAttribute, TextPane};

use super::{AttributedChar, BitFont, Palette, Size};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum IceMode {
    Blink = 0,
    Ice = 1,
    Unlimited = 2,
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum TagPlacement {
    InText,
    WithGotoXY,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum TagRole {
    Displaycode,
    Hyperlink,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tag {
    pub is_enabled: bool,
    pub preview: String,
    pub replacement_value: String,

    pub position: Position,
    pub length: usize,
    #[serde(with = "alignment_serde")]
    pub alignment: Alignment,
    pub tag_placement: TagPlacement,
    pub tag_role: TagRole,

    pub attribute: TextAttribute,
}

/// Custom serde implementation for `std::fmt::Alignment`
mod alignment_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::fmt::Alignment;

    pub fn serialize<S>(alignment: &Alignment, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match alignment {
            Alignment::Left => "left",
            Alignment::Right => "right",
            Alignment::Center => "center",
        };
        s.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Alignment, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "left" => Ok(Alignment::Left),
            "right" => Ok(Alignment::Right),
            "center" => Ok(Alignment::Center),
            _ => Ok(Alignment::Left),
        }
    }
}

impl Tag {
    pub fn contains(&self, cur: Position) -> bool {
        self.position.y == cur.y && self.position.x <= cur.x && cur.x < self.position.x + self.len() as i32
    }

    pub fn len(&self) -> usize {
        // Always use the preview length for display purposes
        // The length field is reserved for future use
        self.preview.chars().count()
    }

    fn get_char_at(&self, x: i32) -> AttributedChar {
        let ch = match self.alignment {
            Alignment::Left => self.preview.chars().nth(x as usize).unwrap_or(' '),
            Alignment::Right => self.preview.chars().nth(self.len() - x as usize - 1).unwrap_or(' '),
            Alignment::Center => {
                let half = self.len() / 2;
                if (x as usize) < half {
                    self.preview.chars().nth(x as usize).unwrap_or(' ')
                } else {
                    self.preview.chars().nth(self.len() - x as usize - 1).unwrap_or(' ')
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

    pub terminal_state: TerminalState,

    pub buffer_type: BufferType,
    pub ice_mode: IceMode,
    pub font_mode: FontMode,

    pub palette: Palette,

    font_table: HashMap<u8, BitFont>,
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

    /// Font cell size for this document (determines rendering and layout)
    /// This is the expected font size - fonts with different sizes will be clipped/padded
    font_cell_size: Size,

    /// Dirty flag: set when buffer content changes, cleared when rendered
    buffer_dirty: AtomicBool,

    /// Generation counter: incremented on every buffer change for cache invalidation
    buffer_version: AtomicU64,

    /// Dirty line range: tracks which lines have been modified since last render.
    /// -1 means no dirty lines. These are updated atomically for thread-safety.
    dirty_line_start: AtomicI32,
    dirty_line_end: AtomicI32,

    /// Overlay dirty flag: set when only shader overlays need updating (selection, markers).
    /// This allows updating selection display without invalidating the tile cache.
    overlay_dirty: AtomicBool,
}

impl std::fmt::Debug for TextBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("width", &self.width())
            .field("height", &self.height())
            .field("custom_palette", &self.palette)
            .field("layers", &self.layers)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for TextBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut str = String::new();

        for y in 0..self.height() {
            str.extend(format!("{y:3}: ").chars());
            for x in 0..self.width() {
                let ch = self.char_at((x, y).into());
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
            terminal_state: self.terminal_state.clone(),
            buffer_type: self.buffer_type,
            ice_mode: self.ice_mode,
            font_mode: self.font_mode,
            palette: self.palette.clone(),
            font_table: self.font_table.clone(),
            is_font_table_dirty: self.is_font_table_dirty,
            layers: self.layers.clone(),
            use_letter_spacing: self.use_letter_spacing,
            use_aspect_ratio: self.use_aspect_ratio,
            show_tags: self.show_tags,
            tags: self.tags.clone(),
            max_scrollback_lines: self.max_scrollback_lines,
            font_cell_size: self.font_cell_size,
            buffer_dirty: AtomicBool::new(self.buffer_dirty.load(Ordering::Relaxed)),
            buffer_version: AtomicU64::new(self.buffer_version.load(Ordering::Relaxed)),
            dirty_line_start: AtomicI32::new(self.dirty_line_start.load(Ordering::Relaxed)),
            dirty_line_end: AtomicI32::new(self.dirty_line_end.load(Ordering::Relaxed)),
            overlay_dirty: AtomicBool::new(self.overlay_dirty.load(Ordering::Relaxed)),
        }
    }
}

impl TextBuffer {
    /// Check if a line contains only transparent/empty characters
    pub fn is_line_empty(&self, line: i32) -> bool {
        for i in 0..self.width() {
            if !self.char_at((i, line).into()).is_transparent() {
                return false;
            }
        }
        true
    }

    pub fn scan_buffer_features(&self) -> BufferFeatures {
        let mut result = BufferFeatures::default();
        for layer in &self.layers {
            if !layer.sixels.is_empty() {
                result.use_sixels = true;
            }
            if !layer.hyperlinks.is_empty() {
                result.has_links = true;
            }
            for y in 0..layer.height() {
                for x in 0..layer.width() {
                    let ch = layer.char_at((x, y).into());

                    if ch.attribute.foreground() != 7 || ch.attribute.background() != 0 {
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

    /// Analyze what capabilities this buffer requires from a file format.
    pub fn analyze_capability_requirements(&self) -> crate::formats::BufferCapabilityRequirements {
        use crate::formats::FormatCapabilities as C;

        let features = self.scan_buffer_features();
        let mut required: C = C::empty();

        // Check if palette differs from default DOS palette
        let has_custom_palette = !self.palette.is_default();
        if has_custom_palette {
            required |= C::CUSTOM_PALETTE;
        }

        // Check for truecolor usage (RGB colors, not palette indices)
        let uses_truecolor = self.scan_for_truecolor();
        if uses_truecolor {
            required |= C::TRUECOLOR;
        }

        // Check for ice colors (IceMode::Ice or IceMode::Unlimited with high bg colors)
        let uses_ice_colors = matches!(self.ice_mode, IceMode::Ice | IceMode::Unlimited);
        if uses_ice_colors {
            required |= C::ICE_COLORS;
        }

        // Check for custom font (non-default font in slot 0)
        let has_custom_font = self.font_table.get(&0).is_some_and(|f| !f.is_default());
        if has_custom_font {
            required |= C::CUSTOM_FONT;
        }

        // Check for multiple fonts
        if features.font_count > 1 {
            required |= C::UNLIMITED_FONTS;
        }

        // Check for sixels
        if features.use_sixels {
            required |= C::SIXEL;
        }

        // Check for extended attributes
        if features.use_extended_attributes {
            required |= C::XBIN_EXTENDED;
        }

        // Check for control characters (0x00-0x1F)
        let has_control_chars = self.scan_for_control_chars();
        if has_control_chars {
            required |= C::CONTROL_CHARS;
        }

        crate::formats::BufferCapabilityRequirements {
            required,
            width: self.width(),
            height: self.height(),
            font_count: features.font_count,
            has_custom_palette,
            uses_truecolor,
            uses_ice_colors,
            has_sixels: features.use_sixels,
            has_custom_font,
            uses_extended_attributes: features.use_extended_attributes,
            has_control_chars,
        }
    }

    /// Scan buffer for any truecolor (RGB) usage.
    fn scan_for_truecolor(&self) -> bool {
        for layer in &self.layers {
            for y in 0..layer.height() {
                for x in 0..layer.width() {
                    let ch = layer.char_at((x, y).into());
                    let attr = ch.attribute;
                    if attr.is_foreground_rgb() || attr.is_background_rgb() {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Scan buffer for control characters (0x00-0x1F).
    fn scan_for_control_chars(&self) -> bool {
        for layer in &self.layers {
            for y in 0..layer.height() {
                for x in 0..layer.width() {
                    let ch = layer.char_at((x, y).into());
                    if !ch.is_transparent() {
                        let code = ch.ch as u32;
                        if code < 32 {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn merge_layer_char(&self, found_char: &mut AttributedChar, cur_layer: &Layer, pos: Position) {
        let cur_char = cur_layer.char_at(pos);
        match cur_layer.properties.mode {
            crate::Mode::Normal => {
                let underlying_char = *found_char;
                if cur_char.is_visible() {
                    *found_char = cur_char;
                }

                if found_char.attribute.is_foreground_transparent() || found_char.attribute.is_background_transparent() {
                    *found_char = self.make_solid_color(*found_char, underlying_char);
                }

                if !cur_layer.properties.has_alpha_channel {
                    found_char.attribute.attr &= !attribute::INVISIBLE;
                    if found_char.attribute.is_background_transparent() {
                        found_char.attribute.set_background_color(crate::AttributeColor::Palette(0));
                    }
                    if found_char.attribute.is_foreground_transparent() {
                        found_char.attribute.set_foreground_color(crate::AttributeColor::Palette(0));
                    }
                }
            }
            crate::Mode::Chars => {
                if !cur_char.is_transparent() {
                    found_char.ch = cur_char.ch;
                    found_char.set_font_page(cur_char.font_page());
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

pub fn analyze_font_usage(buf: &TextBuffer) -> Vec<u8> {
    // Fast path for the common case (typically <= 2 font pages) without hashing.
    // Fallback to an ordered set if the number of distinct pages grows.
    let mut small: Vec<u8> = Vec::new();
    let mut set: Option<std::collections::BTreeSet<u8>> = None;

    for y in 0..buf.height() {
        for x in 0..buf.width() {
            let page = buf.char_at((x, y).into()).font_page();

            if let Some(set) = set.as_mut() {
                set.insert(page);
                continue;
            }

            if small.contains(&page) {
                continue;
            }

            small.push(page);
            // Switch to a BTreeSet if we see many distinct pages.
            if small.len() >= 16 {
                let mut tree = std::collections::BTreeSet::new();
                for p in small.drain(..) {
                    tree.insert(p);
                }
                set = Some(tree);
            }
        }
    }

    if let Some(set) = set {
        set.into_iter().collect()
    } else {
        small.sort_unstable();
        small
    }
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

    /// Custom background color for selected text (requires selection to be Some)\
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
            original_size: size,
            size,
            terminal_state: TerminalState::from(size),

            buffer_type: BufferType::CP437,
            ice_mode: IceMode::Unlimited,
            font_mode: FontMode::Sauce,

            palette: Palette::dos_default(),

            font_table,
            is_font_table_dirty: false,
            layers: vec![Layer::new("Background", size)],
            use_letter_spacing: false,
            use_aspect_ratio: false,
            show_tags: true,
            tags: Vec::new(),
            //            ansi_music: Vec::new(),
            max_scrollback_lines: 10000,      // Reasonable default
            font_cell_size: Size::new(8, 16), // Default VGA 8x16 font

            buffer_dirty: AtomicBool::new(true),
            buffer_version: AtomicU64::new(0),
            dirty_line_start: AtomicI32::new(-1),
            dirty_line_end: AtomicI32::new(-1),
            overlay_dirty: AtomicBool::new(false),
        }
    }

    /// Mark the buffer as dirty (content changed). This increments the version counter.
    /// Should be called by any method that modifies buffer content.
    /// For more efficient cache invalidation, use `mark_line_dirty()` when possible.
    pub fn mark_dirty(&self) {
        self.buffer_dirty.store(true, Ordering::Release);
        self.buffer_version.fetch_add(1, Ordering::Relaxed);
        // Mark all lines as dirty (full invalidation)
        self.dirty_line_start.store(0, Ordering::Relaxed);
        self.dirty_line_end.store(self.height(), Ordering::Relaxed);
    }

    /// Mark a specific line as dirty. This is more efficient than `mark_dirty()`
    /// as it allows partial cache invalidation.
    pub fn mark_line_dirty(&self, line: i32) {
        self.buffer_dirty.store(true, Ordering::Release);
        self.buffer_version.fetch_add(1, Ordering::Relaxed);

        // Atomically extend the dirty range to include this line
        loop {
            let current_start = self.dirty_line_start.load(Ordering::Relaxed);
            let new_start = if current_start < 0 { line } else { current_start.min(line) };
            if self
                .dirty_line_start
                .compare_exchange_weak(current_start, new_start, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        loop {
            let current_end = self.dirty_line_end.load(Ordering::Relaxed);
            let new_end = if current_end < 0 { line + 1 } else { current_end.max(line + 1) };
            if self
                .dirty_line_end
                .compare_exchange_weak(current_end, new_end, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Mark a range of lines as dirty.
    pub fn mark_lines_dirty(&self, start_line: i32, end_line: i32) {
        self.buffer_dirty.store(true, Ordering::Release);
        self.buffer_version.fetch_add(1, Ordering::Relaxed);

        // Atomically extend the dirty range
        loop {
            let current_start = self.dirty_line_start.load(Ordering::Relaxed);
            let new_start = if current_start < 0 { start_line } else { current_start.min(start_line) };
            if self
                .dirty_line_start
                .compare_exchange_weak(current_start, new_start, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        loop {
            let current_end = self.dirty_line_end.load(Ordering::Relaxed);
            let new_end = if current_end < 0 { end_line } else { current_end.max(end_line) };
            if self
                .dirty_line_end
                .compare_exchange_weak(current_end, new_end, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Check if the buffer is dirty (needs re-rendering)
    pub fn is_dirty(&self) -> bool {
        self.buffer_dirty.load(Ordering::Acquire)
    }

    /// Clear the dirty flag (called after rendering)
    pub fn clear_dirty(&self) {
        self.buffer_dirty.store(false, Ordering::Release);
    }

    /// Get the current buffer version (increments on each modification)
    /// Used for cache invalidation
    pub fn version(&self) -> u64 {
        self.buffer_version.load(Ordering::Relaxed)
    }

    /// Get the dirty line range and clear it atomically.
    /// Returns (`start_line`, `end_line`) or None if no lines are dirty.
    /// `end_line` is exclusive.
    pub fn get_and_clear_dirty_lines(&self) -> Option<(i32, i32)> {
        let start = self.dirty_line_start.swap(-1, Ordering::Relaxed);
        let end = self.dirty_line_end.swap(-1, Ordering::Relaxed);

        if start < 0 || end < 0 {
            None
        } else {
            Some((start, end))
        }
    }

    /// Get the dirty line range without clearing it.
    /// Returns (`start_line`, `end_line`) or None if no lines are dirty.
    /// `end_line` is exclusive.
    pub fn get_dirty_lines(&self) -> Option<(i32, i32)> {
        let start = self.dirty_line_start.load(Ordering::Relaxed);
        let end = self.dirty_line_end.load(Ordering::Relaxed);
        if start < 0 || end < 0 {
            None
        } else {
            Some((start, end))
        }
    }

    /// Mark the overlay as dirty (selection, markers changed).
    /// This triggers a shader-only update without invalidating the tile cache.
    pub fn mark_overlay_dirty(&self) {
        self.overlay_dirty.store(true, Ordering::Release);
    }

    /// Check if the overlay is dirty (needs shader update)
    pub fn is_overlay_dirty(&self) -> bool {
        self.overlay_dirty.load(Ordering::Acquire)
    }

    /// Clear the overlay dirty flag
    pub fn clear_overlay_dirty(&self) {
        self.overlay_dirty.store(false, Ordering::Release);
    }

    pub fn clear_font_table(&mut self) {
        self.font_table.clear();
        self.is_font_table_dirty = true;
    }

    pub fn has_fonts(&self) -> bool {
        !self.font_table.is_empty()
    }

    pub fn has_font(&self, id: u8) -> bool {
        self.font_table.contains_key(&id)
    }

    pub fn is_font_table_updated(&self) -> bool {
        self.is_font_table_dirty
    }

    pub fn set_font_table_is_updated(&mut self) {
        self.is_font_table_dirty = false;
    }

    pub fn search_font_by_name(&self, name: impl Into<String>) -> Option<u8> {
        let name = name.into();
        for (i, font) in &self.font_table {
            if font.name() == name {
                return Some(*i);
            }
        }
        None
    }

    pub fn font_iter(&self) -> impl Iterator<Item = (&u8, &BitFont)> {
        self.font_table.iter()
    }

    pub fn font_iter_mut(&mut self) -> impl Iterator<Item = (&u8, &mut BitFont)> {
        self.font_table.iter_mut()
    }

    pub fn font(&self, font_number: u8) -> Option<&BitFont> {
        if let Some(font) = self.font_table.get(&font_number) {
            Some(font)
        } else if let Some(font) = BitFont::from_ansi_font_page(font_number, self.font_cell_size.height as u8) {
            Some(font)
        } else {
            None
        }
    }

    /// Get the appropriate font for rendering.
    /// The 9th pixel for letter spacing mode is generated at render time.
    /// For use during rendering (immutable access).
    pub fn font_for_render(&self, font_number: u8) -> Option<&BitFont> {
        self.font_table.get(&font_number)
    }

    /// Get the appropriate font for rendering.
    /// The 9th pixel for letter spacing mode is generated at render time.
    pub fn render_font(&mut self, font_number: u8) -> Option<&BitFont> {
        self.font_table.get(&font_number)
    }

    pub fn set_font(&mut self, font_number: u8, font: BitFont) {
        self.font_table.insert(font_number, font);
        self.is_font_table_dirty = true;
    }

    pub fn remove_font(&mut self, font_number: u8) -> Option<BitFont> {
        self.font_table.remove(&font_number)
    }

    pub fn font_count(&self) -> usize {
        self.font_table.len()
    }

    pub fn font_table(&self) -> HashMap<u8, BitFont> {
        self.font_table.clone()
    }

    pub fn set_font_table(&mut self, font_table: HashMap<u8, BitFont>) {
        self.font_table = font_table;
    }

    pub fn append_font(&mut self, font: BitFont) -> u8 {
        let mut i = 0;
        while self.font_table.contains_key(&i) {
            i += 1;
        }
        self.font_table.insert(i, font);
        i
    }

    pub fn real_buffer_width(&self) -> i32 {
        let mut w = 0;
        for layer in &self.layers {
            for line in &layer.lines {
                w = max(w, line.line_length());
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
    pub fn first_visible_line(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            max(0, self.size.height.saturating_sub(self.terminal_state.height()))
        } else {
            0
        }
    }

    pub fn last_visible_line(&self) -> i32 {
        self.first_visible_line() + self.height()
    }

    pub fn first_editable_line(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((start, _)) = self.terminal_state.margins_top_bottom() {
                return self.first_visible_line() + start;
            }
        }
        self.first_visible_line()
    }

    pub fn first_editable_column(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((start, _)) = self.terminal_state.margins_left_right() {
                return start;
            }
        }
        0
    }

    pub fn last_editable_column(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((_, end)) = self.terminal_state.margins_left_right() {
                return end;
            }
        }
        self.width().saturating_sub(1)
    }

    #[must_use]
    pub fn last_editable_line(&self) -> i32 {
        if self.terminal_state.is_terminal_buffer {
            if let Some((_, end)) = self.terminal_state.margins_top_bottom() {
                self.first_visible_line() + end
            } else {
                (self.first_visible_line() + self.height()).saturating_sub(1)
            }
        } else {
            max(self.layers[0].lines.len() as i32, self.height().saturating_sub(1))
        }
    }

    #[must_use]
    pub fn create(size: impl Into<Size>) -> Self {
        let size = size.into();
        let mut res = TextBuffer::new(size);
        res.layers[0].lines.resize(size.height as usize, crate::Line::create(size.width));

        res
    }

    /// Get a reference to the glyph for the given attributed character.
    /// Returns None if the font for the character's font page doesn't exist.
    #[must_use]
    pub fn glyph(&self, ch: &AttributedChar) -> Option<&crate::fonts::CompactGlyph> {
        self.font(ch.font_page()).map(|font| font.glyph(ch.ch))
    }

    #[must_use]
    pub fn font_dimensions(&self) -> Size {
        self.font_cell_size
    }

    /// Get font dimensions with aspect ratio correction applied
    /// Returns the effective display size of a font cell
    #[must_use]
    pub fn font_dimensions_with_aspect_ratio(&self) -> Size {
        if !self.use_aspect_ratio {
            return self.font_cell_size;
        }

        let stretch_factor = self.get_aspect_ratio_stretch_factor();
        if stretch_factor <= 0.0 {
            return self.font_cell_size;
        }

        Size::new(self.font_cell_size.width, (self.font_cell_size.height as f32 * stretch_factor).round() as i32)
    }

    /// Get the aspect ratio stretch factor for this buffer
    /// Returns 0.0 if no stretching should be applied
    #[must_use]
    pub fn get_aspect_ratio_stretch_factor(&self) -> f32 {
        match self.buffer_type {
            BufferType::Petscii => 1.2,
            BufferType::Atascii => 1.25,
            _ => {
                let mut res = if self.use_letter_spacing { 1.35 } else { 1.2 };
                if let Some(font) = self.font(0) {
                    if font.name().starts_with("IBM EGA") {
                        res = 1.3714;
                    }
                    if font.name().starts_with("IBM VGA25G") {
                        res = 0.0;
                    }
                    if font.name().starts_with("Amiga") {
                        res = 1.4;
                    }
                    if font.name().starts_with("C64") {
                        res = 1.2;
                    }
                    if font.name().starts_with("Atari ATASCII") {
                        res = 1.25;
                    }
                }
                res
            }
        }
    }

    /// Set the font cell size for this document
    pub fn set_font_dimensions(&mut self, size: Size) {
        self.font_cell_size = size;
    }

    pub fn to_screenx(&self, x: i32) -> f64 {
        let font_dimensions = self.font_dimensions();
        x as f64 * font_dimensions.width as f64
    }

    pub fn to_screeny(&self, y: i32) -> f64 {
        let font_dimensions = self.font_dimensions();
        y as f64 * font_dimensions.height as f64
    }

    pub fn use_letter_spacing(&self) -> bool {
        self.use_letter_spacing
    }

    pub fn set_use_letter_spacing(&mut self, use_letter_spacing: bool) {
        if self.use_letter_spacing != use_letter_spacing {
            self.use_letter_spacing = use_letter_spacing;
            self.is_font_table_dirty = true;
            // 9th pixel is now generated at render time, no cache needed
        }
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
                if transparent_char.attribute.is_foreground_transparent() {
                    transparent_char.attribute.set_foreground_color(half_block.upper_block_color);
                }
                if transparent_char.attribute.is_background_transparent() {
                    transparent_char.attribute.set_background_color(half_block.lower_block_color);
                }
            }
            crate::paint::HALF_BLOCK_BOTTOM => {
                if transparent_char.attribute.is_background_transparent() {
                    transparent_char.attribute.set_background_color(half_block.upper_block_color);
                }
                if transparent_char.attribute.is_foreground_transparent() {
                    transparent_char.attribute.set_foreground_color(half_block.lower_block_color);
                }
            }
            _ => {
                if transparent_char.attribute.is_foreground_transparent() {
                    transparent_char.attribute.set_foreground_color(half_block.lower_block_color);
                }
                if transparent_char.attribute.is_background_transparent() {
                    transparent_char.attribute.set_background_color(half_block.lower_block_color);
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
    fn width(&self) -> i32 {
        self.size.width
    }

    fn height(&self) -> i32 {
        self.size.height
    }

    fn line_count(&self) -> i32 {
        if let Some(len) = self.layers.iter().map(super::TextPane::line_count).max() {
            len
        } else {
            self.size.height
        }
    }

    fn char_at(&self, pos: Position) -> AttributedChar {
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
                let pos: Position = pos - cur_layer.offset();
                if pos.x >= 0 && pos.y >= 0 && pos.x < cur_layer.width() && pos.y < cur_layer.height() {
                    self.merge_layer_char(&mut found_char, cur_layer, pos);
                }
            }
        }

        found_char
    }

    fn line_length(&self, line: i32) -> i32 {
        let mut length = 0;
        let mut pos = Position::new(0, line);
        let mut last_char = AttributedChar::invisible();
        for x in 0..self.width() {
            pos.x = x;
            let ch = self.char_at(pos);
            if x > 0 && ch.is_transparent() {
                // Check if the previous char has a visible background
                if !last_char.attribute.is_background_transparent() && last_char.attribute.background() > 0 {
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

    fn size(&self) -> Size {
        self.size
    }

    fn rectangle(&self) -> Rectangle {
        Rectangle::from_min_size((0, 0), (self.width(), self.height()))
    }
}

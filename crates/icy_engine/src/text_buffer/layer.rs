use serde::{Deserialize, Serialize};

use crate::{BufferType, Color, Line, Position, Rectangle, Sixel, Size, TextPane};

use super::AttributedChar;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Normal,
    Chars,
    Attributes,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    #[default]
    Normal,
    Image,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerProperties {
    pub title: String,
    pub color: Option<Color>,
    pub is_visible: bool,
    pub is_locked: bool,
    pub is_position_locked: bool,
    pub is_alpha_channel_locked: bool,
    pub has_alpha_channel: bool,
    pub mode: Mode,
    pub offset: Position,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub role: Role,
    pub properties: LayerProperties,

    preview_offset: Option<Position>,
    size: Size,
    pub lines: Vec<Line>,

    #[serde(skip)]
    pub sixels: Vec<Sixel>,
    pub hyperlinks: Vec<HyperLink>,
}

impl std::fmt::Display for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut str = String::new();

        for y in 0..self.line_count() {
            str.extend(format!("{y:3}: ").chars());
            for x in 0..self.width() {
                let ch: AttributedChar = self.char_at((x, y).into());
                str.push(BufferType::CP437.convert_to_unicode(ch.ch));
            }
            str.push('\n');
        }
        write!(f, "{str}")
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct HyperLink {
    pub url: Option<String>,
    pub position: Position,
    pub length: i32,
}

impl HyperLink {
    pub fn url(&self, buf: &dyn TextPane) -> String {
        if let Some(ref url) = self.url {
            url.clone()
        } else {
            buf.string_at(self.position, self.length as usize)
        }
    }
}

impl TextPane for Layer {
    fn char_at(&self, pos: Position) -> AttributedChar {
        if pos.x < 0 || pos.y < 0 || pos.x >= self.width() || pos.y >= self.height() {
            return AttributedChar::invisible();
        }
        let y = pos.y;
        if y < self.lines.len() as i32 {
            let cur_line = &self.lines[y as usize];
            if pos.x < cur_line.chars.len() as i32 {
                return cur_line.chars[pos.x as usize];
            }
        }
        AttributedChar::invisible()
    }

    fn line_count(&self) -> i32 {
        // Find the last line with content (length > 0)
        for i in (0..self.lines.len()).rev() {
            if !self.lines[i].is_effective_empty() {
                return (i + 1) as i32;
            }
        }
        0
    }

    fn line_length(&self, line: i32) -> i32 {
        self.lines[line as usize].line_length()
    }

    fn width(&self) -> i32 {
        self.size.width
    }

    fn height(&self) -> i32 {
        self.size.height
    }

    fn size(&self) -> Size {
        self.size
    }

    fn rectangle(&self) -> Rectangle {
        Rectangle::from_min_size(self.offset(), (self.width(), self.height()))
    }
}

impl Layer {
    pub fn new(title: impl Into<String>, size: impl Into<Size>) -> Self {
        let size = size.into();

        let mut lines = Vec::new();
        lines.resize(size.height as usize, Line::create(size.width));

        Layer {
            properties: LayerProperties {
                title: title.into(),
                is_visible: true,
                ..Default::default()
            },
            size,
            lines,
            ..Default::default()
        }
    }

    pub fn offset(&self) -> Position {
        if let Some(offset) = self.preview_offset {
            return offset;
        }
        self.properties.offset
    }

    pub fn base_offset(&self) -> Position {
        self.properties.offset
    }

    pub fn set_offset(&mut self, pos: impl Into<Position>) {
        if self.properties.is_position_locked {
            return;
        }
        self.preview_offset = None;
        self.properties.offset = pos.into();
    }

    pub fn is_visible(&self) -> bool {
        self.properties.is_visible
    }

    pub fn set_is_visible(&mut self, is_visible: bool) {
        self.properties.is_visible = is_visible;
    }

    pub fn title(&self) -> &str {
        &self.properties.title
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.properties.title = title.into();
    }

    pub fn join(&mut self, layer: &Layer) {
        for y in 0..layer.lines.len() {
            let line = &layer.lines[y];
            for x in 0..line.chars.len() {
                let ch = line.chars[x];
                if ch.is_visible() {
                    self.set_char((x as i32, y as i32), ch);
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.hyperlinks.clear();
        self.sixels.clear();
    }

    pub fn set_char(&mut self, pos: impl Into<Position>, attributed_char: AttributedChar) {
        let pos = pos.into();
        if pos.x < 0 || pos.y < 0 || pos.x >= self.width() || pos.y >= self.height() {
            return;
        }
        if self.properties.is_locked || !self.properties.is_visible {
            return;
        }
        if pos.y >= self.lines.len() as i32 {
            self.lines.resize(pos.y as usize + 1, Line::create(self.size.width));
        }

        if self.properties.has_alpha_channel && self.properties.is_alpha_channel_locked {
            let old_char = self.char_at(pos);
            if !old_char.is_visible() {
                return;
            }
        }

        let cur_line = &mut self.lines[pos.y as usize];
        cur_line.set_char(pos.x, attributed_char);
        let font_dims = Size::new(8, 16);
        self.sixels.retain(|x| !x.as_rectangle(font_dims).is_inside(pos) || pos.y != x.position.y);
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn remove_line(&mut self, index: i32) {
        if self.properties.is_locked || !self.properties.is_visible {
            return;
        }
        assert!(!(index < 0 || index >= self.lines.len() as i32), "line out of range");
        self.lines.remove(index as usize);
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn insert_line(&mut self, index: i32, line: Line) {
        if self.properties.is_locked || !self.properties.is_visible {
            return;
        }
        assert!(index >= 0, "line out of range");
        if index > self.lines.len() as i32 {
            self.lines.resize(index as usize, Line::create(self.size.width));
        }

        self.lines.insert(index as usize, line);
    }

    pub fn swap_char(&mut self, pos1: impl Into<Position>, pos2: impl Into<Position>) {
        let pos1 = pos1.into();
        let pos2 = pos2.into();
        let tmp = self.char_at(pos1);
        self.set_char(pos1, self.char_at(pos2));
        self.set_char(pos2, tmp);
    }

    pub fn add_hyperlink(&mut self, p: HyperLink) {
        self.hyperlinks.push(p);
    }

    pub fn hyperlinks(&self) -> &Vec<HyperLink> {
        &self.hyperlinks
    }

    pub fn set_width(&mut self, width: i32) {
        self.size.width = width;
    }

    pub fn set_height(&mut self, height: i32) {
        self.size.height = height;
    }

    pub fn preview_offset(&self) -> Option<Position> {
        self.preview_offset
    }

    pub fn set_preview_offset(&mut self, pos: Option<Position>) {
        self.preview_offset = pos;
    }

    pub fn set_size(&mut self, size: impl Into<Size>) {
        self.size = size.into();
    }

    /// Pre-allocate lines for the given size with invisible chars
    /// This is an optimization for formats like `XBin` where size is known upfront
    /// Allows direct access to chars without bounds checks
    pub fn preallocate_lines(&mut self, width: i32, height: i32) {
        self.size = Size::new(width, height);
        self.lines.clear();
        self.lines.reserve_exact(height as usize);
        for _ in 0..height {
            self.lines.push(Line::create(width));
        }
    }

    /// Set a char without any bounds checking - caller must ensure pos is valid
    /// This is an optimization for bulk loading where bounds are guaranteed
    ///
    /// # Safety
    /// Caller must ensure:
    /// - pos.y < `self.lines.len()`
    /// - pos.x < self.lines[pos.y].`chars.len()`
    #[inline(always)]
    pub fn set_char_unchecked(&mut self, pos: Position, attributed_char: AttributedChar) {
        // SAFETY: Caller guarantees bounds are valid
        unsafe {
            let line = self.lines.get_unchecked_mut(pos.y as usize);
            *line.chars.get_unchecked_mut(pos.x as usize) = attributed_char;
        }
    }
    /*
    pub(crate) fn from_layer(layer: &Layer, area: Rectangle) -> Layer {
        let mut result = Layer::new("new", area.size());

        for y in area.y_range() {
            for x in area.x_range() {
                let pos = Position::new(x, y) - area.start;
                result.set_char(pos, layer.char_at((x, y).into()).into());
            }
        }
        result
    }

    pub(crate) fn stamp(&mut self, target_pos: Position, layer: &Layer) {
        let area = layer.rectangle();
        for y in area.y_range() {
            for x in area.x_range() {
                let pos = Position::new(x, y);
                self.set_char(pos + target_pos, layer.char_at(pos));
            }
        }
    }*/
}

use crate::{Buffer, Color, Line, Position, Rectangle, Sixel, Size, TextPane, UnicodeConverter};

use super::AttributedChar;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    Chars,
    Attributes,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    #[default]
    Normal,
    PastePreview,
    PasteImage,
    Image,
}

impl Role {
    pub fn is_paste(&self) -> bool {
        matches!(self, Role::PastePreview | Role::PasteImage)
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Properties {
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

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Layer {
    pub role: Role,
    pub transparency: u8,
    pub properties: Properties,

    // Font page "default" chars are generated with
    // (needed for font mapping)
    pub default_font_page: usize,

    preview_offset: Option<Position>,
    size: Size,
    pub lines: Vec<Line>,

    pub sixels: Vec<Sixel>,
    pub(crate) hyperlinks: Vec<HyperLink>,
}

impl std::fmt::Display for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut str = String::new();
        let p = crate::parsers::ascii::CP437Converter::default();

        for y in 0..self.get_line_count() {
            str.extend(format!("{y:3}: ").chars());
            for x in 0..self.get_width() {
                let ch = self.get_char((x, y));
                str.push(p.convert_to_unicode(ch));
            }
            str.push('\n');
        }
        write!(f, "{str}")
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct HyperLink {
    pub url: Option<String>,
    pub position: Position,
    pub length: i32,
}

impl HyperLink {
    pub fn get_url(&self, buf: &Buffer) -> String {
        if let Some(ref url) = self.url {
            url.clone()
        } else {
            buf.get_string(self.position, self.length as usize)
        }
    }
}

impl TextPane for Layer {
    fn get_char(&self, pos: impl Into<Position>) -> AttributedChar {
        let pos = pos.into();
        if pos.x < 0 || pos.y < 0 || pos.x >= self.get_width() || pos.y >= self.get_height() {
            return AttributedChar::invisible().with_font_page(self.default_font_page);
        }
        let y = pos.y;
        if y < self.lines.len() as i32 {
            let cur_line = &self.lines[y as usize];
            if pos.x < cur_line.chars.len() as i32 {
                return cur_line.chars[pos.x as usize];
            }
        }
        AttributedChar::invisible().with_font_page(self.default_font_page)
    }

    fn get_line_count(&self) -> i32 {
        self.lines.len() as i32
    }

    fn get_line_length(&self, line: i32) -> i32 {
        self.lines[line as usize].get_line_length()
    }

    fn get_width(&self) -> i32 {
        self.size.width
    }

    fn get_height(&self) -> i32 {
        self.size.height
    }

    fn get_size(&self) -> Size {
        self.size
    }

    fn get_rectangle(&self) -> Rectangle {
        Rectangle::from_min_size(self.get_offset(), (self.get_width(), self.get_height()))
    }
}

impl Layer {
    pub fn new(title: impl Into<String>, size: impl Into<Size>) -> Self {
        let size = size.into();

        let mut lines = Vec::new();
        lines.resize(size.height as usize, Line::create(size.width));

        Layer {
            properties: Properties {
                title: title.into(),
                is_visible: true,
                ..Default::default()
            },
            size,
            lines,
            ..Default::default()
        }
    }

    pub fn get_offset(&self) -> Position {
        if let Some(offset) = self.preview_offset {
            return offset;
        }
        self.properties.offset
    }

    pub fn get_base_offset(&self) -> Position {
        self.properties.offset
    }

    pub fn set_offset(&mut self, pos: impl Into<Position>) {
        if self.properties.is_position_locked {
            return;
        }
        self.preview_offset = None;
        self.properties.offset = pos.into();
    }

    pub fn get_is_visible(&self) -> bool {
        self.properties.is_visible
    }

    pub fn set_is_visible(&mut self, is_visible: bool) {
        self.properties.is_visible = is_visible;
    }

    pub fn get_title(&self) -> &str {
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
        if pos.x < 0 || pos.y < 0 || pos.x >= self.get_width() || pos.y >= self.get_height() {
            return;
        }
        if self.properties.is_locked || !self.properties.is_visible {
            return;
        }
        if pos.y >= self.lines.len() as i32 {
            self.lines.resize(pos.y as usize + 1, Line::create(self.size.width));
        }

        if self.properties.has_alpha_channel && self.properties.is_alpha_channel_locked {
            let old_char = self.get_char(pos);
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
        let tmp = self.get_char(pos1);
        self.set_char(pos1, self.get_char(pos2));
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

    pub fn get_preview_offset(&self) -> Option<Position> {
        self.preview_offset
    }

    pub fn set_preview_offset(&mut self, pos: Option<Position>) {
        self.preview_offset = pos;
    }

    pub fn set_size(&mut self, size: impl Into<Size>) {
        self.size = size.into();
    }

    pub(crate) fn from_layer(layer: &Layer, area: Rectangle) -> Layer {
        let mut result = Layer::new("new", area.get_size());

        for y in area.y_range() {
            for x in area.x_range() {
                let pos = Position::new(x, y) - area.start;
                result.set_char(pos, layer.get_char((x, y)));
            }
        }
        result
    }

    pub(crate) fn stamp(&mut self, target_pos: Position, layer: &Layer) {
        let area = layer.get_rectangle();
        for y in area.y_range() {
            for x in area.x_range() {
                let pos = Position::new(x, y);
                self.set_char(pos + target_pos, layer.get_char(pos));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use i18n_embed_fl::fl;

    use crate::{AttributedChar, Layer, Line, Rectangle, TextAttribute, TextPane, editor::EditState};

    #[test]
    fn test_get_char() {
        let mut layer = Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-background-name"), (20, 20));
        layer.properties.has_alpha_channel = false;
        let mut line = Line::new();
        line.set_char(10, AttributedChar::new('a', TextAttribute::default()));

        layer.insert_line(0, line);

        assert_eq!(AttributedChar::invisible(), layer.get_char((-1, -1)));
        assert_eq!(AttributedChar::invisible(), layer.get_char((1000, 1000)));
        assert_eq!('a', layer.get_char((10, 0)).ch);
        assert_eq!(AttributedChar::invisible(), layer.get_char((9, 0)));
        assert_eq!(AttributedChar::invisible(), layer.get_char((11, 0)));
    }

    #[test]
    fn test_get_char_intransparent() {
        let mut layer = Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-background-name"), (20, 20));
        layer.properties.has_alpha_channel = true;

        let mut line = Line::new();
        line.set_char(10, AttributedChar::new('a', TextAttribute::default()));

        layer.insert_line(0, line);

        assert_eq!(AttributedChar::invisible(), layer.get_char((-1, -1)));
        assert_eq!(AttributedChar::invisible(), layer.get_char((1000, 1000)));
        assert_eq!('a', layer.get_char((10, 0)).ch);
        assert_eq!(AttributedChar::invisible(), layer.get_char((9, 0)));
        assert_eq!(AttributedChar::invisible(), layer.get_char((11, 0)));
    }

    #[test]
    fn test_insert_line() {
        let mut layer = Layer::new(fl!(crate::LANGUAGE_LOADER, "layer-background-name"), (80, 0));
        let mut line = Line::new();
        line.chars.push(AttributedChar::new('a', TextAttribute::default()));
        layer.insert_line(10, line);

        assert_eq!('a', layer.lines[10].chars[0].ch);
        assert_eq!(11, layer.lines.len());

        layer.insert_line(11, Line::new());
        assert_eq!(12, layer.lines.len());
    }

    #[test]
    fn test_clipboard() {
        let mut state = EditState::default();

        for i in 0..25 {
            for x in 0..80 {
                state
                    .set_char(
                        (x, i),
                        AttributedChar {
                            ch: unsafe { char::from_u32_unchecked((b'0' + (x % 10)) as u32) },
                            attribute: TextAttribute::default(),
                        },
                    )
                    .unwrap();
            }
        }

        state.set_selection(Rectangle::from_min_size((5, 6), (7, 8))).unwrap();
        let data = state.get_clipboard_data().unwrap();

        let layer = state.from_clipboard_data(&data).unwrap();

        assert_eq!(layer.get_width(), 7);
        assert_eq!(layer.get_height(), 8);

        assert_eq!(layer.properties.offset.x, 5);
        assert_eq!(layer.properties.offset.y, 6);

        assert!(layer.get_char((0, 0)).ch == '5');
    }
}

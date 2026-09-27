//! Model of the BBS-style message editor: CP437 text with DOS colors, word wrap,
//! insert/overwrite typing, selection, undo and SlyEdit-like quoting.
//!
//! Text is kept as paragraphs (hard line breaks) that are wrapped into visual
//! lines for display. Saving writes every visual line as a message line and
//! encodes colors as ANSI SGR sequences, which QWK readers display.

use icy_engine::BufferType;

/// Messages wrap before the last column of an 80 column terminal.
pub const WRAP_WIDTH: usize = 79;
const UNDO_LIMIT: usize = 500;
const TAB_WIDTH: usize = 4;

/// DOS palette index to ANSI color number and back (the mapping is its own inverse).
const ANSI: [u8; 8] = [0, 4, 2, 6, 1, 5, 3, 7];

/// DOS text attribute: foreground 0-15, background 0-7 and blink.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Attr {
    pub fg: u8,
    pub bg: u8,
    pub blink: bool,
}

impl Attr {
    pub const DEFAULT: Attr = Attr { fg: 7, bg: 0, blink: false };

    pub const fn new(fg: u8, bg: u8, blink: bool) -> Self {
        Self {
            fg: fg & 15,
            bg: bg & 7,
            blink,
        }
    }

    /// The SGR sequence that switches any state to this attribute.
    pub fn sgr(self) -> String {
        if self == Self::DEFAULT {
            return "\x1b[0m".into();
        }
        let mut sgr = String::from("\x1b[0");
        if self.fg >= 8 {
            sgr.push_str(";1");
        }
        if self.blink {
            sgr.push_str(";5");
        }
        sgr.push_str(&format!(";3{}", ANSI[usize::from(self.fg & 7)]));
        if self.bg != 0 {
            sgr.push_str(&format!(";4{}", ANSI[usize::from(self.bg & 7)]));
        }
        sgr.push('m');
        sgr
    }

    fn apply_sgr(&mut self, parameters: &str) {
        for parameter in parameters.split(';') {
            let code = parameter.parse::<u16>().unwrap_or(0);
            match code {
                0 => *self = Self::DEFAULT,
                1 => self.fg |= 8,
                2 | 22 => self.fg &= 7,
                5 | 6 => self.blink = true,
                25 => self.blink = false,
                30..=37 => self.fg = (self.fg & 8) | ANSI[usize::from(code - 30)],
                39 => self.fg = (self.fg & 8) | 7,
                40..=47 => self.bg = ANSI[usize::from(code - 40)],
                49 => self.bg = 0,
                90..=97 => self.fg = 8 | ANSI[usize::from(code - 90)],
                100..=107 => self.bg = ANSI[usize::from(code - 100)],
                _ => {}
            }
        }
    }
}

impl Default for Attr {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub attr: Attr,
}

/// Position in the text: paragraph and character offset within it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub para: usize,
    pub offset: usize,
}

impl Pos {
    pub const fn new(para: usize, offset: usize) -> Self {
        Self { para, offset }
    }
}

/// One displayed row: the paragraph cells `start..end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualLine {
    pub para: usize,
    pub start: usize,
    pub end: usize,
    /// Whether this row ends its paragraph (a hard line break follows).
    pub last: bool,
}

impl VisualLine {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp(usize),
    PageDown(usize),
    DocumentStart,
    DocumentEnd,
    WordLeft,
    WordRight,
}

#[derive(Clone)]
struct Snapshot {
    paragraphs: Vec<Vec<Cell>>,
    caret: Pos,
    anchor: Option<Pos>,
}

pub struct Editor {
    paragraphs: Vec<Vec<Cell>>,
    caret: Pos,
    /// Shows a caret at a soft wrap point at the end of the upper row instead of the start of the next.
    upstream: bool,
    anchor: Option<Pos>,
    insert: bool,
    attr: Attr,
    width: usize,
    layout: Vec<VisualLine>,
    para_rows: Vec<usize>,
    goal_column: Option<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// Whether the next typed character joins the previous undo step.
    typing: bool,
    revision: u64,
    edits: u64,
}

impl Default for Editor {
    fn default() -> Self {
        Self::from_body("")
    }
}

impl Editor {
    /// Parses message text containing ANSI color codes.
    pub fn from_body(body: &str) -> Self {
        let mut editor = Self {
            paragraphs: parse(body),
            caret: Pos::default(),
            upstream: false,
            anchor: None,
            insert: true,
            attr: Attr::DEFAULT,
            width: WRAP_WIDTH,
            layout: Vec::new(),
            para_rows: Vec::new(),
            goal_column: None,
            undo: Vec::new(),
            redo: Vec::new(),
            typing: false,
            revision: 0,
            edits: 0,
        };
        editor.relayout();
        editor
    }

    /// Message text: one line per visual row, colors as ANSI SGR sequences.
    pub fn to_body(&self) -> String {
        let mut body = String::new();
        let mut current = Attr::DEFAULT;
        for (index, line) in self.layout.iter().enumerate() {
            if index > 0 {
                body.push('\n');
            }
            let mut cells = &self.paragraphs[line.para][line.start..line.end];
            if !line.last {
                while let Some((last, rest)) = cells.split_last() {
                    if last.ch != ' ' || last.attr.bg != 0 {
                        break;
                    }
                    cells = rest;
                }
            }
            for cell in cells {
                let compatible = cell.ch == ' ' && cell.attr.bg == current.bg;
                if cell.attr != current && !compatible {
                    body.push_str(&cell.attr.sgr());
                    current = cell.attr;
                }
                body.push(cell.ch);
            }
        }
        if current != Attr::DEFAULT {
            body.push_str(&Attr::DEFAULT.sgr());
        }
        body
    }

    pub fn plain_text(&self) -> String {
        self.paragraphs
            .iter()
            .map(|cells| cells.iter().map(|cell| cell.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Changes whenever anything visible changes (text, caret, selection, color or mode).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Changes whenever the text or its colors change.
    pub fn edit_count(&self) -> u64 {
        self.edits
    }

    pub fn visual_lines(&self) -> &[VisualLine] {
        &self.layout
    }

    pub fn cells(&self, line: &VisualLine) -> &[Cell] {
        &self.paragraphs[line.para][line.start..line.end]
    }

    pub fn paragraph_count(&self) -> usize {
        self.paragraphs.len()
    }

    pub fn caret(&self) -> Pos {
        self.caret
    }

    /// Row and column of the caret on screen.
    pub fn caret_visual(&self) -> (usize, usize) {
        let row = self.row_of(self.caret, self.upstream);
        (row, self.caret.offset - self.layout[row].start)
    }

    pub fn attr(&self) -> Attr {
        self.attr
    }

    pub fn insert_mode(&self) -> bool {
        self.insert
    }

    pub fn toggle_insert(&mut self) {
        self.insert = !self.insert;
        self.touch();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Ordered selection range, `None` when nothing is selected.
    pub fn selection(&self) -> Option<(Pos, Pos)> {
        let anchor = self.anchor?;
        (anchor != self.caret).then(|| if anchor < self.caret { (anchor, self.caret) } else { (self.caret, anchor) })
    }

    pub fn is_selected(&self, pos: Pos) -> bool {
        self.selection().is_some_and(|(start, end)| start <= pos && pos < end)
    }

    pub fn clear_selection(&mut self) {
        if self.anchor.take().is_some() {
            self.touch();
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection()?;
        let mut text = String::new();
        for para in start.para..=end.para {
            let cells = &self.paragraphs[para];
            let from = if para == start.para { start.offset } else { 0 };
            let to = if para == end.para { end.offset } else { cells.len() };
            text.extend(cells[from..to].iter().map(|cell| cell.ch));
            if para != end.para {
                text.push('\n');
            }
        }
        Some(text)
    }

    pub fn select_all(&mut self) {
        self.anchor = Some(Pos::default());
        let last = self.paragraphs.len() - 1;
        self.caret = Pos::new(last, self.paragraphs[last].len());
        self.upstream = false;
        self.typing = false;
        self.touch();
    }

    /// Selects the word under `pos`.
    pub fn select_word(&mut self, pos: Pos) {
        let cells = &self.paragraphs[pos.para];
        let word = |index: usize| cells.get(index).is_some_and(|cell| !cell.ch.is_whitespace());
        let mut start = pos.offset.min(cells.len());
        let mut end = start;
        while start > 0 && word(start - 1) {
            start -= 1;
        }
        while word(end) {
            end += 1;
        }
        self.anchor = Some(Pos::new(pos.para, start));
        self.caret = Pos::new(pos.para, end);
        self.upstream = false;
        self.touch();
    }

    /// Sets the color used for typing. With a selection the selected text is recolored.
    pub fn set_attr(&mut self, attr: Attr) {
        if let Some((start, end)) = self.selection() {
            self.checkpoint(false);
            for para in start.para..=end.para {
                let len = self.paragraphs[para].len();
                let from = if para == start.para { start.offset } else { 0 };
                let to = if para == end.para { end.offset } else { len };
                for cell in &mut self.paragraphs[para][from..to] {
                    cell.attr = attr;
                }
            }
            self.edited();
        }
        // Text typed in a new color is a separate undo step.
        self.typing &= self.attr == attr;
        self.attr = attr;
        self.touch();
    }

    /// Inserts one character as its own undo step, as picked from a character table.
    pub fn insert_symbol(&mut self, ch: char) -> bool {
        self.typing = false;
        let inserted = self.type_char(ch);
        self.typing = false;
        inserted
    }

    pub fn move_caret(&mut self, motion: Motion, extend: bool) {
        self.begin_motion(extend);
        let caret = self.caret;
        let (row, column) = self.caret_visual();
        let goal = self.goal_column.unwrap_or(column);
        let mut keep_goal = false;
        match motion {
            Motion::Left => {
                if self.upstream {
                    self.upstream = false;
                }
                if caret.offset > 0 {
                    self.caret.offset -= 1;
                } else if caret.para > 0 {
                    self.caret = Pos::new(caret.para - 1, self.paragraphs[caret.para - 1].len());
                }
            }
            Motion::Right => {
                if self.upstream {
                    self.upstream = false;
                } else if caret.offset < self.paragraphs[caret.para].len() {
                    self.caret.offset += 1;
                } else if caret.para + 1 < self.paragraphs.len() {
                    self.caret = Pos::new(caret.para + 1, 0);
                }
            }
            Motion::Up | Motion::Down | Motion::PageUp(_) | Motion::PageDown(_) => {
                let target = match motion {
                    Motion::Up => row.saturating_sub(1),
                    Motion::Down => row + 1,
                    Motion::PageUp(rows) => row.saturating_sub(rows.max(1)),
                    Motion::PageDown(rows) => row + rows.max(1),
                    _ => unreachable!(),
                };
                let target = target.min(self.layout.len() - 1);
                self.place(target, goal);
                keep_goal = true;
            }
            Motion::Home => {
                self.caret.offset = self.layout[row].start;
                self.upstream = false;
            }
            Motion::End => {
                let line = self.layout[row];
                self.caret.offset = line.end;
                self.upstream = !line.last;
            }
            Motion::DocumentStart => {
                self.caret = Pos::default();
                self.upstream = false;
            }
            Motion::DocumentEnd => {
                let last = self.paragraphs.len() - 1;
                self.caret = Pos::new(last, self.paragraphs[last].len());
                self.upstream = false;
            }
            Motion::WordLeft => {
                self.caret = self.word_left(caret);
                self.upstream = false;
            }
            Motion::WordRight => {
                self.caret = self.word_right(caret);
                self.upstream = false;
            }
        }
        self.goal_column = keep_goal.then_some(goal);
        self.touch();
    }

    /// Moves the caret to a screen row/column, e.g. for a mouse click.
    pub fn set_caret_visual(&mut self, row: usize, column: usize, extend: bool) {
        self.begin_motion(extend);
        self.place(row.min(self.layout.len() - 1), column);
        self.goal_column = None;
        self.touch();
    }

    /// Converts a screen row/column into a text position.
    pub fn pos_at(&self, row: usize, column: usize) -> Pos {
        let line = self.layout[row.min(self.layout.len() - 1)];
        Pos::new(line.para, line.start + column.min(line.len()))
    }

    /// Types one character. Returns `false` if it cannot be written to a QWK message.
    pub fn type_char(&mut self, ch: char) -> bool {
        if !is_message_char(ch) {
            return false;
        }
        self.checkpoint(true);
        self.delete_selected();
        let Pos { para, offset } = self.caret;
        let cell = Cell { ch, attr: self.attr };
        let cells = &mut self.paragraphs[para];
        if self.insert || offset >= cells.len() {
            cells.insert(offset, cell);
        } else {
            cells[offset] = cell;
        }
        self.caret.offset += 1;
        self.upstream = false;
        if ch == ' ' {
            self.typing = false;
        }
        self.edited();
        true
    }

    /// Inserts pasted text. Returns the number of characters replaced by `?`.
    pub fn insert_text(&mut self, text: &str) -> usize {
        self.checkpoint(false);
        self.delete_selected();
        let mut replaced = 0;
        for ch in text.chars() {
            match ch {
                '\r' => {}
                '\n' => self.split(),
                '\t' => {
                    for _ in 0..TAB_WIDTH - self.caret.offset % TAB_WIDTH {
                        self.insert_cell(' ');
                    }
                }
                ch if is_message_char(ch) => self.insert_cell(ch),
                ch if ch.is_control() => {}
                _ => {
                    replaced += 1;
                    self.insert_cell('?');
                }
            }
        }
        self.upstream = false;
        self.edited();
        replaced
    }

    pub fn newline(&mut self) {
        self.checkpoint(false);
        self.delete_selected();
        self.split();
        self.upstream = false;
        self.edited();
    }

    pub fn tab(&mut self) {
        let column = self.caret_visual().1;
        self.checkpoint(false);
        self.delete_selected();
        for _ in 0..TAB_WIDTH - column % TAB_WIDTH {
            self.insert_cell(' ');
        }
        self.edited();
    }

    pub fn backspace(&mut self) {
        if self.selection().is_none() && self.caret == Pos::default() {
            return;
        }
        self.checkpoint(false);
        if !self.delete_selected() {
            let Pos { para, offset } = self.caret;
            if offset > 0 {
                self.paragraphs[para].remove(offset - 1);
                self.caret.offset -= 1;
            } else {
                let cells = self.paragraphs.remove(para);
                let previous = &mut self.paragraphs[para - 1];
                self.caret = Pos::new(para - 1, previous.len());
                previous.extend(cells);
            }
        }
        self.upstream = false;
        self.edited();
    }

    pub fn delete(&mut self) {
        let Pos { para, offset } = self.caret;
        if self.selection().is_none() && offset >= self.paragraphs[para].len() && para + 1 >= self.paragraphs.len() {
            return;
        }
        self.checkpoint(false);
        if !self.delete_selected() {
            if offset < self.paragraphs[para].len() {
                self.paragraphs[para].remove(offset);
            } else {
                let next = self.paragraphs.remove(para + 1);
                self.paragraphs[para].extend(next);
            }
        }
        self.edited();
    }

    pub fn delete_word_back(&mut self) {
        if self.selection().is_none() {
            self.anchor = Some(self.caret);
            self.caret = self.word_left(self.caret);
        }
        self.backspace();
    }

    /// Removes the caret's screen row (SlyEdit Ctrl-D).
    pub fn delete_line(&mut self) {
        self.checkpoint(false);
        self.anchor = None;
        let (row, _) = self.caret_visual();
        let line = self.layout[row];
        let whole = line.start == 0 && line.last;
        if whole && self.paragraphs.len() > 1 {
            self.paragraphs.remove(line.para);
            let para = line.para.min(self.paragraphs.len() - 1);
            self.caret = Pos::new(para, 0);
        } else {
            self.paragraphs[line.para].drain(line.start..line.end);
            self.caret = Pos::new(line.para, line.start);
        }
        self.upstream = false;
        self.edited();
    }

    /// Inserts whole lines (quotes) above the caret's paragraph, splitting it at the caret first.
    pub fn insert_lines(&mut self, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        self.checkpoint(false);
        self.delete_selected();
        if self.caret.offset > 0 {
            self.split();
        }
        let para = self.caret.para;
        for (index, line) in lines.iter().enumerate() {
            let cells = line
                .chars()
                .map(|ch| Cell {
                    ch: if is_message_char(ch) { ch } else { '?' },
                    attr: Attr::DEFAULT,
                })
                .collect();
            self.paragraphs.insert(para + index, cells);
        }
        self.caret = Pos::new(para + lines.len(), 0);
        self.upstream = false;
        self.edited();
    }

    pub fn cut(&mut self) -> Option<String> {
        let text = self.selected_text()?;
        self.checkpoint(false);
        self.delete_selected();
        self.edited();
        Some(text)
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(snapshot);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(snapshot);
        true
    }

    /// Case-insensitive search from the caret, wrapping around. Selects the match.
    pub fn find(&mut self, needle: &str) -> bool {
        let needle: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
        if needle.is_empty() {
            return false;
        }
        let count = self.paragraphs.len();
        let start = self.caret;
        for step in 0..=count {
            let para = (start.para + step) % count;
            let haystack: Vec<char> = self.paragraphs[para]
                .iter()
                .map(|cell| cell.ch.to_lowercase().next().unwrap_or(cell.ch))
                .collect();
            let from = if step == 0 { start.offset } else { 0 };
            let found = (from..=haystack.len().saturating_sub(needle.len()))
                .find(|&index| haystack.len() >= needle.len() && haystack[index..index + needle.len()] == needle[..]);
            let found = found.filter(|&index| step < count || index < start.offset);
            if let Some(index) = found {
                self.anchor = Some(Pos::new(para, index));
                self.caret = Pos::new(para, index + needle.len());
                self.upstream = false;
                self.typing = false;
                self.touch();
                return true;
            }
        }
        false
    }

    fn begin_motion(&mut self, extend: bool) {
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(self.caret);
            }
        } else {
            self.anchor = None;
        }
        self.typing = false;
    }

    fn place(&mut self, row: usize, column: usize) {
        let line = self.layout[row];
        self.caret = Pos::new(line.para, line.start + column.min(line.len()));
        self.upstream = self.caret.offset == line.end && !line.last;
    }

    fn word_left(&self, pos: Pos) -> Pos {
        if pos.offset == 0 {
            return if pos.para > 0 {
                Pos::new(pos.para - 1, self.paragraphs[pos.para - 1].len())
            } else {
                pos
            };
        }
        let cells = &self.paragraphs[pos.para];
        let mut offset = pos.offset;
        while offset > 0 && cells[offset - 1].ch.is_whitespace() {
            offset -= 1;
        }
        while offset > 0 && !cells[offset - 1].ch.is_whitespace() {
            offset -= 1;
        }
        Pos::new(pos.para, offset)
    }

    fn word_right(&self, pos: Pos) -> Pos {
        let cells = &self.paragraphs[pos.para];
        if pos.offset >= cells.len() {
            return if pos.para + 1 < self.paragraphs.len() {
                Pos::new(pos.para + 1, 0)
            } else {
                pos
            };
        }
        let mut offset = pos.offset;
        while offset < cells.len() && !cells[offset].ch.is_whitespace() {
            offset += 1;
        }
        while offset < cells.len() && cells[offset].ch.is_whitespace() {
            offset += 1;
        }
        Pos::new(pos.para, offset)
    }

    fn insert_cell(&mut self, ch: char) {
        let cell = Cell { ch, attr: self.attr };
        self.paragraphs[self.caret.para].insert(self.caret.offset, cell);
        self.caret.offset += 1;
    }

    fn split(&mut self) {
        let Pos { para, offset } = self.caret;
        let tail = self.paragraphs[para].split_off(offset);
        self.paragraphs.insert(para + 1, tail);
        self.caret = Pos::new(para + 1, 0);
    }

    /// Deletes the selection without recording an undo step.
    fn delete_selected(&mut self) -> bool {
        let Some((start, end)) = self.selection() else {
            self.anchor = None;
            return false;
        };
        let tail = self.paragraphs[end.para].split_off(end.offset);
        self.paragraphs.drain(start.para + 1..=end.para);
        let cells = &mut self.paragraphs[start.para];
        cells.truncate(start.offset);
        cells.extend(tail);
        self.caret = start;
        self.anchor = None;
        true
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            paragraphs: self.paragraphs.clone(),
            caret: self.caret,
            anchor: self.anchor,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.paragraphs = snapshot.paragraphs;
        self.caret = snapshot.caret;
        self.anchor = snapshot.anchor;
        self.upstream = false;
        self.typing = false;
        self.edited();
    }

    /// Records an undo step unless this keystroke continues the current typing run.
    fn checkpoint(&mut self, typing: bool) {
        if !(typing && self.typing) {
            self.undo.push(self.snapshot());
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.typing = typing;
    }

    fn edited(&mut self) {
        self.edits = self.edits.wrapping_add(1);
        self.goal_column = None;
        self.relayout();
        self.touch();
    }

    fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn relayout(&mut self) {
        if self.paragraphs.is_empty() {
            self.paragraphs.push(Vec::new());
        }
        self.layout.clear();
        self.para_rows.clear();
        for (para, cells) in self.paragraphs.iter().enumerate() {
            self.para_rows.push(self.layout.len());
            let breaks = wrap(cells, self.width);
            let count = breaks.len();
            self.layout.extend(breaks.into_iter().enumerate().map(|(index, (start, end))| VisualLine {
                para,
                start,
                end,
                last: index + 1 == count,
            }));
        }
        let para = self.caret.para.min(self.paragraphs.len() - 1);
        self.caret = Pos::new(para, self.caret.offset.min(self.paragraphs[para].len()));
        if let Some(anchor) = self.anchor {
            let para = anchor.para.min(self.paragraphs.len() - 1);
            self.anchor = Some(Pos::new(para, anchor.offset.min(self.paragraphs[para].len())));
        }
    }

    fn row_of(&self, pos: Pos, upstream: bool) -> usize {
        let first = self.para_rows[pos.para];
        let mut row = first;
        while row + 1 < self.layout.len() && self.layout[row + 1].para == pos.para {
            let line = self.layout[row];
            if pos.offset < line.end || (pos.offset == line.end && upstream) {
                break;
            }
            row += 1;
        }
        row
    }
}

/// Greedy word wrap into `(start, end)` ranges. A space may hang in the last terminal column.
fn wrap(cells: &[Cell], width: usize) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut start = 0;
    while cells.len() - start > width {
        let window = &cells[start..start + width];
        let end = if cells[start + width].ch == ' ' {
            start + width + 1
        } else {
            window
                .iter()
                .rposition(|cell| cell.ch == ' ')
                .filter(|&index| index > 0)
                .map_or(start + width, |index| start + index + 1)
        };
        lines.push((start, end));
        start = end;
    }
    lines.push((start, cells.len()));
    lines
}

fn parse(body: &str) -> Vec<Vec<Cell>> {
    let mut paragraphs = vec![Vec::new()];
    let mut attr = Attr::DEFAULT;
    let mut chars = body.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\x1b' => {
                if chars.peek() == Some(&'[') {
                    chars.next();
                    let mut parameters = String::new();
                    for next in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&next) {
                            if next == 'm' {
                                attr.apply_sgr(&parameters);
                            }
                            break;
                        }
                        parameters.push(next);
                    }
                } else {
                    chars.next();
                }
            }
            '\r' => {}
            '\n' => paragraphs.push(Vec::new()),
            '\t' => {
                let cells = paragraphs.last_mut().unwrap();
                for _ in 0..TAB_WIDTH - cells.len() % TAB_WIDTH {
                    cells.push(Cell { ch: ' ', attr });
                }
            }
            ch if ch.is_control() => {}
            ch => paragraphs.last_mut().unwrap().push(Cell {
                ch: if is_message_char(ch) { ch } else { '?' },
                attr,
            }),
        }
    }
    paragraphs
}

/// Unicode character for a CP437 byte.
pub fn cp437_char(byte: u8) -> char {
    BufferType::CP437.convert_to_unicode(char::from(byte))
}

/// CP437 byte of a character, if it has one.
pub fn cp437_byte(ch: char) -> Option<u8> {
    BufferType::CP437
        .try_convert_from_unicode(ch)
        .and_then(|byte| u8::try_from(u32::from(byte)).ok())
}

/// Whether a CP437 byte can be typed into a message. Control codes are
/// interpreted by terminals and 0xE3 is QWK's line separator.
pub fn is_insertable(byte: u8) -> bool {
    byte >= 0x20 && byte != 0x7f && byte != 0xe3
}

/// Whether a character can be stored in a QWK message body.
pub fn is_message_char(ch: char) -> bool {
    !ch.is_control() && ch != '\u{e3}' && cp437_byte(ch).is_some_and(is_insertable)
}

/// Message bytes as text, keeping line breaks and escape sequences (so colors survive).
pub fn decode_message(bytes: &[u8]) -> String {
    bytes
        .iter()
        .filter_map(|&byte| match byte {
            b'\n' | 0x1b => Some(char::from(byte)),
            0..=31 | 127 => None,
            byte => Some(cp437_char(byte)),
        })
        .collect()
}

/// CP437 bytes of an edited message, for display; characters without a CP437 code become `?`.
pub fn encode_message(text: &str) -> Vec<u8> {
    text.chars()
        .map(|ch| match ch {
            '\n' | '\x1b' => ch as u8,
            ch => cp437_byte(ch).unwrap_or(b'?'),
        })
        .collect()
}

/// Removes ANSI escape sequences and `|nn` pipe color codes.
pub fn strip_codes(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '\x1b' => {
                index += 1;
                if chars.get(index) == Some(&'[') {
                    index += 1;
                    while index < chars.len() && !('\u{40}'..='\u{7e}').contains(&chars[index]) {
                        index += 1;
                    }
                }
                index += 1;
            }
            '|' if chars.get(index + 1).is_some_and(char::is_ascii_digit) && chars.get(index + 2).is_some_and(char::is_ascii_digit) => {
                index += 3;
            }
            ch => {
                plain.push(ch);
                index += 1;
            }
        }
    }
    plain
}

/// Initials of an author for FidoNet style quote prefixes (`" JD> "`).
pub fn initials(name: &str) -> String {
    name.split(|ch: char| ch.is_whitespace() || ch == '.' || ch == '_' || ch == '-')
        .filter_map(|word| word.chars().find(|ch| ch.is_alphabetic()))
        .take(2)
        .flat_map(char::to_uppercase)
        .collect()
}

/// Quote lines for a reply, re-quoting existing quotes (`" AB> "` becomes `" AB>> "`) and wrapping long lines.
pub fn quote_lines(author: &str, text: &str, width: usize) -> Vec<String> {
    let prefix = format!(" {}> ", initials(author));
    let text = strip_codes(text);
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            lines.push(prefix.trim_end().to_string());
            continue;
        }
        if let Some(requoted) = requote(line) {
            lines.extend(wrap_text(&requoted, width, ""));
        } else {
            lines.extend(wrap_text(line, width, &prefix));
        }
    }
    while lines.last().is_some_and(|line| line.trim() == prefix.trim()) {
        lines.pop();
    }
    lines
}

fn requote(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let letters = trimmed.chars().take_while(char::is_ascii_uppercase).count();
    if letters > 3 || trimmed.chars().nth(letters) != Some('>') {
        return None;
    }
    let (marker, rest) = trimmed.split_at(letters);
    Some(format!(" {marker}>{rest}"))
}

fn wrap_text(line: &str, width: usize, prefix: &str) -> Vec<String> {
    let available = width.saturating_sub(prefix.chars().count()).max(10);
    let cells: Vec<Cell> = line.chars().map(|ch| Cell { ch, attr: Attr::DEFAULT }).collect();
    wrap(&cells, available)
        .into_iter()
        .map(|(start, end)| format!("{prefix}{}", cells[start..end].iter().map(|cell| cell.ch).collect::<String>().trim_end()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> Editor {
        let mut editor = Editor::default();
        for ch in text.chars() {
            if ch == '\n' {
                editor.newline();
            } else {
                assert!(editor.type_char(ch));
            }
        }
        editor
    }

    #[test]
    fn typing_wraps_words_and_saves_visual_lines() {
        let word = "abcdefghi ";
        let editor = typed(&word.repeat(9));
        let lines: Vec<_> = editor.to_body().lines().map(str::to_string).collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].len() <= WRAP_WIDTH && !lines[0].ends_with(' '));
        assert_eq!(lines[1], "abcdefghi ");
        assert_eq!(editor.caret_visual(), (1, 10));
    }

    #[test]
    fn long_words_break_hard_and_backspace_joins_lines() {
        let mut editor = typed(&"x".repeat(WRAP_WIDTH + 5));
        assert_eq!(editor.visual_lines().len(), 2);
        assert_eq!(editor.caret_visual(), (1, 5));
        editor.move_caret(Motion::DocumentStart, false);
        editor.move_caret(Motion::End, false);
        assert_eq!(editor.caret_visual(), (0, WRAP_WIDTH), "End stays on the wrapped row");
        let mut editor = typed("one\ntwo");
        editor.move_caret(Motion::Home, false);
        editor.backspace();
        assert_eq!(editor.plain_text(), "onetwo");
        assert_eq!(editor.caret(), Pos::new(0, 3));
    }

    #[test]
    fn colors_round_trip_as_ansi() {
        let mut editor = Editor::default();
        editor.type_char('A');
        editor.set_attr(Attr::new(12, 1, false));
        editor.type_char('B');
        editor.type_char(' ');
        editor.set_attr(Attr::new(14, 0, true));
        editor.type_char('C');
        let body = editor.to_body();
        assert_eq!(body, "A\x1b[0;1;31;44mB \x1b[0;1;5;33mC\x1b[0m");
        let reloaded = Editor::from_body(&body);
        assert_eq!(reloaded.to_body(), body);
        let cells = reloaded.cells(&reloaded.visual_lines()[0]);
        assert_eq!(cells[1].attr, Attr::new(12, 1, false));
        assert_eq!(cells[3].attr, Attr::new(14, 0, true));
        assert_eq!(Editor::from_body("\x1b[1;32mhi\x1b[2Jx").plain_text(), "hix");
    }

    #[test]
    fn selection_recolor_cut_and_undo() {
        let mut editor = typed("hello world");
        editor.move_caret(Motion::Home, false);
        editor.move_caret(Motion::WordRight, true);
        assert_eq!(editor.selected_text().as_deref(), Some("hello "));
        editor.set_attr(Attr::new(10, 0, false));
        assert!(editor.to_body().starts_with("\x1b[0;1;32mhello"));
        assert_eq!(editor.cut().as_deref(), Some("hello "));
        assert_eq!(editor.plain_text(), "world");
        assert!(editor.undo());
        assert_eq!(editor.plain_text(), "hello world");
        assert!(editor.undo());
        assert_eq!(editor.to_body(), "hello world");
        assert!(editor.redo());
        assert!(editor.to_body().contains("\x1b[0;1;32m"));
    }

    #[test]
    fn typing_runs_undo_by_word_and_overwrite_replaces() {
        let mut editor = typed("abc def");
        editor.undo();
        assert_eq!(editor.plain_text(), "abc ");
        editor.move_caret(Motion::Home, false);
        editor.toggle_insert();
        editor.type_char('X');
        assert_eq!(editor.plain_text(), "Xbc ");
        assert!(!editor.type_char('€'));
        assert!(!editor.type_char('\u{263a}'), "control-range glyphs are not stored");
        assert_eq!(editor.insert_text("a€\tb"), 1);
    }

    #[test]
    fn delete_line_find_and_quotes() {
        let mut editor = typed("first\nsecond\nthird");
        editor.move_caret(Motion::Up, false);
        editor.delete_line();
        assert_eq!(editor.plain_text(), "first\nthird");
        editor.move_caret(Motion::DocumentStart, false);
        assert!(editor.find("THI"));
        assert_eq!(editor.selected_text().as_deref(), Some("thi"));
        assert!(!editor.find("missing"));
        editor.move_caret(Motion::DocumentStart, false);
        editor.move_caret(Motion::Right, false);
        editor.insert_lines(&[" A> quoted".into()]);
        assert_eq!(editor.plain_text(), "f\n A> quoted\nirst\nthird");
        assert_eq!(editor.caret(), Pos::new(2, 0));
    }

    #[test]
    fn quote_lines_use_initials_requote_and_strip_codes() {
        let text = "Hello |07there\x1b[1;31m!\n\n XY> older\n> plain\n\n";
        let lines = quote_lines("John Doe", text, WRAP_WIDTH);
        assert_eq!(lines, vec![" JD> Hello there!", " JD>", " XY>> older", " >> plain"]);
        let long = quote_lines("alice", &"word ".repeat(30), WRAP_WIDTH);
        assert!(long.len() == 2 && long.iter().all(|line| line.starts_with(" A> ") && line.len() <= WRAP_WIDTH));
        assert_eq!(decode_message(b"a\x1b[1m\x01\xb0\n"), "a\x1b[1m\u{2591}\n");
    }
}

//! The modern reading mode: the message as selectable, wrapping text in the application font that
//! keeps its ANSI colors but follows the theme. ANSI art is drawn with the BBS font, as in the
//! classic view, since block graphics only line up in their own cell grid.

use std::ops::Range;

use eframe::egui::{self, Color32};
use icy_engine::{Rectangle, RenderOptions, TextPane, TextScreen};
use icy_engine_gui::egui::appearance;
use icy_mail::{
    editor,
    options::{ModernFont, ReadingMode, MODERN_FONT_SIZES},
    reader::{render_body, render_body_wide, render_file_page, render_file_page_wide, Pane},
    text::{styled_lines, StyledSpan},
};

use super::{app::MailApp, widgets};

/// Space between the text and the edges of the reading pane.
const MARGIN: egui::Vec2 = egui::vec2(24.0, 16.0);
/// Blank rows between two art paragraphs that still belong to one picture.
const ART_GAP: usize = 3;
/// Rows of colored or aligned text between two art paragraphs that still belong to the picture,
/// like the address and network lines of a BBS ad.
const ART_TEXT_BRIDGE: usize = 12;
/// Art textures are stored at this multiple of the font's pixels, so scaling them to the text size
/// stays sharp.
const ART_OVERSAMPLING: usize = 2;

/// One row of text with how it is set.
#[derive(Debug)]
pub struct ModernLine {
    pub spans: Vec<StyledSpan>,
    /// Box drawing or column alignment need the fixed-width font.
    pub fixed: bool,
    /// An uncolored quote line (`> …`, ` JD> …`), drawn dimmed.
    pub quote: bool,
}

/// A message split into text and art.
#[derive(Debug)]
pub enum Block {
    Text(ModernLine),
    /// Rows of the rendered message drawn with the BBS font, `columns` wide.
    Art {
        rows: Range<i32>,
        columns: i32,
    },
}

/// What the content pane shows, in either display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Document {
    /// A message of the packet by its index.
    Message(usize),
    /// A page of a bulletin, news or new files screen.
    File(usize, usize),
    /// An outbox draft by its id and a hash of its text, so edits show up.
    Draft(u64, u64),
}

/// What the modern view shows, with the art already rendered.
pub enum Item {
    Text(ModernLine),
    Art(egui::TextureHandle),
}

/// Splits a message into text lines and art. `classic` is the message on the 80 column terminal,
/// which art relies on; `wide` the same message without that limit, which tells which text rows the
/// terminal only wrapped, so those lines are joined again and wrap at the pane's width instead.
pub fn blocks(classic: &TextScreen, wide: &TextScreen) -> Vec<Block> {
    let lines = styled_lines(classic);
    let in_art = art_rows(&lines);
    let columns = classic.buffer.width().max(1) as usize;
    let continues = soft_wraps(&lines, wide, columns);

    let mut blocks = Vec::new();
    let mut lines = lines.into_iter().enumerate().peekable();
    while let Some((row, mut spans)) = lines.next() {
        if in_art[row] {
            let mut columns = width(&spans);
            let mut end = row + 1;
            while let Some((_, spans)) = lines.next_if(|(next, _)| in_art[*next]) {
                columns = columns.max(width(&spans));
                end += 1;
            }
            blocks.push(Block::Art {
                rows: row as i32..end as i32,
                columns: columns as i32,
            });
            continue;
        }
        let mut last = row;
        while continues[last] && !in_art[last + 1] {
            let Some((next, more)) = lines.next() else { break };
            // The row was full; a space the terminal wrapped at was trimmed with the row's blanks.
            let missing = columns.saturating_sub(width(&spans));
            if missing > 0 {
                spans.push(StyledSpan {
                    text: " ".repeat(missing),
                    foreground: None,
                    background: None,
                    bold: false,
                    italic: false,
                    underline: false,
                    strikethrough: false,
                });
            }
            spans.extend(more);
            last = next;
        }
        let text: String = spans.iter().map(|span| span.text.as_str()).collect();
        let colored = spans.iter().any(|span| span.foreground.is_some());
        blocks.push(Block::Text(ModernLine {
            fixed: text.chars().any(is_box_drawing) || is_aligned(&text),
            quote: !colored && is_quote(&text),
            spans,
        }));
    }
    blocks
}

/// Rows drawn as a picture. Paragraphs (runs of rows between blank rows) holding art are pictures as
/// a whole; two of them form one picture when only a few blank rows or short colored or aligned
/// paragraphs lie between them. Plain text paragraphs switch back to text.
fn art_rows(lines: &[Vec<StyledSpan>]) -> Vec<bool> {
    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Art,
        Decorated,
        Plain,
    }
    let blank = |spans: &Vec<StyledSpan>| spans.iter().all(|span| span.text.trim().is_empty() && span.background.is_none());
    let mut paragraphs: Vec<(Range<usize>, Kind)> = Vec::new();
    let mut row = 0;
    while row < lines.len() {
        if blank(&lines[row]) {
            row += 1;
            continue;
        }
        let start = row;
        while row < lines.len() && !blank(&lines[row]) {
            row += 1;
        }
        let rows = &lines[start..row];
        let kind = if rows.iter().any(|spans| is_art(spans)) {
            Kind::Art
        } else if rows.iter().any(|spans| {
            let text: String = spans.iter().map(|span| span.text.as_str()).collect();
            spans.iter().any(|span| span.foreground.is_some()) || text.chars().any(is_box_drawing) || is_aligned(&text)
        }) {
            Kind::Decorated
        } else {
            Kind::Plain
        };
        paragraphs.push((start..row, kind));
    }

    let mut art = vec![false; lines.len()];
    let mut index = 0;
    while index < paragraphs.len() {
        if paragraphs[index].1 != Kind::Art {
            index += 1;
            continue;
        }
        let start = paragraphs[index].0.start;
        let mut end = paragraphs[index].0.end;
        let mut next = index + 1;
        // Extend over decorated paragraphs up to the next art paragraph, if it is close enough.
        loop {
            let mut bridged = 0;
            let mut candidate = next;
            let mut previous_end = end;
            let mut reached = None;
            while candidate < paragraphs.len() {
                let (rows, kind) = &paragraphs[candidate];
                if rows.start - previous_end > ART_GAP {
                    break;
                }
                match kind {
                    Kind::Art => {
                        reached = Some(candidate);
                        break;
                    }
                    Kind::Decorated if bridged + rows.len() <= ART_TEXT_BRIDGE => bridged += rows.len(),
                    _ => break,
                }
                previous_end = rows.end;
                candidate += 1;
            }
            match reached {
                Some(found) => {
                    end = paragraphs[found].0.end;
                    next = found + 1;
                }
                None => break,
            }
        }
        art[start..end].iter_mut().for_each(|row| *row = true);
        index = next;
    }
    art
}

/// Which rows of the classic rendering continue on the next row because their line was longer than
/// the terminal. Each line of the wide rendering takes as many classic rows as its length needs;
/// when that does not add up to the classic rows (cursor movement, for example), nothing is joined.
fn soft_wraps(classic: &[Vec<StyledSpan>], wide: &TextScreen, columns: usize) -> Vec<bool> {
    let mut continues = vec![false; classic.len()];
    let mut row = 0;
    for spans in styled_lines(wide) {
        let length = width(&spans);
        let wrapped = length.div_ceil(columns).max(1);
        // A line filling its last row makes the terminal wrap once more, leaving an empty row.
        let rows = wrapped + usize::from(length > 0 && length.is_multiple_of(columns));
        if row + rows > classic.len() {
            // The empty row after the message's last line was dropped with the trailing blanks.
            if row + wrapped == classic.len() {
                continues[row..row + wrapped - 1].iter_mut().for_each(|flag| *flag = true);
                return continues;
            }
            return vec![false; classic.len()];
        }
        continues[row..row + wrapped - 1].iter_mut().for_each(|flag| *flag = true);
        row += rows;
    }
    if row == classic.len() {
        continues
    } else {
        vec![false; classic.len()]
    }
}

/// Renders the art of `blocks` into textures; text lines are kept as they are.
pub fn items(context: &egui::Context, screen: &TextScreen, blocks: Vec<Block>) -> Vec<Item> {
    blocks
        .into_iter()
        .enumerate()
        .map(|(index, block)| match block {
            Block::Text(line) => Item::Text(line),
            Block::Art { rows, columns } => {
                let options: RenderOptions = Rectangle::from(0, rows.start, columns, rows.end - rows.start).into();
                let (size, rgba) = screen.buffer.render_to_rgba(&options, false);
                let (width, height) = (size.width.max(0) as usize, size.height.max(0) as usize);
                let image = if width > 0 && rgba.len() >= width * height * 4 {
                    oversample(&egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba[..width * height * 4]))
                } else {
                    egui::ColorImage::new([1, 1], vec![Color32::TRANSPARENT])
                };
                let options = egui::TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    minification: egui::TextureFilter::Linear,
                    ..Default::default()
                };
                Item::Art(context.load_texture(format!("modern-art-{index}"), image, options))
            }
        })
        .collect()
}

/// Enlarges pixel art by [`ART_OVERSAMPLING`] without smoothing.
fn oversample(image: &egui::ColorImage) -> egui::ColorImage {
    let [width, height] = image.size;
    let factor = ART_OVERSAMPLING;
    let mut pixels = Vec::with_capacity(width * height * factor * factor);
    for y in 0..height * factor {
        for x in 0..width * factor {
            pixels.push(image.pixels[(y / factor) * width + x / factor]);
        }
    }
    egui::ColorImage::new([width * factor, height * factor], pixels)
}

fn width(spans: &[StyledSpan]) -> usize {
    spans.iter().map(|span| span.text.chars().count()).sum()
}

/// Block elements and shades (▀ ▄ █ ▌ ▐ ░ ▒ ▓) or colored backgrounds: picture rather than text.
/// CP437's geometric shapes (■ ▬ ▲ ► ○ ◘ …) are bullets and markers in taglines and lists, so they
/// stay text.
fn is_art(spans: &[StyledSpan]) -> bool {
    spans
        .iter()
        .any(|span| span.background.is_some() || span.text.chars().any(|ch| ('\u{2580}'..='\u{259F}').contains(&ch)))
}

/// Box drawing lines, as in tables.
fn is_box_drawing(ch: char) -> bool {
    ('\u{2500}'..='\u{257F}').contains(&ch)
}

/// Runs of spaces inside a line line up columns, which only a fixed-width font keeps.
fn is_aligned(text: &str) -> bool {
    text.trim().contains("   ")
}

/// `> text`, `JD> text` or ` AB>> text`: up to four initials before the quote marker.
pub fn is_quote(text: &str) -> bool {
    let text = text.trim_start();
    let initials = text.chars().take_while(|ch| ch.is_alphanumeric()).count();
    initials <= 4 && text.chars().nth(initials) == Some('>')
}

/// The 16 DOS colors on the modern page, in DOS order. Each normal color and its light variant stay
/// clearly apart (light gray and white too), blue keeps its saturation, and all read well on the
/// theme's page. Black text stays visible (dimmed on dark pages); where index 0 is the background,
/// the page color replaces it.
pub fn modern_palette(dark_mode: bool) -> [Color32; 16] {
    let rgb = |value: u32| Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8);
    let colors: [u32; 16] = if dark_mode {
        [
            0x4A5058, 0x4A78F0, 0x2EA043, 0x13A8B8, 0xD0453D, 0xB14FC5, 0xC28526, 0xAAB0B8, //
            0x767C85, 0x8DB4FF, 0x6BE07A, 0x6FE6F2, 0xFF7B72, 0xF08CF5, 0xF4DE62, 0xFFFFFF,
        ]
    } else {
        [
            0x000000, 0x1D4FC4, 0x1A7F37, 0x0B7C8A, 0xB42318, 0x8A2BA0, 0x8A5A00, 0x4E555E, //
            0x8C939C, 0x4B83F2, 0x2FA84F, 0x1FA3B3, 0xE5483E, 0xC04CD6, 0xB08D00, 0x000000,
        ]
    };
    colors.map(rgb)
}

/// The standard VGA palette the message renderer uses for the 16 DOS colors.
const DOS_COLORS: [[u8; 3]; 16] = [
    [0, 0, 0],
    [0, 0, 170],
    [0, 170, 0],
    [0, 170, 170],
    [170, 0, 0],
    [170, 0, 170],
    [170, 85, 0],
    [170, 170, 170],
    [85, 85, 85],
    [85, 85, 255],
    [85, 255, 85],
    [85, 255, 255],
    [255, 85, 85],
    [255, 85, 255],
    [255, 255, 85],
    [255, 255, 255],
];

/// A message color on the modern page: the 16 DOS colors take their [`modern_palette`] entry,
/// other colors (256 color and true color) are kept readable with [`readable`].
pub fn modern_color(color: [u8; 3], on_background: bool, dark_mode: bool) -> Color32 {
    match DOS_COLORS.iter().position(|dos| *dos == color) {
        Some(index) => modern_palette(dark_mode)[index],
        None => readable(color, on_background, dark_mode),
    }
}

/// Keeps a message color readable on the page: dark colors are lightened on dark themes and
/// light ones darkened on light themes. Colors drawn on their own background stay as they are.
pub fn readable(color: [u8; 3], on_background: bool, dark_mode: bool) -> Color32 {
    let color = Color32::from_rgb(color[0], color[1], color[2]);
    if on_background {
        return color;
    }
    let luminance = (0.2126 * f32::from(color.r()) + 0.7152 * f32::from(color.g()) + 0.0722 * f32::from(color.b())) / 255.0;
    if dark_mode && luminance < 0.35 {
        color.lerp_to_gamma(Color32::WHITE, 0.45)
    } else if !dark_mode && luminance > 0.55 {
        color.lerp_to_gamma(Color32::BLACK, 0.45)
    } else {
        color
    }
}

impl MailApp {
    /// The text of the content pane in the chosen display: the BBS terminal, or the modern view.
    /// Messages, bulletins and outbox drafts all show through here.
    pub fn document_body(&mut self, ui: &mut egui::Ui, document: Document) -> egui::Response {
        match self.reading_mode {
            ReadingMode::Modern => self.modern_body(ui, document),
            ReadingMode::Classic => self.terminal_body(ui),
        }
    }

    /// The document on the 80 column terminal and without that limit, see [`blocks`].
    fn document_screens(&self, document: Document) -> Result<(TextScreen, TextScreen), String> {
        let package = self.reader.package.as_ref();
        let screens = match document {
            Document::Message(index) => package
                .ok_or_else(String::new)?
                .get_message(index)
                .and_then(|message| Ok((render_body(&message.text)?, render_body_wide(&message.text)?))),
            Document::File(index, page) => {
                let file = package.and_then(|package| package.files.get(index)).ok_or_else(String::new)?;
                render_file_page(&file.data, page).and_then(|classic| Ok((classic, render_file_page_wide(&file.data, page)?)))
            }
            Document::Draft(id, _) => {
                let draft = self
                    .drafts
                    .as_ref()
                    .and_then(|store| store.drafts().iter().find(|draft| draft.id == id).cloned())
                    .ok_or_else(String::new)?;
                let text = editor::encode_message(&draft.text());
                render_body(&text).and_then(|classic| Ok((classic, render_body_wide(&text)?)))
            }
        };
        screens.map_err(|error| error.to_string())
    }

    /// A document in the modern reading mode. Scrolling reuses the terminal's offsets, so the
    /// keyboard, Space, "continue with the next unread message" and bulletin pages behave the same.
    fn modern_body(&mut self, ui: &mut egui::Ui, document: Document) -> egui::Response {
        let dark_mode = ui.visuals().dark_mode;
        let package = self.reader.package.as_ref().map_or(0, |package| std::sync::Arc::as_ptr(package) as usize);
        let key = (package, document);
        let rebuilt = self.modern_items.as_ref().is_none_or(|(cached, _)| *cached != key);
        if rebuilt {
            let rendered = match self.document_screens(document) {
                Ok((classic, wide)) => items(ui.ctx(), &classic, blocks(&classic, &wide)),
                Err(error) => {
                    if !error.is_empty() {
                        self.error = Some(error);
                    }
                    Vec::new()
                }
            };
            self.modern_items = Some((key, rendered));
        }
        if let Some(zoom) = ui.input(|input| (input.zoom_delta() != 1.0).then(|| input.zoom_delta())) {
            if ui.rect_contains_pointer(ui.available_rect_before_wrap()) {
                let step = if zoom > 1.0 { 1.0 } else { -1.0 };
                self.modern_font_size = (self.modern_font_size + step).clamp(*MODERN_FONT_SIZES.start(), *MODERN_FONT_SIZES.end());
                self.options_save_after = Some(ui.input(|input| input.time) + 0.25);
            }
        }
        let visuals = ui.visuals().clone();
        let area = ui.available_rect_before_wrap();
        ui.painter().rect_filled(area, 0.0, visuals.extreme_bg_color);
        if ui.input(|input| input.pointer.primary_pressed() && input.pointer.interact_pos().is_some_and(|pos| area.contains(pos))) {
            self.set_focus(Pane::Content, ui.ctx());
        }

        let size = self.modern_font_size;
        let fixed = egui::FontId::monospace(size);
        let text_font = match self.modern_font {
            ModernFont::Proportional => egui::FontId::proportional(size),
            ModernFont::Monospace => fixed.clone(),
        };
        let bold = egui::FontId::new(size, appearance::bold_family(ui));
        let palette = modern_palette(dark_mode);
        let needle = if matches!(document, Document::Message(_)) {
            self.reader.filter.trim().to_owned()
        } else {
            String::new()
        };
        let text_width = (area.width() - MARGIN.x * 2.0).max(40.0);
        // One art cell as wide as a character of the fixed-width text; the font's cells are 8 pixels wide.
        let cell = ui.ctx().fonts_mut(|fonts| fonts.glyph_width(&fixed, 'M'));
        let art_scale = cell / (8.0 * ART_OVERSAMPLING as f32);

        // Consecutive text lines share one label; art sits between them as images.
        let new_job = || {
            let mut job = egui::text::LayoutJob::default();
            job.wrap.max_width = text_width;
            job
        };
        let mut job = new_job();
        let mut parts: Vec<Result<egui::text::LayoutJob, egui::TextureHandle>> = Vec::new();
        for item in self.modern_items.as_ref().map_or(&[][..], |(_, items)| items.as_slice()) {
            let line = match item {
                Item::Art(texture) => {
                    if !job.text.is_empty() {
                        parts.push(Ok(std::mem::replace(&mut job, new_job())));
                    }
                    parts.push(Err(texture.clone()));
                    continue;
                }
                Item::Text(line) => line,
            };
            if !job.text.is_empty() {
                job.append(
                    "\n",
                    0.0,
                    egui::TextFormat {
                        font_id: text_font.clone(),
                        ..Default::default()
                    },
                );
            }
            let proportional = !line.fixed && self.modern_font == ModernFont::Proportional;
            if line.spans.is_empty() {
                // A blank line still needs a glyph to take up a row at the start of a label.
                job.append(
                    " ",
                    0.0,
                    egui::TextFormat {
                        font_id: text_font.clone(),
                        ..Default::default()
                    },
                );
            }
            for span in &line.spans {
                let default = if line.quote { visuals.weak_text_color() } else { palette[7] };
                let color = span
                    .foreground
                    .map_or(default, |color| modern_color(color, span.background.is_some(), dark_mode));
                let font = if !proportional {
                    fixed.clone()
                } else if span.bold {
                    bold.clone()
                } else {
                    text_font.clone()
                };
                job.append(
                    &span.text,
                    0.0,
                    egui::TextFormat {
                        font_id: font,
                        color,
                        italics: span.italic,
                        underline: if span.underline { egui::Stroke::new(1.0, color) } else { egui::Stroke::NONE },
                        strikethrough: if span.strikethrough {
                            egui::Stroke::new(1.0, color)
                        } else {
                            egui::Stroke::NONE
                        },
                        ..Default::default()
                    },
                );
            }
        }
        if !job.text.is_empty() {
            parts.push(Ok(job));
        }

        let mut scroll = egui::ScrollArea::vertical().id_salt("modern-body").auto_shrink([false, false]);
        // Clamped here, as the scroll area keeps an offset past the end; a new document is laid out
        // once first, so the end is known.
        if !rebuilt {
            if let Some(offset) = self.screen.scroll_to.take() {
                scroll = scroll.vertical_scroll_offset(offset.y.clamp(0.0, self.screen.max_offset.y));
            }
        } else if self.screen.scroll_to.is_some() {
            ui.ctx().request_repaint();
        }
        let output = scroll.show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(MARGIN.x as i8, MARGIN.y as i8))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let mut response = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
                    for part in parts {
                        let part_response = match part {
                            Ok(mut job) => {
                                widgets::highlight(&mut job, 0, &needle, ui);
                                ui.add(egui::Label::new(job).selectable(true))
                            }
                            Err(texture) => {
                                let size = texture.size_vec2() * art_scale;
                                let size = size * (text_width / size.x).min(1.0);
                                ui.add(egui::Image::new((texture.id(), size)))
                            }
                        };
                        response = response.union(part_response);
                    }
                    response
                })
                .inner
        });
        let max = (output.content_size - output.inner_rect.size()).max(egui::Vec2::ZERO);
        self.screen.offset = egui::vec2(0.0, output.state.offset.y.min(max.y));
        self.screen.max_offset = egui::vec2(0.0, max.y);
        self.content_rect = output.inner_rect;
        // Hovering anywhere in the pane counts, e.g. for turning bulletin pages with the wheel.
        let area = ui.interact(output.inner_rect, ui.id().with("modern-area"), egui::Sense::hover());
        area.union(output.inner)
    }
}

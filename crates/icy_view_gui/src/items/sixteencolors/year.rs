use async_trait::async_trait;
use i18n_embed_fl::fl;
use icy_engine::{AttributedChar, Position, TextAttribute, TextBuffer, TextPane};
use icy_engine_gui::ui::FileIcon;
use retrofont::{Cell, Font, FontTarget, RenderOptions};
use tokio_util::sync::CancellationToken;

use crate::items::{Item, ItemError, create_text_buffer_preview, sort_folder};
use crate::ui::thumbnail_view::RgbaData;

use super::{API_PATH, SixteenColorsPack, cache::fetch_json_async, get_cache};

/// Embedded ZETRAX.TDF font data
const ZETRAX_TDF: &[u8] = include_bytes!("ZETRAX.TDF");

lazy_static::lazy_static! {
    static ref TDF_FONTS: Vec<Font> = Font::load(ZETRAX_TDF).unwrap_or(Vec::new());
}

/// A renderer that writes to a TextBuffer
struct BufferRenderer<'a> {
    buffer: &'a mut TextBuffer,
    cur_x: i32,
    cur_y: i32,
    start_x: i32,
    start_y: i32,
}

impl<'a> BufferRenderer<'a> {
    fn new(buffer: &'a mut TextBuffer, start_x: i32, start_y: i32) -> Self {
        Self {
            buffer,
            cur_x: start_x,
            cur_y: start_y,
            start_x,
            start_y,
        }
    }

    /// Reset to the next character position (advances X, resets Y to start)
    fn next_char(&mut self) {
        // Find the maximum X used so far
        self.start_x = self.cur_x;
        self.cur_y = self.start_y;
    }
}

impl FontTarget for BufferRenderer<'_> {
    type Error = std::fmt::Error;

    fn draw(&mut self, cell: Cell) -> std::result::Result<(), Self::Error> {
        if self.cur_x >= 0 && self.cur_x < self.buffer.width() && self.cur_y >= 0 && self.cur_y < self.buffer.height() {
            let fg = cell.fg.unwrap_or(15);
            let bg = cell.bg.unwrap_or(0);
            let attr = TextAttribute::from_color(fg, bg);

            self.buffer.layers[0].set_char(
                Position::new(self.cur_x, self.cur_y),
                AttributedChar::new(self.buffer.buffer_type.convert_from_unicode(cell.ch), attr),
            );
        }
        self.cur_x += 1;
        Ok(())
    }

    fn next_line(&mut self) -> std::result::Result<(), Self::Error> {
        self.cur_y += 1;
        self.cur_x = self.start_x;
        Ok(())
    }
}

/// Render text using TDF font to a TextBuffer and return as RgbaData
fn render_year_thumbnail(year: u64) -> RgbaData {
    let fonts = &TDF_FONTS;
    if fonts.is_empty() {
        if let Err(err) = Font::load(ZETRAX_TDF) {
            log::error!("Failed to load embedded TDF font: {}", err);
        } else {
            log::error!("ZETRAX.TDF font loaded, but fonts vector was empty");
        }
        return crate::items::create_text_preview(&year.to_string());
    }

    // Select font based on year
    let font_idx = (year as usize) % fonts.len();
    let font = &fonts[font_idx];

    let text = year.to_string();
    let mut buffer = TextBuffer::new((80, 25));

    // Try to center the text - TDF fonts are typically 8-12 chars wide per glyph
    // For a 4-digit year, we need roughly 40-50 chars width
    let start_x = 7;
    let start_y = 7;

    let mut renderer = BufferRenderer::new(&mut buffer, start_x, start_y);
    let options = RenderOptions::default();

    for ch in text.chars() {
        if font.render_glyph(&mut renderer, ch, &options).is_err() {
            // If rendering fails, fall back to simple text
            return crate::items::create_text_preview(&text);
        }
        renderer.next_char();
    }

    create_text_buffer_preview(&buffer)
}

/// A year folder containing release packs
pub struct SixteenColorsYear {
    pub year: u64,
    pub packs: u64,
}

impl SixteenColorsYear {
    pub fn new(year: u64, packs: u64) -> Self {
        Self { year, packs }
    }
}

#[async_trait]
impl Item for SixteenColorsYear {
    fn get_label(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "label-sixteencolors_year", year = self.year, packs = self.packs)
            .chars()
            .filter(|c| c.is_ascii())
            .collect::<String>()
    }

    fn get_file_path(&self) -> String {
        self.year.to_string()
    }

    fn is_container(&self) -> bool {
        true
    }

    fn get_file_icon(&self) -> FileIcon {
        FileIcon::FolderData
    }

    async fn get_thumbnail_preview(&self, _cancel_token: &CancellationToken) -> Option<RgbaData> {
        Some(render_year_thumbnail(self.year))
    }

    async fn get_subitems(&self, _cancel_token: &CancellationToken) -> Result<Vec<Box<dyn Item>>, ItemError> {
        let url = format!("{}/year/{}?rows=0", API_PATH, self.year);
        let cache = get_cache();
        let json = fetch_json_async(&cache, &url).await?;

        let mut result: Vec<Box<dyn Item>> = Vec::new();
        if let Some(packs) = json.as_array() {
            for pack in packs {
                let filename = pack["filename"].as_str().unwrap_or_default().to_string();
                let month = pack["month"].as_u64().unwrap_or(0);
                let year = pack["year"].as_u64().unwrap_or(0);
                let name = pack["name"].as_str().unwrap_or_default().to_string();
                result.push(Box::new(SixteenColorsPack::new(filename, month, year, name)));
            }
            sort_folder(&mut result);
        }
        Ok(result)
    }

    fn clone_box(&self) -> Box<dyn Item> {
        Box::new(SixteenColorsYear::new(self.year, self.packs))
    }
}

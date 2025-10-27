use std::hash::BuildHasher;
use std::hash::Hasher;
use std::hash::RandomState;

use ratatui::buffer::Cell;
use ratatui::style::Modifier;
use rustybuzz::Face;

/// A Font which can be used for rendering.
#[derive(Clone)]
pub struct Font<'a> {
    font: Face<'a>,
    advance: f32,
    id: u64,
}

impl<'a> Font<'a> {
    /// Create a new Font from data. Returns [`None`] if the font cannot
    /// be parsed.
    pub fn new(data: &'a [u8]) -> Option<Self> {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write(data);

        Face::from_slice(data, 0).map(|font| {
            let advance = font
                .glyph_hor_advance(font.glyph_index('m').unwrap_or_default())
                .unwrap_or_default() as f32;
            Self {
                font,
                advance,
                id: hasher.finish(),
            }
        })
    }
}

impl Font<'_> {
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn font(&'_ self) -> &'_ Face<'_> {
        &self.font
    }

    pub(crate) fn char_width(&self, height_px: u32) -> u32 {
        let scale = height_px as f32 / self.font.height() as f32;
        (self.advance * scale) as u32
    }
}

/// A collection of fonts to use for rendering. Supports font fallback.
///
/// It is recommended, but not required, that all fonts have the same/very
/// similar aspect ratio, or you may get unexpected results during rendering due
/// to fallback.
pub struct Fonts<'a> {
    char_width: u32,
    char_height: u32,

    last_resort: Font<'a>,

    regular: Vec<Font<'a>>,
    bold: Vec<Font<'a>>,
    italic: Vec<Font<'a>>,
    bold_italic: Vec<Font<'a>>,
}

impl<'a> Fonts<'a> {
    /// Create a new, empty set of fonts. The provided font will be used as a
    /// last-resort fallback if no other fonts can render a particular
    /// character. Rendering will attempt to fake bold/italic styles using this
    /// font where appropriate.
    ///
    /// The provided size_px will be the rendered height in pixels of all fonts
    /// in this collection.
    pub fn new(font: Font<'a>, size_px: u32) -> Self {
        Self {
            char_width: font.char_width(size_px),
            char_height: size_px,
            last_resort: font,
            regular: vec![],
            bold: vec![],
            italic: vec![],
            bold_italic: vec![],
        }
    }

    /// The height (in pixels) of all fonts.
    #[inline]
    pub fn height_px(&self) -> u32 {
        self.char_height
    }

    /// Change the height of all fonts in this collection to the specified
    /// height in pixels.
    pub fn set_size_px(&mut self, height_px: u32) {
        self.char_height = height_px;

        self.char_width = std::iter::once(&self.last_resort)
            .chain(self.regular.iter())
            .chain(self.bold.iter())
            .chain(self.italic.iter())
            .chain(self.bold_italic.iter())
            .map(|font| font.char_width(height_px))
            .min()
            .unwrap_or_default();
    }

    /// Add a collection of fonts for various styles. They will automatically be
    /// added to the appropriate fallback font list based on the font's
    /// bold/italic properties. Note that this will automatically organize fonts
    /// by relative width in order to optimize fallback rendering quality. The
    /// ordering of already provided fonts will remain unchanged.
    pub fn add_fonts(&mut self, fonts: impl IntoIterator<Item = Font<'a>>) {
        let bold_italic_len = self.bold_italic.len();
        let italic_len = self.italic.len();
        let bold_len = self.bold.len();
        let regular_len = self.regular.len();

        for font in fonts {
            if !font.font().is_monospaced() {
                warn!("Non monospace font used in add_fonts, this may cause unexpected rendering.");
            }

            self.char_width = self.char_width.min(font.char_width(self.char_height));
            if font.font().is_italic() && font.font().is_bold() {
                self.bold_italic.push(font);
            } else if font.font().is_italic() {
                self.italic.push(font);
            } else if font.font().is_bold() {
                self.bold.push(font);
            } else {
                self.regular.push(font);
            }
        }

        self.bold_italic[bold_italic_len..].sort_by_key(|font| font.char_width(self.char_height));
        self.italic[italic_len..].sort_by_key(|font| font.char_width(self.char_height));
        self.bold[bold_len..].sort_by_key(|font| font.char_width(self.char_height));
        self.regular[regular_len..].sort_by_key(|font| font.char_width(self.char_height));
    }

    /// Add a new collection of fonts for regular styled text. These fonts will
    /// come _after_ previously provided fonts in the fallback order.
    pub fn add_regular_fonts(&mut self, fonts: impl IntoIterator<Item = Font<'a>>) {
        self.char_width = self.char_width.min(Self::add_fonts_internal(
            &mut self.regular,
            fonts,
            self.char_height,
        ));
    }

    /// Add a new collection of fonts for bold styled text. These fonts will
    /// come _after_ previously provided fonts in the fallback order.
    ///
    /// You do not have to provide these for bold text to be supported. If no
    /// bold fonts are supplied, rendering will fallback to the regular fonts
    /// with fake bolding.
    pub fn add_bold_fonts(&mut self, fonts: impl IntoIterator<Item = Font<'a>>) {
        self.char_width = self.char_width.min(Self::add_fonts_internal(
            &mut self.bold,
            fonts,
            self.char_height,
        ));
    }

    /// Add a new collection of fonts for italic styled text. These fonts will
    /// come _after_ previously provided fonts in the fallback order.
    ///
    /// It is recommended, but not required, that you provide italic fonts if
    /// your application intends to make use of italics. If no italic fonts
    /// are supplied, rendering will fallback to the regular fonts with fake
    /// italics.
    pub fn add_italic_fonts(&mut self, fonts: impl IntoIterator<Item = Font<'a>>) {
        self.char_width = self.char_width.min(Self::add_fonts_internal(
            &mut self.italic,
            fonts,
            self.char_height,
        ));
    }

    /// Add a new collection of fonts for bold italic styled text. These fonts
    /// will come _after_ previously provided fonts in the fallback order.
    ///
    /// You do not have to provide these for bold text to be supported. If no
    /// bold fonts are supplied, rendering will fallback to the italic fonts
    /// with fake bolding.
    pub fn add_bold_italic_fonts(&mut self, fonts: impl IntoIterator<Item = Font<'a>>) {
        self.char_width = self.char_width.min(Self::add_fonts_internal(
            &mut self.bold_italic,
            fonts,
            self.char_height,
        ));
    }
}

impl<'a> Fonts<'a> {
    /// The minimum width (in pixels) across all fonts.
    pub(crate) fn min_width_px(&self) -> u32 {
        self.char_width
    }

    pub(crate) fn count(&self) -> usize {
        1 + self.bold.len() + self.italic.len() + self.bold_italic.len() + self.regular.len()
    }

    pub(crate) fn font_for_cell(&'_ self, cell: &Cell) -> (&'_ Font<'_>, bool, bool) {
        if cell.modifier.contains(Modifier::BOLD | Modifier::ITALIC) {
            self.select_font(
                cell.symbol(),
                self.bold_italic
                    .iter()
                    .map(|f| (f, false, false))
                    .chain(self.italic.iter().map(|f| (f, true, false)))
                    .chain(self.bold.iter().map(|f| (f, false, true)))
                    .chain(self.regular.iter().map(|f| (f, true, true))),
                true,
                true,
            )
        } else if cell.modifier.contains(Modifier::BOLD) {
            self.select_font(
                cell.symbol(),
                self.bold
                    .iter()
                    .map(|f| (f, false, false))
                    .chain(self.regular.iter().map(|f| (f, true, false))),
                true,
                false,
            )
        } else if cell.modifier.contains(Modifier::ITALIC) {
            self.select_font(
                cell.symbol(),
                self.italic
                    .iter()
                    .map(|f| (f, false, false))
                    .chain(self.regular.iter().map(|f| (f, false, true))),
                false,
                true,
            )
        } else {
            self.select_font(
                cell.symbol(),
                self.regular.iter().map(|f| (f, false, false)),
                false,
                false,
            )
        }
    }

    fn select_font<'fonts>(
        &'fonts self,
        cluster: &str,
        fonts: impl IntoIterator<Item = (&'fonts Font<'a>, bool, bool)>,
        last_resort_fake_bold: bool,
        last_resort_fake_italic: bool,
    ) -> (&'fonts Font<'a>, bool, bool) {
        let mut max = 0;
        let mut font = None;
        for (candidate, fake_bold, fake_italic) in fonts.into_iter().chain(std::iter::once((
            &self.last_resort,
            last_resort_fake_bold,
            last_resort_fake_italic,
        ))) {
            let (count, last_idx) =
                cluster
                    .chars()
                    .enumerate()
                    .fold((0, 0), |(mut count, _), (idx, ch)| {
                        count += usize::from(candidate.font().glyph_index(ch).is_some());
                        (count, idx)
                    });
            if count > max {
                max = count;
                font = Some((candidate, fake_bold, fake_italic));
            }

            if count == last_idx + 1 {
                break;
            }
        }

        *font.get_or_insert((
            &self.last_resort,
            last_resort_fake_bold,
            last_resort_fake_italic,
        ))
    }

    fn add_fonts_internal(
        target: &mut Vec<Font<'a>>,
        fonts: impl IntoIterator<Item = Font<'a>>,
        char_height: u32,
    ) -> u32 {
        let len = target.len();
        target.extend(fonts);

        target[len..]
            .iter()
            .map(|font| font.char_width(char_height))
            .min()
            .unwrap_or(u32::MAX)
    }
}

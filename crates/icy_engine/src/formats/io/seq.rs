use super::super::{apply_sauce_to_buffer, LoadData, SaveOptions};
use crate::{
    AttributedChar, BitFont, EditableScreen, Palette, Position, Result, Size, TextBuffer, TextPane, TextScreen, C128_LOWER, C16_LOWER, C16_UPPER,
    C64_DEFAULT_PALETTE, C64_SHIFTED, C64_UNSHIFTED, PET_LOWER, PET_UPPER, VIC20_LOWER, VIC20_UPPER,
};

/// The Commodore machines whose 40 column text screen can be edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetsciiMachine {
    C64,
    C128,
    Vic20,
    Pet,
    C16,
    /// The 80 column PETs (8032).
    Pet80,
    /// The C128's 80 column screen (VDC): the character set and blinking and underlining are
    /// chosen per character.
    C128Vdc,
}

impl PetsciiMachine {
    pub const ALL: [Self; 7] = [Self::C64, Self::C128, Self::C128Vdc, Self::Vic20, Self::Pet, Self::Pet80, Self::C16];

    /// The text screen in characters.
    pub fn screen_size(self) -> Size {
        match self {
            Self::Vic20 => Size::new(22, 23),
            Self::Pet80 | Self::C128Vdc => Size::new(80, 25),
            _ => Size::new(40, 25),
        }
    }

    /// Whether every character picks its character set, like the VDC's alternate set attribute.
    pub fn charset_per_character(self) -> bool {
        self == Self::C128Vdc
    }

    /// The colors, by the machine's color numbers: the C16's are luminance × 16 + hue.
    pub fn palette(self) -> Palette {
        match self {
            Self::C64 | Self::C128 => Palette::from_slice(&C64_DEFAULT_PALETTE),
            Self::Vic20 => Palette::from_slice(&VIC20_PALETTE),
            Self::Pet | Self::Pet80 => Palette::from_slice(&PET_PALETTE),
            Self::C16 => ted_palette(),
            Self::C128Vdc => Palette::from_slice(&VDC_PALETTE),
        }
    }

    /// How many of the colors characters can have: the VIC-20 has eight text colors, and the PET
    /// draws in its monitor's single color.
    pub fn text_colors(self) -> u32 {
        match self {
            Self::C64 | Self::C128 | Self::C128Vdc => 16,
            Self::Vic20 => 8,
            Self::Pet | Self::Pet80 => 2,
            Self::C16 => 128,
        }
    }

    /// The text and screen color the machine starts with.
    pub fn start_colors(self) -> (u32, u32) {
        match self {
            Self::C64 => (14, 6),
            // Light green on dark gray.
            Self::C128 => (13, 11),
            Self::Vic20 => (6, 1),
            Self::Pet | Self::Pet80 => (1, 0),
            // White on black.
            Self::C128Vdc => (15, 0),
            // Black on white, luminance 7.
            Self::C16 => (0, 0x71),
        }
    }

    /// The border color the machine starts with; the PETs and the VDC have no border.
    pub fn start_border(self) -> Option<u32> {
        match self {
            Self::C64 => Some(14),
            Self::C128 => Some(13),
            Self::Vic20 => Some(3),
            // Pink, luminance 6.
            Self::C16 => Some(0x6B),
            Self::Pet | Self::Pet80 | Self::C128Vdc => None,
        }
    }

    /// The colors the border can have: the VIC-20's are its eight text colors.
    pub fn border_colors(self) -> u32 {
        match self {
            Self::Vic20 => 8,
            _ => self.palette().len() as u32,
        }
    }

    /// Whether SEQ text sets colors: the PET has none.
    pub fn has_color_codes(self) -> bool {
        !matches!(self, Self::Pet | Self::Pet80)
    }
}

/// The VDC's RGBI colors.
const VDC_PALETTE: [crate::Color; 16] = [
    rgb(0x00, 0x00, 0x00),
    rgb(0x55, 0x55, 0x55),
    rgb(0x00, 0x00, 0xAA),
    rgb(0x55, 0x55, 0xFF),
    rgb(0x00, 0xAA, 0x00),
    rgb(0x55, 0xFF, 0x55),
    rgb(0x00, 0xAA, 0xAA),
    rgb(0x55, 0xFF, 0xFF),
    rgb(0xAA, 0x00, 0x00),
    rgb(0xFF, 0x55, 0x55),
    rgb(0xAA, 0x00, 0xAA),
    rgb(0xFF, 0x55, 0xFF),
    rgb(0xAA, 0x55, 0x00),
    rgb(0xFF, 0xFF, 0x55),
    rgb(0xAA, 0xAA, 0xAA),
    rgb(0xFF, 0xFF, 0xFF),
];

/// The VDC color the C128 shows for each C64 color code in 80 columns.
pub const VDC_COLORS_OF_C64: [u8; 16] = [0, 15, 8, 7, 11, 4, 2, 13, 10, 12, 9, 1, 6, 5, 3, 14];

/// `font` with every row twice, for screens whose pixels are twice as high as wide (80 columns).
fn doubled_rows(font: &BitFont, name: &str) -> BitFont {
    let mut bytes = Vec::with_capacity(256 * 16);
    for code in 0..256u32 {
        for row in &font.glyph(char::from_u32(code).unwrap_or(' ')).data[..8] {
            bytes.extend([*row, *row]);
        }
    }
    BitFont::create_8(name, 8, 16, &bytes)
}

/// The VIC-20's colors (PAL), as Petmate shows them.
const VIC20_PALETTE: [crate::Color; 16] = [
    rgb(0x00, 0x00, 0x00),
    rgb(0xff, 0xff, 0xff),
    rgb(0xae, 0x26, 0x27),
    rgb(0x6d, 0xef, 0xfe),
    rgb(0xb1, 0x40, 0xfe),
    rgb(0x5d, 0xe1, 0x39),
    rgb(0x33, 0x31, 0xfd),
    rgb(0xda, 0xd7, 0x29),
    rgb(0xc2, 0x57, 0x14),
    rgb(0xe4, 0xb1, 0x75),
    rgb(0xe1, 0x93, 0x94),
    rgb(0xa6, 0xf6, 0xfc),
    rgb(0xdd, 0xa0, 0xfe),
    rgb(0x98, 0xe3, 0x93),
    rgb(0x87, 0x8f, 0xfe),
    rgb(0xe3, 0xde, 0x87),
];

/// The PET's black screen and green phosphor.
const PET_PALETTE: [crate::Color; 2] = [rgb(0x00, 0x00, 0x00), rgb(0x41, 0xff, 0x00)];

/// The monitors a PET came with, for its text color.
pub const PET_PHOSPHORS: [(u8, u8, u8); 3] = [(0x41, 0xff, 0x00), (0xff, 0xff, 0xff), (0xff, 0xa8, 0x00)];

const fn rgb(r: u8, g: u8, b: u8) -> crate::Color {
    crate::Color { name: None, r, g, b }
}

/// The TED's (C16, Plus/4) PAL colors by hue and luminance, as Petmate shows them.
const TED_HUES: [[u32; 8]; 16] = [
    [0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000],
    [0x202020, 0x404040, 0x606060, 0x808080, 0x9f9f9f, 0xbfbfbf, 0xdfdfdf, 0xffffff],
    [0x651517, 0x722224, 0x7c2c2e, 0x8b3b3d, 0xad5d5f, 0xd18183, 0xe79799, 0xffcdcf],
    [0x004643, 0x045350, 0x0c5d5a, 0x1b6c69, 0x3d8e8b, 0x61b2af, 0x77c8c5, 0xadf2f0],
    [0x5b0a6a, 0x681777, 0x722181, 0x813090, 0xa352b2, 0xc776d6, 0xdd8cec, 0xfcc2ff],
    [0x005101, 0x085e09, 0x126813, 0x217722, 0x439944, 0x67bd68, 0x7dd37e, 0xb3f7b4],
    [0x202190, 0x2d2e9d, 0x3738a7, 0x4647b6, 0x6869d8, 0x8c8df5, 0xa2a3ff, 0xd8d9ff],
    [0x3a3a00, 0x474700, 0x515100, 0x606000, 0x828212, 0xa6a636, 0xbcbc4c, 0xecec82],
    [0x592300, 0x663000, 0x703a05, 0x804912, 0xa16b34, 0xc58f58, 0xdba56e, 0xfbdba4],
    [0x4c2f00, 0x593c00, 0x634600, 0x725503, 0x94771e, 0xb89b42, 0xceb158, 0xf5e68e],
    [0x1e4800, 0x2b5500, 0x355f00, 0x446e00, 0x669012, 0x8ab436, 0xa0ca4c, 0xd6f382],
    [0x661031, 0x731d3e, 0x7d2748, 0x8c3657, 0xae5879, 0xd27c9d, 0xe892b3, 0xffc8e7],
    [0x004b2d, 0x04583a, 0x0b6244, 0x1a7153, 0x3c9375, 0x60b799, 0x76cdaf, 0xacf4e5],
    [0x0b2f7e, 0x183c8b, 0x224695, 0x3155a4, 0x5377c6, 0x779bea, 0x8db1f6, 0xc3e6ff],
    [0x2d1995, 0x3a26a2, 0x4430ac, 0x533fbb, 0x7561dd, 0x9985f7, 0xaf9bff, 0xe5d1ff],
    [0x0e4e00, 0x1b5b00, 0x256500, 0x347404, 0x569620, 0x7aba44, 0x90d05a, 0xc6f690],
];

/// The TED's 128 colors, numbered luminance × 16 + hue like its color registers.
fn ted_palette() -> Palette {
    let colors: Vec<crate::Color> = (0..8)
        .flat_map(|luminance| {
            (0..16).map(move |hue| {
                let value = TED_HUES[hue][luminance];
                rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
            })
        })
        .collect();
    Palette::from_slice(&colors)
}

/// The two character sets: upper case with graphics (the machine starts with it) or lower and
/// upper case. A screen shows one of them at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetsciiCase {
    Upper,
    Lower,
}

/// The character set of `machine` in `case`, named so that [`petscii_charset`] recognizes it.
pub fn petscii_font(machine: PetsciiMachine, case: PetsciiCase) -> BitFont {
    let (mut font, name) = match (machine, case) {
        // The files are named after the shift key: shifted is upper case with graphics.
        (PetsciiMachine::C64, PetsciiCase::Upper) => (C64_SHIFTED.clone(), "C64 PETSCII upper"),
        (PetsciiMachine::C64, PetsciiCase::Lower) => (C64_UNSHIFTED.clone(), "C64 PETSCII lower"),
        (PetsciiMachine::C128, PetsciiCase::Upper) => (C64_SHIFTED.clone(), "C128 PETSCII upper"),
        (PetsciiMachine::C128, PetsciiCase::Lower) => (C128_LOWER.clone(), "C128 PETSCII lower"),
        (PetsciiMachine::Vic20, PetsciiCase::Upper) => (VIC20_UPPER.clone(), "VIC-20 PETSCII upper"),
        (PetsciiMachine::Vic20, PetsciiCase::Lower) => (VIC20_LOWER.clone(), "VIC-20 PETSCII lower"),
        (PetsciiMachine::Pet, PetsciiCase::Upper) => (PET_UPPER.clone(), "PET PETSCII upper"),
        (PetsciiMachine::Pet, PetsciiCase::Lower) => (PET_LOWER.clone(), "PET PETSCII lower"),
        (PetsciiMachine::C16, PetsciiCase::Upper) => (C16_UPPER.clone(), "C16 PETSCII upper"),
        (PetsciiMachine::C16, PetsciiCase::Lower) => (C16_LOWER.clone(), "C16 PETSCII lower"),
        (PetsciiMachine::Pet80, PetsciiCase::Upper) => return doubled_rows(&PET_UPPER, "PET 80 PETSCII upper"),
        (PetsciiMachine::Pet80, PetsciiCase::Lower) => return doubled_rows(&PET_LOWER, "PET 80 PETSCII lower"),
        (PetsciiMachine::C128Vdc, PetsciiCase::Upper) => return doubled_rows(&C64_SHIFTED, "C128 VDC PETSCII upper"),
        (PetsciiMachine::C128Vdc, PetsciiCase::Lower) => return doubled_rows(&C128_LOWER, "C128 VDC PETSCII lower"),
    };
    font.set_name(name);
    font
}

/// The machine and character set of a PETSCII screen, from the name of its font.
pub fn petscii_charset(buffer: &TextBuffer) -> (PetsciiMachine, PetsciiCase) {
    if let Some(crate::MachineMode::Petscii { machine, charset }) = buffer.machine_mode {
        return (machine, charset);
    }
    let name = buffer.font(0).map(|font| font.name().to_ascii_lowercase()).unwrap_or_default();
    let machine = if name.starts_with("c128 vdc") {
        PetsciiMachine::C128Vdc
    } else if name.starts_with("c128") {
        PetsciiMachine::C128
    } else if name.starts_with("pet 80") {
        PetsciiMachine::Pet80
    } else if name.starts_with("vic-20") {
        PetsciiMachine::Vic20
    } else if name.starts_with("pet") {
        PetsciiMachine::Pet
    } else if name.starts_with("c16") {
        PetsciiMachine::C16
    } else {
        PetsciiMachine::C64
    };
    let case = if name.contains("unshifted") || name.contains("lower") {
        PetsciiCase::Lower
    } else {
        PetsciiCase::Upper
    };
    (machine, case)
}

/// The explicit screen color, or the machine's start-up color for buffers without metadata.
pub fn petscii_background(buffer: &TextBuffer) -> u32 {
    buffer.background_color.unwrap_or_else(|| petscii_charset(buffer).0.start_colors().1)
}

/// A PETSCII screen `width` × `height` of spaces in `foreground` on `background`, with the
/// machine's character set and the C64 palette.
pub fn petscii_buffer(machine: PetsciiMachine, case: PetsciiCase, size: Size, foreground: u32, background: u32) -> TextBuffer {
    let mut buffer = TextBuffer::new(size);
    buffer.clear_font_table();
    let font = petscii_font(machine, case);
    buffer.set_font_dimensions(font.size());
    buffer.set_font(0, font);
    if machine.charset_per_character() {
        // Characters with the alternate set attribute use the lower case set.
        buffer.set_font(0, petscii_font(machine, PetsciiCase::Upper));
        buffer.set_font(1, petscii_font(machine, PetsciiCase::Lower));
    }
    buffer.palette = machine.palette();
    buffer.border_color = machine.start_border();
    buffer.background_color = Some(background);
    buffer.machine_mode = Some(crate::MachineMode::Petscii {
        machine,
        charset: if machine.charset_per_character() { PetsciiCase::Upper } else { case },
    });
    buffer.buffer_type = crate::BufferType::Petscii;
    buffer.terminal_state.is_terminal_buffer = false;
    let mut blank = AttributedChar::new(' ', crate::TextAttribute::default());
    blank.attribute.set_foreground(foreground);
    blank.attribute.set_background(background);
    for y in 0..size.height {
        for x in 0..size.width {
            buffer.layers[0].set_char((x, y), blank);
        }
    }
    buffer
}

/// The screen code typed for `ch` in the character set `case`: letters are upper case in the
/// graphics set, and both cases in the other.
pub fn petscii_screen_code(ch: char, case: PetsciiCase) -> Option<u8> {
    match ch {
        '@' => Some(0),
        'a'..='z' => Some(ch as u8 - b'a' + 1),
        'A'..='Z' if case == PetsciiCase::Lower => Some(ch as u8),
        'A'..='Z' => Some(ch as u8 - b'A' + 1),
        '[' => Some(0x1B),
        '£' => Some(0x1C),
        ']' => Some(0x1D),
        '↑' | '^' => Some(0x1E),
        '←' | '_' => Some(0x1F),
        ' '..='?' => Some(ch as u8),
        'π' if case == PetsciiCase::Upper => Some(0x5E),
        _ => None,
    }
}

/// SEQ bytes that set the text color, by C64 color index.
const COLOR_CODES: [u8; 16] = [0x90, 0x05, 0x1C, 0x9F, 0x9C, 0x1E, 0x1F, 0x9E, 0x81, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9B];
const REVERSE_ON: u8 = 0x12;
const REVERSE_OFF: u8 = 0x92;
const RETURN: u8 = 0x0D;
const CURSOR_LEFT: u8 = 0x9D;
const INSERT: u8 = 0x94;

/// A character to write: its screen code, the C64 color code of its color, and the C128's
/// per character attributes.
#[derive(Clone, Copy, Default)]
struct SeqCell {
    code: u8,
    color: usize,
    lower: bool,
    underline: bool,
    blink: bool,
}

/// The text color and modes the SEQ so far leaves the machine in.
#[derive(Default)]
struct SeqWriter {
    /// Whether the machine has colors to set.
    colors: bool,
    /// Whether the character set, underline and flashing are chosen per character (C128 VDC).
    attributes: bool,
    color: Option<usize>,
    reverse: bool,
    lower: bool,
    underline: bool,
    blink: bool,
}

impl SeqWriter {
    /// Prints `cell`, switching color, reverse and the VDC's attributes where they change.
    fn print(&mut self, result: &mut Vec<u8>, cell: SeqCell) {
        if self.colors && self.color != Some(cell.color) {
            result.push(COLOR_CODES[cell.color]);
            self.color = Some(cell.color);
        }
        if self.attributes {
            if cell.lower != self.lower {
                self.lower = cell.lower;
                result.push(if cell.lower { 0x0E } else { 0x8E });
            }
            if cell.underline != self.underline {
                self.underline = cell.underline;
                result.push(if cell.underline { 0x02 } else { 0x82 });
            }
            if cell.blink != self.blink {
                self.blink = cell.blink;
                result.push(if cell.blink { 0x0F } else { 0x8F });
            }
        }
        if (cell.code >= 0x80) != self.reverse {
            self.reverse = cell.code >= 0x80;
            result.push(if self.reverse { REVERSE_ON } else { REVERSE_OFF });
        }
        result.push(print_code(cell.code));
    }
}

/// The PETSCII byte that prints the screen code `code` (without its reverse bit).
fn print_code(code: u8) -> u8 {
    match code & 0x7F {
        code @ 0x00..=0x1F => code + 0x40,
        code @ 0x40..=0x5D => code + 0x80,
        0x5E => 0xFF,
        0x5F => 0xDF,
        code @ 0x60..=0x7F => code + 0x40,
        code => code,
    }
}

/// Writes the screen as SEQ: clear screen, the character set, then the rows with color and
/// reverse codes. Rows shorter than 40 columns end with RETURN; full rows wrap by themselves.
pub(crate) fn save_seq(buf: &TextBuffer, _options: &SaveOptions) -> Result<Vec<u8>> {
    if buf.buffer_type != crate::BufferType::Petscii {
        return Err(crate::EngineError::BufferTypeMismatch {
            expected: "Petscii".to_string(),
        });
    }
    let (machine, case) = petscii_charset(buf);
    let screen = machine.screen_size();
    let columns = screen.width;
    if buf.width() > columns {
        return Err(crate::EngineError::Generic(format!(
            "SEQ screens are {columns} columns wide, not {}",
            buf.width()
        )));
    }
    let mut result = vec![0x93, if case == PetsciiCase::Lower { 0x0E } else { 0x8E }];
    let vdc = machine == PetsciiMachine::C128Vdc;
    let cell = |x: i32, y: i32| {
        let ch = buf.char_at(Position::new(x, y));
        let code = if ch.is_visible() { u8::try_from(ch.ch as u32).unwrap_or(b' ') } else { b' ' };
        let foreground = ch.attribute.foreground() & 0x0F;
        // In 80 columns the C128 shows each C64 color code as a VDC color.
        let color = if vdc {
            VDC_COLORS_OF_C64.iter().position(|&vdc| u32::from(vdc) == foreground).unwrap_or(1)
        } else {
            foreground as usize
        };
        SeqCell {
            code,
            color,
            lower: vdc && ch.attribute.font_page() == 1,
            underline: vdc && ch.attribute.is_underlined(),
            blink: vdc && ch.attribute.is_blinking(),
        }
    };
    let row_length = |y: i32| (0..buf.width()).rposition(|x| cell(x, y).code != b' ').map_or(0, |last| last as i32 + 1);
    let height = (0..buf.height()).rposition(|y| row_length(y) > 0).map_or(0, |last| last as i32 + 1);
    let mut writer = SeqWriter {
        colors: machine.has_color_codes(),
        attributes: vdc,
        lower: case == PetsciiCase::Lower,
        ..SeqWriter::default()
    };
    for y in 0..height {
        let length = row_length(y);
        let full = buf.width() == columns && length == columns;
        if full && y == screen.height - 1 {
            // Printing into the lower right corner scrolls the screen: print the last character
            // one column early, then insert a space in front of it for the one before.
            for x in 0..columns - 2 {
                writer.print(&mut result, cell(x, y));
            }
            writer.print(&mut result, cell(columns - 1, y));
            result.extend([CURSOR_LEFT, INSERT]);
            writer.print(&mut result, cell(columns - 2, y));
            break;
        }
        for x in 0..length {
            writer.print(&mut result, cell(x, y));
        }
        if !full && y + 1 < height {
            result.push(RETURN);
            // RETURN ends reverse mode.
            writer.reverse = false;
        }
    }
    Ok(result)
}

/// Shows the whole screen in one character set, as the machine does: the one most characters
/// were printed in.
fn single_charset(result: &mut TextScreen) {
    let buffer = &mut result.buffer;
    let mut counts = [0usize; 2];
    for layer in &buffer.layers {
        for line in &layer.lines {
            for ch in line.chars.iter().filter(|ch| ch.is_visible() && ch.ch != ' ') {
                counts[usize::from(ch.attribute.font_page() == 1)] += 1;
            }
        }
    }
    let case = if counts[0] > counts[1] { PetsciiCase::Lower } else { PetsciiCase::Upper };
    buffer.clear_font_table();
    buffer.set_font(0, petscii_font(PetsciiMachine::C64, case));
    for layer in &mut buffer.layers {
        for line in &mut layer.lines {
            for ch in &mut line.chars {
                ch.attribute.set_font_page(0);
            }
        }
    }
    result.caret_mut().set_font_page(0);
}

pub(crate) fn load_seq(data: &[u8], _load_data_opt: Option<&LoadData>, sauce_opt: Option<&icy_sauce::SauceRecord>) -> Result<TextScreen> {
    let mut result = TextScreen::new((40, 25));

    result.buffer.clear_font_table();
    result.buffer.set_font(0, C64_UNSHIFTED.clone());
    result.buffer.set_font(1, C64_SHIFTED.clone());
    result.buffer.set_font_dimensions(Size::new(8, 8)); // C64 uses 8x8 fonts

    result.buffer.palette = Palette::from_slice(&C64_DEFAULT_PALETTE);
    result.buffer.buffer_type = crate::BufferType::Petscii;
    result.buffer.background_color = Some(0);
    result.buffer.terminal_state.is_terminal_buffer = false;

    // Apply SAUCE settings early
    if let Some(sauce) = sauce_opt {
        apply_sauce_to_buffer(&mut result.buffer, sauce);
    }

    seq_prepare(&mut result);
    crate::load_with_parser(&mut result, &mut icy_parser_core::PetsciiParser::default(), data, true, 25)?;
    single_charset(&mut result);
    let (machine, charset) = petscii_charset(&result.buffer);
    result.buffer.machine_mode = Some(crate::MachineMode::Petscii { machine, charset });
    Ok(result)
}

pub fn seq_prepare(result: &mut dyn EditableScreen) {
    for y in 0..result.height() {
        for x in 0..result.width() {
            let mut ch = AttributedChar::default();
            ch.attribute.set_foreground(7);
            ch.attribute.set_background(0);
            result.set_char((x, y).into(), ch);
        }
    }
    result.caret_mut().set_foreground(7);
    result.caret_mut().set_background(0);
    result.caret_mut().set_font_page(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileFormat;

    fn round_trip(buffer: &TextBuffer) -> TextBuffer {
        let mut options = SaveOptions::default();
        options.preprocess.optimize_colors = false;
        let bytes = FileFormat::Petscii.to_bytes(buffer, &options).unwrap();
        FileFormat::Petscii.from_bytes(&bytes, None).unwrap().screen.buffer
    }

    #[test]
    fn every_screen_code_and_color_survives_saving_and_loading() {
        for case in [PetsciiCase::Upper, PetsciiCase::Lower] {
            let mut buffer = petscii_buffer(PetsciiMachine::C64, case, Size::new(40, 8), 14, 0);
            for code in 0..256u32 {
                let mut ch = AttributedChar::new(char::from_u32(code).unwrap(), crate::TextAttribute::default());
                ch.attribute.set_foreground(code % 16);
                buffer.layers[0].set_char((code as i32 % 40, code as i32 / 40), ch);
            }
            let loaded = round_trip(&buffer);
            assert_eq!(petscii_charset(&loaded), (PetsciiMachine::C64, case), "the character set is kept");
            for code in 0..256u32 {
                let position = Position::new(code as i32 % 40, code as i32 / 40);
                let ch = loaded.char_at(position);
                assert_eq!((ch.ch as u32, ch.attribute.foreground()), (code, code % 16), "{case:?} code {code:#04X}");
            }
        }
    }

    #[test]
    fn short_rows_end_with_return_and_the_lower_right_corner_does_not_scroll() {
        let mut buffer = petscii_buffer(PetsciiMachine::C64, PetsciiCase::Upper, Size::new(40, 25), 1, 0);
        buffer.layers[0].set_char((0, 0), AttributedChar::new('\u{1}', crate::TextAttribute::default()));
        for x in 0..40 {
            let mut ch = AttributedChar::new(char::from(0x81 + (x % 26) as u8), crate::TextAttribute::default());
            ch.attribute.set_foreground(x as u32 % 16);
            buffer.layers[0].set_char((x, 24), ch);
        }
        let bytes = FileFormat::Petscii.to_bytes(&buffer, &SaveOptions::default()).unwrap();
        assert!(
            bytes.ends_with(&[CURSOR_LEFT, INSERT, COLOR_CODES[38 % 16], print_code(0x81 + 38 % 26)]),
            "the last row ends with the insert trick: {bytes:02X?}"
        );
        let loaded = round_trip(&buffer);
        assert_eq!(loaded.char_at((0, 0).into()).ch as u32, 1, "nothing scrolled away");
        for x in 0..40 {
            let (saved, read) = (buffer.char_at((x, 24).into()), loaded.char_at((x, 24).into()));
            assert_eq!((read.ch, read.attribute.foreground()), (saved.ch, saved.attribute.foreground()), "column {x}");
        }
    }

    #[test]
    fn typing_follows_the_character_set() {
        assert_eq!(petscii_screen_code('A', PetsciiCase::Upper), Some(1));
        assert_eq!(petscii_screen_code('a', PetsciiCase::Upper), Some(1));
        assert_eq!(petscii_screen_code('A', PetsciiCase::Lower), Some(65));
        assert_eq!(petscii_screen_code('a', PetsciiCase::Lower), Some(1));
        assert_eq!(petscii_screen_code('£', PetsciiCase::Lower), Some(0x1C));
        assert_eq!(petscii_screen_code('7', PetsciiCase::Upper), Some(b'7'));
        assert_eq!(petscii_screen_code('€', PetsciiCase::Upper), None);
    }

    #[test]
    fn a_loaded_screen_shows_one_character_set() {
        let loaded = FileFormat::Petscii.from_bytes(b"\x0eHELLO\x8eX", None).unwrap().screen.buffer;
        assert_eq!(petscii_charset(&loaded).1, PetsciiCase::Lower, "most characters were printed in lower case");
        assert_eq!(loaded.font_count(), 1);
    }
}

#[cfg(test)]
mod machine_tests {
    use super::*;
    use crate::FileFormat;

    #[test]
    fn every_machine_has_its_screen_font_and_colors() {
        for machine in PetsciiMachine::ALL {
            for case in [PetsciiCase::Upper, PetsciiCase::Lower] {
                let (foreground, background) = machine.start_colors();
                let buffer = petscii_buffer(machine, case, machine.screen_size(), foreground, background);
                // The VDC has both sets, upper case first.
                let expected = if machine.charset_per_character() { PetsciiCase::Upper } else { case };
                assert_eq!(petscii_charset(&buffer), (machine, expected));
                assert!(
                    foreground < machine.text_colors() && (background as usize) < buffer.palette.len(),
                    "{machine:?}"
                );
            }
        }
        assert_eq!(PetsciiMachine::C16.palette().len(), 128);
        assert_eq!(PetsciiMachine::C16.palette().rgb(0x71), (0xff, 0xff, 0xff), "luminance 7 white");
        assert_eq!(PetsciiMachine::Vic20.screen_size(), Size::new(22, 23));
        for machine in [PetsciiMachine::Pet80, PetsciiMachine::C128Vdc] {
            let font = petscii_font(machine, PetsciiCase::Upper);
            assert_eq!(font.size(), Size::new(8, 16), "80 column pixels are twice as high as wide");
            assert_eq!(machine.screen_size(), Size::new(80, 25));
        }
    }

    #[test]
    fn vdc_seq_translates_colors_and_writes_attributes_per_character() {
        let mut buffer = petscii_buffer(PetsciiMachine::C128Vdc, PetsciiCase::Upper, Size::new(80, 25), 15, 0);
        let mut red = crate::TextAttribute::default();
        red.set_foreground(8);
        buffer.layers[0].set_char((0, 0), AttributedChar::new('\u{1}', red));
        let mut lower = red;
        lower.set_font_page(1);
        lower.set_is_underlined(true);
        lower.set_is_blinking(true);
        buffer.layers[0].set_char((1, 0), AttributedChar::new('\u{1}', lower));
        buffer.layers[0].set_char((2, 0), AttributedChar::new('\u{1}', red));
        let bytes = FileFormat::Petscii.to_bytes(&buffer, &SaveOptions::default()).unwrap();
        // VDC red is the C64's red color code; then lower case, underline and flashing on and off.
        assert_eq!(bytes, [0x93, 0x8E, 0x1C, 0x41, 0x0E, 0x02, 0x0F, 0x41, 0x8E, 0x82, 0x8F, 0x41]);
    }

    #[test]
    fn vic20_rows_wrap_at_22_columns_and_the_pet_has_no_colors() {
        let mut buffer = petscii_buffer(PetsciiMachine::Vic20, PetsciiCase::Upper, Size::new(22, 23), 2, 1);
        for x in 0..22 {
            buffer.layers[0].set_char((x, 0), AttributedChar::new('\u{1}', crate::TextAttribute::default()));
            buffer.layers[0].set_char((x, 22), AttributedChar::new('\u{2}', crate::TextAttribute::default()));
        }
        let bytes = FileFormat::Petscii.to_bytes(&buffer, &SaveOptions::default()).unwrap();
        let returns = bytes.iter().filter(|&&byte| byte == RETURN).count();
        assert_eq!(returns, 21, "the full first row wraps by itself, the 21 empty ones end with RETURN");
        assert!(bytes.ends_with(&[CURSOR_LEFT, INSERT, print_code(2)]), "the lower right corner of row 23");

        let mut buffer = petscii_buffer(PetsciiMachine::Pet, PetsciiCase::Upper, Size::new(40, 25), 1, 0);
        buffer.layers[0].set_char((0, 0), AttributedChar::new('\u{1}', crate::TextAttribute::default()));
        let bytes = FileFormat::Petscii.to_bytes(&buffer, &SaveOptions::default()).unwrap();
        assert_eq!(bytes, [0x93, 0x8E, print_code(1)]);
    }
}

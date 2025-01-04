use std::collections::HashMap;
use std::path::Path;

use codepages::tables::CP437_TO_UNICODE;
use icy_sauce::char_caps::ContentType;
use icy_sauce::SauceInformation;

use crate::ansi::constants::COLOR_OFFSETS;
use crate::ansi::MusicOption;
use crate::{
    analyze_font_usage, parse_with_parser, parsers, BitFont, Buffer, BufferFeatures, OutputFormat, Rectangle, Tag, TextPane, ANSI_FONTS, DOS_DEFAULT_PALETTE,
    XTERM_256_PALETTE,
};
use crate::{Color, TextAttribute};

use super::SaveOptions;

#[derive(Default)]
pub(crate) struct Ansi {
    pub ansi_music: Option<MusicOption>,
}

impl OutputFormat for Ansi {
    fn get_file_extension(&self) -> &str {
        "ans"
    }

    fn get_alt_extensions(&self) -> Vec<String> {
        vec!["ice".to_string(), "diz".to_string()]
    }

    fn get_name(&self) -> &str {
        "Ansi"
    }

    fn analyze_features(&self, _features: &BufferFeatures) -> String {
        String::new()
    }

    fn to_bytes(&self, buf: &crate::Buffer, options: &SaveOptions) -> anyhow::Result<Vec<u8>> {
        let mut result = Vec::new();

        let mut gen = StringGenerator::new(options.clone());
        gen.tags = buf.tags.clone();

        gen.screen_prep(buf);
        gen.generate(buf, buf);
        gen.screen_end(buf);
        gen.add_sixels(buf);
        result.extend(gen.get_data());

        if options.save_sauce {
            buf.write_sauce_info(icy_sauce::SauceDataType::Character, ContentType::Ansi, &mut result)?;
        }
        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], sauce_opt: Option<SauceInformation>) -> anyhow::Result<crate::Buffer> {
        let mut result: Buffer = Buffer::new((80, 25));
        result.is_terminal_buffer = false;
        result.file_name = Some(file_name.into());
        if let Some(sauce) = sauce_opt {
            result.load_sauce(sauce);
        }
        let mut parser = parsers::ansi::Parser::default();
        if let Some(music) = self.ansi_music {
            parser.ansi_music = music;
        }
        parser.bs_is_ctrl_char = false;
        let (text, is_unicode) = crate::convert_ansi_to_utf8(data);
        if is_unicode {
            result.buffer_type = crate::BufferType::Unicode;
        }
        parse_with_parser(&mut result, &mut parser, &text, true)?;
        Ok(result)
    }
}
#[derive(Debug)]
struct CharCell {
    ch: char,
    sgr: Vec<u8>,
    sgr_tc: Vec<u8>,
    font_page: usize,
    cur_state: AnsiState,
}

pub struct StringGenerator {
    output: Vec<u8>,
    options: SaveOptions,
    last_line_break: usize,
    max_output_line_length: usize,
    extended_color_hash: HashMap<(u8, u8, u8), u8>,

    pub line_offsets: Vec<usize>,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone)]
struct AnsiState {
    pub is_bold: bool,
    pub is_blink: bool,
    pub is_faint: bool,
    pub is_italic: bool,
    pub is_underlined: bool,
    pub is_double_underlined: bool,
    pub is_crossed_out: bool,
    pub is_concealed: bool,

    pub fg_idx: u32,
    pub fg: Color,

    pub bg_idx: u32,
    pub bg: Color,
}

impl StringGenerator {
    pub fn new(options: SaveOptions) -> Self {
        let max_output_line_length = options.output_line_length.unwrap_or(usize::MAX);
        let mut output = Vec::new();

        if options.modern_terminal_output {
            // write UTF-8 BOM as unicode indicator.
            output.extend([0xEF, 0xBB, 0xBF]);
        }

        let mut extended_color_hash = HashMap::new();
        if options.use_extended_colors {
            for (i, (_, col)) in XTERM_256_PALETTE.iter().enumerate() {
                extended_color_hash.insert(col.get_rgb(), i as u8);
            }
        }

        Self {
            output,
            options,
            last_line_break: 0,
            max_output_line_length,
            extended_color_hash,
            line_offsets: Vec::new(),
            tags: Vec::new(),
        }
    }

    fn get_color(&self, buf: &Buffer, attr: TextAttribute, mut state: AnsiState) -> (AnsiState, Vec<u8>, Vec<u8>) {
        let mut sgr = Vec::new();
        let mut sgr_tc = Vec::new();

        let fg = attr.get_foreground();
        let cur_fore_color = buf.palette.get_color(fg);
        let cur_fore_rgb = cur_fore_color.get_rgb();

        let bg = attr.get_background();
        let cur_back_color = buf.palette.get_color(bg);
        let cur_back_rgb = cur_back_color.get_rgb();

        let mut fore_idx = DOS_DEFAULT_PALETTE.iter().position(|c| c.get_rgb() == cur_fore_rgb);
        let mut back_idx = DOS_DEFAULT_PALETTE.iter().position(|c| c.get_rgb() == cur_back_rgb);

        let mut is_bold = attr.is_bold();
        let mut is_blink = attr.is_blinking();
        let is_faint = attr.is_faint();
        let is_italic = attr.is_italic();
        let is_underlined = attr.is_underlined();
        let is_double_underlined = attr.is_double_underlined();
        let is_crossed_out = attr.is_crossed_out();
        let is_concealed = attr.is_concealed();

        if let Some(idx) = fore_idx {
            if idx < 8 {
                is_bold = false;
            } else if idx > 7 && idx < 16 {
                is_bold = true;
                fore_idx = Some(idx - 8);
            }
        }

        match buf.ice_mode {
            crate::IceMode::Unlimited => {
                if let Some(idx) = back_idx {
                    if idx > 7 {
                        back_idx = None;
                    }
                }
            }
            crate::IceMode::Blink => {
                if let Some(idx) = back_idx {
                    if idx > 7 && idx < 16 {
                        back_idx = None;
                    }
                }
            }
            crate::IceMode::Ice => {
                if let Some(idx) = back_idx {
                    if idx < 8 {
                        is_blink = false | attr.is_blinking();
                    } else if idx > 7 && idx < 16 {
                        is_blink = true;
                        back_idx = Some(idx - 8);
                    }
                }
            }
        }

        if !is_bold && state.is_bold
            || !is_blink && state.is_blink
            || !is_italic && state.is_italic
            || !is_faint && state.is_faint
            || !is_underlined && state.is_underlined
            || !is_underlined && state.is_underlined
            || !is_double_underlined && state.is_double_underlined
            || !is_crossed_out && state.is_crossed_out
            || !is_concealed && state.is_concealed
            || is_bold && !state.is_bold && !DOS_DEFAULT_PALETTE.iter().any(|c| c.get_rgb() == state.fg.get_rgb())
        // special case if bold changes but fore color is custom rgb - color needs to reset
        {
            sgr.push(0);
            state.is_bold = false;
            state.is_blink = false;
            state.is_italic = false;
            state.is_faint = false;
            state.is_underlined = false;
            state.is_double_underlined = false;
            state.is_crossed_out = false;

            state.fg_idx = 7;
            state.fg = DOS_DEFAULT_PALETTE[7].clone();

            state.bg_idx = 0;
            state.bg = DOS_DEFAULT_PALETTE[0].clone();
        }

        if is_bold && !state.is_bold {
            sgr.push(1);
            state.fg_idx += 8;
            if state.fg_idx < 16 {
                state.fg = DOS_DEFAULT_PALETTE[state.fg_idx as usize].clone();
            }
            state.is_bold = true;
        }
        if is_faint && !state.is_faint {
            sgr.push(2);
            state.is_faint = true;
        }
        if is_italic && !state.is_italic {
            sgr.push(3);
            state.is_italic = true;
        }
        if is_underlined && !state.is_underlined {
            sgr.push(4);
            state.is_underlined = true;
        }

        if is_blink && !state.is_blink {
            sgr.push(5);
            state.is_blink = true;
        }

        if is_concealed && !state.is_concealed {
            sgr.push(8);
            state.is_blink = true;
        }

        if is_crossed_out && !state.is_crossed_out {
            sgr.push(9);
            state.is_crossed_out = true;
        }

        if is_double_underlined && !state.is_double_underlined {
            sgr.push(21);
            state.is_double_underlined = true;
        }

        if cur_fore_rgb != state.fg.get_rgb() {
            if let Some(fg_idx) = fore_idx {
                sgr.push(COLOR_OFFSETS[fg_idx] + 30);
            } else if let Some(ext_color) = self.extended_color_hash.get(&cur_fore_rgb) {
                sgr.push(38);
                sgr.push(5);
                sgr.push(*ext_color);
            } else {
                sgr_tc.push(1);
                sgr_tc.push(cur_fore_rgb.0);
                sgr_tc.push(cur_fore_rgb.1);
                sgr_tc.push(cur_fore_rgb.2);
            }
            state.fg_idx = fg;
            state.fg = cur_fore_color;
        }
        if cur_back_rgb != state.bg.get_rgb() {
            if let Some(bg_idx) = back_idx {
                sgr.push(COLOR_OFFSETS[bg_idx] + 40);
                state.bg_idx = bg_idx as u32;
            } else if let Some(ext_color) = self.extended_color_hash.get(&cur_back_rgb) {
                sgr.push(48);
                sgr.push(5);
                sgr.push(*ext_color);
            } else {
                sgr_tc.push(0);
                sgr_tc.push(cur_back_rgb.0);
                sgr_tc.push(cur_back_rgb.1);
                sgr_tc.push(cur_back_rgb.2);
                state.bg_idx = bg;
            }
            state.bg = cur_back_color;
        }
        (state, sgr, sgr_tc)
    }

    fn generate_cells<T: TextPane>(&self, buf: &Buffer, layer: &T, area: Rectangle, font_map: &HashMap<usize, usize>) -> Vec<Vec<CharCell>> {
        let mut result = Vec::new();
        let mut state = AnsiState {
            is_bold: false,
            is_blink: false,
            is_italic: false,
            is_faint: false,
            is_underlined: false,
            is_double_underlined: false,
            is_crossed_out: false,
            is_concealed: false,
            fg_idx: 7,
            fg: DOS_DEFAULT_PALETTE[7].clone(),
            bg: DOS_DEFAULT_PALETTE[0].clone(),
            bg_idx: 0,
        };
        for y in area.y_range() {
            let mut line = Vec::new();

            if self.options.longer_terminal_output {
                if let Some(skip_lines) = &self.options.skip_lines {
                    if skip_lines.contains(&(y as usize)) {
                        result.push(line);
                        continue;
                    }
                }
            }

            let mut len = if self.options.compress && !self.options.preserve_line_length {
                let mut last = area.get_width() - 1;
                let last_attr = layer.get_char((last, y)).attribute;
                if last_attr.background_color == 0 {
                    while last > area.left() {
                        let c = layer.get_char((last, y));

                        if c.ch != ' ' && c.ch != 0xFF as char && c.ch != 0 as char {
                            break;
                        }
                        if c.attribute != last_attr {
                            break;
                        }
                        last -= 1;
                    }
                }
                let last = last + 1;
                if last >= area.get_width() - 1 {
                    // don't compress if we have only one char, since eol are 2 chars
                    area.get_width()
                } else {
                    last
                }
            } else {
                area.get_width()
            };

            // previewlen == 0 tags are invisible, so they need to be checked.
            for t in self.tags.iter() {
                if t.is_enabled && t.position.y == y as i32 {
                    len = len.max(t.position.x + t.len() as i32);
                }
            }
            let mut x = 0;
            while x < len {
                let mut found_tag = false;
                for t in self.tags.iter() {
                    if t.is_enabled && t.position.y == y as i32 && t.position.x == x as i32 {
                        for ch in t.replacement_value.chars() {
                            line.push(CharCell {
                                ch,
                                sgr: Vec::new(),
                                sgr_tc: Vec::new(),
                                font_page: 0,
                                cur_state: state.clone(),
                            });
                        }
                        x += t.len() as i32;
                        found_tag = true;
                        break;
                    }
                }
                if found_tag {
                    continue;
                }

                let ch = layer.get_char((x, y));
                if ch.is_visible() {
                    let (new_state, sgr, sgr_tc) = self.get_color(buf, ch.attribute, state);
                    state = new_state;
                    line.push(CharCell {
                        ch: ch.ch,
                        sgr,
                        sgr_tc,
                        font_page: *font_map.get(&ch.get_font_page()).unwrap(),
                        cur_state: state.clone(),
                    });
                } else {
                    line.push(CharCell {
                        ch: ' ',
                        sgr: Vec::new(),
                        sgr_tc: Vec::new(),
                        font_page: *font_map.get(&ch.get_font_page()).unwrap(),
                        cur_state: state.clone(),
                    });
                }
                x += 1;
            }

            result.push(line);
        }
        result
    }

    fn generate_ansi_font_map(buf: &Buffer) -> HashMap<usize, usize> {
        let mut font_map = HashMap::new();

        let mut ansi_fonts = Vec::new();
        for i in 0..ANSI_FONTS {
            ansi_fonts.push(BitFont::from_ansi_font_page(i).unwrap());
        }
        for (page, font) in buf.font_iter() {
            let mut to_page = *page;
            for (i, ansi_font) in ansi_fonts.iter().enumerate() {
                if ansi_font.get_checksum() == font.get_checksum() {
                    to_page = i;
                    break;
                }
            }
            font_map.insert(*page, to_page);
        }

        font_map
    }

    pub fn screen_prep(&mut self, buf: &Buffer) {
        if matches!(buf.ice_mode, crate::IceMode::Ice) {
            self.push_result(&mut b"\x1b[?33h".to_vec());
        }

        match self.options.screen_preparation {
            super::ScreenPreperation::None => {}
            super::ScreenPreperation::ClearScreen => {
                self.push_result(&mut b"\x1b[2J".to_vec());
            }
            super::ScreenPreperation::Home => {
                self.push_result(&mut b"\x1b[1;1H".to_vec());
            }
        }
    }

    pub fn screen_end(&mut self, buf: &Buffer) {
        if matches!(buf.ice_mode, crate::IceMode::Ice) {
            self.push_result(&mut b"\x1b[?33l".to_vec());
        }
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn generate<T: TextPane>(&mut self, buf: &Buffer, layer: &T) {
        let mut result = Vec::new();

        let used_fonts = analyze_font_usage(buf);
        for font_slot in used_fonts {
            if font_slot >= 100 {
                if let Some(font) = buf.get_font(font_slot) {
                    result.extend_from_slice(font.encode_as_ansi(font_slot).as_bytes());
                }
            }
        }
        let font_map = StringGenerator::generate_ansi_font_map(buf);
        let cells = self.generate_cells(buf, layer, layer.get_rectangle(), &font_map);
        let mut cur_font_page = 0;

        let mut is_first_output_line = true;

        for (y, line) in cells.iter().enumerate() {
            let mut x = 0;

            if !self.output.is_empty() {
                self.line_offsets.push(self.output.len());
            }

            if self.options.longer_terminal_output {
                if let Some(skip_lines) = &self.options.skip_lines {
                    if skip_lines.contains(&y) {
                        continue;
                    }
                }
                if is_first_output_line {
                    is_first_output_line = false;
                    result.extend_from_slice(b"\x1b[0m");
                }
                result.extend_from_slice(b"\x1b[");
                result.extend_from_slice((y + 1).to_string().as_bytes());
                result.push(b'H');
                self.push_result(&mut result);
            }

            let len = line.len();
            while x < len {
                let cell = &line[x];
                if cur_font_page != cell.font_page && !self.options.modern_terminal_output {
                    cur_font_page = cell.font_page;
                    result.extend_from_slice(b"\x1b[0;");
                    result.extend_from_slice(cur_font_page.to_string().as_bytes());
                    result.extend_from_slice(b" D");
                    self.push_result(&mut result);
                }

                if !cell.sgr.is_empty() {
                    result.extend_from_slice(b"\x1b[");
                    for i in 0..cell.sgr.len() - 1 {
                        result.extend_from_slice(cell.sgr[i].to_string().as_bytes());
                        result.push(b';');
                    }
                    result.extend_from_slice(cell.sgr.last().unwrap().to_string().as_bytes());
                    result.push(b'm');
                    self.push_result(&mut result);
                }
                let mut idx = 0;
                while idx < cell.sgr_tc.len() {
                    result.extend_from_slice(b"\x1b[");
                    for i in 0..3 {
                        result.extend_from_slice(cell.sgr_tc[idx + i].to_string().as_bytes());
                        result.push(b';');
                    }
                    result.extend_from_slice(cell.sgr_tc[idx + 3].to_string().as_bytes());
                    result.push(b't');
                    self.push_result(&mut result);
                    idx += 4;
                }

                let cell_char = if self.options.modern_terminal_output {
                    if cell.ch == '\0' {
                        vec![b' ']
                    } else {
                        let uni_ch = CP437_TO_UNICODE[cell.ch as usize].to_string();
                        uni_ch.as_bytes().to_vec()
                    }
                } else if StringGenerator::CONTROL_CHARS.contains(cell.ch) {
                    match self.options.control_char_handling {
                        crate::ControlCharHandling::Ignore => {
                            vec![cell.ch as u8]
                        }
                        crate::ControlCharHandling::IcyTerm => {
                            vec![b'\x1B', cell.ch as u8]
                        }
                        crate::ControlCharHandling::FilterOut => {
                            vec![b'.']
                        }
                    }
                } else {
                    vec![cell.ch as u8]
                };

                if self.options.compress {
                    let mut rle = x + 1;
                    while rle < len {
                        if line[rle].ch != line[x].ch || !line[rle].sgr.is_empty() || !line[rle].sgr_tc.is_empty() || line[rle].font_page != line[x].font_page {
                            break;
                        }
                        rle += 1;
                    }
                    // rle is always >= x + 1 but "x - 1" may overflow.
                    rle -= 1;
                    rle -= x;
                    if self.options.use_cursor_forward && line[x].ch == ' ' && line[x].cur_state.bg_idx == 0 && !line[x].cur_state.is_blink {
                        let fmt = &format!("\x1B[{}C", rle + 1);
                        let output = fmt.as_bytes();
                        if output.len() <= rle {
                            self.push_result(&mut result);
                            result.extend_from_slice(output);
                            self.push_result(&mut result);
                            x += rle + 1;
                            continue;
                        }
                    }
                    if self.options.use_repeat_sequences {
                        let fmt = &format!("\x1B[{rle}b");
                        let output = fmt.as_bytes();
                        if output.len() <= rle {
                            self.push_result(&mut result);
                            result.extend_from_slice(&cell_char);
                            result.extend_from_slice(output);
                            self.push_result(&mut result);
                            x += rle + 1;
                            continue;
                        }
                    }
                }

                result.extend_from_slice(&cell_char);
                self.push_result(&mut result);

                x += 1;
            }

            if !self.options.longer_terminal_output {
                if self.options.modern_terminal_output {
                    result.extend_from_slice(b"\x1b[0m");
                    result.push(10);
                    self.last_line_break = result.len();
                } else if x < layer.get_width() as usize && y + 1 < layer.get_height() as usize {
                    if self.options.compress && x + 1 >= layer.get_width() as usize {
                        // if it's shorter to line break with 1 space, do that
                        result.push(b' ');
                    } else {
                        result.push(13);
                        result.push(10);
                    }
                    self.last_line_break = result.len();
                }
            }
        }
    }

    const CONTROL_CHARS: &'static str = "\x1b\x07\x08\x09\x0C\x7F\r\n";

    pub fn add_sixels(&mut self, buf: &Buffer) {
        for layer in &buf.layers {
            for sixel in &layer.sixels {
                match icy_sixel::sixel_string(
                    &sixel.picture_data,
                    sixel.get_width(),
                    sixel.get_height(),
                    icy_sixel::PixelFormat::RGBA8888,
                    icy_sixel::DiffusionMethod::None,
                    icy_sixel::MethodForLargest::Auto,
                    icy_sixel::MethodForRep::Auto,
                    icy_sixel::Quality::AUTO,
                ) {
                    Err(err) => log::error!("{err}"),
                    Ok(data) => {
                        let p = layer.get_offset() + sixel.position;
                        self.output.extend(format!("\x1b[{};{}H", p.y + 1, p.x + 1).as_bytes());
                        self.output.extend(data.as_bytes());
                    }
                }
            }
        }
    }

    pub fn get_data(&self) -> &[u8] {
        &self.output
    }

    fn push_result(&mut self, result: &mut Vec<u8>) {
        if self.output.len() + result.len() - self.last_line_break > self.max_output_line_length {
            self.output.extend_from_slice(b"\x1b[s");
            self.output.push(13);
            self.output.push(10);
            self.last_line_break = self.output.len();
            self.output.extend_from_slice(b"\x1b[u");
        }
        self.output.append(result);
        result.clear();
    }
}

pub fn get_save_sauce_default_ans(buf: &Buffer) -> (bool, String) {
    if buf.get_width() != 80 {
        return (true, "width != 80".to_string());
    }

    if buf.has_sauce() {
        return (true, String::new());
    }

    (false, String::new())
}
/*
#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use crate::{Buffer, SaveOptions, StringGenerator, TextPane};

    fn is_hidden(entry: &walkdir::DirEntry) -> bool {
        entry
            .file_name()
            .to_str()
            .map_or(false, |s| s.starts_with('.'))
    }

    #[test]
    fn test_roundtrip() {
        let walker = walkdir::WalkDir::new("../sixteencolors-archive").into_iter();
        let mut num = 0;

        for entry in walker.filter_entry(|e| !is_hidden(e)) {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.is_dir() {
                continue;
            }
            let extension = path.extension();
            if extension.is_none() {
                continue;
            }
            let extension = extension.unwrap().to_str();
            if extension.is_none() {
                continue;
            }
            let extension = extension.unwrap().to_lowercase();

            let mut found = false;
            for format in &*crate::FORMATS {
                if format.get_file_extension() == extension
                    || format.get_alt_extensions().contains(&extension)
                {
                    found = true;
                }
            }
            if !found {
                continue;
            }
            num += 1;
            if num < 0 {
                continue;
            }

            let orig_bytes = fs::read(path).unwrap();

            if let Ok(buf) = Buffer::from_bytes(path, true, &orig_bytes) {
                if buf.get_width() != 80 {
                    continue;
                }
                if buf.palette.len() > 16 {
                    continue;
                }
                let mut opt = SaveOptions::default();
                opt.control_char_handling = crate::ControlCharHandling::IcyTerm;
                opt.compress = true;
                opt.save_sauce = buf.has_sauce();
                let mut draw = StringGenerator::new(opt);
                draw.screen_prep(&buf);
                draw.generate(&buf, &buf);
                draw.screen_end(&buf);
                let bytes = draw.get_data().to_vec();
                let buf2 = Buffer::from_bytes(Path::new("test.ans"), true, &bytes).unwrap();
                if buf.get_height() != buf2.get_height() {
                    continue;
                }

                /*
                for x in 23..30 {
                    let ch = buf2.layers[0].get_char((x, 0));
                    "{:?} {:?}", ch, buf2.palette.get_color(ch.attribute.get_foreground()));
                }
                */

                crate::compare_buffers(
                    &buf,
                    &buf2,
                    crate::CompareOptions {
                        compare_palette: false,
                        compare_fonts: false,
                        ignore_invisible_chars: true,
                    },
                );
            }
        }
    }
}
*/

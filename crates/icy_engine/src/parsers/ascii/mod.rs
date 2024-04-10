use super::BufferParser;
use crate::{AttributedChar, Buffer, CallbackAction, Caret, EngineResult, UnicodeConverter, BEL, BS, CR, FF, LF};
#[derive(Default)]
pub struct Parser {}

#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct CP437Converter {}

impl UnicodeConverter for CP437Converter {
    fn convert_from_unicode(&self, ch: char, _font_page: usize) -> char {
        if let Some(tch) = UNICODE_TO_CP437.get(&ch) {
            *tch
        } else {
            ch
        }
    }

    fn convert_to_unicode(&self, attributed_char: AttributedChar) -> char {
        match CP437_TO_UNICODE.get(attributed_char.ch as usize) {
            Some(out_ch) => *out_ch,
            _ => attributed_char.ch,
        }
    }
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret, ch: char) -> EngineResult<CallbackAction> {
        match ch {
            '\x00' | '\u{00FF}' => {
                caret.reset_color_attribute();
            }
            BEL => {
                return Ok(CallbackAction::Beep);
            }
            LF => caret.lf(buf, current_layer),
            FF => caret.ff(buf, current_layer),
            CR => caret.cr(buf),
            BS => caret.bs(buf, current_layer),
            '\x7F' => caret.del(buf, current_layer),
            _ => buf.print_value(current_layer, caret, ch as u16),
        }
        Ok(CallbackAction::NoUpdate)
    }
}

lazy_static::lazy_static! {
    static ref UNICODE_TO_CP437: std::collections::HashMap<char,char> = {
        let mut res = std::collections::HashMap::new();
        (0..256).for_each(|a| {
            res.insert(CP437_TO_UNICODE[a], char::from_u32(a as u32).unwrap());
        });
        res
    };
}

pub const CP437_TO_UNICODE: [char; 256] = [
    '\u{0000}', '\u{263a}', '\u{263b}', '\u{2665}', '\u{2666}', '\u{2663}', '\u{2660}', '\u{2022}', '\u{25d8}', '\u{25cb}', '\u{25d9}', '\u{2642}', '\u{2640}',
    '\u{266a}', '\u{266b}', '\u{263c}', '\u{25ba}', '\u{25c4}', '\u{2195}', '\u{203c}', '\u{00b6}', '\u{00a7}', '\u{25ac}', '\u{21a8}', '\u{2191}', '\u{2193}',
    '\u{2192}', '\u{2190}', '\u{221f}', '\u{2194}', '\u{25b2}', '\u{25bc}', '\u{0020}', '\u{0021}', '\u{0022}', '\u{0023}', '\u{0024}', '\u{0025}', '\u{0026}',
    '\u{0027}', '\u{0028}', '\u{0029}', '\u{002a}', '\u{002b}', '\u{002c}', '\u{002d}', '\u{002e}', '\u{002f}', '\u{0030}', '\u{0031}', '\u{0032}', '\u{0033}',
    '\u{0034}', '\u{0035}', '\u{0036}', '\u{0037}', '\u{0038}', '\u{0039}', '\u{003a}', '\u{003b}', '\u{003c}', '\u{003d}', '\u{003e}', '\u{003f}', '\u{0040}',
    '\u{0041}', '\u{0042}', '\u{0043}', '\u{0044}', '\u{0045}', '\u{0046}', '\u{0047}', '\u{0048}', '\u{0049}', '\u{004a}', '\u{004b}', '\u{004c}', '\u{004d}',
    '\u{004e}', '\u{004f}', '\u{0050}', '\u{0051}', '\u{0052}', '\u{0053}', '\u{0054}', '\u{0055}', '\u{0056}', '\u{0057}', '\u{0058}', '\u{0059}', '\u{005a}',
    '\u{005b}', '\u{005c}', '\u{005d}', '\u{005e}', '\u{005f}', '\u{0060}', '\u{0061}', '\u{0062}', '\u{0063}', '\u{0064}', '\u{0065}', '\u{0066}', '\u{0067}',
    '\u{0068}', '\u{0069}', '\u{006a}', '\u{006b}', '\u{006c}', '\u{006d}', '\u{006e}', '\u{006f}', '\u{0070}', '\u{0071}', '\u{0072}', '\u{0073}', '\u{0074}',
    '\u{0075}', '\u{0076}', '\u{0077}', '\u{0078}', '\u{0079}', '\u{007a}', '\u{007b}', '\u{007c}', '\u{007d}', '\u{007e}', '\u{007f}', '\u{00c7}', '\u{00fc}',
    '\u{00e9}', '\u{00e2}', '\u{00e4}', '\u{00e0}', '\u{00e5}', '\u{00e7}', '\u{00ea}', '\u{00eb}', '\u{00e8}', '\u{00ef}', '\u{00ee}', '\u{00ec}', '\u{00c4}',
    '\u{00c5}', '\u{00c9}', '\u{00e6}', '\u{00c6}', '\u{00f4}', '\u{00f6}', '\u{00f2}', '\u{00fb}', '\u{00f9}', '\u{00ff}', '\u{00d6}', '\u{00dc}', '\u{00a2}',
    '\u{00a3}', '\u{00a5}', '\u{20a7}', '\u{0192}', '\u{00e1}', '\u{00ed}', '\u{00f3}', '\u{00fa}', '\u{00f1}', '\u{00d1}', '\u{00aa}', '\u{00ba}', '\u{00bf}',
    '\u{2310}', '\u{00ac}', '\u{00bd}', '\u{00bc}', '\u{00a1}', '\u{00ab}', '\u{00bb}', '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}',
    '\u{2562}', '\u{2556}', '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}', '\u{255c}', '\u{255b}', '\u{2510}', '\u{2514}', '\u{2534}', '\u{252c}',
    '\u{251c}', '\u{2500}', '\u{253c}', '\u{255e}', '\u{255f}', '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256c}', '\u{2567}',
    '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256b}', '\u{256a}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}',
    '\u{258c}', '\u{2590}', '\u{2580}', '\u{03b1}', '\u{00df}', '\u{0393}', '\u{03c0}', '\u{03a3}', '\u{03c3}', '\u{00b5}', '\u{03c4}', '\u{03a6}', '\u{0398}',
    '\u{03a9}', '\u{03b4}', '\u{221e}', '\u{03c6}', '\u{03b5}', '\u{2229}', '\u{2261}', '\u{00b1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00f7}',
    '\u{2248}', '\u{00b0}', '\u{2219}', '\u{00b7}', '\u{221a}', '\u{207f}', '\u{00b2}', '\u{25a0}', '\u{00a0}',
];

use std::thread;

use base64::{Engine, engine::general_purpose};

use super::{Parser, parse_next_number};
use crate::{BitFont, CallbackAction, EditableScreen, EngineResult, HEX_TABLE, ParserError, Sixel};

#[derive(Debug, Clone, Copy)]
enum HexMacroState {
    FirstHex,
    SecondHex(char),
    RepeatNumber(i32),
}

impl Parser {
    pub(super) fn execute_dcs(&mut self, buf: &mut dyn EditableScreen) -> EngineResult<CallbackAction> {
        if self.parse_string.starts_with("CTerm:Font:") {
            return self.load_custom_font(buf);
        }
        let mut i = 0;
        self.parsed_numbers.clear();
        for ch in self.parse_string.chars() {
            match ch {
                '0'..='9' => {
                    let d = match self.parsed_numbers.pop() {
                        Some(number) => number,
                        _ => 0,
                    };
                    self.parsed_numbers.push(parse_next_number(d, ch as u8));
                }
                ';' => {
                    self.parsed_numbers.push(0);
                }
                _ => {
                    break;
                }
            }
            i += 1;
        }

        if self.parse_string[i..].starts_with("!z") {
            return self.parse_macro(i + 2);
        }

        if self.parse_string[i..].starts_with('q') {
            let vertical_scale = match self.parsed_numbers.first() {
                Some(0 | 1 | 5 | 6) | None => 2,
                Some(2) => 5,
                Some(3 | 4) => 3,
                _ => 1,
            };

            let bg_color = if let Some(1) = self.parsed_numbers.get(1) {
                [0, 0, 0, 0]
            } else {
                let (r, g, b) = buf.palette().get_rgb(buf.caret().attribute.get_background());
                [0xff, r, g, b]
            };

            let p = buf.caret().position();
            let dcs_string = std::mem::take(&mut self.parse_string);
            let handle = thread::spawn(move || Sixel::parse_from(p, 1, vertical_scale, bg_color, &dcs_string[i + 1..]));

            buf.push_sixel_thread(handle);

            return Ok(CallbackAction::NoUpdate);
        }

        Err(ParserError::UnsupportedDCSSequence(self.parse_string.clone()).into())
    }

    fn parse_macro(&mut self, start_index: usize) -> EngineResult<CallbackAction> {
        if let Some(pid) = self.parsed_numbers.first() {
            if let Some(pdt) = self.parsed_numbers.get(1) {
                // 0 - or omitted overwrites macro
                // 1 - clear all macros before defining this macro
                if *pdt == 1 {
                    self.macros.clear();
                }
            }
            match self.parsed_numbers.get(2) {
                Some(0) => {
                    self.parse_macro_sequence(*pid as usize, start_index);
                }
                Some(1) => {
                    self.parse_hex_macro_sequence(*pid as usize, start_index)?;
                }
                _ => {
                    return Err(ParserError::UnsupportedDCSSequence(format!(
                        "encountered p3 in macro definition: '{}' only 0 and 1 are valid.",
                        self.parse_string
                    ))
                    .into());
                }
            };
            return Ok(CallbackAction::NoUpdate);
        }
        Err(ParserError::UnsupportedDCSSequence(format!("encountered unsupported macro definition: '{}'", self.parse_string)).into())
    }

    fn parse_macro_sequence(&mut self, id: usize, start_index: usize) {
        self.macros.insert(id, self.parse_string[start_index..].to_string());
    }

    fn parse_hex_macro_sequence(&mut self, id: usize, start_index: usize) -> EngineResult<CallbackAction> {
        let mut state = HexMacroState::FirstHex;
        let mut read_repeat = false;
        let mut repeat_rec = String::new();
        let mut repeat_number = 0;
        let mut marco_rec = String::new();

        for ch in self.parse_string[start_index..].chars() {
            match &state {
                HexMacroState::FirstHex => {
                    if ch == ';' && read_repeat {
                        read_repeat = false;
                        (0..repeat_number).for_each(|_| marco_rec.push_str(&repeat_rec));
                        continue;
                    }
                    if ch == '!' {
                        state = HexMacroState::RepeatNumber(0);
                        continue;
                    }
                    state = HexMacroState::SecondHex(ch);
                }
                HexMacroState::SecondHex(first) => {
                    let cc = ch.to_ascii_uppercase();
                    let second = HEX_TABLE.iter().position(|&x| x == cc as u8);
                    let first = HEX_TABLE.iter().position(|&x| x == *first as u8);
                    if let (Some(first), Some(second)) = (first, second) {
                        let cc = unsafe { char::from_u32_unchecked((first * 16 + second) as u32) };
                        if read_repeat {
                            repeat_rec.push(cc);
                        } else {
                            marco_rec.push(cc);
                        }
                        state = HexMacroState::FirstHex;
                    } else {
                        return Err(ParserError::Error("Invalid hex number in macro sequence".to_string()).into());
                    }
                }
                HexMacroState::RepeatNumber(n) => {
                    if ch.is_ascii_digit() {
                        state = HexMacroState::RepeatNumber(parse_next_number(*n, ch as u8));
                        continue;
                    }
                    if ch == ';' {
                        repeat_number = *n;
                        repeat_rec.clear();
                        read_repeat = true;
                        state = HexMacroState::FirstHex;
                        continue;
                    }
                    return Err(ParserError::Error(format!("Invalid end of repeat number {ch}")).into());
                }
            }
        }
        if read_repeat {
            (0..repeat_number).for_each(|_| marco_rec.push_str(&repeat_rec));
        }

        self.macros.insert(id, marco_rec);

        Ok(CallbackAction::NoUpdate)
    }

    fn load_custom_font(&mut self, buf: &mut dyn EditableScreen) -> EngineResult<CallbackAction> {
        let start_index = "CTerm:Font:".len();
        if let Some(idx) = self.parse_string[start_index..].find(':') {
            let idx = idx + start_index;

            if let Ok(num) = self.parse_string[start_index..idx].parse::<usize>() {
                if let Ok(font_data) = general_purpose::STANDARD.decode(self.parse_string[idx + 1..].as_bytes()) {
                    match BitFont::from_bytes(format!("custom font {num}"), &font_data) {
                        Ok(font) => {
                            log::info!("loaded custom font {num}", num = num);
                            buf.set_font(num, font);
                            return Ok(CallbackAction::NoUpdate);
                        }
                        Err(err) => {
                            return Err(ParserError::UnsupportedDCSSequence(format!("Can't load bit font from dcs: {err}")).into());
                        }
                    }
                }
                return Err(ParserError::UnsupportedDCSSequence(format!("Can't decode base64 in dcs: {}", self.parse_string)).into());
            }
        }

        Err(ParserError::UnsupportedDCSSequence(format!("invalid custom font in dcs: {}", self.parse_string)).into())
    }
}

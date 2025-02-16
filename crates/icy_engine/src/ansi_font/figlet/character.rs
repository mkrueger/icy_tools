use regex::Regex;
use std::io::{BufReader, Read};

use crate::EngineResult;

use super::{errors::FigError, header::Header, read_line};

#[derive(Debug, PartialEq)]
pub enum FIGChar {
    HardBlank,
    Char(char),
}

#[derive(Debug, PartialEq)]
pub struct Character {
    pub ch: Option<char>,
    pub comment: Option<String>,
    pub lines: Vec<Vec<FIGChar>>,
}

lazy_static::lazy_static! {
    static ref CODE_TAG : Regex = Regex::new(r"((0x[a-fA-F0-9]+)|(\d+))\s+(.+)").unwrap();
}

impl Character {
    pub(crate) fn read<R: Read>(reader: &mut BufReader<R>, header: &Header, has_tag: bool) -> EngineResult<Self> {
        let mut ch = None;
        let mut comment = None;
        let mut lines = Vec::new();
        if has_tag {
            let line = read_line(reader)?;
            let Some(caps) = CODE_TAG.captures(&line) else {
                return Err(FigError::InvalidCharTag(line).into());
            };
            let number = caps[1].to_string();
            if number.starts_with("0x") {
                ch = char::from_u32(u32::from_str_radix(&number[2..], 16)?);
            } else if number.starts_with("0") {
                ch = char::from_u32(u32::from_str_radix(&number[1..], 8)?);
            } else {
                ch = char::from_u32(number.parse::<u32>()?);
            }
            comment = Some(caps[4].to_string());
        }
        let mut eol = '@';

        for i in 0..header.height() {
            let mut line = read_line(reader)?.chars().collect::<Vec<char>>();
            if i == 0 {
                if let Some(last) = line.last() {
                    eol = *last;
                }
            }
            if line.ends_with(&[eol, eol]) {
                line.pop();
                line.pop();
                lines.push(convert_line(line, header));
                break;
            } else if line.ends_with(&[eol]) {
                line.pop();
                lines.push(convert_line(line, header));
            } else {
                return Err(FigError::InvalidCharLine.into());
            }
        }
        Ok(Self { ch, comment, lines })
    }

    pub(crate) fn generate_string(&self, header: &Header) -> String {
        let mut res = String::new();
        if let Some(ch) = self.ch {
            res.push_str(&format!("0x{:04x}  {}\n", ch as u32, self.comment.as_ref().unwrap()));
        }
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                res.push('\n');
            }
            for ch in line {
                match ch {
                    FIGChar::HardBlank => res.push_str(&format!("{}", header.hard_blank_char())),
                    FIGChar::Char(ch) => res.push(*ch),
                }
            }
            res.push('@');
        }
        res.push_str("@\n");
        res
    }
}

fn convert_line(line: Vec<char>, header: &Header) -> Vec<FIGChar> {
    line.into_iter()
        .map(|c| {
            if c == header.hard_blank_char() {
                FIGChar::HardBlank
            } else {
                FIGChar::Char(c)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_character() {
        let data = r"flf2a$ 6 5 20 15 0 0 143 229
 _   _ @
(_) (_)@
| | | |@
| |_| |@
 \__,_|@
       @@";

        let mut reader = BufReader::new(data.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        let character = Character::read(&mut reader, &header, false).unwrap();
        assert_eq!(character.ch, None);
        assert_eq!(character.comment, None);
        assert_eq!(
            character.lines,
            vec![
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char('_'),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char('_'),
                    FIGChar::Char(' ')
                ],
                vec![
                    FIGChar::Char('('),
                    FIGChar::Char('_'),
                    FIGChar::Char(')'),
                    FIGChar::Char(' '),
                    FIGChar::Char('('),
                    FIGChar::Char('_'),
                    FIGChar::Char(')')
                ],
                vec![
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|')
                ],
                vec![
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char('_'),
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char('\\'),
                    FIGChar::Char('_'),
                    FIGChar::Char('_'),
                    FIGChar::Char(','),
                    FIGChar::Char('_'),
                    FIGChar::Char('|')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' ')
                ]
            ]
        );
    }

    #[test]
    pub fn test_tag_parse_character() {
        let data = r"flf2a$ 6 5 20 15 0 0 143 229
162  CENT SIGN
   _  @
  | | @
 / __)@
| (__ @
 \   )@
  |_| @@";

        let mut reader = BufReader::new(data.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        let character = Character::read(&mut reader, &header, true).unwrap();
        assert_eq!(character.ch, Some('¢'));
        assert_eq!(character.comment, Some("CENT SIGN".to_string()));
        assert_eq!(
            character.lines,
            vec![
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char('_'),
                    FIGChar::Char(' '),
                    FIGChar::Char(' ')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char(' ')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char('/'),
                    FIGChar::Char(' '),
                    FIGChar::Char('_'),
                    FIGChar::Char('_'),
                    FIGChar::Char(')')
                ],
                vec![
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('('),
                    FIGChar::Char('_'),
                    FIGChar::Char('_'),
                    FIGChar::Char(' ')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char('\\'),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(')')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char('_'),
                    FIGChar::Char('|'),
                    FIGChar::Char(' ')
                ]
            ]
        );
    }

    #[test]
    pub fn test_tag_parse_hex() {
        let data = r"flf2a$ 6 5 20 15 0 0 143 229
0xA2  CENT SIGN
   _  @
  | | @
 / __)@
| (__ @
 \   )@
  |_| @@";

        let mut reader = BufReader::new(data.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        let character = Character::read(&mut reader, &header, true).unwrap();
        assert_eq!(character.ch, Some('¢'));
        assert_eq!(character.comment, Some("CENT SIGN".to_string()));
        assert_eq!(
            character.lines,
            vec![
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char('_'),
                    FIGChar::Char(' '),
                    FIGChar::Char(' ')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char(' ')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char('/'),
                    FIGChar::Char(' '),
                    FIGChar::Char('_'),
                    FIGChar::Char('_'),
                    FIGChar::Char(')')
                ],
                vec![
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('('),
                    FIGChar::Char('_'),
                    FIGChar::Char('_'),
                    FIGChar::Char(' ')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char('\\'),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(')')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char('_'),
                    FIGChar::Char('|'),
                    FIGChar::Char(' ')
                ]
            ]
        );
    }

    #[test]
    pub fn test_tag_parse_oct() {
        let data = r"flf2a$ 6 5 20 15 0 0 143 229
0242  CENT SIGN
   _  @
  | | @
 / __)@
| (__ @
 \   )@
  |_| @@";

        let mut reader = BufReader::new(data.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        let character = Character::read(&mut reader, &header, true).unwrap();
        assert_eq!(character.ch, Some('¢'));
        assert_eq!(character.comment, Some("CENT SIGN".to_string()));
        assert_eq!(
            character.lines,
            vec![
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char('_'),
                    FIGChar::Char(' '),
                    FIGChar::Char(' ')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char(' ')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char('/'),
                    FIGChar::Char(' '),
                    FIGChar::Char('_'),
                    FIGChar::Char('_'),
                    FIGChar::Char(')')
                ],
                vec![
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('('),
                    FIGChar::Char('_'),
                    FIGChar::Char('_'),
                    FIGChar::Char(' ')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char('\\'),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(')')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char('_'),
                    FIGChar::Char('|'),
                    FIGChar::Char(' ')
                ]
            ]
        );
    }

    #[test]
    pub fn test_hard_blank() {
        let data = r"flf2a$ 6 5 20 15 0 0 143 229
$_   _ @
(_) (_)@
| | | |@
| |_| |@
$\__,_|@
       @@";

        let mut reader = BufReader::new(data.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        let character = Character::read(&mut reader, &header, false).unwrap();
        assert_eq!(character.ch, None);
        assert_eq!(character.comment, None);
        assert_eq!(
            character.lines,
            vec![
                vec![
                    FIGChar::HardBlank,
                    FIGChar::Char('_'),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char('_'),
                    FIGChar::Char(' ')
                ],
                vec![
                    FIGChar::Char('('),
                    FIGChar::Char('_'),
                    FIGChar::Char(')'),
                    FIGChar::Char(' '),
                    FIGChar::Char('('),
                    FIGChar::Char('_'),
                    FIGChar::Char(')')
                ],
                vec![
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|')
                ],
                vec![
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|'),
                    FIGChar::Char('_'),
                    FIGChar::Char('|'),
                    FIGChar::Char(' '),
                    FIGChar::Char('|')
                ],
                vec![
                    FIGChar::HardBlank,
                    FIGChar::Char('\\'),
                    FIGChar::Char('_'),
                    FIGChar::Char('_'),
                    FIGChar::Char(','),
                    FIGChar::Char('_'),
                    FIGChar::Char('|')
                ],
                vec![
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' '),
                    FIGChar::Char(' ')
                ]
            ]
        );
    }

    #[test]
    pub fn test_character_generation() {
        let data = r"flf2a$ 6 5 20 15 0 0 143 229
 _   _ @
(_) (_)@
| | | |@
| |_| |@
 \__,_|@
       @@";

        let mut reader = BufReader::new(data.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        let character = Character::read(&mut reader, &header, false).unwrap();
        assert_eq!(character.ch, None);
        assert_eq!(character.comment, None);
        assert_eq!(
            character.generate_string(&header),
            r" _   _ @
(_) (_)@
| | | |@
| |_| |@
 \__,_|@
       @@
"
        );
    }

    #[test]
    pub fn test_character_generation_comment() {
        let data = r"flf2a$ 6 5 20 15 0 0 143 229
0x00a2  CENT SIGN
   _  @
  | | @
 / __)@
| (__ @
 \   )@
  |_| @@";

        let mut reader = BufReader::new(data.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        let character = Character::read(&mut reader, &header, true).unwrap();
        assert_eq!(character.ch, Some('¢'));
        assert_eq!(character.comment, Some("CENT SIGN".to_string()));
        assert_eq!(
            character.generate_string(&header),
            r"0x00a2  CENT SIGN
   _  @
  | | @
 / __)@
| (__ @
 \   )@
  |_| @@
"
        );
    }
}

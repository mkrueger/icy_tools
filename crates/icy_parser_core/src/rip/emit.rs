use super::*;
use crate::rip::command::RipCommand;

impl RipParser {
    pub fn emit_command(&mut self, sink: &mut dyn CommandSink) {
        let cmd = match (self.builder.level, self.builder.cmd_char) {
            // Level 0 commands
            (0, b'w') if self.builder.u16_params.len() >= 5 => RipCommand::TextWindow {
                x0: self.builder.u16_params[0],
                y0: self.builder.u16_params[1],
                x1: self.builder.u16_params[2],
                y1: self.builder.u16_params[3],
                wrap: self.builder.u16_params[4] != 0,
                size: *self.builder.u16_params.get(5).unwrap_or(&0),
            },
            (0, b'v') if self.builder.u16_params.len() >= 4 => RipCommand::ViewPort {
                x0: self.builder.u16_params[0],
                y0: self.builder.u16_params[1],
                x1: self.builder.u16_params[2],
                y1: self.builder.u16_params[3],
            },
            (0, b'*') => RipCommand::ResetWindows,
            (0, b'e') => RipCommand::EraseWindow,
            (0, b'E') => RipCommand::EraseView,
            (0, b'g') if self.builder.u16_params.len() >= 2 => RipCommand::GotoXY {
                x: self.builder.u16_params[0],
                y: self.builder.u16_params[1],
            },
            (0, b'H') => RipCommand::Home,
            (0, b'>') => RipCommand::EraseEOL,
            (0, b'c') if !self.builder.u16_params.is_empty() => RipCommand::Color { c: self.builder.u16_params[0] },
            (0, b'Q') => RipCommand::SetPalette {
                colors: self.builder.u16_params.clone(),
            },
            (0, b'a') if self.builder.u16_params.len() >= 2 => RipCommand::OnePalette {
                color: self.builder.u16_params[0],
                value: self.builder.u16_params[1],
            },
            (0, b'W') if !self.builder.u16_params.is_empty() => {
                let mode_value = self.builder.u16_params[0];
                let mode = match WriteMode::try_from(mode_value) {
                    Ok(m) => m,
                    Err(_) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "RIP_WRITE_MODE",
                                value: mode_value.to_string(),
                                expected: Some("0 (Normal) or 1 (XOR)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        return;
                    }
                };
                RipCommand::WriteMode { mode }
            }
            (0, b'm') if self.builder.u16_params.len() >= 2 => RipCommand::Move {
                x: self.builder.u16_params[0],
                y: self.builder.u16_params[1],
            },
            (0, b'T') => RipCommand::Text {
                text: self.builder.string_param.clone(),
            },
            (0, b'@') if self.builder.u16_params.len() >= 2 => RipCommand::TextXY {
                x: self.builder.u16_params[0],
                y: self.builder.u16_params[1],
                text: self.builder.string_param.clone(),
            },
            (0, b'Y') if self.builder.u16_params.len() >= 4 => RipCommand::FontStyle {
                font: self.builder.u16_params[0],
                direction: self.builder.u16_params[1],
                size: self.builder.u16_params[2],
                res: self.builder.u16_params[3],
            },
            (0, b'X') if self.builder.u16_params.len() >= 2 => RipCommand::Pixel {
                x: self.builder.u16_params[0],
                y: self.builder.u16_params[1],
            },
            (0, b'L') if self.builder.u16_params.len() >= 4 => RipCommand::Line {
                x0: self.builder.u16_params[0],
                y0: self.builder.u16_params[1],
                x1: self.builder.u16_params[2],
                y1: self.builder.u16_params[3],
            },
            (0, b'R') if self.builder.u16_params.len() >= 4 => RipCommand::Rectangle {
                x0: self.builder.u16_params[0],
                y0: self.builder.u16_params[1],
                x1: self.builder.u16_params[2],
                y1: self.builder.u16_params[3],
            },
            (0, b'B') if self.builder.u16_params.len() >= 4 => RipCommand::Bar {
                x0: self.builder.u16_params[0],
                y0: self.builder.u16_params[1],
                x1: self.builder.u16_params[2],
                y1: self.builder.u16_params[3],
            },
            (0, b'C') if self.builder.u16_params.len() >= 3 => RipCommand::Circle {
                x_center: self.builder.u16_params[0],
                y_center: self.builder.u16_params[1],
                radius: self.builder.u16_params[2],
            },
            (0, b'O') if self.builder.u16_params.len() >= 6 => RipCommand::Oval {
                x: self.builder.u16_params[0],
                y: self.builder.u16_params[1],
                st_ang: self.builder.u16_params[2],
                end_ang: self.builder.u16_params[3],
                x_rad: self.builder.u16_params[4],
                y_rad: self.builder.u16_params[5],
            },
            (0, b'o') if self.builder.u16_params.len() >= 4 => RipCommand::FilledOval {
                x: self.builder.u16_params[0],
                y: self.builder.u16_params[1],
                x_rad: self.builder.u16_params[2],
                y_rad: self.builder.u16_params[3],
            },
            (0, b'A') if self.builder.u16_params.len() >= 5 => RipCommand::Arc {
                x: self.builder.u16_params[0],
                y: self.builder.u16_params[1],
                st_ang: self.builder.u16_params[2],
                end_ang: self.builder.u16_params[3],
                radius: self.builder.u16_params[4],
            },
            (0, b'V') if self.builder.u16_params.len() >= 6 => RipCommand::OvalArc {
                x: self.builder.u16_params[0],
                y: self.builder.u16_params[1],
                st_ang: self.builder.u16_params[2],
                end_ang: self.builder.u16_params[3],
                x_rad: self.builder.u16_params[4],
                y_rad: self.builder.u16_params[5],
            },
            (0, b'I') if self.builder.u16_params.len() >= 5 => RipCommand::PieSlice {
                x: self.builder.u16_params[0],
                y: self.builder.u16_params[1],
                st_ang: self.builder.u16_params[2],
                end_ang: self.builder.u16_params[3],
                radius: self.builder.u16_params[4],
            },
            (0, b'i') if self.builder.u16_params.len() >= 6 => RipCommand::OvalPieSlice {
                x: self.builder.u16_params[0],
                y: self.builder.u16_params[1],
                st_ang: self.builder.u16_params[2],
                end_ang: self.builder.u16_params[3],
                x_rad: self.builder.u16_params[4],
                y_rad: self.builder.u16_params[5],
            },
            (0, b'Z') if self.builder.u16_params.len() >= 9 => RipCommand::Bezier {
                x1: self.builder.u16_params[0],
                y1: self.builder.u16_params[1],
                x2: self.builder.u16_params[2],
                y2: self.builder.u16_params[3],
                x3: self.builder.u16_params[4],
                y3: self.builder.u16_params[5],
                x4: self.builder.u16_params[6],
                y4: self.builder.u16_params[7],
                cnt: self.builder.u16_params[8],
            },
            (0, b'P') => RipCommand::Polygon {
                points: self.builder.u16_params.clone(),
            },
            (0, b'p') => RipCommand::FilledPolygon {
                points: self.builder.u16_params.clone(),
            },
            (0, b'l') => RipCommand::PolyLine {
                points: self.builder.u16_params.clone(),
            },
            (0, b'F') if self.builder.u16_params.len() >= 3 => RipCommand::Fill {
                x: self.builder.u16_params[0],
                y: self.builder.u16_params[1],
                border: self.builder.u16_params[2],
            },
            (0, b'=') if self.builder.u16_params.len() >= 3 => {
                let style_value = self.builder.u16_params[0];
                let style = match LineStyle::try_from(style_value as i32) {
                    Ok(s) => s,
                    Err(_) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "RIP_LINE_STYLE",
                                value: style_value.to_string(),
                                expected: Some("0-4 (Solid, Dotted, Center, Dashed, User)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        return;
                    }
                };
                RipCommand::LineStyle {
                    style,
                    user_pat: self.builder.u16_params[1],
                    thick: self.builder.u16_params[2],
                }
            }
            (0, b'S') if self.builder.u16_params.len() >= 2 => {
                let pattern_value = self.builder.u16_params[0];
                let pattern = match FillStyle::try_from(pattern_value as u8) {
                    Ok(p) => p,
                    Err(_) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "RIP_FILL_STYLE",
                                value: pattern_value.to_string(),
                                expected: Some(
                                    "0x00-0x0B (Empty, Solid, Line, LtSlash, Slash, BkSlash, LtBkSlash, Hatch, XHatch, Interleave, WideDot, CloseDot)"
                                        .to_string(),
                                ),
                            },
                            crate::ErrorLevel::Error,
                        );
                        return;
                    }
                };
                RipCommand::FillStyle {
                    pattern,
                    color: self.builder.u16_params[1],
                }
            }
            (0, b's') if self.builder.u16_params.len() >= 9 => RipCommand::FillPattern {
                c1: self.builder.u16_params[0],
                c2: self.builder.u16_params[1],
                c3: self.builder.u16_params[2],
                c4: self.builder.u16_params[3],
                c5: self.builder.u16_params[4],
                c6: self.builder.u16_params[5],
                c7: self.builder.u16_params[6],
                c8: self.builder.u16_params[7],
                col: self.builder.u16_params[8],
            },
            (0, b'$') => RipCommand::TextVariable {
                text: self.builder.string_param.clone(),
            },
            (0, b'#') => RipCommand::NoMore,

            // Level 1 commands
            (1, b'M') if self.builder.u16_params.len() >= 8 => RipCommand::Mouse {
                num: self.builder.u16_params[0],
                x0: self.builder.u16_params[1],
                y0: self.builder.u16_params[2],
                x1: self.builder.u16_params[3],
                y1: self.builder.u16_params[4],
                clk: self.builder.u16_params[5],
                clr: self.builder.u16_params[6],
                res: self.builder.u16_params[7],
                text: self.builder.string_param.clone(),
            },
            (1, b'K') => RipCommand::MouseFields,
            (1, b'T') if self.builder.u16_params.len() >= 5 => RipCommand::BeginText {
                x0: self.builder.u16_params[0],
                y0: self.builder.u16_params[1],
                x1: self.builder.u16_params[2],
                y1: self.builder.u16_params[3],
                res: self.builder.u16_params[4],
            },
            (1, b't') => RipCommand::RegionText {
                justify: !self.builder.u16_params.is_empty() && self.builder.u16_params[0] != 0,
                text: self.builder.string_param.clone(),
            },
            (1, b'E') => RipCommand::EndText,
            (1, b'C') if self.builder.u16_params.len() >= 5 => RipCommand::GetImage {
                x0: self.builder.u16_params[0],
                y0: self.builder.u16_params[1],
                x1: self.builder.u16_params[2],
                y1: self.builder.u16_params[3],
                res: self.builder.u16_params[4],
            },
            (1, b'P') if self.builder.u16_params.len() >= 4 => {
                let mode_value = self.builder.u16_params[2];
                let mode = match ImagePasteMode::try_from(mode_value) {
                    Ok(m) => m,
                    Err(_) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "RIP_PUT_IMAGE",
                                value: mode_value.to_string(),
                                expected: Some("0-4 (Copy, Xor, Or, And, Not)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        return;
                    }
                };
                RipCommand::PutImage {
                    x: self.builder.u16_params[0],
                    y: self.builder.u16_params[1],
                    mode,
                    res: self.builder.u16_params[3],
                }
            }
            (1, b'W') => RipCommand::WriteIcon {
                res: self.builder.char_param,
                data: self.builder.string_param.clone(),
            },
            (1, b'I') if self.builder.u16_params.len() >= 5 => {
                let mode_value = self.builder.u16_params[2];
                let mode = match ImagePasteMode::try_from(mode_value) {
                    Ok(m) => m,
                    Err(_) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "RIP_LOAD_ICON",
                                value: mode_value.to_string(),
                                expected: Some("0-4 (Copy, Xor, Or, And, Not)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        return;
                    }
                };
                RipCommand::LoadIcon {
                    x: self.builder.u16_params[0],
                    y: self.builder.u16_params[1],
                    mode,
                    clipboard: self.builder.u16_params[3],
                    res: self.builder.u16_params[4],
                    file_name: self.builder.string_param.clone(),
                }
            }
            (1, b'B') if self.builder.u16_params.len() >= 15 => RipCommand::ButtonStyle {
                wid: self.builder.u16_params[0],
                hgt: self.builder.u16_params[1],
                orient: self.builder.u16_params[2],
                flags: self.builder.u16_params[3],
                bevsize: self.builder.u16_params[4],
                dfore: self.builder.u16_params[5],
                dback: self.builder.u16_params[6],
                bright: self.builder.u16_params[7],
                dark: self.builder.u16_params[8],
                surface: self.builder.u16_params[9],
                grp_no: self.builder.u16_params[10],
                flags2: self.builder.u16_params[11],
                uline_col: self.builder.u16_params[12],
                corner_col: self.builder.u16_params[13],
                res: self.builder.u16_params[14],
            },
            (1, b'U') if self.builder.u16_params.len() >= 7 => RipCommand::Button {
                x0: self.builder.u16_params[0],
                y0: self.builder.u16_params[1],
                x1: self.builder.u16_params[2],
                y1: self.builder.u16_params[3],
                hotkey: self.builder.u16_params[4],
                flags: self.builder.u16_params[5],
                res: self.builder.u16_params[6],
                text: self.builder.string_param.clone(),
            },
            (1, b'D') if self.builder.u16_params.len() >= 2 => RipCommand::Define {
                flags: self.builder.u16_params[0],
                res: self.builder.u16_params[1],
                text: self.builder.string_param.clone(),
            },
            (1, 0x1B) if self.builder.u16_params.len() >= 2 => {
                let mode_value = self.builder.u16_params[0];
                let mode = match QueryMode::try_from(mode_value) {
                    Ok(m) => m,
                    Err(_) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "RIP_QUERY",
                                value: mode_value.to_string(),
                                expected: Some("0-2 (ProcessNow, OnClickGraphics, OnClickText)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        return;
                    }
                };
                RipCommand::Query {
                    mode,
                    res: self.builder.u16_params[1],
                    text: self.builder.string_param.clone(),
                }
            }
            (1, b'G') if self.builder.u16_params.len() >= 6 => RipCommand::CopyRegion {
                x0: self.builder.u16_params[0],
                y0: self.builder.u16_params[1],
                x1: self.builder.u16_params[2],
                y1: self.builder.u16_params[3],
                res: self.builder.u16_params[4],
                dest_line: self.builder.u16_params[5],
            },
            (1, b'R') => RipCommand::ReadScene {
                file_name: self.builder.string_param.clone(),
            },
            (1, b'F') if self.builder.u16_params.len() >= 2 => {
                let mode_value = self.builder.u16_params[0];
                let mode = match FileQueryMode::try_from(mode_value) {
                    Ok(m) => m,
                    Err(_) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "RIP_FILE_QUERY",
                                value: mode_value.to_string(),
                                expected: Some("0-4 (FileExists, FileExistsWithCR, QueryWithSize, QueryExtended, QueryWithFilename)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        return;
                    }
                };
                RipCommand::FileQuery {
                    mode,
                    res: self.builder.u16_params[1],
                    file_name: self.builder.string_param.clone(),
                }
            }

            // Level 9 commands
            (9, 0x1B) if self.builder.u16_params.len() >= 4 => {
                let mode_value = self.builder.u16_params[0];
                let mode = match BlockTransferMode::try_from(mode_value) {
                    Ok(m) => m,
                    Err(_) => {
                        sink.report_error(
                            crate::ParseError::InvalidParameter {
                                command: "RIP_ENTER_BLOCK_MODE",
                                value: mode_value.to_string(),
                                expected: Some("0-7 (XmodemChecksum, XmodemCrc, Xmodem1K, Xmodem1KG, Kermit, Ymodem, YmodemG, Zmodem)".to_string()),
                            },
                            crate::ErrorLevel::Error,
                        );
                        return;
                    }
                };
                RipCommand::EnterBlockMode {
                    mode,
                    proto: self.builder.u16_params[1],
                    file_type: self.builder.u16_params[2],
                    res: self.builder.u16_params[3],
                    file_name: self.builder.string_param.clone(),
                }
            }

            _ => {
                // Unknown command - report error
                sink.report_error(
                    crate::ParseError::InvalidParameter {
                        command: "RIP",
                        value: (self.builder.cmd_char as u16).to_string(),
                        expected: Some("valid RIP command character".to_string()),
                    },
                    crate::ErrorLevel::Error,
                );
                return;
            }
        };

        sink.emit_rip(cmd);
    }
}

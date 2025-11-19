//! SGR (Select Graphic Rendition) handling
//!
//! Handles parsing of SGR escape sequences (CSI...m) for text styling and colors.

use crate::{Blink, Color, CommandSink, Frame, Intensity, ParseError, SgrAttribute, TerminalCommand, Underline};

pub const ANSI_COLOR_OFFSETS: [u8; 8] = [0, 4, 2, 6, 1, 5, 3, 7];

/// SGR lookup table entry - describes what a particular SGR parameter code means
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SgrLutEntry {
    /// Regular SGR attribute that can be directly used
    SetAttribute(SgrAttribute),
    /// Extended foreground color (38) - needs sub-parameters (38;5;n or 38;2;r;g;b)
    ExtendedForeground,
    /// Extended background color (48) - needs sub-parameters (48;5;n or 48;2;r;g;b)
    ExtendedBackground,
    /// Undefined/unsupported SGR code
    Undefined,
}

// SGR lookup table: maps SGR parameter values (0-107) to their meaning
static SGR_LUT: [SgrLutEntry; 108] = [
    SgrLutEntry::SetAttribute(SgrAttribute::Reset),                                          // 0
    SgrLutEntry::SetAttribute(SgrAttribute::Intensity(Intensity::Bold)),                     // 1
    SgrLutEntry::SetAttribute(SgrAttribute::Intensity(Intensity::Faint)),                    // 2
    SgrLutEntry::SetAttribute(SgrAttribute::Italic(true)),                                   // 3
    SgrLutEntry::SetAttribute(SgrAttribute::Underline(Underline::Single)),                   // 4
    SgrLutEntry::SetAttribute(SgrAttribute::Blink(Blink::Slow)),                             // 5
    SgrLutEntry::SetAttribute(SgrAttribute::Blink(Blink::Rapid)),                            // 6
    SgrLutEntry::SetAttribute(SgrAttribute::Inverse(true)),                                  // 7
    SgrLutEntry::SetAttribute(SgrAttribute::Concealed(true)),                                // 8
    SgrLutEntry::SetAttribute(SgrAttribute::CrossedOut(true)),                               // 9
    SgrLutEntry::SetAttribute(SgrAttribute::Font(0)),                                        // 10
    SgrLutEntry::SetAttribute(SgrAttribute::Font(1)),                                        // 11
    SgrLutEntry::SetAttribute(SgrAttribute::Font(2)),                                        // 12
    SgrLutEntry::SetAttribute(SgrAttribute::Font(3)),                                        // 13
    SgrLutEntry::SetAttribute(SgrAttribute::Font(4)),                                        // 14
    SgrLutEntry::SetAttribute(SgrAttribute::Font(5)),                                        // 15
    SgrLutEntry::SetAttribute(SgrAttribute::Font(6)),                                        // 16
    SgrLutEntry::SetAttribute(SgrAttribute::Font(7)),                                        // 17
    SgrLutEntry::SetAttribute(SgrAttribute::Font(8)),                                        // 18
    SgrLutEntry::SetAttribute(SgrAttribute::Font(9)),                                        // 19
    SgrLutEntry::SetAttribute(SgrAttribute::Fraktur),                                        // 20
    SgrLutEntry::SetAttribute(SgrAttribute::Underline(Underline::Double)),                   // 21
    SgrLutEntry::SetAttribute(SgrAttribute::Intensity(Intensity::Normal)),                   // 22
    SgrLutEntry::SetAttribute(SgrAttribute::Italic(false)),                                  // 23
    SgrLutEntry::SetAttribute(SgrAttribute::Underline(Underline::Off)),                      // 24
    SgrLutEntry::SetAttribute(SgrAttribute::Blink(Blink::Off)),                              // 25
    SgrLutEntry::Undefined,                                                                  // 26 - proportional spacing (rarely supported)
    SgrLutEntry::SetAttribute(SgrAttribute::Inverse(false)),                                 // 27
    SgrLutEntry::SetAttribute(SgrAttribute::Concealed(false)),                               // 28
    SgrLutEntry::SetAttribute(SgrAttribute::CrossedOut(false)),                              // 29
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(ANSI_COLOR_OFFSETS[0]))), // 30 - Black
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(ANSI_COLOR_OFFSETS[1]))), // 31 - Red
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(ANSI_COLOR_OFFSETS[2]))), // 32 - Green
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(ANSI_COLOR_OFFSETS[3]))), // 33 - Yellow
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(ANSI_COLOR_OFFSETS[4]))), // 34 - Blue
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(ANSI_COLOR_OFFSETS[5]))), // 35 - Magenta
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(ANSI_COLOR_OFFSETS[6]))), // 36 - Cyan
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(ANSI_COLOR_OFFSETS[7]))), // 37 - White
    SgrLutEntry::ExtendedForeground,                                                         // 38 - extended foreground (needs sub-params)
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Default)),                     // 39
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(ANSI_COLOR_OFFSETS[0]))), // 40 - Black
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(ANSI_COLOR_OFFSETS[1]))), // 41 - Red
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(ANSI_COLOR_OFFSETS[2]))), // 42 - Green
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(ANSI_COLOR_OFFSETS[3]))), // 43 - Yellow
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(ANSI_COLOR_OFFSETS[4]))), // 44 - Blue
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(ANSI_COLOR_OFFSETS[5]))), // 45 - Magenta
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(ANSI_COLOR_OFFSETS[6]))), // 46 - Cyan
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(ANSI_COLOR_OFFSETS[7]))), // 47 - White
    SgrLutEntry::ExtendedBackground,                                                         // 48 - extended background (needs sub-params)
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Default)),                     // 49
    SgrLutEntry::Undefined,                                                                  // 50 - disable proportional spacing
    SgrLutEntry::SetAttribute(SgrAttribute::Frame(Frame::Framed)),                           // 51
    SgrLutEntry::SetAttribute(SgrAttribute::Frame(Frame::Encircled)),                        // 52
    SgrLutEntry::SetAttribute(SgrAttribute::Overlined(true)),                                // 53
    SgrLutEntry::SetAttribute(SgrAttribute::Frame(Frame::Off)),                              // 54
    SgrLutEntry::SetAttribute(SgrAttribute::Overlined(false)),                               // 55
    SgrLutEntry::Undefined,                                                                  // 56 - reserved
    SgrLutEntry::Undefined,                                                                  // 57 - reserved
    SgrLutEntry::Undefined,                                                                  // 58 - underline color (rarely supported)
    SgrLutEntry::Undefined,                                                                  // 59 - default underline color
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramUnderline),                              // 60
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramDoubleUnderline),                        // 61
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramOverline),                               // 62
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramDoubleOverline),                         // 63
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramStress),                                 // 64
    SgrLutEntry::SetAttribute(SgrAttribute::IdeogramAttributesOff),                          // 65
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined, // 66-70
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined, // 71-75
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined, // 76-80
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined, // 81-85
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,                                                                      // 86-89
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(8 + ANSI_COLOR_OFFSETS[0]))), // 90 - Bright Black
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(8 + ANSI_COLOR_OFFSETS[1]))), // 91 - Bright Red
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(8 + ANSI_COLOR_OFFSETS[2]))), // 92 - Bright Green
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(8 + ANSI_COLOR_OFFSETS[3]))), // 93 - Bright Yellow
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(8 + ANSI_COLOR_OFFSETS[4]))), // 94 - Bright Blue
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(8 + ANSI_COLOR_OFFSETS[5]))), // 95 - Bright Magenta
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(8 + ANSI_COLOR_OFFSETS[6]))), // 96 - Bright Cyan
    SgrLutEntry::SetAttribute(SgrAttribute::Foreground(Color::Base(8 + ANSI_COLOR_OFFSETS[7]))), // 97 - Bright White
    SgrLutEntry::Undefined,
    SgrLutEntry::Undefined,                                                                      // 98-99
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(8 + ANSI_COLOR_OFFSETS[0]))), // 100 - Bright Black
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(8 + ANSI_COLOR_OFFSETS[1]))), // 101 - Bright Red
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(8 + ANSI_COLOR_OFFSETS[2]))), // 102 - Bright Green
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(8 + ANSI_COLOR_OFFSETS[3]))), // 103 - Bright Yellow
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(8 + ANSI_COLOR_OFFSETS[4]))), // 104 - Bright Blue
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(8 + ANSI_COLOR_OFFSETS[5]))), // 105 - Bright Magenta
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(8 + ANSI_COLOR_OFFSETS[6]))), // 106 - Bright Cyan
    SgrLutEntry::SetAttribute(SgrAttribute::Background(Color::Base(8 + ANSI_COLOR_OFFSETS[7]))), // 107 - Bright White
];

/// Parse SGR (Select Graphic Rendition) parameters and emit commands
///
/// Handles CSI...m sequences for text styling and colors, including:
/// - Standard attributes (bold, italic, underline, etc.)
/// - 16-color palette (30-37, 40-47, 90-97, 100-107)
/// - 256-color mode (38;5;n, 48;5;n)
/// - RGB true color (38;2;r;g;b, 48;2;r;g;b)
pub(crate) fn parse_sgr(params: &[u16], sink: &mut dyn CommandSink) {
    let mut i = 0;
    while i < params.len() {
        let code = params[i] as usize;

        // Look up the code in the lookup table
        let lut_entry = if code < SGR_LUT.len() { SGR_LUT[code] } else { SgrLutEntry::Undefined };

        match lut_entry {
            SgrLutEntry::SetAttribute(attr) => {
                sink.emit(TerminalCommand::CsiSelectGraphicRendition(attr));
                i += 1;
            }
            SgrLutEntry::ExtendedForeground => {
                // Extended foreground: 38;5;n (256-color) or 38;2;r;g;b (RGB)
                if i + 2 < params.len() {
                    match params[i + 1] {
                        5 if i + 2 < params.len() => {
                            // 256-color mode: 38;5;n
                            let color_index = params[i + 2] as u8;
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Extended(
                                color_index,
                            ))));
                            i += 3;
                        }
                        2 if i + 4 < params.len() => {
                            // RGB mode: 38;2;r;g;b
                            let r = params[i + 2] as u8;
                            let g = params[i + 3] as u8;
                            let b = params[i + 4] as u8;
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Rgb(r, g, b))));
                            i += 5;
                        }
                        _ => {
                            // Invalid sub-parameter for 38
                            sink.report_errror(
                                ParseError::InvalidParameter {
                                    command: "CsiSelectGraphicRendition",
                                    value: params[i + 1].to_string(),
                                    expected: Some("5 (256-color) or 2 (RGB)".to_string()),
                                },
                                crate::ErrorLevel::Error,
                            );
                            i += 1;
                        }
                    }
                } else {
                    // Missing sub-parameters
                    sink.report_errror(
                        ParseError::IncompleteSequence {
                            context: "Extended foreground color requires sub-parameters (38;5;n or 38;2;r;g;b)",
                        },
                        crate::ErrorLevel::Error,
                    );
                    i += 1;
                }
            }
            SgrLutEntry::ExtendedBackground => {
                // Extended background: 48;5;n (256-color) or 48;2;r;g;b (RGB)
                if i + 2 < params.len() {
                    match params[i + 1] {
                        5 if i + 2 < params.len() => {
                            // 256-color mode: 48;5;n
                            let color_index = params[i + 2] as u8;
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Extended(
                                color_index,
                            ))));
                            i += 3;
                        }
                        2 if i + 4 < params.len() => {
                            // RGB mode: 48;2;r;g;b
                            let r = params[i + 2] as u8;
                            let g = params[i + 3] as u8;
                            let b = params[i + 4] as u8;
                            sink.emit(TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Rgb(r, g, b))));
                            i += 5;
                        }
                        _ => {
                            // Invalid sub-parameter for 48
                            sink.report_errror(
                                ParseError::InvalidParameter {
                                    command: "CsiSelectGraphicRendition",
                                    value: params[i + 1].to_string(),
                                    expected: Some("5 (256-color) or 2 (RGB)".to_string()),
                                },
                                crate::ErrorLevel::Error,
                            );
                            i += 1;
                        }
                    }
                } else {
                    // Missing sub-parameters
                    sink.report_errror(
                        ParseError::IncompleteSequence {
                            context: "Extended background color requires sub-parameters (48;5;n or 48;2;r;g;b)",
                        },
                        crate::ErrorLevel::Error,
                    );
                    i += 1;
                }
            }
            SgrLutEntry::Undefined => {
                // Unrecognized or unsupported SGR code
                sink.report_errror(
                    ParseError::InvalidParameter {
                        command: "CsiSelectGraphicRendition",
                        value: format!("{}", code).to_string(),
                        expected: Some("valid SGR attribute code (0-107)".to_string()),
                    },
                    crate::ErrorLevel::Error,
                );
                i += 1;
            }
        }
    }
}

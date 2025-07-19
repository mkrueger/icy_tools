#![allow(clippy::unnecessary_wraps)]
use super::{BaudEmulation, EngineState, Parser, constants::COLOR_OFFSETS, set_font_selection_success};
use crate::{AttributedChar, BitFont, Buffer, CallbackAction, Caret, EngineResult, FontSelectionState, ParserError, TextPane, XTERM_256_PALETTE, update_crc16};

impl Parser {
    /// Sequence: `CSI Ps ... m`</p>
    /// Mnemonic: SGR</p>
    /// Description: Select graphic rendition</p>
    ///
    /// Parameter default value: Ps is 0
    ///
    /// SGR is used to establish one or more graphic rendition aspects for
    /// subsequent text. The established aspects remain in effect until the
    /// next occurrence of SGR in the data stream, depending on the setting of
    /// the GRAPHIC RENDITION COMBINATION MODE (GRCM). Each graphic rendition
    /// aspect is specified by a parameter value:
    ///
    /// [ In the following list, items are marked with their source on the right.
    ///   Items with no marking are ECMA-48 standard ones. ]
    ///
    /// 0   default rendition (implementation-defined), cancels the effect of
    ///     any preceding occurrence of SGR in the data stream regardless of the
    ///     setting of the GRAPHIC RENDITION COMBINATION MODE (GRCM)
    /// 1   bold or increased intensity
    /// 2   faint, decreased intensity or second colour
    /// 2   Sets the normal colors. This sequence takes the
    ///     next two arguments as the foreground and background
    ///     color to set, respectively  Uses SCO colour numbers.              [SCOANSI]
    /// 3   italicized
    /// 3   If backwards compatibility mode is enabled, then this sequence is
    ///     used to control the role of the blink bit in the M6845
    ///     video controller.  The argument following the 3
    ///     indicated whether this bit should be interpreted as
    ///     blink, or as bold background.  For example, the
    ///     sequence CSI 3;1 m will enable blinking text, whereas
    ///     the sequence CSI 3;0 m will enable bright background
    ///     colors.                                                           [SCOANSI]
    /// 4   singly underlined
    /// 5   slowly blinking (less then 150 per minute)
    /// 6   rapidly blinking (150 per minute or more)
    /// 6   VGA only: if blink (5) is on, turn blink off and background color to
    ///     its light equivalent (that is, brown to yellow)                     [`iBCS2`]
    /// 6   steady (not blinking)                                             [SCOANSI]
    /// 7   negative image
    /// 8   concealed characters
    /// 9   crossed-out (characters still legible but marked as to be deleted)
    /// 10  primary (default) font
    /// 10  reset selected mapping, display control flag, and toggle meta flag. [`iBCS2`]
    /// 11  first alternative font
    /// 11  select null mapping, set display control flag, reset toggle meta
    ///     flag.                                                               [`iBCS2`]
    /// 12  second alternative font
    /// 12  select null mapping, set display control flag, set toggle meta
    ///     flag. (The toggle meta flag causes the high bit of a byte to be
    ///     toggled before the mapping table translation is done.)              [Linux]
    /// 13  third alternative font
    /// 14  fourth alternative font
    /// 15  fifth alternative font
    /// 16  sixth alternative font
    /// 17  seventh alternative font
    /// 18  eighth alternative font
    /// 19  ninth alternative font
    /// 20  Fraktur (Gothic)
    /// 21  doubly underlined
    /// 21  set normal intensity                                                [Linux]
    /// 22  normal colour or normal intensity (neither bold nor faint)
    /// 23  not italicized, not fraktur
    /// 24  not underlined (neither singly nor doubly)
    /// 25  steady (not blinking)
    /// 26  (reserved for proportional spacing as specified in CCITT
    ///     Recommendation T.61)
    /// 27  positive image
    /// 28  revealed characters
    /// 29  not crossed out
    /// 30  black display
    /// 31  red display
    /// 32  green display
    /// 33  yellow display
    /// 34  blue display
    /// 35  magenta display
    /// 36  cyan display
    /// 37  white display
    /// 38  (reserved for future standardization; intended for setting
    ///     character foreground colour as specified in ISO 8613-6 [CCITT
    ///     Recommendation T.416])
    /// 38  set underscore on, set default foreground color                     [Linux]
    /// 38  If next two parameters are 5 and Ps, set foreground color to Ps     [xterm]
    /// 38  enables underline option; white foreground with white underscore    [`iBCS2`]
    /// 39  default display colour (implementation-defined)
    /// 39  disables underline option                                           [`iBCS2`]
    /// 40  black background
    /// 41  red background
    /// 42  green background
    /// 43  yellow background
    /// 44  blue background
    /// 45  magenta background
    /// 46  cyan background
    /// 47  white background
    /// 48  (reserved for future standardization; intended for setting
    ///     character background colour as specified in ISO 8613-6 [CCITT
    ///     Recommendation T.416])
    /// 48  If next two parameters are 5 and Ps, set background color to Ps     [xterm]
    /// 49  default background colour (implementation-defined)
    /// 50  (reserved for cancelling the effect of the rendering aspect
    ///     established by parameter value 26)
    /// 50  Reset to the original color pair                                  [SCOANSI]
    /// 51  framed
    /// 51  Reset all colors to the system default                            [SCOANSI]
    /// 52  encircled
    /// 53  overlined
    /// 54  not framed, not encircled
    /// 55  not overlined
    /// 56  (reserved for future standardization)
    /// 57  (reserved for future standardization)
    /// 58  (reserved for future standardization)
    /// 59  (reserved for future standardization)
    /// 60  ideogram underline or right side line
    /// 61  ideogram double underline or double line on the right side
    /// 62  ideogram overline or left side line
    /// 63  ideogram double overline or double line on the left side
    /// 64  ideogram stress marking
    /// 65  cancels the effect of the rendition aspects established by
    ///     parameter values 60 to 64
    /// 90  Set foreground color to (bright) Black                            [aixterm]
    /// 91  Set foreground color to (bright) Red                              [aixterm]
    /// 92  Set foreground color to (bright) Green                            [aixterm]
    /// 93  Set foreground color to (bright) Yellow                           [aixterm]
    /// 94  Set foreground color to (bright) Blue                             [aixterm]
    /// 95  Set foreground color to (bright) Magenta                          [aixterm]
    /// 96  Set foreground color to (bright) Cyan                             [aixterm]
    /// 97  Set foreground color to (bright) White                            [aixterm]
    ///
    /// 90  Set foreground color to (bright) Black                            [SCOANSI]
    /// 91  Set foreground color to (bright) Blue                             [SCOANSI]
    /// 92  Set foreground color to (bright) Green                            [SCOANSI]
    /// 93  Set foreground color to (bright) Cyan                             [SCOANSI]
    /// 94  Set foreground color to (bright) Red                              [SCOANSI]
    /// 95  Set foreground color to (bright) Magenta                          [SCOANSI]
    /// 96  Set foreground color to (bright) Yellow                           [SCOANSI]
    /// 97  Set foreground color to (bright) White                            [SCOANSI]
    ///
    /// 100 Set foreground and background color to default                       [rxvt]
    /// 100 Set background color to (bright) Black                            [aixterm]
    /// 101 Set background color to (bright) Red                              [aixterm]
    /// 102 Set background color to (bright) Green                            [aixterm]
    /// 103 Set background color to (bright) Yellow                           [aixterm]
    /// 104 Set background color to (bright) Blue                             [aixterm]
    /// 105 Set background color to (bright) Magenta                          [aixterm]
    /// 106 Set background color to (bright) Cyan                             [aixterm]
    /// 107 Set background color to (bright) White                            [aixterm]
    ///
    /// 100 Set background color to (bright) Black                            [SCOANSI]
    /// 101 Set background color to (bright) Blue                             [SCOANSI]
    /// 102 Set background color to (bright) Green                            [SCOANSI]
    /// 103 Set background color to (bright) Cyan                             [SCOANSI]
    /// 104 Set background color to (bright) Red                              [SCOANSI]
    /// 105 Set background color to (bright) Magenta                          [SCOANSI]
    /// 106 Set background color to (bright) Yellow                           [SCOANSI]
    /// 107 Set background color to (bright) White                            [SCOANSI]
    ///
    /// DEC private SGRs:
    /// ?1  Set secondary overprint mode                                        [LQP02]
    /// ?2  Enable shadow print                                                 [LQP02]
    ///
    /// NOTE
    /// The usable combinations of parameter values are determined by the
    /// implementation.
    ///
    /// Source: `ECMA-48 5th Ed. 8.3.118`
    /// Source: `Linux console_codes(4)`
    /// Source: `termtypes.master 10.2.7`
    /// Source: `XFree86: xc/doc/specs/xterm/ctlseqs.ms,v 3.29 1999/09/27 06:29:05`
    /// Source: `XFree86: xc/doc/specs/xterm/ctlseqs.ms,v 3.52 2004/04/18 15:18:48`
    /// Source: `UnixWare 7 display(7)`
    /// Source: `OpenServer 5.0.6 screen(HW)`
    /// Status: `standard; Linux, iBCS2, aixterm extensions`
    pub(crate) fn select_graphic_rendition(&mut self, caret: &mut Caret, buf: &mut Buffer) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        if self.parsed_numbers.is_empty() {
            caret.reset_color_attribute(); // Reset or normal
        }
        let mut i = 0;
        while i < self.parsed_numbers.len() {
            let n = self.parsed_numbers[i];
            match n {
                0 => caret.reset_color_attribute(), // Reset or normal
                1 => caret.attribute.set_is_bold(true),
                2 => {
                    caret.attribute.set_is_faint(true);
                }
                3 => {
                    caret.attribute.set_is_italic(true);
                }
                4 => caret.attribute.set_is_underlined(true),
                5 | 6 => {
                    caret.attribute.set_is_blinking(true);
                }
                7 => {
                    let fg = caret.attribute.get_foreground();
                    caret.attribute.set_foreground(caret.attribute.get_background());
                    caret.attribute.set_background(fg);
                }
                8 => {
                    caret.attribute.set_is_concealed(true);
                }
                9 => caret.attribute.set_is_crossed_out(true),
                10 => caret.set_font_page(0),                       // Primary (default) font
                11..=20 => { /* ignore alternate fonts for now */ } //return Err(ParserError::UnsupportedEscapeSequence(self.current_sequence.clone()).into()),
                21 => caret.attribute.set_is_double_underlined(true),
                22 => {
                    caret.attribute.set_is_bold(false);
                    caret.attribute.set_is_faint(false);
                }
                23 => caret.attribute.set_is_italic(false),
                24 => caret.attribute.set_is_underlined(false),
                25 => caret.attribute.set_is_blinking(false),
                27 => {
                    // 27  positive image ?
                    return Err(ParserError::UnsupportedEscapeSequence.into());
                }
                28 => caret.attribute.set_is_concealed(false),
                29 => caret.attribute.set_is_crossed_out(false),
                // set foreaground color
                30..=37 => caret.attribute.set_foreground(COLOR_OFFSETS[n as usize - 30] as u32),
                38 => {
                    caret.attribute.set_foreground(self.parse_extended_colors(buf, &mut i)?);
                    continue;
                }
                39 => caret.attribute.set_foreground(7), // Set foreground color to default, ECMA-48 3rd
                // set background color
                40..=47 => caret.attribute.set_background(COLOR_OFFSETS[n as usize - 40] as u32),
                48 => {
                    caret.attribute.set_background(self.parse_extended_colors(buf, &mut i)?);
                    continue;
                }
                49 => caret.attribute.set_background(0), // Set background color to default, ECMA-48 3rd
                /*
                50  (reserved for cancelling the effect of the rendering aspect
                    established by parameter value 26)
                50  Reset to the original color pair                                  [SCOANSI]
                51  framed
                51  Reset all colors to the system default                            [SCOANSI]
                52  encircled
                54  not framed, not encircled
                */
                53 => caret.attribute.set_is_overlined(true),
                55 => caret.attribute.set_is_overlined(false),
                // high intensity colors
                90..=97 => caret.attribute.set_foreground(8 + COLOR_OFFSETS[n as usize - 90] as u32),
                100..=107 => caret.attribute.set_background(8 + COLOR_OFFSETS[n as usize - 100] as u32),

                _ => {
                    return Err(ParserError::UnsupportedEscapeSequence.into());
                }
            }
            i += 1;
        }

        Ok(CallbackAction::Update)
    }

    fn parse_extended_colors(&mut self, buf: &mut Buffer, i: &mut usize) -> EngineResult<u32> {
        if *i + 1 >= self.parsed_numbers.len() {
            return Err(ParserError::UnsupportedEscapeSequence.into());
        }
        match self.parsed_numbers.get(*i + 1) {
            Some(5) => {
                // ESC[38/48;5;⟨n⟩m Select fg/bg color from 256 color lookup
                if *i + 3 > self.parsed_numbers.len() {
                    return Err(ParserError::UnsupportedEscapeSequence.into());
                }
                let color = self.parsed_numbers[*i + 2];
                *i += 3;
                if (0..=255).contains(&color) {
                    let color = buf.palette.insert_color(XTERM_256_PALETTE[color as usize].1.clone());
                    Ok(color)
                } else {
                    Err(ParserError::UnsupportedEscapeSequence.into())
                }
            }
            Some(2) => {
                // ESC[38/48;2;⟨r⟩;⟨g⟩;⟨b⟩ m Select RGB fg/bg color
                if *i + 5 > self.parsed_numbers.len() {
                    return Err(ParserError::UnsupportedEscapeSequence.into());
                }
                let r = self.parsed_numbers[*i + 2];
                let g = self.parsed_numbers[*i + 3];
                let b = self.parsed_numbers[*i + 4];
                *i += 5;
                if (0..=255).contains(&r) && (0..=255).contains(&g) && (0..=255).contains(&b) {
                    let color = buf.palette.insert_color_rgb(r as u8, g as u8, b as u8);
                    Ok(color)
                } else {
                    Err(ParserError::UnsupportedEscapeSequence.into())
                }
            }
            _ => Err(ParserError::UnsupportedEscapeSequence.into()),
        }
    }

    /// Sequence: `CSI Pn SP @`</p>
    /// Mnemonic: SL</p>
    /// Description: Scroll left</p>
    ///
    /// Parameter default value: Pn = 1
    ///
    /// SL causes the data in the presentation component to be moved by n
    /// character positions if the line orientation is horizontal, or by n
    /// line positions if the line orientation is vertical, such that the data
    /// appear to move to the left; where n equals the value of Pn.
    ///
    /// The active presentation position is not affected by this control function.
    ///
    /// Source: ECMA-48 5th Ed. 8.3.121
    /// Status: standard
    pub(crate) fn scroll_left(&mut self, buf: &mut Buffer, layer: usize) {
        let num = if let Some(number) = self.parsed_numbers.first() { *number } else { 1 };
        (0..num).for_each(|_| buf.scroll_left(layer));
    }

    /// Sequence: `CSI Pn SP A`</p>
    /// Mnemonic: SR</p>
    /// Description: Scroll right</p>
    ///
    /// Parameter default value: Pn = 1
    ///
    /// SR causes the data in the presentation component to be moved by n
    /// character positions if the line orientation is horizontal, or by n
    /// line positions if the line orientation is vertical, such that the data
    /// appear to move to the right; where n equals the value of Pn.
    ///
    /// The active presentation position is not affected by this control
    /// function.
    ///
    /// Source: ECMA-48 5th Ed. 8.3.135
    /// Status: standard
    pub(crate) fn scroll_right(&mut self, buf: &mut Buffer, layer: usize) {
        let num = if let Some(number) = self.parsed_numbers.first() { *number } else { 1 };
        (0..num).for_each(|_| buf.scroll_right(layer));
    }

    /// Sequence: `CSI Pt ; Pb r`</p>
    /// Mnemonic: DECSTBM</p>
    /// Description: Set top and bottom margins</p>
    ///
    /// Pt is the number of the top line of the scrolling region;
    /// Pb is the number of the bottom line of the scrolling region
    /// and must be greater than  Pt.
    /// (The default for Pt is line 1, the default for Pb is the end
    /// of the screen)
    ///
    /// Source: <URL:http://www.cs.utk.edu/~shuford/terminal/vt100_reference_card.txt>
    /// Status: DEC private; VT100
    pub(crate) fn set_top_and_bottom_margins(&mut self, buf: &mut Buffer, caret: &mut Caret) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        let (top, bottom) = match self.parsed_numbers.len() {
            2 => (self.parsed_numbers[0] - 1, self.parsed_numbers[1] - 1),
            1 => (0, self.parsed_numbers[0] - 1),
            0 => (0, buf.terminal_state.get_height()),
            _ => {
                return Err(ParserError::UnsupportedEscapeSequence.into());
            }
        };
        // CSI Pt ; Pb r
        // DECSTBM - Set Top and Bottom Margins

        buf.terminal_state.set_margins_top_bottom(top, bottom);
        caret.pos = buf.upper_left_position();
        Ok(CallbackAction::NoUpdate)
    }

    /// Sequence: `CSI Pn1 ; Pn2 s`</p>
    /// Mnemonic: DECSLRM</p>
    /// Description: Set left and right margins</p>
    ///
    /// Sets left margin to Pn1, right margin to Pn2
    ///
    /// Source: DEC Terminals and Printers Handbook 1985 EB 26291-56 pE10
    /// Status: DEC private; VT400, printers
    pub(crate) fn set_left_and_right_margins(&mut self, buf: &mut Buffer) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        let (left, right) = match self.parsed_numbers.len() {
            2 => (self.parsed_numbers[0] - 1, self.parsed_numbers[1] - 1),
            1 => (0, self.parsed_numbers[0] - 1),
            0 => (0, buf.terminal_state.get_height()),
            _ => {
                return Err(ParserError::UnsupportedEscapeSequence.into());
            }
        };
        // Set Left and Right Margins
        buf.terminal_state.set_margins_left_right(left, right);

        Ok(CallbackAction::NoUpdate)
    }

    /// Sequence: `CSI Pn1 ; Pn2 ; Pn3 ; Pn4 r`</p>
    /// Mnemonic: CSR</p>
    /// Description: Change Scrolling Region</p>
    ///
    /// Where 3 or more parameters are specified, the parameters are the top,
    /// bottom, left and right margins respectively. If you omit the last
    /// parameter, the extreme edge of the screen is assumed to be the right
    /// margin.
    ///
    /// If any of the parameters are out of bounds, they are clipped. If any
    /// of the parameters would cause an overlap (i.e. the bottom margin is
    /// higher than the top margin, or the right margin is less that the left
    /// margin), then this command is ignored and no scrolling region or
    /// window will be active. If all of the parameters are correct, then the
    /// cursor is moved to the top left hand corner of the newly-created
    /// region. The new region will now define the bounds of all scroll and
    /// cursor motion operations.
    pub(crate) fn change_scrolling_region(&mut self, buf: &mut Buffer, caret: &mut Caret) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        let (top, bottom, left, right) = match self.parsed_numbers.len() {
            3 => (
                self.parsed_numbers[0] - 1,
                self.parsed_numbers[1] - 1,
                self.parsed_numbers[2] - 1,
                buf.terminal_state.get_width(),
            ),
            4 => (
                self.parsed_numbers[0] - 1,
                self.parsed_numbers[1] - 1,
                self.parsed_numbers[2] - 1,
                self.parsed_numbers[3] - 1,
            ),
            _ => {
                return Err(ParserError::UnsupportedEscapeSequence.into());
            }
        };

        caret.pos = buf.upper_left_position();
        buf.terminal_state.set_margins_top_bottom(top, bottom);
        buf.terminal_state.set_margins_left_right(left, right);

        Ok(CallbackAction::NoUpdate)
    }

    /// Sequence: CSI = Ps ; Pn m
    /// Mnemonic: SSM
    /// Description: Set specific margin
    ///
    ///  This sequence can be used to set any one of the 4 margins. Parameter
    ///  Ps indicates which margin to set (Ps=0 for the top margin, Ps=1 for
    ///  the bottom, Ps=2 for the left and Ps=3 for the right). Pn is the row
    ///  or column to set the margin to. If after this control sequence has
    ///  been processed, the top or bottom margins are not at the top of the
    ///  screen, and the left and right margins are at the screen boundary,
    ///  then the scrolling region is set to the size specified.  If either of
    ///  the left or right margins are not at the screen boundary then the
    ///  scrolling region is bound by the current margins.
    pub(crate) fn set_specific_margin(&mut self, buf: &mut Buffer) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        let n = self.parsed_numbers[1] - 1;

        match self.parsed_numbers.first() {
            Some(0) => {
                let top = if let Some((t, _)) = buf.terminal_state.get_margins_top_bottom() {
                    t
                } else {
                    0
                };
                buf.terminal_state.set_margins_top_bottom(top, n);
            }
            Some(1) => {
                let bottom = if let Some((_, b)) = buf.terminal_state.get_margins_top_bottom() {
                    b
                } else {
                    buf.terminal_state.get_height() - 1
                };
                buf.terminal_state.set_margins_top_bottom(n, bottom);
            }
            Some(2) => {
                let left = if let Some((l, _)) = buf.terminal_state.get_margins_left_right() {
                    l
                } else {
                    0
                };
                buf.terminal_state.set_margins_left_right(left, n);
            }
            Some(3) => {
                let right = if let Some((_, r)) = buf.terminal_state.get_margins_left_right() {
                    r
                } else {
                    buf.terminal_state.get_width() - 1
                };
                buf.terminal_state.set_margins_left_right(n, right);
            }
            Some(_n) => {
                return Err(ParserError::UnsupportedEscapeSequence.into());
            }
            None => {
                return Err(ParserError::UnsupportedEscapeSequence.into());
            }
        }
        Ok(CallbackAction::NoUpdate)
    }

    /// Sequence: `CSI = r`</p>
    /// Mnemonic: RSM</p>
    /// Description: Reset margins</p>
    ///
    ///  This sequence can be used to reset all of the margins to cover the
    ///  entire screen. This will deactivate the scrolling region (if
    ///  defined). If not, this sequence has no effect.  The cursor is not
    ///  moved.
    ///
    /// Source: `OpenServer 5.0.6 screen(HW)`
    /// Status: SCO private
    pub(crate) fn reset_margins(&mut self, buf: &mut Buffer) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        buf.terminal_state.clear_margins_left_right();
        buf.terminal_state.clear_margins_top_bottom();
        Ok(CallbackAction::NoUpdate)
    }

    /// Sequence: `CSI s`</p>
    /// Mnemonic: SCP</p>
    /// Description: Save cursor position</p>
    ///
    /// Save the current cursor position. The cursor position can be restored
    /// later using the RCP sequence.
    ///
    /// Source: `OpenServer 5.0.6 screen(HW)`
    /// Status: SCO private
    pub(crate) fn save_cursor_position(&mut self, caret: &Caret) {
        self.state = EngineState::Default;
        self.saved_pos = caret.pos;
    }

    /// Sequence: `CSI u`</p>
    /// Mnemonic: RCP</p>
    /// Description: Restore cursor position</p>
    ///
    /// Restore the cursor to the position it occupied at the last time an SCP
    /// sequence was received.
    ///
    /// Source: `OpenServer 5.0.6 screen(HW)`
    /// Status: SCO private
    pub(crate) fn restore_cursor_position(&mut self, caret: &mut Caret) {
        // CSI u
        // RCP - Restore Cursor Position
        self.state = EngineState::Default;
        caret.pos = self.saved_pos;
    }

    /// Sequence: `CSI Pn X`</p>
    /// Mnemonic: ECH</p>
    /// Description: Erase character</p>
    ///
    /// Parameter default value: Pn = 1
    ///
    /// If the DEVICE COMPONENT SELECT MODE (DCSM) is set to PRESENTATION, ECH
    /// causes the active presentation position and the n-1 following
    /// character positions in the presentation component to be put into the
    /// erased state, where n equals the value of Pn.
    ///
    /// If the DEVICE COMPONENT SELECT MODE (DCSM) is set to DATA, ECH causes
    /// the active data position and the n-1 following character positions in
    /// the data component to be put into the erased state, where n equals the
    /// value of Pn.
    ///
    /// Whether the character positions of protected areas are put into the
    /// erased state, or the character positions of unprotected areas only,
    /// depends on the setting of the ERASURE MODE (ERM).
    ///
    /// Source: ECMA-48 5th Ed. 8.3.38
    /// Status: standard
    pub(crate) fn erase_character(&mut self, caret: &mut Caret, buf: &mut Buffer, current_layer: usize) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        // ECH - Erase character

        if let Some(number) = self.parsed_numbers.first() {
            caret.erase_charcter(buf, current_layer, *number);
        } else {
            caret.erase_charcter(buf, current_layer, 1);
            if self.parsed_numbers.len() != 1 {
                return Err(ParserError::UnsupportedEscapeSequence.into());
            }
        }
        Ok(CallbackAction::NoUpdate)
    }

    /// Sequence: `CSI Ps1 ; Ps2 SP D`</p>
    /// Mnemonic: FNT</p>
    /// Description: Font selection</p>
    ///
    /// Parameter default values: Ps1 = 0; Ps2 =0
    ///
    /// FNT is used to identify the character font to be selected as primary
    /// or alternative font by subsequent occurrences of SELECT GRAPHIC
    /// RENDITION (SGR) in the data stream. Ps1 specifies the primary or
    /// alternative font concerned:
    ///
    /// 0 primary font
    /// 1 first alternative font
    /// 2 second alternative font
    /// 3 third alternative font
    /// 4 fourth alternative font
    /// 5 fifth alternative font
    /// 6 sixth alternative font
    /// 7 seventh alternative font
    /// 8 eighth alternative font
    /// 9 ninth alternative font
    ///
    /// Ps2 identifies the character font according to a register which is to
    /// be established.
    ///
    /// Source: ECMA-48 5th Ed. 8.3.53
    /// Status: standard
    ///
    pub(crate) fn font_selection(&mut self, buf: &mut Buffer, caret: &mut Caret) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        if self.parsed_numbers.len() != 2 {
            return Err(ParserError::UnsupportedEscapeSequence.into());
        }

        // Ignore Ps1 for now
        if let Some(nr) = self.parsed_numbers.get(1) {
            let nr = *nr as usize;
            if buf.get_font(nr).is_some() {
                set_font_selection_success(buf, caret, nr);
                return Ok(CallbackAction::NoUpdate);
            }
            match BitFont::from_ansi_font_page(nr) {
                Ok(font) => {
                    set_font_selection_success(buf, caret, nr);
                    buf.set_font(nr, font);
                }
                Err(err) => {
                    buf.terminal_state.font_selection_state = FontSelectionState::Failure;
                    return Err(err);
                }
            }
        }
        Ok(CallbackAction::NoUpdate)
    }

    /// Sequence: `CSI Pn SP d`</p>
    /// Mnemonic: TSR</p>
    /// Description: Tabulation stop remove</p>
    ///
    /// No parameter default value.
    ///
    /// TSR causes any character tabulation stop at character position n in
    /// the active line (the line that contains the active presentation
    /// position) and lines of subsequent text in the presentation component
    /// to be cleared, but does not affect other tabulation stops. n equals
    /// the value of Pn.
    ///
    /// Source: ECMA-48 5th Ed. 8.3.156
    /// Status: standard
    pub(crate) fn tabulation_stop_remove(&mut self, buf: &mut Buffer) -> EngineResult<CallbackAction> {
        if self.parsed_numbers.len() != 1 {
            return Err(ParserError::UnsupportedEscapeSequence.into());
        }
        // tab stop remove
        if let Some(num) = self.parsed_numbers.first() {
            buf.terminal_state.remove_tab_stop(*num - 1);
        }
        Ok(CallbackAction::NoUpdate)
    }

    /// Sequence: `CSI ! p`</p>
    /// Mnemonic: DECSTR</p>
    /// Description: Soft terminal reset</p>
    ///
    /// sets terminal to power-up default states
    ///
    /// Source: <URL:http://www.cs.utk.edu/~shuford/terminal/dec_vt220_codes.txt>
    ///         "VT220 Programmer Pocket Guide" EK-VT220-HR-001, page 33
    ///
    /// (keeps screen)
    ///
    /// Source: <URL:http://www.cs.utk.edu/~shuford/terminal/msvibm_vt.txt>
    /// Status: DEC private; VT220
    pub(crate) fn soft_terminal_reset(&mut self, buf: &mut Buffer, caret: &mut Caret) {
        self.state = EngineState::Default;
        buf.reset_terminal();
        caret.reset();
    }

    /// Sequence: `CSI Ps1 ; Ps2 * r`</p>
    /// Mnemonic: DECSCS</p>
    /// Description: Select Communication Speed</p>
    ///
    /// Select a communication speed for a communication line.
    ///
    /// Parameters
    ///
    /// Ps1 indicates the communication line.
    ///
    /// Ps1           Comm Line Type
    /// 1, 0 or none  Host Transmit
    /// 2             Host Receive
    /// 3             Printer
    /// 4             Modem Hi
    /// 5             Modem Lo
    ///
    /// Ps2 indicates the communication speed.
    ///
    /// Ps2          Speed      Ps2  Speed
    /// 0 or none  default      6     9600
    /// 1              300      7    19200
    /// 2              600      8    38400
    /// 3             1200      9    57600
    /// 4             2400      10   76800
    /// 5             4800      11  115200
    ///
    /// The default value depends on the type of communication line.
    ///
    /// Communication Line  Default Communication Speed
    /// Host Transmit       9600
    /// Host Receive        Receive=transmit
    /// Printer             4800
    /// Modem Hi            Ignore
    /// Modem Lo            Ignore
    pub(crate) fn select_communication_speed(&mut self, buf: &mut Buffer) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        let ps1 = self.parsed_numbers.first().unwrap_or(&0);
        if *ps1 != 0 && *ps1 != 1 {
            // silently ignore all other options
            // 2 	Host Receive
            // 3 	Printer
            // 4 	Modem Hi
            // 5 	Modem Lo
            return Ok(CallbackAction::NoUpdate);
        }

        // no ps2 or 0 are equiv in disabling baud emulation
        let ps2 = self.parsed_numbers.get(1).unwrap_or(&0);
        let baud_option = *BaudEmulation::OPTIONS.get(*ps2 as usize).unwrap_or(&BaudEmulation::Off);

        buf.terminal_state.set_baud_rate(baud_option);
        Ok(CallbackAction::ChangeBaudEmulation(baud_option))
        // DECSCS—Select Communication Speed https://vt100.net/docs/vt510-rm/DECSCS.html
    }

    /// Sequence: `CSI Pn1 ; Pn2 ; Pn3 ; Pn4 ; Pn5 ; Pn6 * y`</p>
    /// Mnemonic: DECRQCRA</p>
    /// Description: Request checksum of rectangular area</p>
    ///
    /// Request a memory checksum of a rectangular area on a specified
    /// page. The terminal returns a checksum report (DECCKSR) in
    /// response. DECRQCRA also works on the status line.
    ///
    /// Parameters:
    ///
    /// Pn1 A numeric label you give to identify the checksum request
    /// (DECCKSR returns this number).
    ///
    /// Pn2 The number of the page on which the rectangular area is
    /// located. If Pn2 is 0 or omitted, the terminal ignores the remaining
    /// parameters and reports a checksum for all pages in page memory. If
    /// <n2> is more than the number of pages, Reflection does a checksum on
    /// the last page.
    ///
    /// Pn3 to Pn6 define the area to be checksummed:
    ///
    /// Pn3 Top row
    /// Pn4 Right column
    /// Pn5 Bottom row
    /// Pn6 Left column
    ///
    /// If Pn3 .. Pn6 are omitted, the entire page is checksummed.  The
    /// co-ordinates are affected by DECOM.
    ///
    /// Source: Reflection TRM (VT) Version 7.0
    /// Status: DEC private; VT400
    pub(crate) fn request_checksum_of_rectangular_area(&mut self, buf: &Buffer) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        if self.parsed_numbers.len() != 6 {
            return Err(ParserError::UnsupportedEscapeSequence.into());
        }
        let pt = self.parsed_numbers[2];
        let pl = self.parsed_numbers[3];
        let pb = self.parsed_numbers[4];
        let pr = self.parsed_numbers[5];
        if pt > pb || pl > pr || pr > buf.terminal_state.get_width() || pb > buf.terminal_state.get_height() || pl < 0 || pt < 0 {
            return Err(ParserError::UnsupportedEscapeSequence.into());
        }
        let mut crc16 = 0;
        for y in pt..pb {
            for x in pl..pr {
                let ch = buf.get_char((x, y));
                if ch.is_visible() {
                    crc16 = update_crc16(crc16, ch.ch as u8);
                    for b in ch.attribute.attr.to_be_bytes() {
                        crc16 = update_crc16(crc16, b);
                    }
                    for b in ch.attribute.get_foreground().to_be_bytes() {
                        crc16 = update_crc16(crc16, b);
                    }
                    for b in ch.attribute.get_background().to_be_bytes() {
                        crc16 = update_crc16(crc16, b);
                    }
                }
            }
        }
        Ok(CallbackAction::SendString(format!("\x1BP{}!~{crc16:04X}\x1B\\", self.parsed_numbers[0])))
        // DECRQCRA—Request Checksum of Rectangular Area
        // <https://vt100.net/docs/vt510-rm/DECRQCRA.html>
    }

    /// Sequence: `CSI Pn * z`</p>
    /// Mnemonic: DECINVM</p>
    /// Description: Invoke Macro</p>
    ///
    /// Invoke a stored macro. Pn is the macro ID number used in DECDMAC. If
    /// Pn is not associated with a particular macro, Reflection ignores this
    /// control function. If a macro definition includes control functions,
    /// these functions remain in effect after the macro is invoked.
    ///
    /// Source: Reflection TRM (VT) Version 7.0
    /// Status: DEC private; VT400
    pub(crate) fn invoke_macro(&mut self, buf: &mut Buffer, current_layer: usize, caret: &mut Caret) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        if let Some(id) = self.parsed_numbers.first() {
            self.invoke_macro_by_id(buf, current_layer, caret, *id);
        }
        Ok(CallbackAction::Update)
    }

    /// Sequence: `CSI Ps c`</p>
    /// Mnemonic: DA</p>
    /// Description: Device attributes</p>
    ///
    /// Parameter default value: Ps is 0
    ///
    /// With a parameter value not equal to 0, DA is used to identify the
    /// device which sends the DA. The parameter value is a device type
    /// identification code according to a register which is to be
    /// established. If the parameter value is 0, DA is used to request an
    /// identifying DA from a device.
    ///
    /// Source: ECMA-48 5th Ed 8.3.24
    /// Status: standard
    pub(crate) fn device_attributes(&mut self) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;
        // respond with IcyTerm as ASCII followed by the package version.
        Ok(CallbackAction::SendString(format!(
            "\x1b[=73;99;121;84;101;114;109;{};{};{}c",
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH")
        )))
    }

    /// Sequence: `CSI Ps ; Pn1 ; Pn2 ; Pn3 t`</p>
    /// Mnemonic: CT24BC</p>
    /// Description: Select a 24-bit colour</p>
    ///
    /// If Ps is 0, sets the background colour.
    /// If Ps is 1, sets the foreground colour.
    /// Pn1, Pn2, Pn3 contains the RGB value to set.
    /// `CTerm` handles this with an internal temporary palette, so scrollback
    /// may not have the correct colours.  The internal palette is large
    /// enough for all cells in a 132x60 screen to have unique foreground
    /// and background colours though, so the current screen should always
    /// be as expected.
    ///
    /// Source: CTerm.txt
    /// Status: NON-STANDARD EXTENSION
    pub(crate) fn select_24bit_color(&mut self, buf: &mut Buffer, caret: &mut Caret) -> EngineResult<CallbackAction> {
        let r = self.parsed_numbers[1];
        let g = self.parsed_numbers[2];
        let b = self.parsed_numbers[3];
        let color = buf.palette.insert_color_rgb(r as u8, g as u8, b as u8);
        match self.parsed_numbers.first() {
            Some(0) => {
                caret.attribute.set_background(color);
            }
            Some(1) => {
                caret.attribute.set_foreground(color);
            }
            _ => {
                return Err(ParserError::UnsupportedEscapeSequence.into());
            }
        }
        Ok(CallbackAction::Update)
    }

    /// Sequence: `CSI Ps ; Ps ; Ps t`</p>
    /// Mnemonic: </p>
    /// Description: Manipulates the terminal window</p>
    ///
    /// Window manipulation (from dtterm, as well as extensions).
    /// These controls may be disabled using the `allowWindowOps`
    /// resource.  Valid values for the first (and any additional parameters) are:
    ///
    /// Ps is 1  -> Deiconify window.
    ///  Ps is 2  -> Iconify window.
    ///  Ps is 3  ;  x ;  y -> Move window to [x, y].
    ///  Ps is 4  ;  height ;  width -> Resize the xterm window to
    ///given height and width in pixels.  Omitted parameters reuse
    ///the current height or width.  Zero parameters use the dis-
    ///play's height or width.
    ///  Ps is 5  -> Raise the xterm window to the front of the stack-ing order.
    ///  Ps is 6  -> Lower the xterm window to the bottom of the stacking order.
    ///  Ps is 7  -> Refresh the xterm window.
    ///  Ps is 8  ;  height ;  width -> Resize the text area to given
    ///height and width in characters.  Omitted parameters reuse the
    ///current height or width.  Zero parameters use the display's
    ///height or width.
    ///  Ps is 9  ;  0  -> Restore maximized window.
    ///  Ps is 9  ;  1  -> Maximize window (i.e., resize to screen size).
    ///  Ps is 9  ;  2  -> Maximize window vertically.
    ///  Ps is 9  ;  3  -> Maximize window horizontally.
    ///  Ps is 1 0  ;  0  -> Undo full-screen mode.
    ///  Ps is 1 0  ;  1  -> Change to full-screen.
    ///  Ps is 1 0  ;  2  -> Toggle full-screen.
    ///  Ps is 1 1  -> Report xterm window state.  If the xterm window
    ///is open (non-iconified), it returns CSI 1 t .  If the xterm
    ///window is iconified, it returns CSI 2 t .
    ///  Ps is 1 3  -> Report xterm window position. Result is CSI 3 ; x ; y t
    ///  Ps is 1 4  -> Report xterm window in pixels. Result is CSI  4  ;  height ;  width t
    ///  Ps is 1 8  -> Report the size of the text area in characters. Result is CSI  8  ;  height ;  width t
    ///  Ps is 1 9  -> Report the size of the screen in characters. Result is CSI  9  ;  height ;  width t
    ///  Ps is 2 0  -> Report xterm window's icon label. Result is OSC  L  label ST
    ///  Ps is 2 1  -> Report xterm window's title. Result is OSC  l  label ST
    ///  Ps is 2 2  ;  0  -> Save xterm icon and window title on stack.
    ///  Ps is 2 2  ;  1  -> Save xterm icon title on stack.
    ///  Ps is 2 2  ;  2  -> Save xterm window title on stack.
    ///  Ps is 2 3  ;  0  -> Restore xterm icon and window title from stack.
    ///  Ps is 2 3  ;  1  -> Restore xterm icon title from stack.
    ///  Ps is 2 3  ;  2  -> Restore xterm window title from stack.
    ///  Ps >= 2 4  -> Resize to Ps lines (DECSLPP).    /// Source: XTerm-Control-Sequences.txt
    /// Status: NON-STANDARD EXTENSION
    pub(crate) fn window_manipulation(&mut self, buf: &mut Buffer) -> EngineResult<CallbackAction> {
        match self.parsed_numbers.first() {
            Some(8) => {
                if self.parsed_numbers.len() != 3 {
                    return Err(ParserError::UnsupportedEscapeSequence.into());
                }
                let width = self.parsed_numbers[2].min(132).max(1);
                let height = self.parsed_numbers[1].min(60).max(1);
                buf.terminal_state.set_width(width);
                buf.terminal_state.set_height(height);
                Ok(CallbackAction::ResizeTerminal(width, height))
            }
            _ => Err(ParserError::UnsupportedEscapeSequence.into()),
        }
    }

    /// Sequence: `CSI Pn1 ; Pn2 ; Pn3 ; Pn4 ; Pn5 $ x`</p>
    /// Mnemonic: DECFRA</p>
    /// Description: Fill rectangular area</p>
    ///
    /// Fill an area in display memory with a specified character. The fill
    /// character takes on the visual attributes set by the last SGR control
    /// function, not the attributes of the characters that it replaces.
    /// Current line attributes (for example, the attributes that specify
    /// double-wide, double-high characters) remain unchanged. The parameters
    /// are:
    ///
    /// Pn1 Decimal code of fill character
    /// Pn2 Top line
    /// Pn3 Left column
    /// Pn4 Bottom line
    /// Pn5 Right column
    ///
    /// Source: Reflection TRM (VT) Version 7.0
    /// Status: DEC private; VT400
    pub(crate) fn fill_rectangular_area(&mut self, buf: &mut Buffer, caret: &Caret) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;

        if self.parsed_numbers.len() != 5 {
            return Err(ParserError::UnsupportedEscapeSequence.into());
        }
        let ch: char = unsafe { char::from_u32_unchecked(self.parsed_numbers[0] as u32) };

        let (top_line, left_column, bottom_line, right_column) = self.get_rect_area(buf, 1);
        for y in top_line..=bottom_line {
            for x in left_column..=right_column {
                buf.layers[0].set_char((x, y), AttributedChar::new(ch, caret.attribute));
            }
        }

        Ok(CallbackAction::Update)
    }

    fn get_rect_area(&self, buf: &Buffer, offset: usize) -> (i32, i32, i32, i32) {
        let top_line: i32 = self.parsed_numbers[offset]
            .max(1)
            .min(buf.get_line_count().max(buf.terminal_state.get_height()))
            - 1;
        let left_column = self.parsed_numbers[offset + 1].max(1).min(buf.terminal_state.get_width()) - 1;

        let bottom_line = self.parsed_numbers[offset + 2]
            .max(1)
            .min(buf.get_line_count().max(buf.terminal_state.get_height()))
            - 1;
        let right_column = self.parsed_numbers[offset + 3].max(1).min(buf.terminal_state.get_width()) - 1;

        (top_line, left_column, bottom_line, right_column)
    }

    /// Sequence: `CSU Pn1 ; Pn2 ; Pn3 ; Pn4 $ z`</p>
    /// Mnemonic: DECERA</p>
    /// Description: Erase rectangular area</p>
    ///
    /// Erase the characters (and their visual attributes) in the specified
    /// rectangular area and replace each one with a space (decimal 32). Line
    /// attributes (for example, the attributes that specify double-wide,
    /// double-high characters) are not erased. The areas to erase are:
    ///
    /// Pn1 Top line
    /// Pn2 Left column
    /// Pn3 Bottom line
    /// Pn4 Right column
    ///
    /// Source: Reflection TRM (VT) Version 7.0
    /// Status: DEC private; VT400
    pub(crate) fn erase_rectangular_area(&mut self, buf: &mut Buffer) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;

        if self.parsed_numbers.len() != 4 {
            return Err(ParserError::UnsupportedEscapeSequence.into());
        }

        let (top_line, left_column, bottom_line, right_column) = self.get_rect_area(buf, 0);

        for y in top_line..=bottom_line {
            for x in left_column..=right_column {
                buf.layers[0].set_char((x, y), AttributedChar::default());
            }
        }

        Ok(CallbackAction::Update)
    }

    /// Sequence: `CSI Pn1 ; Pn2 ; Pn3 ; Pn4 $ {`</p>
    /// Mnemonic: DECSERA</p>
    /// Description: Selective erase rectangular area</p>
    ///
    /// Erase all erasable characters from a specified rectangular area in
    /// page memory; a space character replaces erased character
    /// positions. The DECSERA control function does not change:
    ///
    /// * Visual attributes set by the select graphic rendition (SGR) function.
    /// * Protection attributes set by DECSCA.
    /// * Line attributes.
    ///
    /// The parameters are:
    /// Pn1 Top line
    /// Pn2 Left column
    /// Pn3 Bottom line
    /// Pn4 Right column
    ///
    /// Source: Reflection TRM (VT) Version 7.0
    /// Status: DEC private; VT400
    pub(crate) fn selective_erase_rectangular_area(&mut self, buf: &mut Buffer) -> EngineResult<CallbackAction> {
        self.state = EngineState::Default;

        if self.parsed_numbers.len() != 4 {
            return Err(ParserError::UnsupportedEscapeSequence.into());
        }

        let (top_line, left_column, bottom_line, right_column) = self.get_rect_area(buf, 0);

        for y in top_line..=bottom_line {
            for x in left_column..=right_column {
                let ch = buf.get_char((x, y));
                buf.layers[0].set_char((x, y), AttributedChar::new(' ', ch.attribute));
            }
        }

        Ok(CallbackAction::Update)
    }
}

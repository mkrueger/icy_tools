use icy_engine::{AttributedChar, IceMode, Position, TextAttribute, TextBuffer, TextPane};
use semver::Version;

/// Scans through a buffer to find '@' characters and replaces them with
/// a colored version string. Returns the position of '#' if found (for ready message).
///
/// Version format: v{major}.{minor}.{patch}
/// Colors: 'v' white (7), major yellow (14), dots green (10), minor light red (12), patch magenta (13)
pub fn replace_version_marker(buffer: &mut TextBuffer, version: &Version, build: Option<String> ) -> Option<(i32, i32)> {
    let mut ready_position = None;
    let mut had_version = false;
    for y in 0..buffer.get_height() {
        for x in 0..buffer.get_width() {
            let ch = buffer.get_char((x, y).into());

            if ch.ch == '@' {
                if had_version {
                    if let Some(build_date) = &build {
                        for (i, ch) in build_date.chars().enumerate() {
                            let new_x = x + i as i32;
                            if new_x < buffer.get_width() {
                                let new_ch = AttributedChar::new(ch, TextAttribute::from_u8(0x08, icy_engine::IceMode::Ice));
                                buffer.layers[0].set_char(Position::new(new_x, y), new_ch);
                            }
                        }
                    }
                } else { 
                    had_version = true;
                    // Build version string with colors
                    let mut version_chars: Vec<AttributedChar> = Vec::new();

                    // 'v' in white (color 7)
                    version_chars.push(AttributedChar::new('v', TextAttribute::from_u8(0x07, IceMode::Ice)));

                    // Major version in yellow (color 14)
                    let major_str = version.major.to_string();
                    for ch in major_str.chars() {
                        version_chars.push(AttributedChar::new(ch, TextAttribute::from_u8(0x0E, IceMode::Ice)));
                    }

                    // First dot in green (color 10)
                    version_chars.push(AttributedChar::new('.', TextAttribute::from_u8(0x0A, IceMode::Ice)));

                    // Minor version in light red (color 12)
                    let minor_str = version.minor.to_string();
                    for ch in minor_str.chars() {
                        version_chars.push(AttributedChar::new(ch, TextAttribute::from_u8(0x0C, IceMode::Ice)));
                    }

                    // Second dot in green (color 10)
                    version_chars.push(AttributedChar::new('.', TextAttribute::from_u8(0x0A, IceMode::Ice)));

                    // Patch/build version in magenta (color 13)
                    let patch_str = version.patch.to_string();
                    for ch in patch_str.chars() {
                        version_chars.push(AttributedChar::new(ch, TextAttribute::from_u8(0x0D, IceMode::Ice)));
                    }

                    // Place the colored version at the @ position
                    for (i, new_ch) in version_chars.into_iter().enumerate() {
                        let new_x = x + i as i32;
                        if new_x < buffer.get_width() {
                            buffer.layers[0].set_char(Position::new(new_x, y), new_ch);
                        }
                    }
                }
            } else if ch.ch == '#' {
                // Mark position for ready message
                ready_position = Some((x, y));
                // Clear the # character
                let mut cleared = ch;
                cleared.ch = ' ';
                buffer.layers[0].set_char(Position::new(x, y), cleared);
            }
        }
        buffer.update_hyperlinks();
    }

    ready_position
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BrushPrimaryMode {
    #[default]
    Char,
    HalfBlock,
    Shading,
    Replace,
    Blink,
    Colorize,
}

/// Most characters or colors a shade ramp can hold.
pub const MAX_RAMP_LEN: usize = 16;

/// A short fixed-capacity list, so brush settings stay `Copy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ramp<T: Copy> {
    items: [T; MAX_RAMP_LEN],
    len: u8,
}

impl<T: Copy + Default> Ramp<T> {
    /// A ramp of the first [`MAX_RAMP_LEN`] of `items`.
    pub fn new(items: &[T]) -> Self {
        let mut ramp = Self::default();
        let len = items.len().min(MAX_RAMP_LEN);
        ramp.items[..len].copy_from_slice(&items[..len]);
        ramp.len = len as u8;
        ramp
    }

    pub fn as_slice(&self) -> &[T] {
        &self.items[..self.len as usize]
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }
}

impl<T: Copy + Default> Default for Ramp<T> {
    fn default() -> Self {
        Self {
            items: [T::default(); MAX_RAMP_LEN],
            len: 0,
        }
    }
}

/// CP437 characters of a shade ramp, from light to dark. Empty keeps the characters.
pub type CharRamp = Ramp<char>;
/// Palette indices of a color ramp, from the first to the last shading step. Empty uses the
/// brush color.
pub type ColorRamp = Ramp<u8>;

/// The classic `░▒▓█` shade ramp.
pub fn default_char_ramp() -> CharRamp {
    Ramp::new(&icy_engine_edit::brushes::SHADE_GRADIENT)
}

/// Converts typed or pasted text to a CP437 character ramp. Spaces are skipped (the empty cell
/// is always the lightest step); characters CP437 lacks and control codes, which ANSI output
/// cannot show, are an error.
pub fn char_ramp_from_text(text: &str) -> Result<CharRamp, char> {
    let mut chars = Vec::new();
    for ch in text.chars().filter(|ch| !ch.is_whitespace()) {
        let code = icy_engine::BufferType::CP437.convert_from_unicode(ch);
        if !(32..=255).contains(&(code as u32)) {
            return Err(ch);
        }
        if !chars.contains(&code) {
            chars.push(code);
        }
    }
    Ok(Ramp::new(&chars))
}

/// The ramp as Unicode text, e.g. for editing and the settings file.
pub fn char_ramp_text(ramp: &CharRamp) -> String {
    ramp.as_slice().iter().map(|&ch| icy_engine::BufferType::CP437.convert_to_unicode(ch)).collect()
}

/// The user's shade ramps, stored in the settings file.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShadeRamps {
    /// Character ramps as Unicode text, from light to dark.
    #[serde(default = "default_char_ramps")]
    pub characters: Vec<String>,
    /// Color ramps as palette indices.
    #[serde(default = "default_color_ramps")]
    pub colors: Vec<Vec<u8>>,
}

impl Default for ShadeRamps {
    fn default() -> Self {
        Self {
            characters: default_char_ramps(),
            colors: default_color_ramps(),
        }
    }
}

impl ShadeRamps {
    /// The valid, non-empty character ramps.
    pub fn char_ramps(&self) -> Vec<CharRamp> {
        self.characters
            .iter()
            .filter_map(|text| char_ramp_from_text(text).ok())
            .filter(|ramp| !ramp.is_empty())
            .collect()
    }

    /// The non-empty color ramps.
    pub fn color_ramps(&self) -> Vec<ColorRamp> {
        self.colors.iter().filter(|colors| !colors.is_empty()).map(|colors| Ramp::new(colors)).collect()
    }
}

fn default_char_ramps() -> Vec<String> {
    vec!["░▒▓█".into(), "·∙■█".into(), ".:oO@".into()]
}

fn default_color_ramps() -> Vec<Vec<u8>> {
    vec![vec![8, 7, 15], vec![1, 9, 11, 15], vec![4, 12, 14, 15], vec![2, 10, 11, 15], vec![5, 13, 15]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramps_convert_between_text_and_cp437() {
        let ramp = char_ramp_from_text("░ ▒▓█▓").unwrap();
        assert_eq!(ramp.as_slice(), &icy_engine_edit::brushes::SHADE_GRADIENT);
        assert_eq!(char_ramp_text(&ramp), "░▒▓█");
        assert_eq!(char_ramp_from_text("€"), Err('€'));
        assert_eq!(char_ramp_from_text("•"), Err('•'), "CP437 control codes are not usable");
        assert!(char_ramp_from_text("").unwrap().is_empty());
        let long: String = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".into();
        assert_eq!(char_ramp_from_text(&long).unwrap().len(), MAX_RAMP_LEN);
    }

    #[test]
    fn default_ramps_are_valid() {
        let ramps = ShadeRamps::default();
        assert_eq!(ramps.char_ramps().len(), ramps.characters.len());
        assert_eq!(ramps.char_ramps()[0], default_char_ramp());
        assert!(ramps.color_ramps().iter().all(|ramp| ramp.len() >= 2));
        let text = toml::to_string(&ramps).unwrap();
        assert_eq!(toml::from_str::<ShadeRamps>(&text).unwrap(), ramps);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct YaffGlyph {
    pub data: Vec<u8>,
    pub size: Size,
}

impl Display for YaffGlyph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for (y, b) in self.data.iter().enumerate() {
            s.push_str(&format!("{y:2}"));
            for i in 0..8 {
                if *b & (128 >> i) == 0 {
                    s.push('-');
                } else {
                    s.push('#');
                }
            }
            s.push('\n');
        }
        write!(f, "{s}---")
    }
}

impl YaffGlyph {

}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct YaffFont {
    pub name: String,
    pub path_opt: Option<PathBuf>,
    pub length: i32,
    font_type: BitFontType,
    pub glyphs: HashMap<char, Glyph>,
    pub checksum: u32,
}

impl Default for YaffFont {
    fn default() -> Self {
        BitFont::from_ansi_font_page(0).unwrap()
    }
}
use crate::{HyperLink, Position, TextBuffer, TextPane};

impl TextBuffer {
    pub fn parse_hyperlinks(&self) -> Vec<HyperLink> {
        let mut result: Vec<HyperLink> = Vec::new();

        let mut pos = Position::new(self.get_width() - 1, self.get_height() - 1);
        let mut parser = rfind_url::Parser::new();

        loop {
            let attr_char = self.get_char(pos);
            if let rfind_url::ParserState::Url(size) = parser.advance(attr_char.ch) {
                let p = crate::HyperLink {
                    url: None,
                    position: pos,
                    length: size as i32,
                };
                result.push(p);
            }
            if pos.x == 0 {
                pos.x = self.get_width().saturating_sub(1);
                if pos.y == 0 {
                    break;
                }
                pos.y -= 1;
            } else {
                pos.x -= 1;
            }
        }
        result
    }

    fn underline(&mut self, pos: impl Into<Position>, size: i32) {
        let mut pos = pos.into();
        for _ in 0..size {
            let mut ch = self.get_char(pos);
            ch.attribute.set_is_underlined(true);
            self.layers[0].set_char(pos, ch);
            pos.x += 1;
            if pos.x >= self.get_width() {
                pos.x = 0;
                pos.y += 1;
            }
        }
    }

    pub fn update_hyperlinks(&mut self) {
        let links = self.parse_hyperlinks();
        for hl in &links {
            self.underline(hl.position, hl.length);
        }
        self.layers[0].hyperlinks = links;
    }
}

use crate::{Buffer, HyperLink, Position, TextPane};

impl Buffer {
    pub fn get_string(&self, pos: impl Into<Position>, size: usize) -> String {
        let pos = pos.into();
        let mut result = String::new();
        let mut pos = pos;
        for _ in 0..size {
            result.push(self.get_char(pos).ch);
            pos.x += 1;
            if pos.x >= self.get_width() {
                pos.x = 0;
                pos.y += 1;
            }
        }
        result
    }

    pub fn parse_hyperlinks(&self) -> Vec<HyperLink> {
        let mut result = Vec::new();

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

    pub fn is_position_in_range(&self, pos: impl Into<Position>, from: impl Into<Position>, size: i32) -> bool {
        let pos = pos.into();
        let from = from.into();

        match pos.y.cmp(&from.y) {
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => from.x <= pos.x && pos.x < from.x + size,
            std::cmp::Ordering::Greater => {
                let remainder = size.saturating_sub(self.get_width() + from.x);
                let lines = remainder / self.get_width();
                let mut y = from.y + lines;
                let x = if remainder > 0 {
                    y += 1; // remainder > 1 wraps 1 extra line
                    remainder - lines * self.get_width()
                } else {
                    remainder
                };
                pos.y < y || pos.y == y && pos.x < x
            }
        }
    }

    pub fn join_hyperlinks(&mut self, hyperlinks: Vec<HyperLink>) {
        self.layers[0].hyperlinks.retain(|l| l.url.is_none());
        for hl in &hyperlinks {
            self.underline(hl.position, hl.length);
        }
        self.layers[0].hyperlinks.extend(hyperlinks);
    }

    pub fn update_hyperlinks(&mut self) {
        self.join_hyperlinks(self.parse_hyperlinks());
    }
}

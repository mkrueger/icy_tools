pub struct PatternRecognizer {
    pattern: Vec<u8>,
    cur_idx: usize,
    ignore_case: bool,
}

impl PatternRecognizer {
    pub fn from(data: &[u8], ignore_case: bool) -> Self {
        Self {
            pattern: if ignore_case {
                data.iter().map(|c| (*c).to_ascii_uppercase()).collect()
            } else {
                data.to_vec()
            },
            cur_idx: 0,
            ignore_case,
        }
    }

    pub fn reset(&mut self) {
        self.cur_idx = 0;
    }

    pub fn push_ch(&mut self, ch: u8) -> bool {
        let p = self.pattern[self.cur_idx];
        if p == ch || self.ignore_case && ch.is_ascii_lowercase() && ch - b'a' + b'A' == p {
            self.cur_idx += 1;
            if self.cur_idx >= self.pattern.len() {
                self.cur_idx = 0;
                return true;
            }
        } else if self.cur_idx > 0 {
            self.cur_idx = 0;
            return self.push_ch(ch);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::PatternRecognizer;

    #[test]
    fn test_pattern_recognizer() {
        let mut test = PatternRecognizer::from(b"Name", false);

        let mut result = false;
        for b in b"Name" {
            result = test.push_ch(*b);
        }
        assert!(result);

        let mut result = false;
        for b in b"name" {
            result = test.push_ch(*b);
        }
        assert!(!result);
    }

    #[test]
    fn test_pattern_recognizer_ignore_case() {
        let mut test = PatternRecognizer::from(b"Name", true);

        let mut result = false;
        for b in b"name" {
            result = test.push_ch(*b);
        }
        assert!(result);

        let mut result = false;
        for b in b"NaMe" {
            result = test.push_ch(*b);
        }
        assert!(result);

        let mut result = false;
        for b in b"Nmae" {
            result = test.push_ch(*b);
        }
        assert!(!result);
    }

    #[test]
    fn test_pattern_recognizer_recovery() {
        let mut test = PatternRecognizer::from(b"name", false);

        let mut result = false;
        for b in b"namname" {
            result = test.push_ch(*b);
        }
        assert!(result);
    }

    #[test]
    fn test_pattern_recognizer_invalid() {
        let mut test = PatternRecognizer::from(b"name", false);

        let mut result = false;
        for b in b"n_a_m_e" {
            result = test.push_ch(*b);
        }
        assert!(!result);
    }
}

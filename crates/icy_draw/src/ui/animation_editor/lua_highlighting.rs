//! Lua syntax highlighting definitions for the code editor

use std::collections::HashSet;

/// Lua keywords for syntax highlighting
pub const LUA_KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true",
    "until", "while",
];

/// Lua built-in types
pub const LUA_TYPES: &[&str] = &["nil", "boolean", "number", "string", "function", "userdata", "thread", "table"];

/// Animation-specific Lua functions
pub const ANIMATION_FUNCTIONS: &[&str] = &[
    // Buffer management
    "new_buffer",
    "load_buffer",
    "next_frame",
    // Color functions
    "fg_rgb",
    "bg_rgb",
    "set_fg",
    "get_fg",
    "set_bg",
    "get_bg",
    // Character functions
    "set_char",
    "get_char",
    "print",
    "print_centered",
    // Cursor/position
    "gotoxy",
    "clear",
    // Layer functions
    "set_layer_position",
    "get_layer_position",
    "set_layer_visible",
    "get_layer_visible",
    "get_layer_count",
    "add_layer",
    "remove_layer",
    // Timing
    "get_delay",
    "set_delay",
    // Logging
    "log",
    // Drawing
    "draw_line",
    "draw_rect",
    "draw_filled_rect",
    "draw_ellipse",
    "draw_filled_ellipse",
    // Palette
    "set_palette_color",
    "get_palette_color",
];

/// Get a set of all Lua keywords
pub fn keywords() -> HashSet<&'static str> {
    LUA_KEYWORDS.iter().copied().collect()
}

/// Get a set of all Lua types
pub fn types() -> HashSet<&'static str> {
    LUA_TYPES.iter().copied().collect()
}

/// Get a set of all animation-specific functions
pub fn animation_functions() -> HashSet<&'static str> {
    ANIMATION_FUNCTIONS.iter().copied().collect()
}

/// Check if a word is a Lua keyword
pub fn is_keyword(word: &str) -> bool {
    LUA_KEYWORDS.contains(&word)
}

/// Check if a word is an animation function
pub fn is_animation_function(word: &str) -> bool {
    ANIMATION_FUNCTIONS.contains(&word)
}

/// Token types for Lua syntax highlighting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuaTokenType {
    Keyword,
    Type,
    Function,
    AnimationFunction,
    String,
    Number,
    Comment,
    Operator,
    Identifier,
    Punctuation,
    Whitespace,
}

/// Simple Lua tokenizer for syntax highlighting
pub struct LuaTokenizer<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> LuaTokenizer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        self.source[self.pos..].chars().nth(1)
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_while<F: Fn(char) -> bool>(&mut self, predicate: F) {
        while let Some(ch) = self.peek() {
            if predicate(ch) {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Get the next token
    pub fn next_token(&mut self) -> Option<(LuaTokenType, &'a str)> {
        let start = self.pos;
        let ch = self.peek()?;

        // Whitespace
        if ch.is_whitespace() {
            self.skip_while(|c| c.is_whitespace());
            return Some((LuaTokenType::Whitespace, &self.source[start..self.pos]));
        }

        // Comment
        if ch == '-' && self.peek_next() == Some('-') {
            self.advance();
            self.advance();

            // Check for multi-line comment
            if self.peek() == Some('[') && self.peek_next() == Some('[') {
                self.advance();
                self.advance();
                // Read until ]]
                while let Some(c) = self.advance() {
                    if c == ']' && self.peek() == Some(']') {
                        self.advance();
                        break;
                    }
                }
            } else {
                // Single line comment
                self.skip_while(|c| c != '\n');
            }
            return Some((LuaTokenType::Comment, &self.source[start..self.pos]));
        }

        // String (single or double quoted)
        if ch == '"' || ch == '\'' {
            let quote = ch;
            self.advance();
            while let Some(c) = self.advance() {
                if c == quote {
                    break;
                }
                if c == '\\' {
                    self.advance(); // Skip escaped character
                }
            }
            return Some((LuaTokenType::String, &self.source[start..self.pos]));
        }

        // Multi-line string
        if ch == '[' && self.peek_next() == Some('[') {
            self.advance();
            self.advance();
            while let Some(c) = self.advance() {
                if c == ']' && self.peek() == Some(']') {
                    self.advance();
                    break;
                }
            }
            return Some((LuaTokenType::String, &self.source[start..self.pos]));
        }

        // Number
        if ch.is_ascii_digit() || (ch == '.' && self.peek_next().map_or(false, |c| c.is_ascii_digit())) {
            self.skip_while(|c| {
                c.is_ascii_digit() || c == '.' || c == 'x' || c == 'X' || c.is_ascii_hexdigit() || c == 'e' || c == 'E' || c == '+' || c == '-'
            });
            return Some((LuaTokenType::Number, &self.source[start..self.pos]));
        }

        // Identifier or keyword
        if ch.is_alphabetic() || ch == '_' {
            self.skip_while(|c| c.is_alphanumeric() || c == '_');
            let word = &self.source[start..self.pos];

            let token_type = if is_keyword(word) {
                LuaTokenType::Keyword
            } else if LUA_TYPES.contains(&word) {
                LuaTokenType::Type
            } else if is_animation_function(word) {
                LuaTokenType::AnimationFunction
            } else {
                LuaTokenType::Identifier
            };

            return Some((token_type, word));
        }

        // Operators and punctuation
        self.advance();
        let token_type = match ch {
            '+' | '-' | '*' | '/' | '%' | '^' | '#' | '&' | '|' | '~' | '<' | '>' | '=' | '.' => LuaTokenType::Operator,
            '(' | ')' | '{' | '}' | '[' | ']' | ';' | ':' | ',' => LuaTokenType::Punctuation,
            _ => LuaTokenType::Identifier,
        };

        Some((token_type, &self.source[start..self.pos]))
    }
}

impl<'a> Iterator for LuaTokenizer<'a> {
    type Item = (LuaTokenType, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer_keywords() {
        let source = "local x = 10";
        let tokens: Vec<_> = LuaTokenizer::new(source).collect();

        assert_eq!(tokens[0], (LuaTokenType::Keyword, "local"));
    }

    #[test]
    fn test_tokenizer_comment() {
        let source = "-- this is a comment\nx = 1";
        let tokens: Vec<_> = LuaTokenizer::new(source).collect();

        assert_eq!(tokens[0].0, LuaTokenType::Comment);
    }

    #[test]
    fn test_animation_function() {
        let source = "next_frame(buffer)";
        let tokens: Vec<_> = LuaTokenizer::new(source).collect();

        assert_eq!(tokens[0], (LuaTokenType::AnimationFunction, "next_frame"));
    }
}

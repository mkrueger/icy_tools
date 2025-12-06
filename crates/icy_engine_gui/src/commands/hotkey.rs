//! Hotkey parsing and representation
//!
//! Parses strings like "Ctrl+Shift+N" into structured hotkey bindings.
//! Also supports mouse button bindings like "Ctrl+Back".

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Keyboard modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool, // macOS Command key
}

impl Modifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        cmd: false,
    };

    pub const CTRL: Self = Self {
        ctrl: true,
        alt: false,
        shift: false,
        cmd: false,
    };

    pub const ALT: Self = Self {
        ctrl: false,
        alt: true,
        shift: false,
        cmd: false,
    };

    pub const SHIFT: Self = Self {
        ctrl: false,
        alt: false,
        shift: true,
        cmd: false,
    };

    pub const CMD: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        cmd: true,
    };

    pub const CTRL_SHIFT: Self = Self {
        ctrl: true,
        alt: false,
        shift: true,
        cmd: false,
    };

    pub const CTRL_ALT: Self = Self {
        ctrl: true,
        alt: true,
        shift: false,
        cmd: false,
    };

    /// Check if any modifier is pressed
    pub fn any(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.cmd
    }

    /// Check if no modifier is pressed
    pub fn none(&self) -> bool {
        !self.any()
    }
}

impl fmt::Display for Modifiers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.cmd {
            parts.push("Cmd");
        }
        write!(f, "{}", parts.join("+"))
    }
}

/// Represents a keyboard key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Numbers
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Special keys
    Escape,
    Tab,
    Space,
    Backspace,
    Enter,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,

    // Arrow keys
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // Symbols
    Plus,
    Minus,
    Equals,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    Backtick,
}

impl KeyCode {
    /// Parse a key name string into a KeyCode
    pub fn from_str(s: &str) -> Option<Self> {
        let s = s.trim();

        // Single character keys
        if s.len() == 1 {
            let c = s.chars().next().unwrap().to_ascii_uppercase();
            return match c {
                'A' => Some(Self::A),
                'B' => Some(Self::B),
                'C' => Some(Self::C),
                'D' => Some(Self::D),
                'E' => Some(Self::E),
                'F' => Some(Self::F),
                'G' => Some(Self::G),
                'H' => Some(Self::H),
                'I' => Some(Self::I),
                'J' => Some(Self::J),
                'K' => Some(Self::K),
                'L' => Some(Self::L),
                'M' => Some(Self::M),
                'N' => Some(Self::N),
                'O' => Some(Self::O),
                'P' => Some(Self::P),
                'Q' => Some(Self::Q),
                'R' => Some(Self::R),
                'S' => Some(Self::S),
                'T' => Some(Self::T),
                'U' => Some(Self::U),
                'V' => Some(Self::V),
                'W' => Some(Self::W),
                'X' => Some(Self::X),
                'Y' => Some(Self::Y),
                'Z' => Some(Self::Z),
                '0' => Some(Self::Num0),
                '1' => Some(Self::Num1),
                '2' => Some(Self::Num2),
                '3' => Some(Self::Num3),
                '4' => Some(Self::Num4),
                '5' => Some(Self::Num5),
                '6' => Some(Self::Num6),
                '7' => Some(Self::Num7),
                '8' => Some(Self::Num8),
                '9' => Some(Self::Num9),
                '+' => Some(Self::Plus),
                '-' => Some(Self::Minus),
                '=' => Some(Self::Equals),
                '[' => Some(Self::BracketLeft),
                ']' => Some(Self::BracketRight),
                '\\' => Some(Self::Backslash),
                ';' => Some(Self::Semicolon),
                '\'' => Some(Self::Quote),
                ',' => Some(Self::Comma),
                '.' => Some(Self::Period),
                '/' => Some(Self::Slash),
                '`' => Some(Self::Backtick),
                ' ' => Some(Self::Space),
                _ => None,
            };
        }

        // Named keys (case-insensitive)
        match s.to_lowercase().as_str() {
            "escape" | "esc" => Some(Self::Escape),
            "tab" => Some(Self::Tab),
            "space" => Some(Self::Space),
            "backspace" => Some(Self::Backspace),
            "enter" | "return" => Some(Self::Enter),
            "delete" | "del" => Some(Self::Delete),
            "insert" | "ins" => Some(Self::Insert),
            "home" => Some(Self::Home),
            "end" => Some(Self::End),
            "pageup" | "pgup" => Some(Self::PageUp),
            "pagedown" | "pgdn" => Some(Self::PageDown),
            "arrowup" | "up" => Some(Self::ArrowUp),
            "arrowdown" | "down" => Some(Self::ArrowDown),
            "arrowleft" | "left" => Some(Self::ArrowLeft),
            "arrowright" | "right" => Some(Self::ArrowRight),
            "plus" => Some(Self::Plus),
            "minus" => Some(Self::Minus),
            "equals" => Some(Self::Equals),
            "f1" => Some(Self::F1),
            "f2" => Some(Self::F2),
            "f3" => Some(Self::F3),
            "f4" => Some(Self::F4),
            "f5" => Some(Self::F5),
            "f6" => Some(Self::F6),
            "f7" => Some(Self::F7),
            "f8" => Some(Self::F8),
            "f9" => Some(Self::F9),
            "f10" => Some(Self::F10),
            "f11" => Some(Self::F11),
            "f12" => Some(Self::F12),
            _ => None,
        }
    }

    /// Get the display name for the key
    pub fn name(&self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Num0 => "0",
            Self::Num1 => "1",
            Self::Num2 => "2",
            Self::Num3 => "3",
            Self::Num4 => "4",
            Self::Num5 => "5",
            Self::Num6 => "6",
            Self::Num7 => "7",
            Self::Num8 => "8",
            Self::Num9 => "9",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
            Self::Escape => "Escape",
            Self::Tab => "Tab",
            Self::Space => "Space",
            Self::Backspace => "Backspace",
            Self::Enter => "Enter",
            Self::Delete => "Delete",
            Self::Insert => "Insert",
            Self::Home => "Home",
            Self::End => "End",
            Self::PageUp => "PageUp",
            Self::PageDown => "PageDown",
            Self::ArrowUp => "Up",
            Self::ArrowDown => "Down",
            Self::ArrowLeft => "Left",
            Self::ArrowRight => "Right",
            Self::Plus => "+",
            Self::Minus => "-",
            Self::Equals => "=",
            Self::BracketLeft => "[",
            Self::BracketRight => "]",
            Self::Backslash => "\\",
            Self::Semicolon => ";",
            Self::Quote => "'",
            Self::Comma => ",",
            Self::Period => ".",
            Self::Slash => "/",
            Self::Backtick => "`",
        }
    }
}

impl fmt::Display for KeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Represents a mouse button
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

impl MouseButton {
    /// Parse a mouse button name string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "middle" => Some(Self::Middle),
            "back" => Some(Self::Back),
            "forward" => Some(Self::Forward),
            _ => {
                // Try parsing "other(N)" format
                let s = s.to_lowercase();
                s.strip_prefix("other(")
                    .and_then(|s| s.strip_suffix(')'))
                    .and_then(|n| n.parse().ok())
                    .map(Self::Other)
            }
        }
    }

    /// Get the display name for the button
    pub fn name(&self) -> String {
        match self {
            Self::Left => "Left".to_string(),
            Self::Right => "Right".to_string(),
            Self::Middle => "Middle".to_string(),
            Self::Back => "Back".to_string(),
            Self::Forward => "Forward".to_string(),
            Self::Other(n) => format!("Other({})", n),
        }
    }
}

impl fmt::Display for MouseButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A complete mouse binding (modifiers + button)
///
/// Serializes to/from a string like "Ctrl+Back"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseBinding {
    pub button: MouseButton,
    pub modifiers: Modifiers,
}

impl Serialize for MouseBinding {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for MouseBinding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        MouseBinding::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid mouse binding: '{}'", s)))
    }
}

impl MouseBinding {
    /// Create a new mouse binding
    pub fn new(button: MouseButton, modifiers: Modifiers) -> Self {
        Self { button, modifiers }
    }

    /// Parse a mouse binding string like "Ctrl+Back" or "Forward"
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let mut modifiers = Modifiers::default();
        let mut remaining = s;

        // Parse modifiers at the beginning (same logic as Hotkey)
        loop {
            let lower = remaining.to_lowercase();

            if let Some(rest) = lower.strip_prefix("ctrl+").or_else(|| lower.strip_prefix("control+")) {
                modifiers.ctrl = true;
                remaining = &remaining[remaining.len() - rest.len()..];
            } else if let Some(rest) = lower
                .strip_prefix("alt+")
                .or_else(|| lower.strip_prefix("opt+"))
                .or_else(|| lower.strip_prefix("option+"))
            {
                modifiers.alt = true;
                remaining = &remaining[remaining.len() - rest.len()..];
            } else if let Some(rest) = lower.strip_prefix("shift+") {
                modifiers.shift = true;
                remaining = &remaining[remaining.len() - rest.len()..];
            } else if let Some(rest) = lower
                .strip_prefix("cmd+")
                .or_else(|| lower.strip_prefix("command+"))
                .or_else(|| lower.strip_prefix("meta+"))
                .or_else(|| lower.strip_prefix("super+"))
            {
                modifiers.cmd = true;
                remaining = &remaining[remaining.len() - rest.len()..];
            } else {
                break;
            }
        }

        if remaining.is_empty() {
            return None;
        }

        let button = MouseButton::from_str(remaining)?;
        Some(Self { button, modifiers })
    }

    /// Check if this binding matches the given button and modifiers
    pub fn matches(&self, button: MouseButton, modifiers: Modifiers) -> bool {
        self.button == button && self.modifiers == modifiers
    }
}

impl fmt::Display for MouseBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.any() {
            write!(f, "{}+{}", self.modifiers, self.button)
        } else {
            write!(f, "{}", self.button)
        }
    }
}

/// A complete hotkey binding (modifiers + key)
///
/// Serializes to/from a string like "Ctrl+Shift+N"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub key: KeyCode,
    pub modifiers: Modifiers,
}

// Custom serde implementation for Hotkey - serializes as string like "Ctrl+C"
impl Serialize for Hotkey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Hotkey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Hotkey::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid hotkey: '{}'", s)))
    }
}

impl Hotkey {
    /// Create a new hotkey
    pub fn new(key: KeyCode, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    /// Parse a hotkey string like "Ctrl+Shift+N" or "Ctrl++"
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let mut modifiers = Modifiers::default();
        let mut remaining = s;

        // Parse modifiers at the beginning (case-insensitive)
        loop {
            let lower = remaining.to_lowercase();

            if let Some(rest) = lower.strip_prefix("ctrl+").or_else(|| lower.strip_prefix("control+")) {
                modifiers.ctrl = true;
                remaining = &remaining[remaining.len() - rest.len()..];
            } else if let Some(rest) = lower
                .strip_prefix("alt+")
                .or_else(|| lower.strip_prefix("opt+"))
                .or_else(|| lower.strip_prefix("option+"))
            {
                modifiers.alt = true;
                remaining = &remaining[remaining.len() - rest.len()..];
            } else if let Some(rest) = lower.strip_prefix("shift+") {
                modifiers.shift = true;
                remaining = &remaining[remaining.len() - rest.len()..];
            } else if let Some(rest) = lower
                .strip_prefix("cmd+")
                .or_else(|| lower.strip_prefix("command+"))
                .or_else(|| lower.strip_prefix("meta+"))
                .or_else(|| lower.strip_prefix("super+"))
            {
                modifiers.cmd = true;
                remaining = &remaining[remaining.len() - rest.len()..];
            } else {
                // No more modifiers
                break;
            }
        }

        // The rest is the key
        if remaining.is_empty() {
            return None;
        }

        let key = KeyCode::from_str(remaining)?;
        Some(Self { key, modifiers })
    }

    /// Check if this hotkey matches the given key and modifiers
    pub fn matches(&self, key: KeyCode, modifiers: Modifiers) -> bool {
        self.key == key && self.modifiers == modifiers
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();

        if self.modifiers.ctrl {
            parts.push("Ctrl");
        }
        if self.modifiers.alt {
            parts.push("Alt");
        }
        if self.modifiers.shift {
            parts.push("Shift");
        }
        if self.modifiers.cmd {
            parts.push("Cmd");
        }
        parts.push(self.key.name());

        write!(f, "{}", parts.join("+"))
    }
}

#[cfg(test)]
mod hotkey_tests {
    use super::*;

    #[test]
    fn test_parse_simple_key() {
        let hk = Hotkey::parse("A").unwrap();
        assert_eq!(hk.key, KeyCode::A);
        assert!(hk.modifiers.none());
    }

    #[test]
    fn test_parse_ctrl_key() {
        let hk = Hotkey::parse("Ctrl+C").unwrap();
        assert_eq!(hk.key, KeyCode::C);
        assert!(hk.modifiers.ctrl);
        assert!(!hk.modifiers.alt);
        assert!(!hk.modifiers.shift);
    }

    #[test]
    fn test_parse_ctrl_shift() {
        let hk = Hotkey::parse("Ctrl+Shift+N").unwrap();
        assert_eq!(hk.key, KeyCode::N);
        assert!(hk.modifiers.ctrl);
        assert!(hk.modifiers.shift);
        assert!(!hk.modifiers.alt);
    }

    #[test]
    fn test_parse_cmd() {
        let hk = Hotkey::parse("Cmd+C").unwrap();
        assert_eq!(hk.key, KeyCode::C);
        assert!(hk.modifiers.cmd);
        assert!(!hk.modifiers.ctrl);
    }

    #[test]
    fn test_parse_function_key() {
        let hk = Hotkey::parse("F11").unwrap();
        assert_eq!(hk.key, KeyCode::F11);
        assert!(hk.modifiers.none());
    }

    #[test]
    fn test_parse_alt_enter() {
        let hk = Hotkey::parse("Alt+Enter").unwrap();
        assert_eq!(hk.key, KeyCode::Enter);
        assert!(hk.modifiers.alt);
    }

    #[test]
    fn test_parse_plus_key() {
        let hk = Hotkey::parse("Ctrl++").unwrap();
        assert_eq!(hk.key, KeyCode::Plus);
        assert!(hk.modifiers.ctrl);
    }

    #[test]
    fn test_parse_minus_key() {
        let hk = Hotkey::parse("Ctrl+-").unwrap();
        assert_eq!(hk.key, KeyCode::Minus);
        assert!(hk.modifiers.ctrl);
    }

    #[test]
    fn test_parse_case_insensitive() {
        let hk1 = Hotkey::parse("ctrl+shift+n").unwrap();
        let hk2 = Hotkey::parse("CTRL+SHIFT+N").unwrap();
        assert_eq!(hk1, hk2);
    }

    #[test]
    fn test_display() {
        let hk = Hotkey::parse("Ctrl+Shift+N").unwrap();
        assert_eq!(hk.to_string(), "Ctrl+Shift+N");
    }

    #[test]
    fn test_matches() {
        let hk = Hotkey::parse("Ctrl+C").unwrap();
        assert!(hk.matches(KeyCode::C, Modifiers::CTRL));
        assert!(!hk.matches(KeyCode::C, Modifiers::NONE));
        assert!(!hk.matches(KeyCode::V, Modifiers::CTRL));
    }

    #[test]
    fn test_invalid_input() {
        assert!(Hotkey::parse("").is_none());
        assert!(Hotkey::parse("Ctrl+").is_none());
        assert!(Hotkey::parse("InvalidKey").is_none());
    }

    #[test]
    fn test_serde_roundtrip() {
        // TOML requires a table structure, so we wrap in a struct
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Wrapper {
            hotkey: Hotkey,
        }

        let wrapper = Wrapper {
            hotkey: Hotkey::parse("Ctrl+Shift+N").unwrap(),
        };

        let toml_str = toml::to_string(&wrapper).unwrap();
        assert!(toml_str.contains("Ctrl+Shift+N"));

        let parsed: Wrapper = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed, wrapper);
    }

    #[test]
    fn test_serde_in_vec() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Wrapper {
            hotkeys: Vec<Hotkey>,
        }

        let wrapper = Wrapper {
            hotkeys: vec![Hotkey::parse("Ctrl+C").unwrap(), Hotkey::parse("Ctrl+V").unwrap()],
        };

        let toml_str = toml::to_string(&wrapper).unwrap();
        let parsed: Wrapper = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed, wrapper);
    }
}

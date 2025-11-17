#[derive(Debug, Clone, PartialEq)]
pub enum LoopTarget {
    /// Single command identifier, e.g. 'L', 'S', 'G'.
    Single(char),

    /// Chain-Gang sequence, e.g. ">CL@".
    ChainGang {
        /// Raw representation including leading '>' and trailing '@' for roundtrip.
        raw: String,
        /// Extracted command identifiers inside the chain.
        commands: Vec<char>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoopModifiers {
    /// XOR stepping ("|" after the command identifier).
    pub xor_stepping: bool,
    /// For W command: fetch text each iteration ("@" after the command identifier).
    pub refresh_text_each_iteration: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoopParamToken {
    /// Plain numeric value.
    Number(i32),
    /// Symbolic value, usually 'x' or 'y'.
    Symbol(char),
    /// Expression like "+10", "-10", "!99".
    Expr(String),
    /// Group separator corresponding to ':' in the text representation.
    GroupSeparator,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoopCommandData {
    pub from: i32,
    pub to: i32,
    pub step: i32,
    pub delay: i32,
    pub target: LoopTarget,
    pub modifiers: LoopModifiers,
    pub param_count: u16,
    pub params: Vec<LoopParamToken>,
}

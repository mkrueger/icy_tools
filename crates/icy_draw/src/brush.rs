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

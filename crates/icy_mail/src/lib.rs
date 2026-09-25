pub mod drafts;
pub mod editor;
pub mod perf;
pub mod qwk;
pub mod reader;
pub mod state;
pub mod text;
#[path = "ui/threading.rs"]
pub mod threading;

pub type Res<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

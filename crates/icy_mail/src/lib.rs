pub mod drafts;
pub mod perf;
pub mod qwk;
pub mod reader;
#[path = "ui/threading.rs"]
pub mod threading;

pub type Res<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

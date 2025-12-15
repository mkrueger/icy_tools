mod cache;
mod file;
mod pack;
mod provider;
mod root;
mod year;

pub use cache::*;
pub use file::*;
pub use pack::*;
pub use provider::*;
pub use root::*;
pub use year::*;

pub(crate) const MAIN_PATH: &str = "https://16colo.rs";
pub(crate) const API_PATH: &str = "http://api.16colo.rs/v0";

use super::create_shared_cache;

/// Global cache instance - lazily initialized
pub(crate) fn get_cache() -> SharedSixteenColorsCache {
    use std::sync::OnceLock;
    static CACHE: OnceLock<SharedSixteenColorsCache> = OnceLock::new();
    CACHE.get_or_init(create_shared_cache).clone()
}

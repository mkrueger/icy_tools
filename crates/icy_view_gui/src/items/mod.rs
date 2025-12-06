use std::path::{Path, PathBuf};

use icy_sauce::SauceRecord;

mod archive;
mod files;
mod provider;
mod sixteencolors;

pub use archive::{ArchiveContainer, ArchiveFolder, ArchiveItem};
pub use files::*;
pub use provider::*;
pub use sixteencolors::*;

use crate::ui::thumbnail_view::{RgbaData, THUMBNAIL_MAX_HEIGHT, THUMBNAIL_RENDER_WIDTH};
use async_trait::async_trait;
use icy_engine::{AttributedChar, Position, Rectangle, RenderOptions, Selection, TextAttribute, TextBuffer, TextPane, formats::FileFormat};
pub use icy_engine_gui::ui::FileIcon;
use once_cell::sync::Lazy;
use tokio_util::sync::CancellationToken;

/// Global Tokio runtime for blocking operations
/// This avoids creating/dropping runtimes repeatedly which can cause issues
static BLOCKING_RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("item-blocking")
        .enable_all()
        .build()
        .expect("Failed to create blocking runtime")
});

/// Create a new shared 16colors cache
pub fn create_shared_cache() -> SharedSixteenColorsCache {
    std::sync::Arc::new(parking_lot::RwLock::new(SixteenColorsCache::new()))
}

/// Load PNG/JPEG/etc bytes and convert to RgbaData, scaling if needed
pub fn load_image_to_rgba(data: &[u8]) -> Option<RgbaData> {
    let img = ::image::load_from_memory(data).ok()?;
    let (orig_width, orig_height) = (img.width(), img.height());
    // Scale down if needed
    let scale = (THUMBNAIL_RENDER_WIDTH as f32 / orig_width as f32)
        .min(THUMBNAIL_MAX_HEIGHT as f32 / orig_height as f32)
        .min(1.0);

    let new_width = ((orig_width as f32 * scale) as u32).max(1);
    let new_height = ((orig_height as f32 * scale) as u32).max(1);

    let resized = if scale < 1.0 {
        img.resize(new_width, new_height, ::image::imageops::FilterType::Triangle)
    } else {
        img
    };

    let rgba = resized.to_rgba8();
    Some(RgbaData::new(rgba.into_raw(), new_width, new_height))
}

/// Create a simple folder placeholder icon
/// Creates a 64x64 folder icon with a simple design
pub fn create_folder_placeholder() -> RgbaData {
    let width = 128u32;
    let height = 96u32;
    let mut data = vec![0u8; (width * height * 4) as usize];

    // Colors (RGBA)
    let folder_color: [u8; 4] = [180, 140, 60, 255]; // Brownish/tan folder
    let tab_color: [u8; 4] = [160, 120, 40, 255]; // Slightly darker tab
    let outline_color: [u8; 4] = [100, 80, 30, 255]; // Dark outline

    // Draw folder body (main rectangle)
    let body_top = 24;
    let body_left = 8;
    let body_right = width - 8;
    let body_bottom = height - 8;

    for y in body_top..body_bottom {
        for x in body_left..body_right {
            let idx = ((y * width + x) * 4) as usize;
            // Outline
            if x == body_left || x == body_right - 1 || y == body_top || y == body_bottom - 1 {
                data[idx..idx + 4].copy_from_slice(&outline_color);
            } else {
                data[idx..idx + 4].copy_from_slice(&folder_color);
            }
        }
    }

    // Draw folder tab (small rectangle on top-left)
    let tab_top = 12;
    let tab_left = 8;
    let tab_right = 48;
    let tab_bottom = body_top + 4;

    for y in tab_top..tab_bottom {
        for x in tab_left..tab_right {
            let idx = ((y * width + x) * 4) as usize;
            if x == tab_left || x == tab_right - 1 || y == tab_top {
                data[idx..idx + 4].copy_from_slice(&outline_color);
            } else {
                data[idx..idx + 4].copy_from_slice(&tab_color);
            }
        }
    }

    RgbaData::new(data, width, height)
}

#[async_trait]
pub trait Item: Send + Sync {
    /// Display label (can be decorated, e.g. "2020 (32 Packs)")
    fn get_label(&self) -> String;

    /// Navigable path segment (used for path construction in virtual filesystem)
    /// Always uses forward slashes for consistency
    fn get_file_path(&self) -> String;

    /// Get the full path for this item (for thumbnail/sauce matching)
    /// Returns None for virtual items that don't have a filesystem path
    /// Always uses forward slashes for consistency
    fn get_full_path(&self) -> Option<String> {
        None
    }

    fn is_virtual_file(&self) -> bool {
        false
    }

    /// Whether this item is a container (folder, zip file, etc.) that can be navigated into
    fn is_container(&self) -> bool {
        false
    }

    /// Whether this item represents a parent directory navigation entry
    fn is_parent(&self) -> bool {
        false
    }

    /// Get the file size in bytes (for sorting)
    /// Returns None for items that don't have a size (e.g., folders)
    fn get_size(&self) -> Option<u64> {
        None
    }

    /// Get the modification time (for sorting)
    /// Returns None for items that don't have a modification time
    fn get_modified_time(&self) -> Option<std::time::SystemTime> {
        None
    }

    /// Get a synchronous thumbnail for this item (no async loading needed)
    /// Returns Some(RgbaData) if the item can provide a thumbnail immediately without I/O
    /// This is used for folders that just show a static folder icon
    fn get_sync_thumbnail(&self) -> Option<RgbaData> {
        None
    }

    /// Get the FileIcon for this item (for SVG rendering)
    fn get_file_icon(&self) -> FileIcon {
        get_file_icon_for_path(&PathBuf::from(&self.get_file_path()))
    }

    /// Get a thumbnail preview for this item (async)
    /// Returns RgbaData if the item can provide its own thumbnail (e.g., from an API)
    /// For folders, this returns a placeholder icon
    async fn get_thumbnail_preview(&self, _cancel_token: &CancellationToken) -> Option<RgbaData> {
        None
    }

    /// Get subitems (async for network-based items)
    async fn get_subitems(&self, _cancel_token: &CancellationToken) -> Option<Vec<Box<dyn Item>>> {
        None
    }

    /// Read the item's data (async for network/file I/O)
    async fn read_data(&self) -> Option<Vec<u8>> {
        None
    }

    /// Get SAUCE info for this item (async for file I/O)
    /// Returns the SAUCE record if the file has one
    async fn get_sauce_info(&self, _cancel_token: &CancellationToken) -> Option<SauceRecord> {
        // Default implementation reads data and extracts SAUCE
        if let Some(data) = self.read_data().await {
            return SauceRecord::from_bytes(&data).ok().flatten();
        }
        None
    }

    /// Clone this item into a new Box
    /// Used when items need to be passed to background threads
    fn clone_box(&self) -> Box<dyn Item>;
}

/// Get the FileIcon for a given path based on its extension
pub fn get_file_icon_for_path(path: &Path) -> FileIcon {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();

    // Check if FileFormat recognizes this as a supported format
    if let Some(format) = FileFormat::from_extension(&ext) {
        FileIcon::from_format(&format)
    } else {
        FileIcon::Unknown
    }
}

/// Check if a file extension is displayable (can be shown/previewed)
pub fn is_displayable_extension(ext: &str) -> bool {
    let ext = ext.to_ascii_lowercase();

    // Check FileFormat - if recognized, it's displayable
    FileFormat::from_extension(&ext).is_some()
}

impl dyn Item {
    pub async fn is_binary(&self) -> bool {
        if let Some(data) = self.read_data().await {
            for i in data.iter().take(500) {
                if i == &0 || i == &255 {
                    return true;
                }
            }
            false
        } else {
            true
        }
    }

    /// Synchronous wrapper for get_subitems - blocks on async
    /// Use this only in non-async contexts
    pub fn get_subitems_blocking(&self, cancel_token: &CancellationToken) -> Option<Vec<Box<dyn Item>>> {
        BLOCKING_RUNTIME.block_on(self.get_subitems(cancel_token))
    }

    /// Synchronous wrapper for read_data - blocks on async
    /// Use this only in non-async contexts
    pub fn read_data_blocking(&self) -> Option<Vec<u8>> {
        BLOCKING_RUNTIME.block_on(self.read_data())
    }
}

pub fn sort_folder(directories: &mut Vec<Box<dyn Item>>) {
    directories.sort_by(|a, b| a.get_label().to_lowercase().cmp(&b.get_label().to_lowercase()));
}

use crate::SortOrder;

/// Sort items according to the specified sort order
/// Folders are always listed first, then files are sorted within their group
pub fn sort_items(items: &mut Vec<Box<dyn Item>>, order: SortOrder) {
    items.sort_by(|a, b| {
        // Folders always come first
        let a_is_folder = a.is_container() || a.is_parent();
        let b_is_folder = b.is_container() || b.is_parent();

        if a_is_folder && !b_is_folder {
            return std::cmp::Ordering::Less;
        }
        if !a_is_folder && b_is_folder {
            return std::cmp::Ordering::Greater;
        }

        // Both are same type, apply sort order
        match order {
            SortOrder::NameAsc => a.get_label().to_lowercase().cmp(&b.get_label().to_lowercase()),
            SortOrder::NameDesc => b.get_label().to_lowercase().cmp(&a.get_label().to_lowercase()),
            SortOrder::SizeAsc => {
                let a_size = a.get_size().unwrap_or(0);
                let b_size = b.get_size().unwrap_or(0);
                a_size.cmp(&b_size)
            }
            SortOrder::SizeDesc => {
                let a_size = a.get_size().unwrap_or(0);
                let b_size = b.get_size().unwrap_or(0);
                b_size.cmp(&a_size)
            }
            SortOrder::DateAsc => {
                let a_date = a.get_modified_time();
                let b_date = b.get_modified_time();
                a_date.cmp(&b_date)
            }
            SortOrder::DateDesc => {
                let a_date = a.get_modified_time();
                let b_date = b.get_modified_time();
                b_date.cmp(&a_date)
            }
        }
    });
}

/// Create a thumbnail preview from a TextBuffer (80x25 screen)
/// Renders the buffer to RGBA and scales it to thumbnail size
pub fn create_text_buffer_preview(buffer: &TextBuffer) -> RgbaData {
    let width = buffer.get_width();
    let height = buffer.get_height();

    if width == 0 || height == 0 {
        return create_folder_placeholder();
    }

    // Render to RGBA
    let rect = Selection::from(Rectangle::from(0, 0, width, height));
    let opts = RenderOptions {
        rect,
        blink_on: true,
        selection: None,
        selection_fg: None,
        selection_bg: None,
        override_scan_lines: Some(false),
    };

    let (size, rgba) = buffer.render_to_rgba(&opts, false);

    if size.width <= 0 || size.height <= 0 || rgba.is_empty() {
        return create_folder_placeholder();
    }

    let orig_width = size.width as u32;
    let orig_height = size.height as u32;

    // Scale to thumbnail size
    let scale = (THUMBNAIL_RENDER_WIDTH as f32 / orig_width as f32)
        .min(THUMBNAIL_MAX_HEIGHT as f32 / orig_height as f32)
        .min(1.0);

    let new_width = ((orig_width as f32 * scale) as u32).max(1);
    let new_height = ((orig_height as f32 * scale) as u32).max(1);

    if new_width == orig_width && new_height == orig_height {
        RgbaData::new(rgba, orig_width, orig_height)
    } else {
        // Scale using image crate
        match image::RgbaImage::from_raw(orig_width, orig_height, rgba) {
            Some(img) => {
                let resized = image::imageops::resize(&img, new_width, new_height, image::imageops::FilterType::Triangle);
                RgbaData::new(resized.into_raw(), new_width, new_height)
            }
            None => create_folder_placeholder(),
        }
    }
}

/// Create a simple text preview with a message on a black screen
/// Uses 20x6 screen with white text (fg=15) on black background (bg=0)
/// The smaller size will scale up for better visibility
pub fn create_text_preview(message: &str) -> RgbaData {
    let mut buffer = TextBuffer::new((20, 7));

    // Set white on black attribute (fg=15, bg=0)
    let attr = TextAttribute::from_u8(15, icy_engine::IceMode::Blink);

    // Center the message vertically (around line 3)
    let y = 3;
    // Center horizontally
    let start_x = ((20 - message.len() as i32) / 2).max(0);

    for (i, ch) in message.chars().enumerate() {
        let x = start_x + i as i32;
        if x < 20 {
            let ch = icy_engine::BufferType::CP437.try_convert_from_unicode(ch).unwrap_or('?');
            buffer.layers[0].set_char(Position::new(x, y), AttributedChar::new(ch, attr));
        }
    }

    create_text_buffer_preview(&buffer)
}

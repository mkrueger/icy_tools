//! Shared Font Library for Text-Art fonts (TDF/FIGlet)
//!
//! Provides a centralized, shared font library that is loaded once at startup
//! and shared between all windows via `Arc<RwLock<FontLibrary>>`.
//! Includes a file watcher to automatically reload fonts when the font directory changes.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use icy_engine::char_set::TdfBufferRenderer;
use icy_engine::formats::FileFormat;
use icy_engine::{Rectangle, TextBuffer, TextPane};
use icy_ui::widget::image;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::RwLock;
use retrofont::Font;
use walkdir::WalkDir;

use crate::Settings;

/// Preview text to render
const PREVIEW_TEXT: &str = "HALLO";

/// Preview buffer size  
const PREVIEW_BUFFER_WIDTH: i32 = 100;
const PREVIEW_BUFFER_HEIGHT: i32 = 12;

/// Shared font library type
pub type SharedFontLibrary = Arc<RwLock<TextArtFontLibrary>>;

/// Cached font preview
#[derive(Clone)]
pub struct FontPreview {
    pub handle: image::Handle,
    pub width: u32,
    pub height: u32,
}

/// Central font library that holds all loaded TDF/Figlet fonts.
///
/// This is shared between all windows and automatically reloads
/// when the font directory changes.
pub struct TextArtFontLibrary {
    /// All loaded fonts
    fonts: Vec<Font>,
    /// Path to the font directory being watched
    font_dir: Option<PathBuf>,
    /// Cached preview images (font_index -> preview)
    preview_cache: HashMap<usize, FontPreview>,
}

impl Default for TextArtFontLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl TextArtFontLibrary {
    /// Create a new empty font library
    fn new() -> Self {
        Self {
            fonts: Vec::new(),
            font_dir: None,
            preview_cache: HashMap::new(),
        }
    }

    /// Create a shared font library with file watching
    pub fn create_shared() -> SharedFontLibrary {
        let library = Arc::new(RwLock::new(Self::new()));

        // Load fonts in background thread
        Self::reload_async(library.clone());

        // Start the file watcher in a background thread
        Self::start_watcher(library.clone());

        library
    }

    /// Reload fonts asynchronously in a background thread
    /// Only locks the library briefly to swap in the new fonts
    pub fn reload_async(library: SharedFontLibrary) {
        thread::spawn(move || {
            // Get font directory without holding lock
            let font_dir = Settings::text_art_font_dir();

            let Some(font_dir) = font_dir else {
                log::warn!("No font directory configured");
                return;
            };

            // Create directory if it doesn't exist (no lock needed)
            if !font_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(&font_dir) {
                    log::error!("Failed to create font directory: {}", e);
                    return;
                }
            }

            // Load fonts WITHOUT holding any lock - this is the slow part
            log::info!("Loading fonts from {}...", font_dir.display());
            let fonts = load_fonts_from_dir(&font_dir);
            log::info!("Loaded {} fonts from {}", fonts.len(), font_dir.display());

            // Only lock briefly to swap in the new data
            {
                let mut lib = library.write();
                lib.fonts = fonts;
                lib.font_dir = Some(font_dir);
                lib.preview_cache.clear(); // Clear cached previews when fonts reload
            }
        });
    }

    /// Get the number of loaded fonts
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    /// Check if any fonts are loaded
    pub fn has_fonts(&self) -> bool {
        !self.fonts.is_empty()
    }

    /// Get font at index
    pub fn get_font(&self, index: usize) -> Option<&Font> {
        self.fonts.get(index)
    }

    /// Get font name at index
    pub fn font_name(&self, index: usize) -> Option<&str> {
        self.fonts.get(index).map(|f| f.name())
    }

    /// Get all font names
    pub fn font_names(&self) -> Vec<String> {
        self.fonts.iter().map(|f| f.name().to_string()).collect()
    }

    /// Check if font at index has a glyph for the character
    pub fn has_char(&self, index: usize, ch: char) -> bool {
        self.fonts.get(index).map(|f| f.has_char(ch)).unwrap_or(false)
    }

    /// Get character availability for all printable ASCII chars
    pub fn get_char_availability(&self, index: usize) -> Vec<(char, bool)> {
        ('!'..='~').map(|ch| (ch, self.has_char(index, ch))).collect()
    }

    /// Get cached preview for a font (returns None if not yet generated)
    pub fn get_preview(&self, index: usize) -> Option<&FontPreview> {
        self.preview_cache.get(&index)
    }

    /// Check if a preview exists for a font
    pub fn has_preview(&self, index: usize) -> bool {
        self.preview_cache.contains_key(&index)
    }

    /// Generate and cache a preview for a font
    pub fn generate_preview(&mut self, index: usize) -> Option<&FontPreview> {
        if self.preview_cache.contains_key(&index) {
            return self.preview_cache.get(&index);
        }

        let font = self.fonts.get(index)?;

        // Create a buffer for rendering
        let mut buffer = TextBuffer::new((PREVIEW_BUFFER_WIDTH, PREVIEW_BUFFER_HEIGHT));
        let mut renderer = TdfBufferRenderer::new(&mut buffer, 0, 0);
        let options = retrofont::RenderOptions::default();

        // Render preview text
        let preview_text = PREVIEW_TEXT;
        let lowercase = preview_text.to_ascii_lowercase();

        // Use uppercase if available, otherwise lowercase
        let text_to_render: String = if font.has_char(preview_text.chars().next().unwrap_or('H')) {
            preview_text.to_string()
        } else {
            lowercase
        };

        for ch in text_to_render.chars() {
            if font.render_glyph(&mut renderer, ch, &options).is_err() {
                continue;
            }
            renderer.next_char();
        }

        // Render buffer to RGBA
        let rect = Rectangle::from(0, 0, buffer.width(), buffer.height());
        let (size, rgba) = buffer.render_to_rgba(&rect.into(), false);

        if size.width <= 0 || size.height <= 0 || rgba.is_empty() {
            return None;
        }

        let handle = image::Handle::from_rgba(size.width as u32, size.height as u32, rgba);
        let preview = FontPreview {
            handle,
            width: size.width as u32,
            height: size.height as u32,
        };
        self.preview_cache.insert(index, preview);
        self.preview_cache.get(&index)
    }

    fn render_preview_for_font(font: &Font) -> Option<FontPreview> {
        // Create a buffer for rendering
        let mut buffer = TextBuffer::new((PREVIEW_BUFFER_WIDTH, PREVIEW_BUFFER_HEIGHT));
        let mut renderer = TdfBufferRenderer::new(&mut buffer, 0, 0);
        let options = retrofont::RenderOptions::default();

        // Render preview text
        let preview_text = PREVIEW_TEXT;
        let lowercase = preview_text.to_ascii_lowercase();

        // Use uppercase if available, otherwise lowercase
        let text_to_render: String = if font.has_char(preview_text.chars().next().unwrap_or('H')) {
            preview_text.to_string()
        } else {
            lowercase
        };

        for ch in text_to_render.chars() {
            if font.render_glyph(&mut renderer, ch, &options).is_err() {
                continue;
            }
            renderer.next_char();
        }

        // Render buffer to RGBA
        let rect = Rectangle::from(0, 0, buffer.width(), buffer.height());
        let (size, rgba) = buffer.render_to_rgba(&rect.into(), false);

        if size.width <= 0 || size.height <= 0 || rgba.is_empty() {
            return None;
        }

        let handle = image::Handle::from_rgba(size.width as u32, size.height as u32, rgba);
        Some(FontPreview {
            handle,
            width: size.width as u32,
            height: size.height as u32,
        })
    }

    /// Generate previews for indices without holding the write lock during rendering.
    /// Returns the indices that were newly inserted into the cache.
    pub fn generate_previews_for_indices(library: SharedFontLibrary, indices: Vec<usize>) -> Vec<usize> {
        // 1) Render missing previews under a shared read lock.
        // retrofont::Font is not Clone, but rendering only needs an immutable reference.
        // Holding a read lock allows UI reads to proceed concurrently.
        let mut rendered: Vec<(usize, FontPreview)> = Vec::new();
        {
            let lib = library.read();
            for idx in indices {
                if lib.preview_cache.contains_key(&idx) {
                    continue;
                }
                let Some(font) = lib.fonts.get(idx) else {
                    continue;
                };
                if let Some(preview) = Self::render_preview_for_font(font) {
                    rendered.push((idx, preview));
                }
            }
        }

        if rendered.is_empty() {
            return Vec::new();
        }

        // 2) Insert under a short write lock.
        let mut inserted: Vec<usize> = Vec::new();
        {
            let mut lib = library.write();
            for (idx, preview) in rendered {
                if !lib.preview_cache.contains_key(&idx) {
                    lib.preview_cache.insert(idx, preview);
                    inserted.push(idx);
                }
            }
        }

        inserted
    }

    /// Start a file watcher for the font directory
    fn start_watcher(library: SharedFontLibrary) {
        // Get font directory - wait briefly for initial load if needed
        let font_dir = Settings::text_art_font_dir();

        let Some(font_dir) = font_dir else {
            log::warn!("No font directory configured, file watcher not started");
            return;
        };

        thread::spawn(move || {
            if let Err(e) = watch_font_directory(&font_dir, library) {
                log::error!("Font watcher error: {}", e);
            }
        });
    }
}

/// Watch the font directory for changes and reload fonts when needed
fn watch_font_directory(path: &Path, library: SharedFontLibrary) -> notify::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    watcher.watch(path, RecursiveMode::Recursive)?;

    log::info!("Started font directory watcher for: {}", path.display());

    loop {
        match rx.recv() {
            Ok(event) => {
                match event {
                    Ok(event) => {
                        // Only reload on create, modify, or remove events
                        use notify::EventKind;
                        match event.kind {
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                                log::info!("Font directory changed, reloading fonts...");
                                TextArtFontLibrary::reload_async(library.clone());
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        log::error!("Font watch error: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("Font watcher channel error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Load fonts from a directory (recursively)
fn load_fonts_from_dir(dir: &Path) -> Vec<Font> {
    let mut fonts = Vec::new();

    let walker = WalkDir::new(dir).into_iter();
    for entry in walker.filter_entry(|e| !is_hidden(e)) {
        let Ok(entry) = entry else { continue };
        let path = entry.path();

        if path.is_dir() {
            continue;
        }

        // Use FileFormat to detect file type
        let Some(format) = FileFormat::from_path(path) else {
            continue;
        };

        match format {
            FileFormat::CharacterFont(_) => {
                // TDF, FLF, etc. - load directly
                if let Ok(data) = std::fs::read(path) {
                    match Font::load(&data) {
                        Ok(loaded_fonts) => fonts.extend(loaded_fonts),
                        Err(err) => log::debug!("Failed to load font '{}': {}", path.display(), err),
                    }
                }
            }
            FileFormat::Archive(_) => {
                // Archive format (ZIP, ARJ, LHA, etc.) - extract and load fonts
                if let Ok(data) = std::fs::read(path) {
                    load_fonts_from_archive(&data, &format, &mut fonts);
                }
            }
            _ => {}
        }
    }

    // Sort fonts by name for consistent ordering
    fonts.sort_by(|a, b| a.name().to_lowercase().cmp(&b.name().to_lowercase()));

    fonts
}

/// Load fonts from an archive using unarc-rs via FileFormat
fn load_fonts_from_archive(data: &[u8], format: &FileFormat, fonts: &mut Vec<Font>) {
    let cursor = Cursor::new(data);
    let Ok(mut archive) = format.open_archive(cursor) else {
        return;
    };

    // Collect entries first to avoid borrow issues
    let Ok(entries) = archive.entries() else {
        return;
    };

    for entry in entries {
        let name = entry.name();

        // Skip directories (entries ending with /)
        if name.ends_with('/') {
            continue;
        }

        // Extract extension from entry name
        let name_lower = name.to_ascii_lowercase();
        let extension = Path::new(&name_lower).extension().and_then(|e| e.to_str()).unwrap_or("");

        // Check if this is a character font or archive file
        if let Some(entry_format) = FileFormat::from_extension(extension) {
            match entry_format {
                FileFormat::CharacterFont(_) => {
                    // Read and load font
                    if let Ok(font_data) = archive.read(&entry) {
                        if let Ok(loaded_fonts) = Font::load(&font_data) {
                            fonts.extend(loaded_fonts);
                        }
                    }
                }
                FileFormat::Archive(_) => {
                    // Nested archive - recurse
                    if let Ok(nested_data) = archive.read(&entry) {
                        load_fonts_from_archive(&nested_data, &entry_format, fonts);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Check if a directory entry is hidden
fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name().to_str().map(|s| s.starts_with('.')).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_library_default() {
        let library = TextArtFontLibrary::new();
        assert!(!library.has_fonts());
        assert_eq!(library.font_count(), 0);
    }
}

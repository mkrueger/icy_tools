//! Clipboard writes for the iced frontends
//!
//! The clipboard operations return Tasks that need to be executed
//! by the icy_ui runtime. The formats come from [`crate::clipboard_data`].

pub use crate::clipboard_data::*;
use icy_engine::Screen;
use icy_ui::clipboard::{Format, STANDARD};
use icy_ui::Task;

/// Copy prepared clipboard data to the system clipboard
///
/// This returns a Task that writes all formats to the clipboard.
/// The task should be executed by the iced runtime.
///
/// # Arguments
/// * `data` - The prepared clipboard data
///
/// # Returns
/// A Task that performs the clipboard write operation
pub fn copy_to_clipboard<Message: Clone + Send + 'static>(
    data: ClipboardData,
    on_complete: impl Fn(Result<(), ClipboardError>) -> Message + Clone + Send + 'static,
) -> Task<Message> {
    // Build list of entries to write: Vec<(data, formats)>
    let mut entries: Vec<(Vec<u8>, Vec<String>)> = Vec::with_capacity(4);

    // ICY binary format first (for paste between ICY applications)
    if let Some(icy_data) = data.icy_data {
        entries.push((icy_data, vec![ICY_CLIPBOARD_TYPE.to_string()]));
    }

    // RTF format (platform-independent via icy_ui::clipboard::Format)
    if let Some(rtf) = data.rtf {
        let rtf_formats: Vec<String> = Format::Rtf.formats().iter().map(|s| s.to_string()).collect();
        entries.push((rtf.into_bytes(), rtf_formats));
    }

    // Plain text (platform-independent via icy_ui::clipboard::Format)
    let text_formats: Vec<String> = Format::Text.formats().iter().map(|s| s.to_string()).collect();
    entries.push((data.text.into_bytes(), text_formats));

    // Image format - include in write_multi to avoid overwriting other formats
    if let Some((rgba_data, width, height)) = data.image {
        if let Some(img) = image::RgbaImage::from_raw(width, height, rgba_data) {
            let mut png_bytes = Vec::new();
            if img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).is_ok() {
                let image_formats: Vec<String> = Format::Image.formats().iter().map(|s| s.to_string()).collect();
                entries.push((png_bytes, image_formats));
            }
        }
    }

    // Write all formats in a single call
    STANDARD.write_multi(entries).map(move |()| on_complete(Ok(())))
}

/// Convenience function to prepare and copy in one step
///
/// This combines `prepare_clipboard_data` and `copy_to_clipboard`.
pub fn copy_selection<Message: Clone + Send + 'static>(
    screen: &mut dyn Screen,
    on_complete: impl Fn(Result<(), ClipboardError>) -> Message + Clone + Send + 'static,
) -> Result<Task<Message>, ClipboardError> {
    let data = prepare_clipboard_data(screen)?;
    Ok(copy_to_clipboard(data, on_complete))
}

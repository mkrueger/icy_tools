//! The system clipboard for the egui frontends, through clipboard-rs.
//!
//! A copied selection goes to the clipboard in every format Icy tools exchange at once: plain
//! text, RTF, a PNG of the rendered selection and the ICY binary format that pastes losslessly
//! between Icy tools. Pasting takes the richest format available: ICY data, then an image, then
//! text.
//!
//! egui only reports a paste when the clipboard holds text, so paste commands that do not come
//! from egui (menus) read the clipboard directly with [`read`].

use std::sync::{
    atomic::{AtomicBool, Ordering},
    LazyLock, Mutex, PoisonError,
};

use clipboard_rs::{common::RustImage, Clipboard, ClipboardContent, ClipboardContext, RustImageData};
use icy_engine::Screen;

use crate::clipboard_data::{prepare_clipboard_data, ClipboardData, ICY_CLIPBOARD_TYPE};

/// One context for the whole process: on X11 and Wayland the copying program serves the
/// clipboard contents until another program takes over, so it has to outlive each copy.
static CONTEXT: LazyLock<Mutex<Option<ClipboardContext>>> = LazyLock::new(|| {
    Mutex::new(
        ClipboardContext::new()
            .inspect_err(|error| log::warn!("could not open the system clipboard: {error}"))
            .ok(),
    )
});

static DISABLED: AtomicBool = AtomicBool::new(false);

/// Keeps the process off the system clipboard, e.g. in tests: copies go through egui and pastes
/// use the text egui reports.
pub fn disable() {
    DISABLED.store(true, Ordering::Relaxed);
}

fn with_context<R>(operation: impl FnOnce(&ClipboardContext) -> clipboard_rs::common::Result<R>) -> Result<R, String> {
    if DISABLED.load(Ordering::Relaxed) {
        return Err("the system clipboard is disabled".to_string());
    }
    let context = CONTEXT.lock().unwrap_or_else(PoisonError::into_inner);
    let context = context.as_ref().ok_or_else(|| "the system clipboard is not available".to_string())?;
    operation(context).map_err(|error| error.to_string())
}

/// What a paste inserts, in the order it is preferred.
#[derive(Debug, Clone, PartialEq)]
pub enum PasteContent {
    /// Characters and attributes copied from an Icy tool, with the text copied along with them.
    Icy {
        text: String,
        data: Vec<u8>,
    },
    Image(image::RgbaImage),
    Text(String),
}

impl PasteContent {
    /// The text of the paste, e.g. for a terminal; images have none.
    pub fn text(&self) -> Option<&str> {
        match self {
            PasteContent::Icy { text, .. } | PasteContent::Text(text) => Some(text),
            PasteContent::Image(_) => None,
        }
    }
}

fn png_image(image: &image::RgbaImage) -> Option<RustImageData> {
    let mut png = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    RustImageData::from_bytes(&png).ok()
}

/// The clipboard entries for `data`, one per format.
pub fn contents(data: &ClipboardData) -> Vec<ClipboardContent> {
    let mut contents = vec![ClipboardContent::Text(data.text.clone())];
    if let Some(rtf) = &data.rtf {
        contents.push(ClipboardContent::Rtf(rtf.clone()));
    }
    let image = data
        .image
        .as_ref()
        .and_then(|(pixels, width, height)| image::RgbaImage::from_raw(*width, *height, pixels.clone()));
    if let Some(image) = image.as_ref().and_then(png_image) {
        contents.push(ClipboardContent::Image(image));
    }
    if let Some(icy) = &data.icy_data {
        contents.push(ClipboardContent::Other(ICY_CLIPBOARD_TYPE.to_string(), icy.clone()));
    }
    contents
}

/// Replaces the clipboard contents with `contents`.
pub fn write(contents: Vec<ClipboardContent>) -> Result<(), String> {
    with_context(|context| context.set(contents))
}

/// Copies every format of `data`.
pub fn copy_data(data: &ClipboardData) -> Result<(), String> {
    write(contents(data))
}

pub fn copy_text(text: &str) -> Result<(), String> {
    write(vec![ClipboardContent::Text(text.to_string())])
}

pub fn copy_image(image: &image::RgbaImage) -> Result<(), String> {
    let image = png_image(image).ok_or_else(|| "the image could not be encoded".to_string())?;
    write(vec![ClipboardContent::Image(image)])
}

/// Copies an application format together with a text version, which lets egui notice the paste.
pub fn copy_format(format: &str, data: Vec<u8>, text: &str) -> Result<(), String> {
    write(vec![
        ClipboardContent::Text(text.to_string()),
        ClipboardContent::Other(format.to_string(), data),
    ])
}

/// The clipboard data in `format`, if the clipboard holds it.
pub fn read_format(format: &str) -> Option<Vec<u8>> {
    with_context(|context| context.get_buffer(format)).ok().filter(|data| !data.is_empty())
}

/// The richest content on the clipboard: ICY data, then an image, then text.
pub fn read() -> Option<PasteContent> {
    with_context(|context| {
        let text = context.get_text().ok().filter(|text| !text.is_empty());
        if let Some(data) = context.get_buffer(ICY_CLIPBOARD_TYPE).ok().filter(|data| !data.is_empty()) {
            return Ok(Some(PasteContent::Icy {
                text: text.unwrap_or_default(),
                data,
            }));
        }
        if context.has(clipboard_rs::ContentFormat::Image) {
            let image = context
                .get_image()
                .and_then(|image| image.to_png())
                .ok()
                .and_then(|png| image::load_from_memory(png.get_bytes()).ok());
            if let Some(image) = image {
                return Ok(Some(PasteContent::Image(image.to_rgba8())));
            }
        }
        Ok(text.map(PasteContent::Text))
    })
    .ok()
    .flatten()
}

/// Copies the selection of `screen` in every format; without a system clipboard, egui gets the
/// text. Returns false if nothing is selected.
#[cfg(feature = "egui")]
pub fn copy_selection(context: &egui::Context, screen: &dyn Screen) -> bool {
    match prepare_clipboard_data(screen) {
        Ok(data) => {
            copy_data_or_text(context, &data);
            true
        }
        Err(_) => false,
    }
}

/// Copies every format of `data`, or only its text through egui if the system clipboard fails.
#[cfg(feature = "egui")]
pub fn copy_data_or_text(context: &egui::Context, data: &ClipboardData) {
    if let Err(error) = copy_data(data) {
        log::warn!("could not copy to the system clipboard: {error}");
        context.copy_text(data.text.clone());
    }
}

/// Copies `text` to the system clipboard, or through egui if that fails.
#[cfg(feature = "egui")]
pub fn copy_text_or_egui(context: &egui::Context, text: String) {
    if let Err(error) = copy_text(&text) {
        log::warn!("could not copy to the system clipboard: {error}");
        context.copy_text(text);
    }
}

/// Copies `image` to the system clipboard, or through egui if that fails.
#[cfg(feature = "egui")]
pub fn copy_image_or_egui(context: &egui::Context, image: &image::RgbaImage) {
    if let Err(error) = copy_image(image) {
        log::warn!("could not copy to the system clipboard: {error}");
        context.copy_image(egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        ));
    }
}

/// What an egui paste event with `text` inserts: the richest format on the system clipboard,
/// or the text egui read if the system clipboard cannot be read.
pub fn paste_event(text: &str) -> PasteContent {
    read().unwrap_or_else(|| PasteContent::Text(text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_selection_is_copied_in_every_format() {
        let data = ClipboardData {
            text: "HI".into(),
            rtf: Some("{\\rtf1 HI}".into()),
            image: Some((vec![255; 2 * 3 * 4], 2, 3)),
            icy_data: Some(vec![1, 2, 3]),
        };
        let contents = contents(&data);
        assert!(matches!(&contents[0], ClipboardContent::Text(text) if text == "HI"));
        assert!(matches!(&contents[1], ClipboardContent::Rtf(rtf) if rtf.starts_with("{\\rtf1")));
        assert!(matches!(&contents[2], ClipboardContent::Image(image) if image.get_size() == (2, 3)));
        assert!(matches!(&contents[3], ClipboardContent::Other(format, bytes) if format == ICY_CLIPBOARD_TYPE && bytes == &[1, 2, 3]));

        let text_only = ClipboardData {
            text: "HI".into(),
            rtf: None,
            image: None,
            icy_data: None,
        };
        assert_eq!(super::contents(&text_only).len(), 1);
    }

    #[test]
    #[ignore = "needs a desktop clipboard"]
    fn every_format_round_trips_through_the_system_clipboard() {
        let data = ClipboardData {
            text: "ICY".into(),
            rtf: Some("{\\rtf1 ICY}".into()),
            image: Some((vec![200; 4 * 4 * 4], 4, 4)),
            icy_data: Some(vec![9, 8, 7]),
        };
        copy_data(&data).unwrap();
        assert_eq!(
            read(),
            Some(PasteContent::Icy {
                text: "ICY".into(),
                data: vec![9, 8, 7]
            })
        );
        copy_image(&image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]))).unwrap();
        assert!(matches!(read(), Some(PasteContent::Image(image)) if image.dimensions() == (3, 2) && image.get_pixel(1, 1).0 == [10, 20, 30, 255]));
        copy_format("application/x-icy-bitfont", vec![4, 5], "glyph").unwrap();
        assert_eq!(read_format("application/x-icy-bitfont"), Some(vec![4, 5]));
        assert_eq!(read(), Some(PasteContent::Text("glyph".into())));
        copy_text("plain").unwrap();
        assert_eq!(read_format(ICY_CLIPBOARD_TYPE), None, "copying elsewhere drops the Icy data");
        assert_eq!(read(), Some(PasteContent::Text("plain".into())));
    }

    #[test]
    fn pasted_images_have_no_text() {
        assert_eq!(PasteContent::Image(image::RgbaImage::new(1, 1)).text(), None);
        assert_eq!(
            PasteContent::Icy {
                text: "A".into(),
                data: vec![0]
            }
            .text(),
            Some("A")
        );
    }
}

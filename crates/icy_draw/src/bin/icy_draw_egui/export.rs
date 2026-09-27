use icy_draw::document::Document;
use icy_engine::FileFormat;
use icy_engine_gui::egui::export::ExportRequest;

/// Writes the document's buffer (not the editor view with its selection and tool overlays).
pub fn write(document: &Document, request: &ExportRequest) -> Result<(), String> {
    request.write_with(|path| {
        if let FileFormat::Image(image) = request.format {
            document
                .with_state(|state| image.save_buffer(state.get_buffer(), path))
                .map_err(|error| error.to_string())
        } else {
            let bytes = document
                .with_state(|state| request.format.to_bytes(state.get_buffer(), &request.options))
                .map_err(|error| error.to_string())?;
            std::fs::write(path, bytes).map_err(|error| error.to_string())
        }
    })
}

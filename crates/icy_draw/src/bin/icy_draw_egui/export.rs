use icy_draw::document::Document;
use icy_engine::{FileFormat, FormatOptions, SaveOptions};
use icy_engine_gui::ExportSettings;
use std::{io::Write, path::Path};

pub fn options(format: FileFormat, settings: &ExportSettings) -> SaveOptions {
    let mut options = SaveOptions::default();
    options.format = match format {
        FileFormat::Ansi | FileFormat::AnsiMusic => FormatOptions::Ansi(icy_engine::AnsiFormatOptions {
            level: settings.ansi_level,
            always_use_rgb: settings.ansi_rgb_output && settings.ansi_level.supports_truecolor(),
            screen_prep: settings.screen_prep,
            line_length: if settings.max_line_length_enabled {
                icy_engine::LineLength::Maximum(settings.max_line_length)
            } else {
                icy_engine::LineLength::Default
            },
            sixel: settings.sixel_settings.clone(),
            ..Default::default()
        }),
        FileFormat::XBin => FormatOptions::Compressed(icy_engine::CompressedFormatOptions { compress: settings.compress }),
        FileFormat::IcyDraw => SaveOptions::icy_draw().format,
        _ => FormatOptions::Character(icy_engine::CharacterFormatOptions {
            screen_prep: settings.screen_prep,
            unicode: settings.utf8_output,
        }),
    };
    options
}

pub fn write(document: &Document, path: &Path, format: FileFormat, settings: &ExportSettings, sauce: bool) -> Result<(), String> {
    let target = if path.exists() {
        path.canonicalize().map_err(|error| error.to_string())?
    } else {
        path.to_path_buf()
    };
    let parent = target.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .suffix(&format!(".{}", format.primary_extension()))
        .tempfile_in(parent)
        .map_err(|error| error.to_string())?;
    if let FileFormat::Image(image) = format {
        document
            .with_state(|state| image.save_buffer(state.get_buffer(), temporary.path()))
            .map_err(|error| error.to_string())?;
    } else {
        let mut options = options(format, settings);
        if sauce {
            options.sauce = Some(document.with_state(|state| state.get_sauce_meta().clone()));
        }
        let bytes = document
            .with_state(|state| format.to_bytes(state.get_buffer(), &options))
            .map_err(|error| error.to_string())?;
        temporary.write_all(&bytes).map_err(|error| error.to_string())?;
    }
    if let Ok(metadata) = std::fs::metadata(&target) {
        temporary.as_file().set_permissions(metadata.permissions()).map_err(|error| error.to_string())?;
    }
    temporary.as_file().sync_all().map_err(|error| error.to_string())?;
    temporary.persist(&target).map_err(|error| error.to_string())?;
    Ok(())
}

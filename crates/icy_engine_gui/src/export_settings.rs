use icy_engine::{
    formats::{FileFormat, FormatOptions, SixelSettings},
    AnsiCompatibilityLevel, AnsiFormatOptions, CharacterFormatOptions, CompressedFormatOptions, ControlCharHandling, IcyDrawFormatOptions, LineBreakBehavior,
    LineEnding, LineLength, SaveOptions, ScreenPreperation,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSettings {
    #[serde(default)]
    pub export_format_ext: Option<String>,
    #[serde(default)]
    pub ansi_level: AnsiCompatibilityLevel,
    #[serde(default)]
    pub ansi_rgb_output: bool,
    #[serde(default)]
    pub screen_prep: ScreenPreperation,
    #[serde(default)]
    pub max_line_length_enabled: bool,
    #[serde(default = "default_max_line_length")]
    pub max_line_length: u16,
    /// With a line length limit: pad shorter lines to `max_line_length` instead of breaking longer ones.
    #[serde(default)]
    pub line_length_minimum: bool,
    #[serde(default)]
    pub line_break: LineBreakBehavior,
    #[serde(default)]
    pub line_ending: LineEnding,
    #[serde(default)]
    pub control_char_handling: ControlCharHandling,
    #[serde(default = "enabled")]
    pub optimize_colors: bool,
    #[serde(default = "enabled")]
    pub normalize_whitespace: bool,
    #[serde(default)]
    pub utf8_output: bool,
    #[serde(default)]
    pub compress: bool,
    #[serde(default)]
    pub sixel_settings: SixelSettings,
}

fn default_max_line_length() -> u16 {
    80
}

fn enabled() -> bool {
    true
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            export_format_ext: None,
            ansi_level: AnsiCompatibilityLevel::default(),
            ansi_rgb_output: false,
            screen_prep: ScreenPreperation::None,
            max_line_length_enabled: false,
            max_line_length: 80,
            line_length_minimum: false,
            line_break: LineBreakBehavior::Wrap,
            line_ending: LineEnding::Lf,
            control_char_handling: ControlCharHandling::Ignore,
            optimize_colors: true,
            normalize_whitespace: true,
            utf8_output: false,
            compress: false,
            sixel_settings: SixelSettings::default(),
        }
    }
}

/// Which of the [`ExportSettings`] a file format reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportOptionKind {
    /// Pixel images: no options.
    Image,
    /// ANSI: compatibility level, line handling, control characters and sixels.
    Ansi,
    /// Avatar, PCBoard, CtrlA and Renegade: screen preparation (PCBoard also UTF-8).
    Character,
    /// ASCII: UTF-8 output.
    Ascii,
    /// XBin and iCE Draw: compression.
    Compressed,
    /// The native format keeps everything and only compresses.
    IcyDraw,
    /// Formats without options.
    Plain,
}

impl ExportOptionKind {
    pub fn of(format: FileFormat) -> Self {
        match format {
            FileFormat::Image(_) => Self::Image,
            FileFormat::Ansi | FileFormat::AnsiMusic => Self::Ansi,
            FileFormat::Avatar | FileFormat::PCBoard | FileFormat::CtrlA | FileFormat::Renegade => Self::Character,
            FileFormat::Ascii => Self::Ascii,
            FileFormat::XBin | FileFormat::IceDraw => Self::Compressed,
            FileFormat::IcyDraw => Self::IcyDraw,
            _ => Self::Plain,
        }
    }

    /// Whether the format runs the color optimizer before saving.
    pub fn is_text(self) -> bool {
        !matches!(self, Self::Image | Self::IcyDraw)
    }

    pub fn stores_sauce(self) -> bool {
        self != Self::Image
    }
}

impl ExportSettings {
    /// Restores the persisted format if `formats` offers it.
    pub fn format_in(&self, formats: &[FileFormat]) -> Option<FileFormat> {
        let format = FileFormat::from_extension(self.export_format_ext.as_deref()?)?;
        formats.contains(&format).then_some(format)
    }

    pub fn line_length(&self) -> LineLength {
        match (self.max_line_length_enabled, self.line_length_minimum) {
            (false, _) => LineLength::Default,
            (true, true) => LineLength::Minimum(self.max_line_length),
            (true, false) => LineLength::Maximum(self.max_line_length),
        }
    }

    /// Save options for `format`; the caller adds the SAUCE metadata.
    pub fn save_options(&self, format: FileFormat) -> SaveOptions {
        let mut options = SaveOptions::default();
        options.preprocess.optimize_colors = self.optimize_colors;
        options.preprocess.normalize_whitespaces = self.normalize_whitespace;
        options.format = match ExportOptionKind::of(format) {
            ExportOptionKind::Ansi => FormatOptions::Ansi(AnsiFormatOptions {
                always_use_rgb: self.ansi_rgb_output && self.ansi_level.supports_truecolor(),
                screen_prep: self.screen_prep,
                line_length: self.line_length(),
                line_break: self.line_break.clone(),
                line_ending: self.line_ending,
                control_char_handling: self.control_char_handling,
                sixel: self.sixel_settings.clone(),
                ..AnsiFormatOptions::new(self.ansi_level)
            }),
            ExportOptionKind::Character | ExportOptionKind::Ascii => FormatOptions::Character(CharacterFormatOptions {
                screen_prep: self.screen_prep,
                unicode: self.utf8_output,
            }),
            ExportOptionKind::Compressed => FormatOptions::Compressed(CompressedFormatOptions { compress: self.compress }),
            ExportOptionKind::IcyDraw => FormatOptions::IcyDraw(IcyDrawFormatOptions {
                compress: self.compress,
                ..Default::default()
            }),
            ExportOptionKind::Image | ExportOptionKind::Plain => FormatOptions::None,
        };
        options
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_settings_files_keep_lossy_defaults() {
        let settings: ExportSettings = toml::from_str("ansi_level = \"IcyTerm\"\nmax_line_length_enabled = true\n").unwrap();
        assert!(settings.optimize_colors && settings.normalize_whitespace);
        assert!(matches!(settings.line_length(), LineLength::Maximum(80)));
    }

    #[test]
    fn save_options_follow_the_format() {
        let settings = ExportSettings {
            ansi_level: AnsiCompatibilityLevel::AnsiSys,
            ansi_rgb_output: true,
            compress: true,
            utf8_output: true,
            screen_prep: ScreenPreperation::ClearScreen,
            ..Default::default()
        };
        let ansi = settings.save_options(FileFormat::Ansi).ansi_options();
        assert!(!ansi.always_use_rgb, "ANSI.SYS has no truecolor");
        assert_eq!(ansi.screen_prep, ScreenPreperation::ClearScreen);
        assert!(settings.save_options(FileFormat::XBin).compressed_options().compress);
        assert!(settings.save_options(FileFormat::IcyDraw).icy_draw_options().compress);
        assert!(settings.save_options(FileFormat::PCBoard).character_options().unicode);
        assert_eq!(settings.format_in(&[FileFormat::Ansi]), None, "without a stored extension nothing is restored");
    }
}

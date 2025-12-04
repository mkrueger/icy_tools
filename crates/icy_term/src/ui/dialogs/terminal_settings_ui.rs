use i18n_embed_fl::fl;
use iced::{
    Alignment, Length,
    widget::{column, pick_list, row},
};
use icy_engine::{ScreenMode, VGA_MODES};
use icy_engine_gui::settings::left_label;
use icy_engine_gui::ui::{DIALOG_SPACING, SPACE_4, TEXT_SIZE_NORMAL};
use icy_net::telnet::TerminalEmulation;
use icy_parser_core::MusicOption;
use std::fmt;

const COMBO_WIDTH: f32 = 120.0;

/// Wrapper for TerminalEmulation to implement Display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalEmulationWrapper(pub TerminalEmulation);

impl fmt::Display for TerminalEmulationWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            TerminalEmulation::Ansi => write!(f, "ANSI"),
            TerminalEmulation::Utf8Ansi => write!(f, "UTF8ANSI"),
            TerminalEmulation::Ascii => write!(f, "ASCII"),
            TerminalEmulation::Avatar => write!(f, "Avatar"),
            TerminalEmulation::PETscii => write!(f, "PETSCII"),
            TerminalEmulation::ATAscii => write!(f, "ATASCII"),
            TerminalEmulation::ViewData => write!(f, "ViewData"),
            TerminalEmulation::Mode7 => write!(f, "Mode 7"),
            TerminalEmulation::AtariST => write!(f, "Atari ST"),
            TerminalEmulation::Rip => write!(f, "RIP"),
            TerminalEmulation::Skypix => write!(f, "SkyPix"),
        }
    }
}

/// Current terminal settings state
#[derive(Debug, Clone)]
pub struct TerminalSettings {
    pub terminal_type: TerminalEmulation,
    pub screen_mode: ScreenMode,
    pub ansi_music: MusicOption,
}

/// Types of changes that can be made to terminal settings
#[derive(Debug, Clone)]
pub enum TerminalSettingsChange {
    TerminalType(TerminalEmulation),
    ScreenMode(ScreenMode),
    AnsiMusic(MusicOption),
}

/// Check if the terminal type supports VGA modes
pub fn supports_vga_modes(terminal_type: TerminalEmulation) -> bool {
    matches!(
        terminal_type,
        TerminalEmulation::Ansi | TerminalEmulation::Utf8Ansi | TerminalEmulation::Avatar | TerminalEmulation::Ascii
    )
}

/// Check if the terminal type supports ANSI music
pub fn supports_ansi_music(terminal_type: TerminalEmulation) -> bool {
    matches!(terminal_type, TerminalEmulation::Ansi | TerminalEmulation::Utf8Ansi)
}

/// Get the default screen mode for a terminal type
pub fn get_default_screen_mode(terminal_type: TerminalEmulation) -> ScreenMode {
    match terminal_type {
        TerminalEmulation::Ansi | TerminalEmulation::Ascii | TerminalEmulation::Avatar | TerminalEmulation::Rip | TerminalEmulation::Utf8Ansi => {
            ScreenMode::Vga(80, 25)
        }
        TerminalEmulation::AtariST => ScreenMode::AtariST(icy_engine::TerminalResolution::Medium, true),
        TerminalEmulation::PETscii => ScreenMode::Vic,
        TerminalEmulation::ATAscii => ScreenMode::Atascii(40),
        TerminalEmulation::ViewData | TerminalEmulation::Mode7 => ScreenMode::Videotex,
        TerminalEmulation::Skypix => ScreenMode::SkyPix,
    }
}

/// Build the terminal settings UI elements (terminal type, screen mode, ANSI music pickers)
///
/// Returns a Column with the settings controls.
///
/// # Arguments
/// * `settings` - Current terminal settings
/// * `on_change` - Callback when a setting changes
pub fn build_terminal_settings_ui<'a, M: Clone + 'static>(
    settings: &TerminalSettings,
    on_change: impl Fn(TerminalSettingsChange) -> M + 'a + Clone,
) -> iced::widget::Column<'a, M> {
    let terminal_type_label = fl!(crate::LANGUAGE_LOADER, "dialing_directory-terminal_type");
    let screen_mode_label = fl!(crate::LANGUAGE_LOADER, "dialing_directory-screen_mode");
    let ansi_music_label = fl!(crate::LANGUAGE_LOADER, "dialing_directory-music-option");

    let terminal_types = vec![
        TerminalEmulationWrapper(TerminalEmulation::Ansi),
        TerminalEmulationWrapper(TerminalEmulation::Utf8Ansi),
        TerminalEmulationWrapper(TerminalEmulation::Ascii),
        TerminalEmulationWrapper(TerminalEmulation::Rip),
        TerminalEmulationWrapper(TerminalEmulation::AtariST),
        TerminalEmulationWrapper(TerminalEmulation::PETscii),
        TerminalEmulationWrapper(TerminalEmulation::ATAscii),
        TerminalEmulationWrapper(TerminalEmulation::ViewData),
        TerminalEmulationWrapper(TerminalEmulation::Mode7),
        TerminalEmulationWrapper(TerminalEmulation::Skypix),
        TerminalEmulationWrapper(TerminalEmulation::Avatar),
    ];

    let on_change_clone = on_change.clone();
    let term_pick = pick_list(
        terminal_types,
        Some(TerminalEmulationWrapper(settings.terminal_type)),
        move |t: TerminalEmulationWrapper| on_change_clone(TerminalSettingsChange::TerminalType(t.0)),
    )
    .width(Length::Fixed(COMBO_WIDTH))
    .text_size(TEXT_SIZE_NORMAL);

    let mut content = column![
        row![left_label(terminal_type_label), term_pick]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center),
    ]
    .spacing(SPACE_4);

    // Screen mode picker (only for certain terminal types)
    if supports_vga_modes(settings.terminal_type) {
        let mut vga_modes = VGA_MODES.to_vec();
        if settings.screen_mode.is_custom_vga() && !vga_modes.contains(&settings.screen_mode) {
            vga_modes.push(settings.screen_mode);
        }

        let on_change_clone = on_change.clone();
        let screen_mode_pick = pick_list(vga_modes, Some(settings.screen_mode), move |sm| {
            on_change_clone(TerminalSettingsChange::ScreenMode(sm))
        })
        .width(Length::Fixed(COMBO_WIDTH))
        .text_size(TEXT_SIZE_NORMAL);

        content = content.push(
            row![left_label(screen_mode_label), screen_mode_pick]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
        );
    }

    // ANSI music picker (only for ANSI/UTF8ANSI)
    if supports_ansi_music(settings.terminal_type) {
        let music_options = vec![MusicOption::Off, MusicOption::Banana, MusicOption::Conflicting, MusicOption::Both];

        let on_change_clone = on_change.clone();
        let music_pick = pick_list(music_options, Some(settings.ansi_music), move |m| {
            on_change_clone(TerminalSettingsChange::AnsiMusic(m))
        })
        .width(Length::Fixed(COMBO_WIDTH))
        .text_size(TEXT_SIZE_NORMAL);

        content = content.push(
            row![left_label(ansi_music_label), music_pick]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
        );
    }

    content
}

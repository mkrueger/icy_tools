use crate::{ConnectionInformation, ScreenMode, TerminalResult};
//use crate::ui::screen_modes::ScreenMode;
use chrono::{Duration, Utc};
use icy_engine::ansi::{BaudEmulation, MusicOption};
use icy_engine::rip::RIP_SCREEN_SIZE;
use icy_engine::skypix::SKYPIX_SCREEN_SIZE;
use icy_engine::{BufferParser, ansi, ascii, atascii, avatar, mode7, petscii, rip, skypix, viewdata};
use icy_net::ConnectionType;
use icy_net::telnet::TerminalEmulation;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{
    fs::{self},
    path::PathBuf,
};

pub const ALL_TERMINALS: [TerminalEmulation; 11] = [
    TerminalEmulation::Ansi,
    TerminalEmulation::Utf8Ansi,
    TerminalEmulation::Avatar,
    TerminalEmulation::Ascii,
    TerminalEmulation::Rip,
    TerminalEmulation::PETscii,
    TerminalEmulation::ATAscii,
    TerminalEmulation::AtariST,
    TerminalEmulation::Skypix,
    TerminalEmulation::ViewData,
    TerminalEmulation::Mode7,
];

pub fn fmt_terminal_emulation(emulator: &TerminalEmulation) -> &str {
    match emulator {
        TerminalEmulation::Ansi => "ANSI",
        TerminalEmulation::Utf8Ansi => "UTF8ANSI",
        TerminalEmulation::Avatar => "AVATAR",
        TerminalEmulation::Ascii => "Raw (ASCII)",
        TerminalEmulation::PETscii => "C64/C128 (PETSCII)",
        TerminalEmulation::ATAscii => "Atari (ATASCII)",
        TerminalEmulation::ViewData => "Viewdata",
        TerminalEmulation::Mode7 => "BBC Micro Mode 7",
        TerminalEmulation::Rip => "RIPscrip",
        TerminalEmulation::Skypix => "Skypix",
        TerminalEmulation::AtariST => "Atari ST",
    }
}

#[must_use]
pub fn get_parser(emulator: &TerminalEmulation, use_ansi_music: MusicOption, screen_mode: ScreenMode, cache_directory: PathBuf) -> Box<dyn BufferParser> {
    match emulator {
        TerminalEmulation::Ansi | TerminalEmulation::Utf8Ansi => {
            let mut parser = ansi::Parser::default();
            parser.ansi_music = use_ansi_music;
            parser.bs_is_ctrl_char = true;
            Box::new(parser)
        }
        TerminalEmulation::Avatar => Box::<avatar::Parser>::default(),
        TerminalEmulation::Ascii => Box::<ascii::Parser>::default(),
        TerminalEmulation::PETscii => Box::<petscii::Parser>::default(),
        TerminalEmulation::ATAscii => Box::<atascii::Parser>::default(),
        TerminalEmulation::ViewData => Box::<viewdata::Parser>::default(),
        TerminalEmulation::Mode7 => Box::<mode7::Parser>::default(),
        TerminalEmulation::Rip => {
            let mut parser = ansi::Parser::default();
            parser.ansi_music = use_ansi_music;
            parser.bs_is_ctrl_char = true;
            let parser = rip::Parser::new(Box::new(parser), cache_directory, RIP_SCREEN_SIZE);
            Box::new(parser)
        }
        TerminalEmulation::Skypix => {
            let mut parser = ansi::Parser::default();
            parser.ansi_music = use_ansi_music;
            parser.bs_is_ctrl_char = true;
            let parser = skypix::Parser::new(Box::new(parser), cache_directory, SKYPIX_SCREEN_SIZE);
            Box::new(parser)
        }
        TerminalEmulation::AtariST => {
            let res = if let ScreenMode::AtariST(cols) = screen_mode {
                if cols == 80 {
                    icy_engine::igs::TerminalResolution::Medium
                } else {
                    icy_engine::igs::TerminalResolution::Low
                }
            } else {
                icy_engine::igs::TerminalResolution::Low
            };

            Box::new(icy_engine::igs::Parser::new(res))
        }
    }
}

/**/

/*
impl Display for ConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionType::Ssh => write!(f, "SSH"),
            ConnectionType::Raw => write!(f, "Raw"),
            ConnectionType::Telnet => write!(f, "Telnet"),
            ConnectionType::Modem => write!(f, "Modem"),
            ConnectionType::Serial => write!(f, "Serial"),
            ConnectionType::Websocket => write!(f, "WebSocket"),
            ConnectionType::SecureWebsocket => write!(f, "Secure WebSocket"),
        }
    }
}
*/
pub const ALL: [ConnectionType; 8] = [
    ConnectionType::Telnet,
    ConnectionType::Raw,
    ConnectionType::Modem,
    ConnectionType::SSH,
    ConnectionType::SecureWebsocket,
    ConnectionType::Websocket,
    ConnectionType::Rlogin,
    ConnectionType::RloginSwapped,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressBook {
    pub version: Version,

    #[serde(skip)]
    pub write_lock: bool,

    #[serde(skip)]
    created_backup: bool,

    pub addresses: Vec<Address>,
}

impl Default for AddressBook {
    fn default() -> Self {
        let mut res = Self {
            version: Version::new(1, 0, 0),
            write_lock: false,
            created_backup: false,
            addresses: Vec::new(),
        };
        res.load_string(TEMPLATE).unwrap_or_default();
        res
    }
}

/// Global lock to prevent writing the phone book if there was an error loading it
pub static mut PHONE_LOCK: bool = false;

impl AddressBook {
    fn load_string(&mut self, input_text: &str) -> TerminalResult<()> {
        // Parse the TOML using serde
        let loaded: AddressBook = toml::from_str(input_text)?;

        // Check version compatibility
        let current_version = Version::new(1, 1, 0);
        if loaded.version > current_version {
            log::warn!("Newer address book version: {}", loaded.version);
            self.write_lock = true;
        }

        self.version = loaded.version;
        self.addresses = loaded.addresses;

        Ok(())
    }

    pub fn load_phone_book() -> TerminalResult<AddressBook> {
        let mut res = AddressBook::new();

        if let Some(dialing_directory) = Address::get_dialing_directory_file() {
            if !dialing_directory.exists() {
                log::error!("Dialing directory file does not exist: {:?}, creating deafult", dialing_directory);
                return Ok(AddressBook::default());
            }

            match fs::read_to_string(dialing_directory) {
                Ok(input_text) => {
                    if let Err(err) = res.load_string(&input_text) {
                        log::error!("Error parsing phonebook {err}");
                        return Err(err.into());
                    }
                }
                Err(err) => {
                    log::error!("Error reading phonebook {err}");
                    return Err(err.into());
                }
            }
        }
        Ok(res)
    }

    pub fn store_phone_book(&mut self) -> TerminalResult<()> {
        if self.write_lock || unsafe { PHONE_LOCK } {
            return Ok(());
        }

        if let Some(file_name) = Address::get_dialing_directory_file() {
            // Create a copy for serialization (skip the first empty address)
            let mut save_book = self.clone();
            save_book.version = Version::new(1, 1, 0);

            // Remove the first empty address if it exists
            if !save_book.addresses.is_empty() && save_book.addresses[0].system_name.is_empty() {
                save_book.addresses.remove(0);
            }

            // Serialize to TOML using serde
            let toml_string = toml::to_string_pretty(&save_book)?;

            // Create temp file to write the new dialing directory
            let mut write_name: PathBuf = file_name.clone();
            write_name.set_extension("new");
            fs::write(&write_name, toml_string)?;

            let mut backup_file: PathBuf = file_name.clone();
            backup_file.set_extension("bak");

            // Backup old file, if it has contents
            // NOTE: just backup once per session, otherwise it gets overwritten too easily
            if !self.created_backup {
                self.created_backup = true;
                if let Ok(data) = fs::metadata(&file_name) {
                    if data.len() > 0 {
                        std::fs::rename(&file_name, &backup_file)?;
                    }
                }
            }

            // Move temp file to the real file
            std::fs::rename(&write_name, &file_name)?;
        }
        Ok(())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub system_name: String,

    #[serde(default, skip_serializing_if = "is_default_bool")]
    pub is_favored: bool,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_name: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,

    #[serde(default, skip_serializing_if = "is_default_terminal")]
    pub terminal_type: TerminalEmulation,

    pub address: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auto_login: String,

    #[serde(default, skip_serializing_if = "is_default_connection")]
    pub protocol: ConnectionType,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub proxy_command: String,

    #[serde(default, skip_serializing_if = "is_default_bool")]
    pub ice_mode: bool,

    #[serde(default, skip_serializing_if = "is_default_music")]
    pub ansi_music: MusicOption,

    #[serde(default, skip_serializing_if = "is_default_baud")]
    pub baud_emulation: BaudEmulation,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_name: Option<String>,

    #[serde(default, skip_serializing_if = "is_default_screen_mode")]
    pub screen_mode: ScreenMode,

    #[serde(default, skip_serializing_if = "is_default_datetime")]
    pub created: chrono::DateTime<Utc>,

    #[serde(default, skip_serializing_if = "is_default_datetime")]
    pub updated: chrono::DateTime<Utc>,

    #[serde(default, skip_serializing_if = "is_zero_duration")]
    pub overall_duration: chrono::Duration,

    #[serde(default, skip_serializing_if = "is_zero")]
    pub number_of_calls: usize,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_call: Option<chrono::DateTime<Utc>>,

    #[serde(default, skip_serializing_if = "is_zero_duration")]
    pub last_call_duration: chrono::Duration,

    #[serde(default, skip_serializing_if = "is_zero")]
    pub uploaded_bytes: usize,

    #[serde(default, skip_serializing_if = "is_zero")]
    pub downloaded_bytes: usize,
}

impl From<ConnectionInformation> for Address {
    fn from(info: ConnectionInformation) -> Self {
        let time = Utc::now();
        unsafe {
            current_id = current_id.wrapping_add(1);
        }

        // Build the address string (host:port)
        let address = if info.protocol() == ConnectionType::SSH {
            info.to_string()
        } else {
            info.endpoint()
        };

        Self {
            system_name: info.host.clone(),
            user_name: if info.protocol() == ConnectionType::SSH {
                String::new()
            } else {
                info.user_name().clone().unwrap_or_default()
            },
            password: if info.protocol() == ConnectionType::SSH {
                String::new()
            } else {
                info.password().clone().unwrap_or_default()
            },
            comment: String::new(),
            terminal_type: TerminalEmulation::default(),
            font_name: None,
            screen_mode: ScreenMode::default(),
            auto_login: String::new(),
            address,
            proxy_command: String::new(),
            protocol: info.protocol(),
            ansi_music: MusicOption::default(),
            ice_mode: true,
            is_favored: false,
            created: time,
            updated: time,
            overall_duration: Duration::zero(),
            number_of_calls: 0,
            last_call: None,
            last_call_duration: Duration::zero(),
            uploaded_bytes: 0,
            downloaded_bytes: 0,
            baud_emulation: BaudEmulation::default(),
        }
    }
}

// Helper functions for skip_serializing_if
fn is_default_bool(b: &bool) -> bool {
    !*b // Assuming false is the default for most bool fields
}

fn is_default_terminal(t: &TerminalEmulation) -> bool {
    matches!(t, TerminalEmulation::Ansi) // Assuming Ansi is the default
}

fn is_default_connection(c: &ConnectionType) -> bool {
    matches!(c, ConnectionType::Telnet) // Assuming Telnet is the default
}

fn is_default_music(m: &MusicOption) -> bool {
    *m == MusicOption::default()
}

fn is_default_baud(b: &BaudEmulation) -> bool {
    *b == BaudEmulation::default()
}

fn is_default_screen_mode(s: &ScreenMode) -> bool {
    *s == ScreenMode::default()
}

fn is_default_datetime(dt: &chrono::DateTime<Utc>) -> bool {
    // Skip if it's the unix epoch (default uninitialized datetime)
    dt.timestamp() == 0
}

fn is_zero_duration(d: &chrono::Duration) -> bool {
    d.is_zero()
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

const TEMPLATE: &str = include_str!("default_phonebook.toml");

static mut current_id: usize = 0;

impl Address {
    pub fn new(system_name: impl Into<String>) -> Self {
        let time = Utc::now();
        unsafe {
            current_id = current_id.wrapping_add(1);
        }

        Self {
            system_name: system_name.into(),
            user_name: String::new(),
            password: String::new(),
            comment: String::new(),
            terminal_type: TerminalEmulation::default(),
            font_name: None,
            screen_mode: ScreenMode::default(),
            auto_login: String::new(),
            address: String::new(),
            proxy_command: String::new(),
            protocol: ConnectionType::Telnet,
            ansi_music: MusicOption::default(),
            ice_mode: true,
            is_favored: false,
            created: time,
            updated: time,
            overall_duration: Duration::zero(),
            number_of_calls: 0,
            last_call: None,
            last_call_duration: Duration::zero(),
            uploaded_bytes: 0,
            downloaded_bytes: 0,
            baud_emulation: BaudEmulation::default(),
        }
    }

    #[must_use]
    pub fn get_dialing_directory_file() -> Option<PathBuf> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_term") {
            if !proj_dirs.config_dir().exists() && fs::create_dir_all(proj_dirs.config_dir()).is_err() {
                log::error!("Can't create configuration directory {:?}", proj_dirs.config_dir());
                return None;
            }
            let dialing_directory = proj_dirs.config_dir().join("phonebook.toml");
            if !dialing_directory.exists() {
                if let Err(err) = fs::write(&dialing_directory, TEMPLATE) {
                    log::error!("Can't create dialing_directory {dialing_directory:?} : {err}");
                    return None;
                }
            }
            return Some(dialing_directory);
        }
        None
    }

    #[must_use]
    pub fn get_rip_cache(&self) -> Option<PathBuf> {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_term") {
            let mut cache_directory = proj_dirs.config_dir().join("cache");
            if !cache_directory.exists() && fs::create_dir_all(&cache_directory).is_err() {
                log::error!("Can't create cache directory {:?}", &cache_directory);
                return None;
            }
            let mut address = String::new();
            for c in self.address.chars() {
                if c.is_ascii_alphanumeric() {
                    address.push(c);
                } else {
                    address.push('_');
                }
            }
            cache_directory.push(address);
            if !cache_directory.exists() && fs::create_dir_all(&cache_directory).is_err() {
                log::error!("Can't create cache directory {:?}", &cache_directory);
                return None;
            }
            cache_directory = cache_directory.join("rip");
            if !cache_directory.exists() && fs::create_dir_all(&cache_directory).is_err() {
                log::error!("Can't create cache directory {:?}", &cache_directory);
                return None;
            }
            Some(cache_directory)
        } else {
            None
        }
    }

    pub(crate) fn get_screen_mode(&self) -> ScreenMode {
        match self.terminal_type {
            TerminalEmulation::Ansi | TerminalEmulation::Avatar | TerminalEmulation::Ascii => self.screen_mode.clone(),
            TerminalEmulation::Utf8Ansi => match self.screen_mode {
                ScreenMode::Vga(w, h) => ScreenMode::Unicode(w, h),
                _ => ScreenMode::Unicode(80, 25),
            },
            TerminalEmulation::PETscii => ScreenMode::Vic,
            TerminalEmulation::ATAscii => self.screen_mode.clone(),
            TerminalEmulation::ViewData => ScreenMode::Videotex,
            TerminalEmulation::Mode7 => ScreenMode::Mode7,
            TerminalEmulation::Rip => ScreenMode::Rip,
            TerminalEmulation::Skypix => ScreenMode::SkyPix,
            TerminalEmulation::AtariST => ScreenMode::AtariST(40),
        }
    }
}

pub static mut READ_ADDRESSES: bool = false;

fn watch<P: AsRef<Path>>(path: P) -> notify::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    for res in rx {
        match res {
            Ok(_) => unsafe {
                READ_ADDRESSES = true;
            },
            Err(e) => eprintln!("watch error: {e:}"),
        }
    }

    Ok(())
}

impl AddressBook {
    #[must_use]
    pub fn new() -> Self {
        let addresses = vec![Address::new(String::new())];
        Self {
            version: Version::new(1, 1, 0),
            write_lock: false,
            created_backup: false,
            addresses,
        }
    }
}

pub fn start_watch_thread() {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(dialing_directory) = Address::get_dialing_directory_file() {
        if let Err(err) = std::thread::Builder::new().name("file_watcher_thread".to_string()).spawn(move || {
            loop {
                if let Some(path) = dialing_directory.parent() {
                    if watch(path).is_err() {
                        return;
                    }
                }
            }
        }) {
            log::error!("Error starting file watcher thread: {err}");
        }
    }
}

lazy_static::lazy_static! {
    pub static ref vga_regex: Regex = Regex::new("vga\\((\\d+),\\s*(\\d+)\\)").unwrap();
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;

    #[test]
    fn test_load_default_template() {
        let mut res = AddressBook {
            version: Version::new(1, 1, 0),
            write_lock: false,
            created_backup: false,
            addresses: Vec::new(),
        };
        res.load_string(TEMPLATE).unwrap();
    }
}

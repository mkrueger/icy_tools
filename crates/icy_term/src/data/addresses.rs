use crate::{ConnectionInformation, TerminalResult};
//use crate::ui::screen_modes::ScreenMode;
use chrono::{Duration, Utc};
use icy_engine::ScreenMode;
use icy_net::telnet::TerminalEmulation;
use icy_net::ConnectionType;
use icy_parser_core::{BaudEmulation, MusicOption};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::{
    collections::HashSet,
    fs::{self},
    path::PathBuf,
    sync::OnceLock,
};

#[cfg(unix)]
fn secure_phonebook_file(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn secure_phonebook_file(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

static PHONEBOOK_FILE_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

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

#[must_use]
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SshAuthenticationMode {
    #[default]
    Password,
    PrivateKey,
    Agent,
    Auto,
}

impl fmt::Display for SshAuthenticationMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password => formatter.write_str("Password"),
            Self::PrivateKey => formatter.write_str("Private key"),
            Self::Agent => formatter.write_str("SSH agent"),
            Self::Auto => formatter.write_str("Automatic"),
        }
    }
}

#[must_use]
pub fn normalize_screen_mode(terminal_type: TerminalEmulation, screen_mode: ScreenMode) -> ScreenMode {
    match terminal_type {
        TerminalEmulation::Ansi | TerminalEmulation::Avatar | TerminalEmulation::Ascii => screen_mode,
        TerminalEmulation::Utf8Ansi => match screen_mode {
            ScreenMode::Vga(w, h) | ScreenMode::Unicode(w, h) => ScreenMode::Unicode(w, h),
            _ => ScreenMode::Unicode(80, 25),
        },
        TerminalEmulation::PETscii => ScreenMode::Vic,
        TerminalEmulation::ATAscii => match screen_mode {
            ScreenMode::Atascii(w) => ScreenMode::Atascii(w),
            _ => ScreenMode::Atascii(40),
        },
        TerminalEmulation::ViewData => ScreenMode::Videotex,
        TerminalEmulation::Mode7 => ScreenMode::Mode7,
        TerminalEmulation::Rip => ScreenMode::Rip,
        TerminalEmulation::Skypix => ScreenMode::SkyPix,
        TerminalEmulation::AtariST => match screen_mode {
            ScreenMode::AtariST(res, igs) => ScreenMode::AtariST(res, igs),
            _ => ScreenMode::AtariST(icy_engine::TerminalResolution::Medium, false),
        },
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
#[allow(clippy::unsafe_derive_deserialize)] // unsafe here only guards a module-level static lock, not this struct's fields
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
    fn prune_orphaned_cache_dirs(&self, cache_root: &Path) -> std::io::Result<()> {
        if !cache_root.is_dir() {
            return Ok(());
        }
        let active: HashSet<String> = self
            .addresses
            .iter()
            .filter(|address| !address.address.is_empty())
            .map(|address| Address::cache_key(&address.address))
            .collect();
        for entry in fs::read_dir(cache_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() || active.contains(&entry.file_name().to_string_lossy().into_owned()) {
                continue;
            }
            let path = entry.path();
            if path.join("rip").is_dir() {
                fs::remove_dir_all(path)?;
            }
        }
        Ok(())
    }

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
            secure_phonebook_file(&dialing_directory)?;
            if !dialing_directory.exists() {
                log::error!("Dialing directory file does not exist: {}, creating deafult", dialing_directory.display());
                return Ok(AddressBook::default());
            }

            match fs::read_to_string(dialing_directory) {
                Ok(input_text) => {
                    if let Err(err) = res.load_string(&input_text) {
                        log::error!("Error parsing phonebook {err}");
                        return Err(err);
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
            save_book.addresses.retain(|address| address.web_source.is_none());
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
            secure_phonebook_file(&write_name)?;

            let mut backup_file: PathBuf = file_name.clone();
            backup_file.set_extension("bak");

            // Backup old file, if it has contents
            // NOTE: just backup once per session, otherwise it gets overwritten too easily
            if !self.created_backup {
                self.created_backup = true;
                if let Ok(data) = fs::metadata(&file_name) {
                    if data.len() > 0 {
                        std::fs::rename(&file_name, &backup_file)?;
                        secure_phonebook_file(&backup_file)?;
                    }
                }
            }

            // Move temp file to the real file
            std::fs::rename(&write_name, &file_name)?;
            secure_phonebook_file(&file_name)?;
            if let Some(cache_root) = Address::cache_root() {
                if let Err(err) = self.prune_orphaned_cache_dirs(&cache_root) {
                    log::warn!("Unable to prune orphaned BBS caches: {err}");
                }
            }
        }
        Ok(())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::unsafe_derive_deserialize)] // unsafe here only bumps a module-level id counter, not this struct's fields
pub struct Address {
    #[serde(skip)]
    pub web_source: Option<String>,

    pub system_name: String,

    #[serde(default, skip_serializing_if = "is_default_bool")]
    pub is_favored: bool,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_name: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password: String,

    #[serde(default, skip_serializing_if = "is_default_ssh_authentication")]
    pub ssh_authentication: SshAuthenticationMode,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ssh_private_key: String,

    #[serde(skip)]
    pub ssh_key_passphrase: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,

    #[serde(default, skip_serializing_if = "is_default_terminal")]
    pub terminal_type: TerminalEmulation,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub modem_id: String,

    pub address: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auto_login: String,

    #[serde(default, skip_serializing_if = "is_default_connection")]
    pub protocol: ConnectionType,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub proxy_command: String,

    /// Optional per-connection proxy (e.g. SOCKS5 for Tor/I2P). Applies to the
    /// TCP-based connection types (Telnet, Raw, Rlogin, SSH).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<icy_net::proxy::ProxyConfig>,

    #[serde(default, skip_serializing_if = "is_default_bool")]
    pub ice_mode: bool,

    #[serde(default, skip_serializing_if = "is_default_music")]
    pub ansi_music: MusicOption,

    #[serde(default, skip_serializing_if = "is_default_baud")]
    pub baud_emulation: BaudEmulation,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_palette: Option<Vec<[u8; 3]>>,

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

    /// Enable mouse reporting to the remote system (default: true)
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub mouse_reporting_enabled: bool,

    /// Treat received LF as LF+CR. Stored inverted so the default stays enabled,
    /// which is what every BBS expects.
    #[serde(default, rename = "lf_expand_off", skip_serializing_if = "is_default_bool")]
    pub(crate) lf_expand_off: bool,
}

impl Address {
    /// Treat received LF as LF+CR.
    #[must_use]
    pub fn lf_expand(&self) -> bool {
        !self.lf_expand_off
    }

    pub fn set_lf_expand(&mut self, enabled: bool) {
        self.lf_expand_off = !enabled;
    }
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
            custom_palette: None,
            screen_mode: ScreenMode::default(),
            auto_login: String::new(),
            address,
            proxy_command: String::new(),
            proxy: None,
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
            mouse_reporting_enabled: true,
            ..Default::default()
        }
    }
}

// Helper functions for skip_serializing_if
fn is_default_bool(b: &bool) -> bool {
    !*b // Assuming false is the default for most bool fields
}

fn is_true(b: &bool) -> bool {
    *b
}

fn default_true() -> bool {
    true
}

fn is_default_terminal(t: &TerminalEmulation) -> bool {
    matches!(t, TerminalEmulation::Ansi) // Assuming Ansi is the default
}

fn is_default_connection(c: &ConnectionType) -> bool {
    matches!(c, ConnectionType::Telnet) // Assuming Telnet is the default
}

fn is_default_ssh_authentication(authentication: &SshAuthenticationMode) -> bool {
    *authentication == SshAuthenticationMode::default()
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
    fn cache_key(address: &str) -> String {
        address.chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' }).collect()
    }

    fn cache_root() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "GitHub", "icy_term").map(|dirs| dirs.config_dir().join("cache"))
    }

    pub fn set_dialing_directory_file(path: PathBuf) {
        let _ = PHONEBOOK_FILE_OVERRIDE.set(path);
    }

    pub fn new(system_name: impl Into<String>) -> Self {
        let time = Utc::now();
        unsafe {
            current_id = current_id.wrapping_add(1);
        }

        Self {
            web_source: None,
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
            proxy: None,
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
            ..Default::default()
        }
    }

    #[must_use]
    pub fn get_dialing_directory_file() -> Option<PathBuf> {
        if let Some(path) = PHONEBOOK_FILE_OVERRIDE.get() {
            if let Some(parent) = path.parent() {
                if fs::create_dir_all(parent).is_err() {
                    return None;
                }
            }
            if !path.exists() && fs::write(path, TEMPLATE).is_err() {
                return None;
            }
            if let Err(err) = secure_phonebook_file(path) {
                log::error!("Can't secure dialing directory {}: {err}", path.display());
                return None;
            }
            return Some(path.clone());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_term") {
            if !proj_dirs.config_dir().exists() && fs::create_dir_all(proj_dirs.config_dir()).is_err() {
                log::error!("Can't create configuration directory {}", proj_dirs.config_dir().display());
                return None;
            }
            let dialing_directory = proj_dirs.config_dir().join("phonebook.toml");
            if !dialing_directory.exists() {
                if let Err(err) = fs::write(&dialing_directory, TEMPLATE) {
                    log::error!("Can't create dialing_directory {} : {err}", dialing_directory.display());
                    return None;
                }
            }
            if let Err(err) = secure_phonebook_file(&dialing_directory) {
                log::error!("Can't secure dialing directory {}: {err}", dialing_directory.display());
                return None;
            }
            return Some(dialing_directory);
        }
        None
    }

    #[must_use]
    pub fn get_cache_directory(&self) -> Option<PathBuf> {
        if let Some(mut cache_directory) = Self::cache_root() {
            if !cache_directory.exists() && fs::create_dir_all(&cache_directory).is_err() {
                log::error!("Can't create cache directory {}", cache_directory.display());
                return None;
            }
            cache_directory.push(Self::cache_key(&self.address));
            if !cache_directory.exists() && fs::create_dir_all(&cache_directory).is_err() {
                log::error!("Can't create cache directory {}", cache_directory.display());
                return None;
            }
            Some(cache_directory)
        } else {
            None
        }
    }

    #[must_use]
    pub fn get_rip_cache(&self) -> Option<PathBuf> {
        if let Some(mut cache_directory) = self.get_cache_directory() {
            cache_directory = cache_directory.join("rip");
            if !cache_directory.exists() && fs::create_dir_all(&cache_directory).is_err() {
                log::error!("Can't create cache directory {}", cache_directory.display());
                return None;
            }
            Some(cache_directory)
        } else {
            None
        }
    }

    pub(crate) fn get_screen_mode(&self) -> ScreenMode {
        normalize_screen_mode(self.terminal_type, self.screen_mode)
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
        if let Err(err) = std::thread::Builder::new().name("file_watcher_thread".to_string()).spawn(move || loop {
            if let Some(path) = dialing_directory.parent() {
                if watch(path).is_err() {
                    return;
                }
            }
        }) {
            log::error!("Error starting file watcher thread: {err}");
        }
    }
}

pub static vga_regex: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| Regex::new("vga\\((\\d+),\\s*(\\d+)\\)").unwrap());

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;

    #[test]
    fn lf_expand_defaults_to_enabled() {
        // A bare LF must return to column 1, as it did before the option existed.
        assert!(Address::default().lf_expand());
        assert!(Address::new("test").lf_expand());
    }

    #[test]
    fn lf_expand_survives_a_save_load_round_trip() {
        let mut address = Address::new("test");
        assert!(address.lf_expand());

        // Entries written before the option existed carry no key at all.
        let legacy = toml::to_string(&address).unwrap();
        assert!(!legacy.contains("lf_expand"), "the default must not be written out");
        let restored: Address = toml::from_str(&legacy).unwrap();
        assert!(restored.lf_expand(), "a phonebook without the key must keep LF expansion");

        address.set_lf_expand(false);
        let stored = toml::to_string(&address).unwrap();
        let restored: Address = toml::from_str(&stored).unwrap();
        assert!(!restored.lf_expand(), "an explicit opt-out must survive a round trip");
    }

    #[test]
    fn ssh_key_settings_do_not_persist_the_passphrase() {
        let mut address = Address::new("ssh.example");
        address.ssh_authentication = SshAuthenticationMode::PrivateKey;
        address.ssh_private_key = "/home/user/.ssh/id_ed25519".to_string();
        address.ssh_key_passphrase = "do-not-store".to_string();

        let stored = toml::to_string(&address).unwrap();
        assert!(stored.contains("ssh_authentication = \"private_key\""));
        assert!(stored.contains("ssh_private_key = \"/home/user/.ssh/id_ed25519\""));
        assert!(!stored.contains("do-not-store"));

        let restored: Address = toml::from_str(&stored).unwrap();
        assert_eq!(restored.ssh_authentication, SshAuthenticationMode::PrivateKey);
        assert_eq!(restored.ssh_private_key, "/home/user/.ssh/id_ed25519");
        assert!(restored.ssh_key_passphrase.is_empty());
    }

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

    #[cfg(unix)]
    #[test]
    fn phonebook_permissions_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!("icy_term_phonebook_permissions_{}", std::process::id()));
        fs::write(&path, TEMPLATE).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        secure_phonebook_file(&path).unwrap();

        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn prune_orphaned_cache_dirs_keeps_active_and_unrelated_dirs() {
        let root = std::env::temp_dir().join(format!("icy_term_cache_prune_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let active = root.join(Address::cache_key("bbs.example:23"));
        let orphan = root.join("old_bbs_23");
        let unrelated = root.join("other");
        fs::create_dir_all(active.join("rip")).unwrap();
        fs::create_dir_all(orphan.join("rip")).unwrap();
        fs::create_dir_all(&unrelated).unwrap();

        let mut book = AddressBook::new();
        book.addresses[0].address = "bbs.example:23".to_string();
        book.prune_orphaned_cache_dirs(&root).unwrap();

        assert!(active.is_dir());
        assert!(!orphan.exists());
        assert!(unrelated.is_dir());
        fs::remove_dir_all(root).unwrap();
    }
}

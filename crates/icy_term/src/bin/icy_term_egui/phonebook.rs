use std::{fs, path::PathBuf};

use icy_term::{Address, AddressBook, ConnectionInformation};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum SortOrder {
    #[default]
    Name,
    MostCalled,
    LastCalled,
}

pub struct Phonebook {
    pub book: AddressBook,
    path: PathBuf,
    baseline: Vec<u8>,
    pub selected: Option<usize>,
    pub draft: Option<Address>,
    pub query: String,
    pub favorites_only: bool,
    pub sort: SortOrder,
    pub source: Option<String>,
}

impl Phonebook {
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn load(path: PathBuf) -> Result<Self, String> {
        let baseline = fs::read(&path).map_err(|_| "Cannot read the phonebook file".to_string())?;
        let book = AddressBook::load_from_file(&path).map_err(|_| "Cannot load the phonebook. The file has not been changed.".to_string())?;
        if fs::read(&path).map_err(|error| error.to_string())? != baseline {
            return Err("The phonebook changed while loading. Reload and try again.".into());
        }
        let mut result = Self {
            book,
            path,
            baseline,
            selected: None,
            draft: None,
            query: String::new(),
            favorites_only: false,
            sort: SortOrder::Name,
            source: None,
        };
        result.selected = result.filtered().first().copied();
        Ok(result)
    }

    pub fn filtered(&self) -> Vec<usize> {
        let query = self.query.trim().to_lowercase();
        let mut indices: Vec<_> = self
            .book
            .addresses
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                !(entry.system_name.is_empty() && entry.address.is_empty())
                    && (!self.favorites_only || entry.is_favored)
                    && self.source.as_ref().is_none_or(|source| entry.web_source.as_deref().unwrap_or("") == source)
                    && (query.is_empty()
                        || [&entry.system_name, &display_address(entry), &entry.comment]
                            .iter()
                            .any(|text| text.to_lowercase().contains(&query)))
            })
            .map(|(index, _)| index)
            .collect();
        indices.sort_by(|left, right| {
            let left = &self.book.addresses[*left];
            let right = &self.book.addresses[*right];
            right
                .is_favored
                .cmp(&left.is_favored)
                .then_with(|| match self.sort {
                    SortOrder::Name => std::cmp::Ordering::Equal,
                    SortOrder::MostCalled => right.number_of_calls.cmp(&left.number_of_calls),
                    SortOrder::LastCalled => right.last_call.cmp(&left.last_call),
                })
                .then_with(|| left.system_name.to_lowercase().cmp(&right.system_name.to_lowercase()))
        });
        indices
    }

    pub fn selection(&self) -> Option<&Address> {
        self.selected.and_then(|index| self.book.addresses.get(index))
    }

    /// Total and favorite counts, using the same placeholder exclusion as [`Self::filtered`].
    pub fn totals(&self) -> (usize, usize) {
        self.book
            .addresses
            .iter()
            .filter(|entry| !(entry.system_name.is_empty() && entry.address.is_empty()))
            .fold((0, 0), |(total, favorites), entry| (total + 1, favorites + usize::from(entry.is_favored)))
    }

    pub fn begin_edit(&mut self) {
        self.draft = self.selection().filter(|entry| entry.web_source.is_none()).cloned();
    }

    pub fn begin_new(&mut self, duplicate: bool) {
        let mut entry = if duplicate {
            self.selection().cloned().unwrap_or_default()
        } else {
            Address::default()
        };
        entry.system_name = if duplicate { format!("{} Copy", entry.system_name) } else { String::new() };
        entry.is_favored = false;
        entry.web_source = None;
        if !duplicate {
            entry.ice_mode = true;
            entry.mouse_reporting_enabled = true;
        }
        entry.created = chrono::Utc::now();
        entry.updated = entry.created;
        entry.last_call = None;
        entry.number_of_calls = 0;
        entry.overall_duration = chrono::Duration::zero();
        entry.last_call_duration = chrono::Duration::zero();
        entry.uploaded_bytes = 0;
        entry.downloaded_bytes = 0;
        self.selected = None;
        self.draft = Some(entry);
    }

    fn commit(&mut self, mut candidate: AddressBook) -> Result<(), String> {
        if self.book.write_lock {
            return Err("This phonebook uses a newer format and is read-only".into());
        }
        let current = fs::read(&self.path).map_err(|error| error.to_string())?;
        if current != self.baseline {
            return Err("The phonebook was changed by another application. Discard the draft and reload before saving.".into());
        }
        candidate.store_to_file(&self.path).map_err(|error| error.to_string())?;
        self.baseline = fs::read(&self.path).map_err(|error| error.to_string())?;
        self.book = candidate;
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), String> {
        let mut entry = self.draft.clone().ok_or("No entry is being edited")?;
        validate_entry(&entry)?;
        entry.updated = chrono::Utc::now();
        let mut candidate = self.book.clone();
        let selected = if let Some(index) = self.selected {
            if candidate.addresses[index].web_source.is_some() {
                return Err("Web entries are read-only. Duplicate to create a local entry.".into());
            }
            candidate.addresses[index] = entry;
            index
        } else {
            let index = candidate.addresses.len();
            candidate.addresses.push(entry);
            index
        };
        self.commit(candidate)?;
        self.selected = Some(selected);
        self.draft = None;
        if !self.filtered().contains(&selected) {
            self.query.clear();
            self.favorites_only = false;
            self.source = None;
        }
        Ok(())
    }

    pub fn delete(&mut self) -> Result<(), String> {
        let index = self.selected.ok_or("No entry selected")?;
        if self.book.addresses[index].web_source.is_some() {
            return Err("Web entries are read-only".into());
        }
        let mut candidate = self.book.clone();
        candidate.addresses.remove(index);
        self.commit(candidate)?;
        self.selected = self.filtered().first().copied();
        self.draft = None;
        Ok(())
    }

    pub fn toggle_favorite(&mut self, index: usize) -> Result<(), String> {
        if self.book.addresses[index].web_source.is_some() {
            return Err("Duplicate this web entry to save a favorite".into());
        }
        let mut candidate = self.book.clone();
        candidate.addresses[index].is_favored = !candidate.addresses[index].is_favored;
        self.commit(candidate)
    }

    pub fn record_call(&mut self, entry: &Address) -> Result<(), String> {
        if entry.web_source.is_some() {
            return Ok(());
        }
        let Some(index) = self
            .book
            .addresses
            .iter()
            .position(|saved| saved.web_source.is_none() && saved.system_name == entry.system_name && saved.address == entry.address)
        else {
            return Ok(());
        };
        let mut candidate = self.book.clone();
        candidate.addresses[index].number_of_calls = candidate.addresses[index].number_of_calls.saturating_add(1);
        candidate.addresses[index].last_call = Some(chrono::Utc::now());
        self.commit(candidate)?;
        if self.selected == Some(index) {
            if let Some(draft) = &mut self.draft {
                draft.number_of_calls = self.book.addresses[index].number_of_calls;
                draft.last_call = self.book.addresses[index].last_call;
            }
        }
        Ok(())
    }

    pub fn record_duration(&mut self, entry: &Address, elapsed: std::time::Duration) -> Result<(), String> {
        if entry.web_source.is_some() {
            return Ok(());
        }
        let Some(index) = self
            .book
            .addresses
            .iter()
            .position(|saved| saved.web_source.is_none() && saved.system_name == entry.system_name && saved.address == entry.address)
        else {
            return Ok(());
        };
        let duration = chrono::Duration::from_std(elapsed).map_err(|error| error.to_string())?;
        let mut candidate = self.book.clone();
        candidate.addresses[index].last_call_duration = duration;
        candidate.addresses[index].overall_duration += duration;
        self.commit(candidate)?;
        if self.selected == Some(index) {
            if let Some(draft) = &mut self.draft {
                draft.last_call_duration = self.book.addresses[index].last_call_duration;
                draft.overall_duration = self.book.addresses[index].overall_duration;
            }
        }
        Ok(())
    }
}

pub fn validate_entry(entry: &Address) -> Result<(), String> {
    if entry.system_name.trim().is_empty() {
        return Err("A system name is required".into());
    }
    if entry.address.trim().is_empty() {
        return Err("An address is required".into());
    }
    if entry.protocol != icy_net::ConnectionType::Modem {
        let info = ConnectionInformation::parse(&entry.address).map_err(|_| "Invalid address or port".to_string())?;
        if entry.address.contains("://") && info.protocol.is_none() {
            return Err("Unknown address protocol".into());
        }
        if info.host.is_empty() || info.host.chars().any(char::is_whitespace) {
            return Err("Invalid host name".into());
        }
        if info.protocol.is_some_and(|protocol| protocol != entry.protocol) {
            return Err("The address URL and selected protocol do not match".into());
        }
    }
    icy_term::auto_login::AutoLoginParser::parse(&entry.auto_login).map_err(|_| "Invalid auto-login expression".to_string())?;
    let size = entry.get_screen_mode().window_size();
    if !(1..=500).contains(&size.width) || !(1..=200).contains(&size.height) {
        return Err("Terminal size must be between 1x1 and 500x200".into());
    }
    if let Some(proxy) = &entry.proxy {
        if proxy.host.trim().is_empty() || proxy.host.chars().any(char::is_whitespace) || proxy.port == 0 {
            return Err("A valid proxy host and port are required".into());
        }
    }
    if entry.custom_palette.as_ref().is_some_and(|palette| palette.len() != 16) {
        return Err("The custom palette must contain 16 colors".into());
    }
    Ok(())
}

pub fn display_address(entry: &Address) -> String {
    if entry.protocol == icy_net::ConnectionType::Modem {
        return entry.address.clone();
    }
    ConnectionInformation::parse(&entry.address)
        .map(|mut info| {
            if info.protocol.is_none() {
                info.protocol = Some(entry.protocol);
            }
            info.endpoint()
        })
        .unwrap_or_else(|_| "Invalid address".into())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub struct Fixture(PathBuf);
    impl Fixture {
        pub fn new() -> Self {
            let root = std::env::temp_dir().join(format!("icy_egui_phonebook_{}", fastrand::u64(..)));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        pub fn load(&self) -> Phonebook {
            let path = self.0.join("phonebook.toml");
            let mut book = AddressBook::default();
            book.addresses.clear();
            book.store_to_file(&path).unwrap();
            Phonebook::load(path).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    pub fn add(book: &mut Phonebook, name: &str) {
        book.begin_new(false);
        let entry = book.draft.as_mut().unwrap();
        entry.system_name = name.into();
        entry.address = "localhost:2323".into();
        entry.password = "test-secret".into();
        entry.comment = "Retro graphics".into();
        entry.font_name = Some("IBM VGA".into());
        book.save().unwrap();
    }

    #[test]
    fn call_duration_survives_saving_an_open_draft() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Session");
        let entry = book.selection().unwrap().clone();
        book.record_call(&entry).unwrap();
        book.begin_edit();
        book.record_duration(&entry, std::time::Duration::from_secs(75)).unwrap();
        book.save().unwrap();
        let loaded = Phonebook::load(book.path.clone()).unwrap();
        assert_eq!(loaded.selection().unwrap().last_call_duration.num_seconds(), 75);
        assert_eq!(loaded.selection().unwrap().overall_duration.num_seconds(), 75);
    }

    #[test]
    fn remote_entries_require_a_local_copy() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        let mut remote = Address::new("Community BBS");
        remote.address = "localhost:2323".into();
        remote.web_source = Some("Community".into());
        book.book.addresses.push(remote);
        book.selected = Some(0);
        book.source = Some("Community".into());
        assert_eq!(book.filtered(), [0]);
        book.begin_edit();
        assert!(book.draft.is_none());
        assert!(book.delete().is_err());
        assert!(book.toggle_favorite(0).is_err());
        book.begin_new(true);
        book.save().unwrap();
        assert!(book.selection().unwrap().web_source.is_none());
        assert!(book.source.is_none());
        let loaded = Phonebook::load(book.path.clone()).unwrap();
        assert_eq!(loaded.book.addresses.len(), 1);
        assert!(loaded.selection().unwrap().web_source.is_none());
    }

    #[test]
    fn edits_are_explicit_and_preserve_advanced_fields_and_backup() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        let original = fs::read(&book.path).unwrap();
        add(&mut book, "Example");
        assert_eq!(fs::read(book.path.with_extension("bak")).unwrap(), original);
        book.begin_edit();
        book.draft.as_mut().unwrap().system_name = "Discarded".into();
        book.draft = None;
        assert_eq!(book.selection().unwrap().system_name, "Example");
        book.begin_edit();
        book.draft.as_mut().unwrap().system_name = "Saved".into();
        book.save().unwrap();
        let restored = Phonebook::load(book.path.clone()).unwrap();
        let entry = restored.selection().unwrap();
        assert_eq!(entry.system_name, "Saved");
        assert_eq!(entry.password, "test-secret");
        assert_eq!(entry.font_name.as_deref(), Some("IBM VGA"));
        assert_eq!(fs::read(book.path.with_extension("bak")).unwrap(), original);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&book.path).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn search_sort_duplicate_favorite_and_delete_use_original_indices() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Zulu");
        add(&mut book, "Alpha");
        assert_eq!(book.filtered(), [1, 0]);
        book.toggle_favorite(0).unwrap();
        assert_eq!(book.filtered(), [0, 1]);
        book.query = "GRAPHICS".into();
        book.favorites_only = true;
        assert_eq!(book.filtered(), [0]);
        book.selected = Some(0);
        book.begin_new(true);
        assert!(!book.draft.as_ref().unwrap().is_favored);
        book.save().unwrap();
        assert_eq!(book.selected, Some(2));
        assert_eq!(book.selection().unwrap().password, "test-secret");
        book.delete().unwrap();
        assert_eq!(book.book.addresses.len(), 2);
    }

    #[test]
    fn external_changes_and_invalid_drafts_never_overwrite_the_file() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        let original = fs::read(&book.path).unwrap();
        book.begin_new(false);
        assert!(book.save().is_err());
        assert_eq!(fs::read(&book.path).unwrap(), original);
        book.draft = None;
        add(&mut book, "Example");
        let saved = fs::read(&book.path).unwrap();
        fs::write(book.path.with_extension("new"), b"another writer").unwrap();
        book.begin_edit();
        assert!(book.save().is_err());
        assert_eq!(fs::read(&book.path).unwrap(), saved);
        assert_eq!(fs::read(book.path.with_extension("new")).unwrap(), b"another writer");
        fs::remove_file(book.path.with_extension("new")).unwrap();
        let external = fs::read_to_string(&book.path).unwrap() + "\n# external change\n";
        fs::write(&book.path, &external).unwrap();
        book.begin_edit();
        assert!(book.save().is_err());
        assert!(book.draft.is_some());
        assert_eq!(fs::read_to_string(&book.path).unwrap(), external);
        fs::write(&book.path, "version = '99.0.0'\naddresses = []\n").unwrap();
        let mut future = Phonebook::load(book.path.clone()).unwrap();
        assert!(future.book.write_lock);
        future.begin_new(false);
        let entry = future.draft.as_mut().unwrap();
        entry.system_name = "Future".into();
        entry.address = "localhost".into();
        assert!(future.save().is_err());
    }

    #[test]
    fn list_and_search_do_not_expose_url_passwords() {
        let mut entry = Address::default();
        entry.address = "telnet://name:secret@localhost:2323".into();
        assert_eq!(display_address(&entry), "localhost:2323");
    }

    #[test]
    fn successful_call_updates_statistics_without_losing_an_open_draft() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Example");
        let entry = book.selection().unwrap().clone();
        book.begin_edit();
        book.draft.as_mut().unwrap().comment = "In progress".into();
        book.record_call(&entry).unwrap();
        assert_eq!(book.selection().unwrap().number_of_calls, 1);
        assert!(book.selection().unwrap().last_call.is_some());
        book.save().unwrap();
        assert_eq!(book.selection().unwrap().number_of_calls, 1);
        assert_eq!(book.selection().unwrap().comment, "In progress");
    }
}

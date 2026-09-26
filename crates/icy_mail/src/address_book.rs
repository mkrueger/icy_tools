//! Address book of people to write to, in MultiMail's file format so books can be shared:
//! each entry is a name line and an address line followed by a blank line, and Internet
//! addresses are marked with a leading `I`.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{drafts::atomic_write, editor};

const FILE_NAME: &str = "addressbook.txt";
/// Longest name or address kept; QWK headers themselves hold 25 characters.
pub const FIELD_LENGTH: usize = 72;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contact {
    pub name: String,
    /// Netmail (`1:234/5`) or Internet address, empty when only the name is known.
    pub address: String,
}

impl Contact {
    pub fn new(name: &str, address: &str) -> Self {
        Self {
            name: clean(name),
            address: clean(address),
        }
    }

    pub fn is_internet(&self) -> bool {
        self.address.contains('@')
    }

    /// Whether the name or the address contains `filter`, ignoring case.
    pub fn matches(&self, filter: &str) -> bool {
        let filter = filter.trim().to_lowercase();
        filter.is_empty() || self.name.to_lowercase().contains(&filter) || self.address.to_lowercase().contains(&filter)
    }
}

#[derive(Clone, Debug)]
pub struct AddressBook {
    path: PathBuf,
    /// Sorted by name.
    pub contacts: Vec<Contact>,
}

impl AddressBook {
    pub fn open() -> crate::Res<Self> {
        Self::open_in(&crate::state::data_directory()?)
    }

    pub fn open_in(directory: &Path) -> crate::Res<Self> {
        let path = directory.join(FILE_NAME);
        let contacts = read(&path)?;
        Ok(Self { path, contacts })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Picks up changes made by another window.
    pub fn reload(&mut self) -> crate::Res<()> {
        self.contacts = read(&self.path)?;
        Ok(())
    }

    pub fn save(&self) -> crate::Res<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut content = String::new();
        for contact in &self.contacts {
            let marker = if contact.is_internet() { "I" } else { "" };
            content.push_str(&format!("{}\n{marker}{}\n\n", contact.name, contact.address));
        }
        atomic_write(&self.path, |file| {
            file.write_all(content.as_bytes())?;
            Ok(())
        })?;
        Ok(())
    }

    /// Index of the entry for `name`, ignoring case.
    pub fn find(&self, name: &str) -> Option<usize> {
        let name = clean(name).to_lowercase();
        self.contacts.iter().position(|contact| contact.name.to_lowercase() == name)
    }

    /// Adds `contact`, or updates the address of the entry with the same name; returns its index.
    pub fn add(&mut self, contact: Contact) -> Option<usize> {
        let contact = Contact::new(&contact.name, &contact.address);
        if contact.name.is_empty() {
            return None;
        }
        if let Some(index) = self.find(&contact.name) {
            if !contact.address.is_empty() {
                self.contacts[index].address = contact.address;
            }
            return Some(index);
        }
        self.contacts.push(contact.clone());
        sort(&mut self.contacts);
        self.contacts.iter().position(|existing| *existing == contact)
    }

    /// Replaces the entry at `index`; returns its new index, `None` when the name is empty or
    /// belongs to another entry.
    pub fn update(&mut self, index: usize, contact: Contact) -> Option<usize> {
        let contact = Contact::new(&contact.name, &contact.address);
        if contact.name.is_empty() || index >= self.contacts.len() || self.find(&contact.name).is_some_and(|other| other != index) {
            return None;
        }
        self.contacts[index] = contact.clone();
        sort(&mut self.contacts);
        self.contacts.iter().position(|existing| *existing == contact)
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.contacts.len() {
            self.contacts.remove(index);
        }
    }
}

fn sort(contacts: &mut [Contact]) {
    contacts.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name)));
}

fn read(path: &Path) -> crate::Res<Vec<Contact>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => error.into_bytes().iter().map(|&byte| editor::cp437_char(byte)).collect(),
    };
    let mut contacts: Vec<Contact> = Vec::new();
    let mut lines = text.lines().map(str::trim);
    while let Some(name) = lines.next() {
        if name.is_empty() {
            continue;
        }
        let address = lines.next().unwrap_or_default();
        let address = address.strip_prefix('I').filter(|rest| rest.contains('@')).unwrap_or(address);
        let contact = Contact::new(name, address);
        if !contact.name.is_empty() && !contacts.iter().any(|existing| existing.name.to_lowercase() == contact.name.to_lowercase()) {
            contacts.push(contact);
        }
    }
    sort(&mut contacts);
    Ok(contacts)
}

fn clean(text: &str) -> String {
    let text: String = text.chars().map(|ch| if ch.is_control() { ' ' } else { ch }).collect();
    text.trim().chars().take(FIELD_LENGTH).collect::<String>().trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_multimail_address_books() {
        let dir = crate::qwk::tests::TempDir::new();
        fs::write(
            dir.path().join(FILE_NAME),
            "Zed Zero\n2:240/1\n\n\nalice\nIalice@example.com\n\nBob Only\n\nALICE\n1:1/1\n",
        )
        .unwrap();
        let mut book = AddressBook::open_in(dir.path()).unwrap();
        assert_eq!(
            book.contacts,
            [
                Contact::new("alice", "alice@example.com"),
                Contact::new("Bob Only", ""),
                Contact::new("Zed Zero", "2:240/1")
            ]
        );
        assert!(book.contacts[0].is_internet());
        assert_eq!(book.add(Contact::new("  carol ", "")), Some(2));
        assert_eq!(book.add(Contact::new("BOB ONLY", "1:2/3")), Some(1));
        assert_eq!(book.contacts[1], Contact::new("Bob Only", "1:2/3"));
        assert_eq!(book.add(Contact::new("", "x")), None);
        assert_eq!(book.update(3, Contact::new("Alice", "")), None);
        assert_eq!(book.update(3, Contact::new("Aaron", "")), Some(0));
        book.remove(1);
        book.save().unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join(FILE_NAME)).unwrap(),
            "Aaron\n\n\nBob Only\n1:2/3\n\ncarol\n\n\n"
        );
        book.reload().unwrap();
        assert_eq!(book.contacts.len(), 3);
        assert!(book.contacts[0].matches("aar"));
        assert!(book.contacts[1].matches("1:2"));
        assert!(!book.contacts[1].matches("carol"));
    }
}

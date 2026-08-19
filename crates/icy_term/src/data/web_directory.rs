use std::{fs, io::Read, path::PathBuf, time::Duration};

use super::{Address, AddressBook, WebDirectorySource};

const MAX_DIRECTORY_BYTES: u64 = 2 * 1024 * 1024;

fn cache_directory() -> Option<PathBuf> {
    let directory = directories::ProjectDirs::from("com", "GitHub", "icy_term")?.cache_dir().join("web_directories");
    fs::create_dir_all(&directory).ok()?;
    Some(directory)
}

fn cache_file(source: &WebDirectorySource) -> Option<PathBuf> {
    let name: String = source
        .name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') { ch } else { '_' })
        .take(80)
        .collect();
    if name.is_empty() {
        return None;
    }
    Some(cache_directory()?.join(format!("{name}.toml")))
}

fn parse_directory(input: &str, source_name: &str) -> Result<Vec<Address>, toml::de::Error> {
    let mut book: AddressBook = toml::from_str(input)?;
    for address in &mut book.addresses {
        address.web_source = Some(source_name.to_string());
        address.user_name.clear();
        address.password.clear();
        address.auto_login.clear();
    }
    Ok(book.addresses)
}

fn download(source: &WebDirectorySource) -> Option<String> {
    if !source.url.starts_with("https://") {
        log::warn!("Ignoring non-HTTPS web directory: {}", source.url);
        return None;
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent(concat!("icy_term/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;
    let response = client.get(&source.url).send().ok()?.error_for_status().ok()?;
    if response.content_length().is_some_and(|length| length > MAX_DIRECTORY_BYTES) {
        log::warn!("Web directory '{}' exceeds the size limit", source.name);
        return None;
    }
    let mut bytes = Vec::new();
    response.take(MAX_DIRECTORY_BYTES + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_DIRECTORY_BYTES {
        log::warn!("Web directory '{}' exceeds the size limit", source.name);
        return None;
    }
    String::from_utf8(bytes).ok()
}

pub fn merge_web_directories(book: &mut AddressBook, sources: &[WebDirectorySource]) {
    for source in sources.iter().filter(|source| source.enabled) {
        let cache = cache_file(source);
        let downloaded = download(source);
        let content = downloaded
            .as_deref()
            .and_then(|input| parse_directory(input, &source.name).ok().map(|addresses| (input.to_string(), addresses)))
            .or_else(|| {
                let input = fs::read_to_string(cache.as_ref()?).ok()?;
                let addresses = parse_directory(&input, &source.name).ok()?;
                Some((input, addresses))
            });

        let Some((input, addresses)) = content else {
            log::warn!("Unable to load web directory '{}'", source.name);
            continue;
        };
        if downloaded.is_some() {
            if let Some(cache) = &cache {
                if let Err(error) = fs::write(cache, input) {
                    log::warn!("Unable to cache web directory '{}': {error}", source.name);
                }
            }
        }
        book.addresses.extend(addresses);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_directory;

    #[test]
    fn remote_entries_are_read_only_and_strip_credentials() {
        let input = r#"
version = "1.1.0"

[[addresses]]
system_name = "Remote BBS"
address = "bbs.example.org:23"
user_name = "should-not-import"
password = "secret"
auto_login = "login script"
"#;

        let addresses = parse_directory(input, "Community").unwrap();
        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0].web_source.as_deref(), Some("Community"));
        assert!(addresses[0].user_name.is_empty());
        assert!(addresses[0].password.is_empty());
        assert!(addresses[0].auto_login.is_empty());
    }
}

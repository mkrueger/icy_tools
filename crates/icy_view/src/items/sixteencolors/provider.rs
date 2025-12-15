use icy_engine::formats::FileFormat;

use crate::items::{ArchiveContainer, Item, ItemError, sort_folder};

use super::{API_PATH, SixteenColorsFile, SixteenColorsPack, SixteenColorsYear, cache::fetch_json_async, get_cache};

/// Provider for 16colors.rs web browsing
/// Uses the global cache for all API responses
pub struct SixteenColorsProvider {}

impl SixteenColorsProvider {
    pub fn new() -> Self {
        Self {}
    }

    /// Parse path into components
    /// "" or "/" -> root (years)
    /// "2024" or "/2024" -> year (packs)
    /// "2024/pack_name" or "/2024/pack_name" -> pack (files)
    fn parse_path(path: &str) -> (Option<u64>, Option<String>) {
        let trimmed = path.trim_matches('/');
        if trimmed.is_empty() {
            return (None, None);
        }

        let parts: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
        match parts.len() {
            0 => (None, None),
            1 => {
                // Just year
                let year = parts[0].parse().ok();
                (year, None)
            }
            2 => {
                // Year and pack
                let year = parts[0].parse().ok();
                let pack = Some(parts[1].to_string());
                (year, pack)
            }
            _ => {
                // More than 2 parts - treat as year/pack (ignore extra parts for now)
                let year = parts[0].parse().ok();
                let pack = Some(parts[1].to_string());
                (year, pack)
            }
        }
    }

    /// Fetch years from API (uses global cache)
    async fn fetch_years() -> Result<Vec<YearInfo>, ItemError> {
        let url = format!("{}/year?rows=0", API_PATH);
        let cache = get_cache();
        let json = fetch_json_async(&cache, &url).await?;

        let mut years = Vec::new();
        if let Some(year_list) = json.as_array() {
            for year_data in year_list {
                let year = year_data["year"].as_u64().unwrap_or(0);
                let packs_count = year_data["packs"].as_u64().unwrap_or(0);
                years.push(YearInfo { year, packs_count });
            }
            years.reverse(); // Most recent first
        }
        Ok(years)
    }

    /// Fetch packs for a year from API (uses global cache)
    async fn fetch_packs(year: u64) -> Result<Vec<PackInfo>, ItemError> {
        let url = format!("{}/year/{}?rows=0", API_PATH, year);
        let cache = get_cache();
        let json = fetch_json_async(&cache, &url).await?;

        let mut packs = Vec::new();
        if let Some(pack_list) = json.as_array() {
            for pack_data in pack_list {
                let filename = pack_data["filename"].as_str().unwrap_or_default().to_string();
                let month = pack_data["month"].as_u64().unwrap_or(0);
                let year = pack_data["year"].as_u64().unwrap_or(0);
                let name = pack_data["name"].as_str().unwrap_or_default().to_string();
                packs.push(PackInfo { filename, month, year, name });
            }
        }
        Ok(packs)
    }

    /// Fetch files for a pack from API (uses global cache)
    async fn fetch_files(pack_name: &str) -> Result<Vec<FileInfo>, ItemError> {
        let url = format!("{}/pack/{}?rows=0", API_PATH, pack_name);
        let cache = get_cache();
        let json = fetch_json_async(&cache, &url).await?;

        let mut files = Vec::new();
        if let Some(file_list) = json["files"].as_array() {
            for file_data in file_list {
                let filename = file_data["filename"].as_str().unwrap_or_default().to_string();
                let location = file_data["file_location"].as_str().unwrap_or_default().to_string();
                let uri = file_data["uri"].as_str().unwrap_or_default().to_string();
                let thumbnail = file_data["thumbnail"].as_str().unwrap_or_default().to_string();
                files.push(FileInfo {
                    filename,
                    location,
                    uri,
                    thumbnail,
                });
            }
        }
        Ok(files)
    }
}

#[derive(Clone)]
struct YearInfo {
    year: u64,
    packs_count: u64,
}

#[derive(Clone)]
struct PackInfo {
    filename: String,
    month: u64,
    year: u64,
    name: String,
}

#[derive(Clone)]
struct FileInfo {
    filename: String,
    location: String,
    uri: String,
    thumbnail: String,
}

impl Default for SixteenColorsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SixteenColorsProvider {
    /// Validate a 16colors path against cached data
    /// Returns true if the path is valid or could be valid (not yet cached)
    /// Returns false only if we have cached data that proves the path is invalid
    pub fn validate_path(path: &str) -> bool {
        let (year, pack) = Self::parse_path(path);

        match (year, pack) {
            (None, None) => {
                // Root path is always valid
                true
            }
            (Some(year), None) => {
                // Check if year exists in cache
                let url = format!("{}/year?rows=0", API_PATH);
                let cache = get_cache();
                let cache_read = cache.read();

                if let Some(cached) = cache_read.api_responses.get(&url) {
                    if let Some(json) = cached {
                        // We have cached years - check if this year exists
                        if let Some(year_list) = json.as_array() {
                            return year_list.iter().any(|y| y["year"].as_u64() == Some(year));
                        }
                    }
                    return false; // Request failed previously
                }
                // Not cached yet - assume valid
                true
            }
            (Some(year), Some(ref pack_name)) => {
                // Check if pack exists in cache for this year
                let url = format!("{}/year/{}?rows=0", API_PATH, year);
                let cache = get_cache();
                let cache_read = cache.read();

                if let Some(cached) = cache_read.api_responses.get(&url) {
                    if let Some(json) = cached {
                        // We have cached packs - check if this pack exists
                        if let Some(pack_list) = json.as_array() {
                            return pack_list.iter().any(|p| p["name"].as_str() == Some(pack_name.as_str()));
                        }
                    }
                    return false; // Request failed previously
                }
                // Not cached yet - assume valid
                true
            }
            (None, Some(_)) => {
                // Pack without year is always invalid
                false
            }
        }
    }

    /// Get items at a given 16colors path
    pub async fn get_items(&self, path: &str) -> Result<Vec<Box<dyn Item>>, ItemError> {
        let (year, pack) = Self::parse_path(path);

        match (year, pack) {
            (None, None) => {
                // Root - show years
                let years = Self::fetch_years().await?;
                let items: Vec<Box<dyn Item>> = years
                    .iter()
                    .map(|y| Box::new(SixteenColorsYear::new(y.year, y.packs_count)) as Box<dyn Item>)
                    .collect();
                Ok(items)
            }
            (Some(year), None) => {
                // Year - show packs
                let packs = Self::fetch_packs(year).await?;
                let mut items: Vec<Box<dyn Item>> = packs
                    .into_iter()
                    .map(|p| Box::new(SixteenColorsPack::new(p.filename, p.month, p.year, p.name)) as Box<dyn Item>)
                    .collect();
                sort_folder(&mut items);
                Ok(items)
            }
            (Some(_year), Some(pack_name)) => {
                // Pack - show files
                let files = Self::fetch_files(&pack_name).await?;
                let mut items: Vec<Box<dyn Item>> = files
                    .into_iter()
                    .map(|f| {
                        let file = SixteenColorsFile::new(f.filename, f.location, f.uri, f.thumbnail);
                        if let Some(FileFormat::Archive(format)) = FileFormat::from_extension(&file.filename) {
                            Box::new(ArchiveContainer::new(Box::new(file), format)) as Box<dyn Item>
                        } else {
                            Box::new(file) as Box<dyn Item>
                        }
                    })
                    .collect();
                sort_folder(&mut items);
                Ok(items)
            }
            (None, Some(_)) => {
                // Invalid path (pack without year)
                Err(ItemError::NotFound("Invalid path: pack without year".to_string()))
            }
        }
    }
}

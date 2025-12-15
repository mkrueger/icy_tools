/// Type of provider - only File (local filesystem + ZIP) or Web (16colors)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    /// Local filesystem (can also navigate into ZIP files)
    File,
    /// Web browsing (16colo.rs)
    Web,
}

/// Simple navigation point
///
/// Paths are absolute:
/// - File: `/home/user/folder` or `/home/user/archive.zip/folder/file.ans`
/// - Web: `16colo.rs/pack/name`
#[derive(Debug, Clone, PartialEq)]
pub struct NavPoint {
    /// Type of provider
    pub provider_type: ProviderType,
    /// Full path (filesystem path or web URL path)
    pub path: String,
    /// Currently selected item name (if any)
    pub selected_item: Option<String>,
}

impl NavPoint {
    /// Create a new file system NavPoint
    pub fn file(path: impl Into<String>) -> Self {
        Self {
            provider_type: ProviderType::File,
            path: path.into(),
            selected_item: None,
        }
    }

    /// Create a new web NavPoint
    pub fn web(path: impl Into<String>) -> Self {
        Self {
            provider_type: ProviderType::Web,
            path: path.into(),
            selected_item: None,
        }
    }

    /// Navigate to a new path
    pub fn navigate_to(&mut self, path: String) {
        self.path = path;
        self.selected_item = None;
    }

    /// Navigate up to parent
    /// Returns true if navigation happened, false if at root
    pub fn navigate_up(&mut self) -> bool {
        match self.provider_type {
            ProviderType::File => {
                // For filesystem, just go to parent directory
                // This works for both regular paths and ZIP paths
                if let Some(parent) = std::path::Path::new(&self.path).parent() {
                    let parent_str = parent.to_string_lossy().replace('\\', "/");
                    if !parent_str.is_empty() && parent_str != self.path {
                        self.path = parent_str;
                        self.selected_item = None;
                        return true;
                    }
                }
                false
            }
            ProviderType::Web => {
                // For web, go up in URL path
                if self.path.is_empty() {
                    return false;
                }
                let trimmed = self.path.trim_end_matches('/');
                if let Some(pos) = trimmed.rfind('/') {
                    self.path = trimmed[..pos].to_string();
                    self.selected_item = None;
                    true
                } else {
                    self.path.clear();
                    self.selected_item = None;
                    true
                }
            }
        }
    }

    /// Check if we can navigate up
    pub fn can_navigate_up(&self) -> bool {
        match self.provider_type {
            ProviderType::File => std::path::Path::new(&self.path).parent().map(|p| !p.as_os_str().is_empty()).unwrap_or(false),
            ProviderType::Web => !self.path.is_empty(),
        }
    }

    /// Get display path for the navigation bar (just the path, no icons)
    pub fn display_path(&self) -> String {
        match self.provider_type {
            ProviderType::File => self.path.clone(),
            ProviderType::Web => {
                // Show web paths with leading / (e.g., "/2011" or "/2011/pack")
                if self.path.is_empty() { "/".to_string() } else { format!("/{}", self.path) }
            }
        }
    }

    /// Check if this is a web provider
    pub fn is_web(&self) -> bool {
        self.provider_type == ProviderType::Web
    }
}

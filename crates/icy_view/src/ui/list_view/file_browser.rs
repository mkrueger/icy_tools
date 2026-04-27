use std::{env, path::PathBuf};

use directories::UserDirs;
use icy_engine::formats::FileFormat;
use icy_ui::{
    keyboard::{key::Named, Key},
    widget::container,
    Element, Event, Length, Task,
};

use super::file_list_view::{FileListView, FileListViewMessage};
use super::sauce_loader::SharedSauceCache;
use crate::items::Item;
use crate::items::{get_items_at_path, is_directory, path_exists, sort_items, ItemError, NavPoint, ProviderType, SixteenColorsProvider};
use crate::sort_order::SortOrder;
use icy_engine_gui::{focus, list_focus_style};

/// Messages for the file browser
#[derive(Clone)]
pub enum FileBrowserMessage {
    /// Messages from the file list view
    ListView(FileListViewMessage),
    /// Navigate to parent folder
    ParentFolder,
    /// Refresh the current folder
    Refresh,
    /// Web folder items finished loading
    WebItemsLoaded {
        path: String,
        result: Result<Vec<Box<dyn Item>>, ItemError>,
    },
    /// Filter text changed
    FilterChanged(String),
}

impl std::fmt::Debug for FileBrowserMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ListView(msg) => f.debug_tuple("ListView").field(msg).finish(),
            Self::ParentFolder => write!(f, "ParentFolder"),
            Self::Refresh => write!(f, "Refresh"),
            Self::WebItemsLoaded { path, result } => f
                .debug_struct("WebItemsLoaded")
                .field("path", path)
                .field("result", &result.as_ref().map(|items| items.len()))
                .finish(),
            Self::FilterChanged(filter) => f.debug_tuple("FilterChanged").field(filter).finish(),
        }
    }
}

/// File browser widget with simple path-based navigation
pub struct FileBrowser {
    /// Current navigation point
    nav_point: NavPoint,
    /// Files in current directory (all files)
    pub files: Vec<Box<dyn Item>>,
    /// Indices of visible files after filtering (into files vec)
    visible_indices: Vec<usize>,
    /// Filter text
    pub filter: String,
    /// The file list view with smooth scrolling
    pub list_view: FileListView,
    /// Current sort order
    sort_order: SortOrder,
    /// Shared SAUCE cache for displaying SAUCE info
    sauce_cache: Option<SharedSauceCache>,
    /// Whether a web folder is currently loading
    is_loading: bool,
    /// Item label to select after async web navigation completes
    pending_select_label: Option<String>,
}

impl FileBrowser {
    /// Creates a new FileBrowser.
    /// Returns (Self, Option<PathBuf>) where the second value is the file to select and preview
    /// (for Scenario 3: when initial_path points to a regular file)
    pub fn new(initial_path: Option<PathBuf>) -> (Self, Option<PathBuf>) {
        let (path, file_to_select) = if let Some(initial) = initial_path {
            let path_str = initial.to_string_lossy().to_string();
            if path_exists(&path_str) {
                if is_directory(&path_str) {
                    // Scenario 1 & 2: Directory or Archive - navigate to it
                    (initial, None)
                } else {
                    // It's a file - check if archive
                    if let Some(FileFormat::Archive(_)) = FileFormat::from_path(&initial) {
                        // Scenario 2: Archive - treat as directory
                        (initial, None)
                    } else {
                        // Scenario 3: Regular file - navigate to parent, select the file
                        let parent = initial.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| {
                            UserDirs::new()
                                .map(|d| d.home_dir().to_path_buf())
                                .unwrap_or_else(|| env::current_dir().unwrap_or_default())
                        });
                        (parent, Some(initial))
                    }
                }
            } else {
                // Path doesn't exist - try parent
                let parent = initial.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| {
                    UserDirs::new()
                        .map(|d| d.home_dir().to_path_buf())
                        .unwrap_or_else(|| env::current_dir().unwrap_or_default())
                });
                (parent, None)
            }
        } else {
            (
                UserDirs::new()
                    .map(|d| d.home_dir().to_path_buf())
                    .unwrap_or_else(|| env::current_dir().unwrap_or_default()),
                None,
            )
        };
        let path = path; // Now path is the directory to navigate to

        let nav_point = NavPoint::file(path.to_string_lossy().replace('\\', "/"));

        // Build file list (no parent item - use toolbar for navigation)
        let mut files: Vec<Box<dyn Item>> = Vec::new();
        if let Some(mut items) = get_items_at_path(&nav_point.path) {
            files.append(&mut items);
        }

        let visible_indices: Vec<usize> = (0..files.len()).collect();

        let mut list_view = FileListView::new();
        list_view.set_item_count(visible_indices.len());

        let mut browser = Self {
            nav_point,
            files,
            visible_indices,
            filter: String::new(),
            list_view,
            sort_order: SortOrder::default(),
            sauce_cache: None,
            is_loading: false,
            pending_select_label: None,
        };

        // If we have a file to select, find and select it
        if let Some(ref file_path) = file_to_select {
            if let Some(file_name) = file_path.file_name() {
                browser.select_by_label(&file_name.to_string_lossy());
            }
        }

        (browser, file_to_select)
    }

    /// Returns (should_open_file, scroll_task) tuple
    pub fn update(&mut self, message: FileBrowserMessage) -> (bool, Task<FileBrowserMessage>) {
        match message {
            FileBrowserMessage::ListView(list_msg) => {
                let item_count = self.visible_indices.len();
                let (should_open, scroll_task) = self.list_view.update(list_msg, item_count);

                if should_open {
                    let (file_opened, open_task) = self.open_selected_item();
                    return (file_opened, Task::batch(vec![scroll_task, open_task]));
                }
                (false, scroll_task)
            }
            FileBrowserMessage::ParentFolder => {
                let task = self.navigate_parent();
                (false, task)
            }
            FileBrowserMessage::Refresh => {
                let task = self.refresh();
                (false, task)
            }
            FileBrowserMessage::WebItemsLoaded { path, result } => {
                self.apply_web_items(path, result);
                (false, Task::none())
            }
            FileBrowserMessage::FilterChanged(filter) => {
                self.filter = filter;
                self.update_visible_indices();
                // Reset selection when filter changes
                self.list_view.selected_index = if self.visible_indices.is_empty() { None } else { Some(0) };
                (false, Task::none())
            }
        }
    }

    /// Navigate to parent
    fn navigate_parent(&mut self) -> Task<FileBrowserMessage> {
        if self.nav_point.navigate_up() {
            return self.refresh();
        }
        Task::none()
    }

    /// Open the currently selected item - returns true if it's a file (for preview)
    fn open_selected_item(&mut self) -> (bool, Task<FileBrowserMessage>) {
        let Some(visible_index) = self.list_view.selected_index else {
            return (false, Task::none());
        };
        let Some(&file_index) = self.visible_indices.get(visible_index) else {
            return (false, Task::none());
        };
        if file_index >= self.files.len() {
            return (false, Task::none());
        }

        let is_container = self.files[file_index].is_container();
        let file_path = self.files[file_index].get_file_path();

        if is_container {
            // Navigate into folder (works for both regular folders and ZIPs)
            let new_path = if self.nav_point.path.is_empty() {
                // At root, just use the item path
                file_path
            } else {
                // Append to current path
                format!("{}/{}", self.nav_point.path, file_path)
            };
            self.nav_point.navigate_to(new_path);
            let task = self.refresh();
            (false, task)
        } else {
            // It's a file - signal that we want to open/preview it
            (true, Task::none())
        }
    }

    fn refresh(&mut self) -> Task<FileBrowserMessage> {
        self.files.clear();
        self.filter.clear();
        self.is_loading = false;
        self.pending_select_label = None;

        // Get items based on provider type (no parent item - use toolbar for navigation)
        match self.nav_point.provider_type {
            ProviderType::File => {
                if let Some(mut items) = get_items_at_path(&self.nav_point.path) {
                    self.files.append(&mut items);
                }
            }
            ProviderType::Web => {
                let path = self.nav_point.path.clone();
                self.is_loading = true;
                self.update_visible_indices();
                self.list_view.selected_index = None;
                self.list_view.invalidate();
                return Self::load_web_items(path);
            }
        }

        self.update_visible_indices();
        self.list_view.selected_index = if self.visible_indices.is_empty() { None } else { Some(0) };
        // Always invalidate after refresh to ensure cache is cleared
        self.list_view.invalidate();
        Task::none()
    }

    fn load_web_items(path: String) -> Task<FileBrowserMessage> {
        Task::perform(
            {
                let path = path.clone();
                async move {
                    let provider = SixteenColorsProvider::new();
                    provider.get_items(&path).await
                }
            },
            move |result| FileBrowserMessage::WebItemsLoaded { path, result },
        )
    }

    fn apply_web_items(&mut self, path: String, result: Result<Vec<Box<dyn Item>>, ItemError>) {
        if self.nav_point.provider_type != ProviderType::Web || self.nav_point.path != path {
            log::debug!("Ignoring stale 16colors folder result for {:?}", path);
            return;
        }

        self.is_loading = false;
        self.files.clear();
        match result {
            Ok(mut items) => {
                self.files.append(&mut items);
            }
            Err(err) => {
                log::error!("Failed to load 16colors folder {:?}: {}", path, err);
            }
        }

        self.update_visible_indices();
        if let Some(label) = self.pending_select_label.take() {
            if !self.select_by_label(&label) {
                self.list_view.selected_index = if self.visible_indices.is_empty() { None } else { Some(0) };
            }
        } else {
            self.list_view.selected_index = if self.visible_indices.is_empty() { None } else { Some(0) };
        }
        self.list_view.invalidate();
    }

    fn update_visible_indices(&mut self) {
        if self.filter.is_empty() {
            self.visible_indices = (0..self.files.len()).collect();
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.visible_indices = self
                .files
                .iter()
                .enumerate()
                .filter(|(_, f)| f.get_label().to_lowercase().contains(&filter_lower))
                .map(|(i, _)| i)
                .collect();
        }
        self.list_view.set_item_count(self.visible_indices.len());
    }

    /// Get the count of visible (filtered) files
    pub fn visible_file_count(&self) -> usize {
        self.visible_indices.len()
    }

    pub fn view(&self, theme: &icy_ui::Theme) -> Element<'_, FileBrowserMessage> {
        use i18n_embed_fl::fl;
        use icy_ui::widget::text;

        // If folder is empty (no files at all), show "Folder is empty" message
        if self.is_loading {
            let loading_text = text("Loading…").size(14).style(|theme: &icy_ui::Theme| text::Style {
                color: Some(theme.background.on.scale_alpha(0.5)),
            });
            return container(loading_text)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .padding(20)
                .into();
        }

        if self.files.is_empty() {
            let empty_text = text(fl!(crate::LANGUAGE_LOADER, "folder-empty"))
                .size(14)
                .style(|theme: &icy_ui::Theme| text::Style {
                    color: Some(theme.background.on.scale_alpha(0.5)),
                });
            return container(empty_text)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .padding(20)
                .into();
        }

        // If filter is active but no items match, show "No items found" message
        if !self.filter.is_empty() && self.visible_indices.is_empty() {
            let no_items_text = text(fl!(crate::LANGUAGE_LOADER, "filter-no-items-found"))
                .size(14)
                .style(|theme: &icy_ui::Theme| text::Style {
                    color: Some(theme.background.on.scale_alpha(0.5)),
                });
            return container(no_items_text)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .padding(20)
                .into();
        }

        // Use the custom file list view with files and visible indices
        use super::file_list_shader::FileListThemeColors;
        let theme_colors = FileListThemeColors::from_theme(theme);
        let list_view = self.list_view.view(
            &self.files,
            &self.visible_indices,
            &self.filter,
            theme_colors,
            self.sauce_cache.as_ref(),
            FileBrowserMessage::ListView,
        );

        // Wrap in focusable container to handle keyboard events
        let focusable_list = focus(list_view)
            .on_event(|event, _id| {
                // Handle keyboard events when focused
                if let Event::Keyboard(icy_ui::keyboard::Event::KeyPressed { key, .. }) = event {
                    match key {
                        Key::Named(Named::ArrowUp) => Some(FileBrowserMessage::ListView(FileListViewMessage::SelectPrevious)),
                        Key::Named(Named::ArrowDown) => Some(FileBrowserMessage::ListView(FileListViewMessage::SelectNext)),
                        Key::Named(Named::PageUp) => Some(FileBrowserMessage::ListView(FileListViewMessage::PageUp)),
                        Key::Named(Named::PageDown) => Some(FileBrowserMessage::ListView(FileListViewMessage::PageDown)),
                        Key::Named(Named::Home) => Some(FileBrowserMessage::ListView(FileListViewMessage::Home)),
                        Key::Named(Named::End) => Some(FileBrowserMessage::ListView(FileListViewMessage::End)),
                        Key::Named(Named::Enter) => Some(FileBrowserMessage::ListView(FileListViewMessage::OpenSelected)),
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .style(list_focus_style);

        // Main container (width is set by parent)
        container(focusable_list).width(Length::Fill).height(Length::Fill).into()
    }

    /// Get the currently selected file
    pub fn selected_item(&self) -> Option<&Box<dyn Item>> {
        let visible_index = self.list_view.selected_index?;
        let file_index = *self.visible_indices.get(visible_index)?;
        self.files.get(file_index)
    }

    /// Get the current path
    pub fn current_path(&self) -> Option<PathBuf> {
        Some(PathBuf::from(&self.nav_point.path))
    }

    /// Navigate to a specific filesystem path
    pub fn navigate_to(&mut self, path: PathBuf) {
        self.nav_point = NavPoint::file(path.to_string_lossy().replace('\\', "/"));
        let _ = self.refresh();
    }

    /// Set 16colors mode
    pub fn set_16colors_mode(&mut self, items: Vec<Box<dyn Item>>) {
        self.nav_point = NavPoint::web(String::new());
        self.files = items;
        self.is_loading = false;
        self.update_visible_indices();
        self.list_view.selected_index = if self.visible_indices.is_empty() { None } else { Some(0) };
        self.list_view.invalidate();
    }

    /// Get the display path for the navigation bar
    pub fn get_display_path(&self) -> String {
        self.nav_point.display_path()
    }

    /// Get the current nav point
    pub fn nav_point(&self) -> &NavPoint {
        &self.nav_point
    }

    /// Check if we can navigate to parent
    pub fn can_go_parent(&self) -> bool {
        self.nav_point.can_navigate_up()
    }

    /// Get cloned items for the tile grid (supports virtual files)
    /// Returns Box<dyn Item> for each visible item
    pub fn get_items(&self) -> Vec<Box<dyn Item>> {
        self.visible_indices
            .iter()
            .filter_map(|&i| self.files.get(i))
            .map(|item| item.clone_box())
            .collect()
    }

    /// Select an item by its path
    /// Returns true if the item was found and selected
    pub fn select_by_path(&mut self, path: &PathBuf) -> bool {
        let path_str = path.to_string_lossy().replace('\\', "/");
        // Find the visible index for this path
        for (visible_idx, &file_idx) in self.visible_indices.iter().enumerate() {
            if let Some(item) = self.files.get(file_idx) {
                if item.get_file_path() == path_str {
                    self.list_view.selected_index = Some(visible_idx);
                    // Note: caller should scroll to make selection visible via Task
                    return true;
                }
            }
        }
        false
    }

    /// Select an item by its label
    /// Returns true if the item was found and selected
    pub fn select_by_label(&mut self, label: &str) -> bool {
        for (visible_idx, &file_idx) in self.visible_indices.iter().enumerate() {
            if let Some(item) = self.files.get(file_idx) {
                if item.get_label() == label {
                    self.list_view.selected_index = Some(visible_idx);
                    // Note: caller should scroll to make selection visible via Task
                    return true;
                }
            }
        }
        false
    }

    /// Navigate to a web path (16colors)
    pub fn navigate_to_web_path(&mut self, path: &str) -> Task<FileBrowserMessage> {
        // Remove leading slash if present - internal paths don't have leading /
        let clean_path = path.trim_start_matches('/');
        self.nav_point = NavPoint::web(clean_path.to_string());
        self.refresh()
    }

    /// Select an item by label when the next async web navigation completes.
    pub fn select_by_label_after_load(&mut self, label: String) {
        self.pending_select_label = Some(label);
    }

    /// Check if we're in web mode
    pub fn is_web_mode(&self) -> bool {
        self.nav_point.is_web()
    }

    /// Set the sort order and re-sort the file list
    pub fn set_sort_order(&mut self, order: SortOrder) {
        self.sort_order = order;
        self.resort_files();
    }

    /// Set the SAUCE mode (show/hide SAUCE columns)
    pub fn set_sauce_mode(&mut self, sauce_mode: bool) {
        self.list_view.set_sauce_mode(sauce_mode);
    }

    /// Set the shared SAUCE cache
    pub fn set_sauce_cache(&mut self, cache: SharedSauceCache) {
        self.sauce_cache = Some(cache);
    }

    /// Clear the SAUCE cache (called when directory changes)
    pub fn clear_sauce_cache(&self) {
        if let Some(ref cache) = self.sauce_cache {
            cache.write().clear();
        }
    }

    /// Re-sort the files according to the current sort order
    fn resort_files(&mut self) {
        sort_items(&mut self.files, self.sort_order);
        self.update_visible_indices();
        self.list_view.invalidate();
    }
}

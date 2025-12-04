use std::{path::PathBuf, sync::Arc, time::Instant};

use i18n_embed_fl::fl;
use iced::{
    Element, Event, Length, Rectangle, Task, Theme,
    keyboard::{Key, key::Named},
    widget::{Space, column, container, row, text},
};
use icy_engine_gui::{
    ButtonSet, ConfirmationDialog, DialogType, Toast, ToastManager,
    ui::{ExportDialogMessage, ExportDialogState},
};
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::{
    DEFAULT_TITLE, Item,
    items::{ProviderType, SixteenColorsProvider, SixteenColorsRoot},
};
use icy_engine::formats::FileFormat;

use super::{
    FileBrowser, FileBrowserMessage, FileListViewMessage, HistoryPoint, NavigationBar, NavigationBarMessage, NavigationHistory, Options, PreviewMessage,
    PreviewView, StatusBar, StatusBarMessage, StatusInfo, TileGridMessage, TileGridView,
    dialogs::sauce_dialog::{SauceDialog, SauceDialogMessage},
    dialogs::settings_dialog::{SettingsDialogState, SettingsMessage},
    focus::{focus, list_focus_style},
    options::ViewMode,
};

/// Messages for the main window
#[derive(Clone)]
pub enum Message {
    /// File browser messages
    FileBrowser(FileBrowserMessage),
    /// Navigation bar messages
    Navigation(NavigationBarMessage),
    /// Preview messages
    Preview(PreviewMessage),
    /// Status bar messages
    StatusBar(StatusBarMessage),
    /// Tile grid messages
    TileGrid(TileGridMessage),
    /// Folder preview tile grid messages (for list mode folder preview)
    FolderPreview(TileGridMessage),
    /// Toggle fullscreen mode
    ToggleFullscreen,
    /// Escape key pressed
    Escape,
    /// Focus next widget (Tab)
    FocusNext,
    /// Focus previous widget (Shift+Tab)
    FocusPrevious,
    /// Data was loaded for preview (path for display, data for content)
    DataLoaded(PathBuf, Vec<u8>),
    /// Animation tick for smooth scrolling
    AnimationTick,
    /// SAUCE dialog messages
    SauceDialog(SauceDialogMessage),
    /// Settings dialog messages
    SettingsDialog(SettingsMessage),
    /// Show settings dialog
    ShowSettings,
    /// Close settings dialog
    CloseSettingsDialog,
    /// Execute external command (0-3 for F5-F8)
    ExecuteExternalCommand(usize),
    /// Show error dialog
    ShowErrorDialog { title: String, message: String },
    /// Close error dialog
    CloseErrorDialog,
    /// Show a toast notification
    ShowToast(Toast),
    /// Close a toast notification
    CloseToast(usize),
    /// Show export dialog
    ShowExportDialog,
    /// Export dialog messages
    ExportDialog(ExportDialogMessage),
    /// Copy selection to clipboard
    Copy,
}

/// Main window for icy_view_gui
pub struct MainWindow {
    /// Window ID (1-based, for multi-window support)
    pub id: usize,
    /// Window title
    pub title: String,
    /// File browser widget
    pub file_browser: FileBrowser,
    /// Navigation bar
    pub navigation_bar: NavigationBar,
    /// Navigation history
    pub history: NavigationHistory,
    /// Options
    pub options: Arc<Mutex<Options>>,
    /// Fullscreen mode
    pub fullscreen: bool,
    /// Currently loaded file for preview
    pub current_file: Option<PathBuf>,
    /// Preview view for ANSI files
    pub preview: PreviewView,
    /// Tile grid view for thumbnail preview
    pub tile_grid: TileGridView,
    /// Folder preview tile grid (for list mode when folder is selected)
    pub folder_preview: TileGridView,
    /// Path of the folder being previewed (None means show file preview)
    pub folder_preview_path: Option<PathBuf>,
    /// Current view mode
    pub view_mode: ViewMode,
    /// Last animation tick time for delta calculation
    last_tick: Instant,
    /// SAUCE dialog (shown as modal when Some)
    sauce_dialog: Option<SauceDialog>,
    /// Settings dialog (shown as modal when Some)
    settings_dialog: Option<SettingsDialogState>,
    /// Error dialog (shown as modal when Some)
    error_dialog: Option<ConfirmationDialog>,
    /// Export dialog (shown as modal when Some)
    export_dialog: Option<ExportDialogState>,
    /// Toast notifications
    toasts: Vec<Toast>,
}

impl MainWindow {
    /// Creates a new MainWindow.
    /// Returns (Self, Option<Message>) where the second value is an initial message to process
    /// (e.g., to load a file preview when started with a file path)
    pub fn new(id: usize, initial_path: Option<PathBuf>, options: Arc<Mutex<Options>>, auto_scroll: bool, bps: Option<u32>) -> (Self, Option<Message>) {
        let mut opts = Options::default();
        let view_mode;
        {
            let locked = options.lock();
            opts.auto_scroll_enabled = locked.auto_scroll_enabled;
            opts.scroll_speed = locked.scroll_speed.clone();
            opts.show_settings = locked.show_settings;
            view_mode = locked.view_mode;
        }
        if auto_scroll {
            opts.auto_scroll_enabled = true;
        }

        let (file_browser, file_to_preview) = FileBrowser::new(initial_path.clone());
        let mut history = NavigationHistory::new();
        // Initialize history with current state
        let initial_point = HistoryPoint::new(
            ProviderType::File,
            view_mode,
            file_browser.current_path().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            None,
        );
        history.init(initial_point);

        let mut preview = PreviewView::new();
        preview.set_scroll_speed(opts.scroll_speed);
        // Apply command-line auto-scroll setting
        preview.set_auto_scroll_enabled(opts.auto_scroll_enabled);
        // Apply command-line baud emulation setting
        if let Some(rate) = bps {
            preview.set_baud_emulation(icy_parser_core::BaudEmulation::Rate(rate));
        }

        let mut navigation_bar = NavigationBar::new();
        navigation_bar.set_view_mode(view_mode);
        // Initialize path input with current path
        if let Some(path) = file_browser.current_path() {
            navigation_bar.set_path_input(path.to_string_lossy().to_string());
        }

        let mut tile_grid = TileGridView::new();
        // If starting in tile mode, populate the tiles immediately
        if view_mode == ViewMode::Tiles {
            let items = file_browser.get_items();
            tile_grid.set_items_from_items(items);
        }

        // Set up initial file for preview (from command line)
        let (current_file, title, initial_message) = if let Some(ref file_path) = file_to_preview {
            let title = file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| DEFAULT_TITLE.clone());
            // Read the file data and create a DataLoaded message
            let msg = std::fs::read(file_path).ok().map(|data| Message::DataLoaded(file_path.clone(), data));
            (Some(file_path.clone()), title, msg)
        } else {
            (None, DEFAULT_TITLE.clone(), None)
        };

        (
            Self {
                id,
                title,
                file_browser,
                navigation_bar,
                history,
                options,
                fullscreen: false,
                current_file,
                preview,
                tile_grid,
                folder_preview: TileGridView::new(),
                folder_preview_path: None,
                view_mode,
                last_tick: Instant::now(),
                sauce_dialog: None,
                settings_dialog: None,
                error_dialog: None,
                export_dialog: None,
                toasts: Vec::new(),
            },
            initial_message,
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FileBrowser(msg) => {
                // Check if this will change the directory - use display_path for accurate comparison
                let old_display_path = self.file_browser.get_display_path();
                let old_selection = self.file_browser.selected_item().map(|i| i.get_file_path());

                let file_opened = self.file_browser.update(msg);

                // Update history if path changed - use display_path for both local and web paths
                let new_display_path = self.file_browser.get_display_path();
                if old_display_path != new_display_path {
                    // Record the navigation as a new history point
                    let point = self.current_history_point();
                    self.history.navigate_to(point);
                    // Update path input to show current display path
                    self.navigation_bar.set_path_input(new_display_path.clone());
                    // Also update tile grid when directory changes
                    if self.view_mode == ViewMode::Tiles {
                        let items = self.file_browser.get_items();
                        self.tile_grid.set_items_from_items(items);
                    }
                }

                // Check if selection changed - auto-preview files on selection
                let new_selection = self.file_browser.selected_item().map(|i| i.get_file_path());
                let selection_changed = old_selection != new_selection;

                if selection_changed {
                    // Cancel any ongoing loading operation when selection changes
                    self.preview.cancel_loading();

                    // If selection was cleared, also clear the preview state
                    if new_selection.is_none() {
                        self.current_file = None;
                        self.folder_preview_path = None;
                        self.title = DEFAULT_TITLE.clone();
                    }
                }

                if let Some(item) = self.file_browser.selected_item() {
                    let new_selection = item.get_file_path();
                    let is_container = item.is_container();
                    log::info!(
                        "[MainWindow] Selection: {:?}, is_container={}, selection_changed={}, file_opened={}",
                        new_selection,
                        is_container,
                        selection_changed,
                        file_opened
                    );

                    // Record selection change as new history point
                    if selection_changed && old_display_path == new_display_path {
                        let point = self.current_history_point();
                        self.history.navigate_to(point);
                    }

                    if selection_changed || file_opened {
                        if is_container {
                            // For folders (including zip files), show thumbnail preview of contents
                            self.current_file = None;
                            // Build full path by combining browser's current path with the relative item path
                            let full_path = if let Some(current) = self.file_browser.current_path() {
                                current.join(&new_selection)
                            } else {
                                new_selection.clone()
                            };
                            self.folder_preview_path = Some(full_path.clone());
                            // Load folder contents asynchronously in background thread
                            // This works for both regular folders and zip files
                            if let Some(item) = self.file_browser.selected_item() {
                                self.folder_preview.load_subitems_async(item.clone_box());
                            }
                        } else {
                            // For files, show file preview
                            self.folder_preview_path = None;
                            // Build full path by combining browser's current path with the relative item path
                            let full_path = if let Some(current) = self.file_browser.current_path() {
                                current.join(&new_selection)
                            } else {
                                new_selection.clone()
                            };
                            self.current_file = Some(full_path.clone());
                            self.title = new_selection
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| DEFAULT_TITLE.clone());

                            // Read the data from the item (works for both local and virtual files)
                            if let Some(item_mut) = self.file_browser.selected_item_mut() {
                                if let Some(data) = item_mut.read_data_blocking() {
                                    return Task::done(Message::DataLoaded(full_path, data));
                                }
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::Navigation(nav_msg) => {
                match nav_msg {
                    NavigationBarMessage::Back => {
                        if let Some(point) = self.history.go_back() {
                            self.navigate_to_history_point(&point);
                        }
                    }
                    NavigationBarMessage::Forward => {
                        if let Some(point) = self.history.go_forward() {
                            self.navigate_to_history_point(&point);
                        }
                    }
                    NavigationBarMessage::Up => {
                        // Cancel any ongoing loading operation
                        self.preview.cancel_loading();
                        self.current_file = None;
                        self.folder_preview_path = None;

                        // Save current state before navigating
                        let current_point = self.current_history_point();
                        self.history.navigate_to(current_point);

                        self.file_browser.update(FileBrowserMessage::ParentFolder);
                        // Update path input with the new display path
                        let display_path = self.file_browser.get_display_path();
                        self.navigation_bar.set_path_input(display_path);
                        if self.view_mode == ViewMode::Tiles {
                            let items = self.file_browser.get_items();
                            self.tile_grid.set_items_from_items(items);
                        }
                    }
                    NavigationBarMessage::Refresh => {
                        // Cancel any ongoing loading operation on refresh
                        self.preview.cancel_loading();

                        self.file_browser.update(FileBrowserMessage::Refresh);
                        if self.view_mode == ViewMode::Tiles {
                            let items = self.file_browser.get_items();
                            self.tile_grid.set_items_from_items(items);
                        }
                    }
                    NavigationBarMessage::FilterChanged(filter) => {
                        self.navigation_bar.set_filter(filter.clone());
                        self.file_browser.update(FileBrowserMessage::FilterChanged(filter.clone()));
                        // Also apply filter to tile grid
                        self.tile_grid.apply_filter(&filter);
                        self.folder_preview.apply_filter(&filter);
                    }
                    NavigationBarMessage::ClearFilter => {
                        self.navigation_bar.set_filter(String::new());
                        self.file_browser.update(FileBrowserMessage::ClearFilter);
                        // Also clear filter on tile grid
                        self.tile_grid.clear_filter();
                        self.folder_preview.clear_filter();
                    }
                    NavigationBarMessage::ToggleViewMode => {
                        // Cancel any ongoing loading operation when switching modes
                        self.preview.cancel_loading();
                        self.current_file = None;
                        self.folder_preview_path = None;

                        self.navigation_bar.toggle_view_mode();
                        self.view_mode = self.navigation_bar.view_mode;
                        // Save the preference
                        {
                            let mut locked = self.options.lock();
                            locked.view_mode = self.view_mode;
                        }
                        // When switching to tile mode, populate the tile grid with current items
                        // and apply the current filter
                        if self.view_mode == ViewMode::Tiles {
                            let items = self.file_browser.get_items();
                            self.tile_grid.set_items_from_items(items);
                            // Apply current filter to tile grid
                            let filter = self.navigation_bar.filter.clone();
                            if !filter.is_empty() {
                                self.tile_grid.apply_filter(&filter);
                            }
                        }
                    }
                    NavigationBarMessage::Toggle16Colors => {
                        // Cancel any ongoing loading operation when switching providers
                        self.preview.cancel_loading();
                        self.current_file = None;
                        self.folder_preview_path = None;

                        let now_16colors = !self.navigation_bar.is_16colors_mode;
                        self.navigation_bar.set_16colors_mode(now_16colors);
                        if now_16colors {
                            // Switch to 16colors root
                            let root = SixteenColorsRoot::new();
                            let root_box: Box<dyn crate::Item> = Box::new(root);
                            let cancel_token = CancellationToken::new();
                            let items = root_box.get_subitems_blocking(&cancel_token).unwrap_or_default();
                            self.file_browser.set_16colors_mode(items);
                            self.navigation_bar.set_path_input("/".to_string());
                        } else {
                            // Switch back to home
                            let home = directories::UserDirs::new()
                                .map(|d| d.home_dir().to_path_buf())
                                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                            self.file_browser.navigate_to(home.clone());
                            self.navigation_bar.set_path_input(home.to_string_lossy().to_string());
                        }
                        if self.view_mode == ViewMode::Tiles {
                            let items = self.file_browser.get_items();
                            self.tile_grid.set_items_from_items(items);
                        }
                    }
                    NavigationBarMessage::PathChanged(path) => {
                        self.navigation_bar.path_input = path;
                        // Reset validity when typing - will be validated on submit
                        self.navigation_bar.is_path_valid = true;
                    }
                    NavigationBarMessage::PathSubmitted => {
                        // Cancel any ongoing loading operation when navigating
                        self.preview.cancel_loading();
                        self.current_file = None;
                        self.folder_preview_path = None;

                        let path_str = self.navigation_bar.path_input.clone();
                        // Check if it's a 16colors path (starts with / and we're in 16colors mode, or explicit 16colors prefix)
                        let is_16colors_path = path_str.starts_with("16colors://")
                            || path_str.starts_with("/16colors/")
                            || path_str.starts_with("16colo.rs")
                            || (self.navigation_bar.is_16colors_mode && path_str.starts_with('/'));

                        if is_16colors_path {
                            // Extract the web path part for validation
                            let web_path = if path_str.starts_with("16colo.rs/") {
                                path_str.strip_prefix("16colo.rs/").unwrap_or("")
                            } else if path_str.starts_with("16colo.rs") {
                                ""
                            } else if path_str.starts_with("16colors://") {
                                path_str.strip_prefix("16colors://").unwrap_or("")
                            } else if path_str.starts_with("/16colors/") {
                                path_str.strip_prefix("/16colors/").unwrap_or("")
                            } else if path_str.starts_with('/') {
                                // Simple / path in 16colors mode
                                path_str.strip_prefix('/').unwrap_or("")
                            } else {
                                &path_str
                            };

                            // Validate against cache
                            if SixteenColorsProvider::validate_path(web_path) {
                                // Navigate to 16colors
                                self.navigation_bar.set_16colors_mode(true);
                                self.file_browser.navigate_to_web_path(web_path);
                                // Update path input to reflect the navigated path
                                let display = self.file_browser.get_display_path();
                                self.navigation_bar.set_path_input(display);
                            } else {
                                // Invalid path - show red border
                                self.navigation_bar.set_path_valid(false);
                            }
                        } else {
                            // Try to navigate to filesystem path
                            let path = std::path::PathBuf::from(&path_str);
                            if path.exists() {
                                if path.is_dir() {
                                    // Scenario 1: Directory - navigate to it
                                    self.navigation_bar.set_16colors_mode(false);
                                    self.file_browser.navigate_to(path.clone());
                                    self.navigation_bar.set_path_input(path.to_string_lossy().to_string());
                                    let point = self.current_history_point();
                                    self.history.navigate_to(point);
                                } else if path.is_file() {
                                    // Check if it's an archive (Scenario 2)
                                    if let Some(FileFormat::Archive(_)) = FileFormat::from_path(&path) {
                                        // Scenario 2: Archive - treat as directory, navigate into it
                                        self.navigation_bar.set_16colors_mode(false);
                                        self.file_browser.navigate_to(path.clone());
                                        self.navigation_bar.set_path_input(path.to_string_lossy().to_string());
                                        let point = self.current_history_point();
                                        self.history.navigate_to(point);
                                    } else {
                                        // Scenario 3: Regular file - navigate to parent and select the file
                                        if let Some(parent) = path.parent() {
                                            self.navigation_bar.set_16colors_mode(false);
                                            self.file_browser.navigate_to(parent.to_path_buf());
                                            self.navigation_bar.set_path_input(parent.to_string_lossy().to_string());

                                            // Select the file by its name
                                            if let Some(file_name) = path.file_name() {
                                                self.file_browser.select_by_label(&file_name.to_string_lossy());
                                            }

                                            // Load preview for the selected file
                                            self.current_file = Some(path.clone());
                                            self.title = path
                                                .file_name()
                                                .map(|n| n.to_string_lossy().to_string())
                                                .unwrap_or_else(|| DEFAULT_TITLE.clone());
                                            if let Ok(data) = std::fs::read(&path) {
                                                return Task::done(Message::DataLoaded(path, data));
                                            }

                                            let point = self.current_history_point();
                                            self.history.navigate_to(point);
                                        }
                                    }
                                }
                            } else if !path_str.is_empty() {
                                // Invalid path - show red border
                                self.navigation_bar.set_path_valid(false);
                            }
                        }
                        if self.view_mode == ViewMode::Tiles {
                            let items = self.file_browser.get_items();
                            self.tile_grid.set_items_from_items(items);
                        }
                    }
                    NavigationBarMessage::OpenSettings => {
                        return self.update(Message::ShowSettings);
                    }
                }
                Task::none()
            }
            Message::TileGrid(msg) => {
                // Track old selection for history
                let old_selection = self.tile_grid.selected_index;

                // First update the tile grid to update selection
                let should_open = self.tile_grid.update(msg.clone());

                // Then handle tile grid messages
                match &msg {
                    TileGridMessage::TileClicked(_)
                    | TileGridMessage::SelectPrevious
                    | TileGridMessage::SelectNext
                    | TileGridMessage::SelectLeft
                    | TileGridMessage::SelectRight
                    | TileGridMessage::PageUp
                    | TileGridMessage::PageDown
                    | TileGridMessage::Home
                    | TileGridMessage::End => {
                        // Record selection change as new history point
                        if old_selection != self.tile_grid.selected_index {
                            let point = self.current_history_point();
                            self.history.navigate_to(point);
                        }

                        // Select the item and load preview
                        if let Some((path, _label, is_container)) = self.tile_grid.get_selected_info() {
                            if !is_container {
                                self.current_file = Some(path.clone());
                                self.title = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| DEFAULT_TITLE.clone());

                                // Read data for preview from file
                                if let Ok(data) = std::fs::read(&path) {
                                    return Task::done(Message::DataLoaded(path, data));
                                }
                            }
                        }
                    }
                    TileGridMessage::TileDoubleClicked(index) => {
                        // Open the item (navigate into folder or switch to list view for file)
                        if let Some((item_path, _label, is_container)) = self.tile_grid.get_item_info(*index) {
                            if is_container {
                                // Handle web mode vs file mode differently
                                if self.file_browser.is_web_mode() {
                                    // For 16colors, construct web path
                                    let current_path = self.file_browser.nav_point().path.clone();
                                    let new_path = if current_path.is_empty() {
                                        item_path.to_string_lossy().to_string()
                                    } else {
                                        format!("{}/{}", current_path, item_path.to_string_lossy())
                                    };
                                    self.file_browser.navigate_to_web_path(&new_path);
                                    // Update navigation bar with display path (includes leading /)
                                    self.navigation_bar.set_path_input(self.file_browser.get_display_path());
                                } else {
                                    // Build full path from current browser path + item name
                                    let current_path = self.file_browser.get_display_path();
                                    let full_path = PathBuf::from(&current_path).join(&item_path);
                                    self.file_browser.navigate_to(full_path.clone());
                                    // Update navigation bar
                                    self.navigation_bar.set_path_input(full_path.to_string_lossy().to_string());
                                }
                                // Refresh tile grid with new items
                                let items = self.file_browser.get_items();
                                self.tile_grid.set_items_from_items(items);
                                // Record navigation
                                let point = self.current_history_point();
                                self.history.navigate_to(point);
                            } else {
                                // For files, switch to list view and select the item
                                self.view_mode = ViewMode::List;
                                self.navigation_bar.set_view_mode(ViewMode::List);
                                {
                                    let mut locked = self.options.lock();
                                    locked.view_mode = ViewMode::List;
                                }
                                // Build full path for the file
                                let current_path = self.file_browser.get_display_path();
                                let full_path = PathBuf::from(&current_path).join(&item_path);
                                // Select the item in the file browser
                                self.file_browser.select_by_path(&item_path);
                                // Load preview
                                self.current_file = Some(full_path.clone());
                                self.title = item_path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| DEFAULT_TITLE.clone());
                                // Read data - prefer using Item for virtual files, fall back to fs::read
                                if let Some(item) = self.tile_grid.get_item_at(*index) {
                                    if let Some(data) = item.read_data_blocking() {
                                        return Task::done(Message::DataLoaded(full_path, data));
                                    }
                                }
                                if let Ok(data) = std::fs::read(&full_path) {
                                    return Task::done(Message::DataLoaded(full_path, data));
                                }
                            }
                        }
                    }
                    TileGridMessage::OpenSelected => {
                        // Open selected item (Enter key)
                        if should_open {
                            if let Some((item_path, _label, is_container)) = self.tile_grid.get_selected_info() {
                                if is_container {
                                    // Handle web mode vs file mode differently
                                    if self.file_browser.is_web_mode() {
                                        // For 16colors, construct web path
                                        let current_path = self.file_browser.nav_point().path.clone();
                                        let new_path = if current_path.is_empty() {
                                            item_path.to_string_lossy().to_string()
                                        } else {
                                            format!("{}/{}", current_path, item_path.to_string_lossy())
                                        };
                                        self.file_browser.navigate_to_web_path(&new_path);
                                        // Update navigation bar with display path (includes leading /)
                                        self.navigation_bar.set_path_input(self.file_browser.get_display_path());
                                    } else {
                                        // Build full path from current browser path + item name
                                        let current_path = self.file_browser.get_display_path();
                                        let full_path = PathBuf::from(&current_path).join(&item_path);
                                        self.file_browser.navigate_to(full_path.clone());
                                        // Update navigation bar
                                        self.navigation_bar.set_path_input(full_path.to_string_lossy().to_string());
                                    }
                                    // Refresh tile grid with new items
                                    let items = self.file_browser.get_items();
                                    self.tile_grid.set_items_from_items(items);
                                    // Record navigation
                                    let point = self.current_history_point();
                                    self.history.navigate_to(point);
                                } else {
                                    // For files, switch to list view and select the item
                                    self.view_mode = ViewMode::List;
                                    self.navigation_bar.set_view_mode(ViewMode::List);
                                    {
                                        let mut locked = self.options.lock();
                                        locked.view_mode = ViewMode::List;
                                    }
                                    // Build full path for the file
                                    let current_path = self.file_browser.get_display_path();
                                    let full_path = PathBuf::from(&current_path).join(&item_path);
                                    // Select the item in the file browser
                                    self.file_browser.select_by_path(&item_path);
                                    // Load preview
                                    self.current_file = Some(full_path.clone());
                                    self.title = item_path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| DEFAULT_TITLE.clone());
                                    // Read data - prefer using Item for virtual files, fall back to fs::read
                                    if let Some(item) = self.tile_grid.get_selected_item() {
                                        if let Some(data) = item.read_data_blocking() {
                                            return Task::done(Message::DataLoaded(full_path, data));
                                        }
                                    }
                                    if let Ok(data) = std::fs::read(&full_path) {
                                        return Task::done(Message::DataLoaded(full_path, data));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
                Task::none()
            }
            Message::ToggleFullscreen => {
                self.fullscreen = !self.fullscreen;
                let mode = if self.fullscreen {
                    iced::window::Mode::Fullscreen
                } else {
                    iced::window::Mode::Windowed
                };
                iced::window::latest().and_then(move |window| iced::window::set_mode(window, mode))
            }
            Message::Escape => {
                // Close dialogs in priority order
                if self.error_dialog.is_some() {
                    self.error_dialog = None;
                } else if self.export_dialog.is_some() {
                    self.export_dialog = None;
                } else if self.settings_dialog.is_some() {
                    self.settings_dialog = None;
                } else if self.sauce_dialog.is_some() {
                    self.sauce_dialog = None;
                } else if self.fullscreen {
                    self.fullscreen = false;
                }
                Task::none()
            }
            Message::FocusNext => iced::widget::operation::focus_next(),
            Message::FocusPrevious => iced::widget::operation::focus_previous(),
            Message::DataLoaded(path, data) => {
                // Reset timer when loading new file to prevent animation jumps
                self.last_tick = Instant::now();
                // Load data in preview
                self.preview.load_data(path, data).map(Message::Preview)
            }
            Message::Preview(msg) => {
                // Check for reset timer message
                if matches!(msg, PreviewMessage::ResetAnimationTimer) {
                    self.last_tick = Instant::now();
                    return Task::none();
                }
                self.preview.update(msg).map(Message::Preview)
            }
            Message::StatusBar(msg) => {
                match msg {
                    StatusBarMessage::CycleBaudEmulation | StatusBarMessage::CycleBaudRate => {
                        // Find current index and cycle to next
                        let current = self.preview.get_baud_emulation();
                        let current_idx = icy_parser_core::BaudEmulation::OPTIONS.iter().position(|&b| b == current).unwrap_or(0);
                        let next_idx = (current_idx + 1) % icy_parser_core::BaudEmulation::OPTIONS.len();
                        let next_baud = icy_parser_core::BaudEmulation::OPTIONS[next_idx];
                        self.preview.set_baud_emulation(next_baud);

                        let toast_msg = match next_baud {
                            icy_parser_core::BaudEmulation::Off => fl!(crate::LANGUAGE_LOADER, "toast-baud-rate-off"),
                            icy_parser_core::BaudEmulation::Rate(rate) => fl!(crate::LANGUAGE_LOADER, "toast-baud-rate", rate = rate),
                        };
                        let toast = Toast::info(toast_msg);
                        return Task::done(Message::ShowToast(toast));
                    }
                    StatusBarMessage::CycleBaudRateBackward => {
                        // Find current index and cycle to previous
                        let current = self.preview.get_baud_emulation();
                        let current_idx = icy_parser_core::BaudEmulation::OPTIONS.iter().position(|&b| b == current).unwrap_or(0);
                        let prev_idx = if current_idx == 0 {
                            icy_parser_core::BaudEmulation::OPTIONS.len() - 1
                        } else {
                            current_idx - 1
                        };
                        let prev_baud = icy_parser_core::BaudEmulation::OPTIONS[prev_idx];
                        self.preview.set_baud_emulation(prev_baud);

                        let toast_msg = match prev_baud {
                            icy_parser_core::BaudEmulation::Off => fl!(crate::LANGUAGE_LOADER, "toast-baud-rate-off"),
                            icy_parser_core::BaudEmulation::Rate(rate) => fl!(crate::LANGUAGE_LOADER, "toast-baud-rate", rate = rate),
                        };
                        let toast = Toast::info(toast_msg);
                        return Task::done(Message::ShowToast(toast));
                    }
                    StatusBarMessage::SetBaudRateOff => {
                        // Set baud rate to Off (max speed, index 0)
                        self.preview.set_baud_emulation(icy_parser_core::BaudEmulation::Off);
                        return Task::none();
                    }
                    StatusBarMessage::CycleScrollSpeed => {
                        // Cycle scroll speed: Slow -> Medium -> Fast -> Slow
                        use super::options::ScrollSpeed;
                        let current = self.preview.get_scroll_speed();
                        let next = match current {
                            ScrollSpeed::Slow => ScrollSpeed::Medium,
                            ScrollSpeed::Medium => ScrollSpeed::Fast,
                            ScrollSpeed::Fast => ScrollSpeed::Slow,
                        };
                        self.preview.set_scroll_speed(next.clone());
                        self.tile_grid.set_scroll_speed(next.clone());
                        self.folder_preview.set_scroll_speed(next.clone());

                        // Save to options
                        self.options.lock().scroll_speed = self.preview.get_scroll_speed();

                        let toast_msg = match next {
                            ScrollSpeed::Slow => fl!(crate::LANGUAGE_LOADER, "toast-scroll-slow"),
                            ScrollSpeed::Medium => fl!(crate::LANGUAGE_LOADER, "toast-scroll-medium"),
                            ScrollSpeed::Fast => fl!(crate::LANGUAGE_LOADER, "toast-scroll-fast"),
                        };
                        let toast = Toast::info(toast_msg);
                        return Task::done(Message::ShowToast(toast));
                    }
                    StatusBarMessage::ToggleAutoScroll => {
                        // Toggle auto-scroll enabled setting
                        let new_enabled = !self.preview.is_auto_scroll_enabled();
                        self.preview.set_auto_scroll_enabled(new_enabled);

                        if new_enabled {
                            // Start scrolling and reset timer
                            self.last_tick = Instant::now();

                            // Show toast for auto-scroll enabled
                            let toast = Toast::info(fl!(crate::LANGUAGE_LOADER, "toast-auto-scroll-on"));

                            // Start auto-scroll on the appropriate view based on mode
                            match self.view_mode {
                                ViewMode::Tiles => {
                                    // Sync scroll speed and start tile grid auto-scroll
                                    self.tile_grid.set_scroll_speed(self.preview.get_scroll_speed());
                                    self.tile_grid.start_auto_scroll();
                                    return Task::done(Message::ShowToast(toast));
                                }
                                ViewMode::List => {
                                    if self.folder_preview_path.is_some() {
                                        // Folder preview mode - scroll the folder tile grid
                                        self.folder_preview.set_scroll_speed(self.preview.get_scroll_speed());
                                        self.folder_preview.start_auto_scroll();
                                        return Task::done(Message::ShowToast(toast));
                                    } else {
                                        // File preview mode - scroll the preview
                                        self.toasts.push(toast);
                                        return self.preview.start_auto_scroll().map(Message::Preview);
                                    }
                                }
                            }
                        } else {
                            // Stop scrolling on all views
                            self.preview.stop_auto_scroll();
                            self.tile_grid.stop_auto_scroll();
                            self.folder_preview.stop_auto_scroll();

                            // Show toast for auto-scroll disabled
                            let toast = Toast::info(fl!(crate::LANGUAGE_LOADER, "toast-auto-scroll-off"));
                            return Task::done(Message::ShowToast(toast));
                        }
                    }
                    StatusBarMessage::ShowSauceInfo => {
                        // Get SAUCE info from the appropriate source based on view mode
                        let sauce_info = if self.view_mode == ViewMode::Tiles {
                            self.tile_grid.get_status_info().and_then(|(_, _, _, sauce)| sauce)
                        } else {
                            self.preview.get_sauce_info().cloned()
                        };

                        if let Some(sauce) = sauce_info {
                            self.sauce_dialog = Some(SauceDialog::new(sauce));
                        }
                        return Task::none();
                    }
                }
            }
            Message::SauceDialog(msg) => {
                if let Some(ref mut dialog) = self.sauce_dialog {
                    let should_close = dialog.update(msg);
                    if should_close {
                        self.sauce_dialog = None;
                    }
                }
                Task::none()
            }
            Message::FolderPreview(msg) => {
                // Handle folder preview tile grid messages
                let should_open = self.folder_preview.update(msg.clone());

                // Handle single-click to update current_file and load preview
                if let TileGridMessage::TileClicked(index) = &msg {
                    if let Some((item_path, label, is_container)) = self.folder_preview.get_item_info(*index) {
                        // Only update preview for non-container items (files)
                        if !is_container {
                            if let Some(ref preview_folder) = self.folder_preview_path {
                                let full_path = preview_folder.join(&item_path);
                                self.current_file = Some(full_path.clone());
                                self.title = label;

                                // Load preview data
                                if let Some(item) = self.folder_preview.get_item_at(*index) {
                                    if let Some(data) = item.read_data_blocking() {
                                        return Task::done(Message::DataLoaded(full_path, data));
                                    }
                                }
                                // Fallback to filesystem read
                                if let Ok(data) = std::fs::read(&full_path) {
                                    return Task::done(Message::DataLoaded(full_path, data));
                                }
                            }
                        }
                    }
                }

                // Handle double-click or Enter to navigate/open
                match &msg {
                    TileGridMessage::TileDoubleClicked(index) if should_open => {
                        if let Some((item_path, _label, is_container)) = self.folder_preview.get_item_info(*index) {
                            // Get the item BEFORE navigating (items will change after navigation)
                            let item_for_data = self.folder_preview.get_item_at(*index);

                            // Check if we're in web mode
                            let is_web_mode = self.file_browser.is_web_mode();

                            // First navigate to the folder being previewed
                            if let Some(preview_folder) = self.folder_preview_path.take() {
                                if is_web_mode {
                                    // For web mode, use path string without PathBuf operations
                                    let preview_path_str = preview_folder.to_string_lossy();
                                    let item_path_str = item_path.to_string_lossy();

                                    self.file_browser.navigate_to_web_path(&preview_path_str);
                                    self.navigation_bar.set_path_input(self.file_browser.get_display_path());

                                    // Construct path for item
                                    let full_path_str = format!("{}/{}", preview_path_str, item_path_str);

                                    if is_container {
                                        // Navigate into the subfolder
                                        self.file_browser.navigate_to_web_path(&full_path_str);
                                        self.navigation_bar.set_path_input(self.file_browser.get_display_path());
                                        self.file_browser.list_view.selected_index = Some(0);
                                        // Record navigation
                                        let point = self.current_history_point();
                                        self.history.navigate_to(point);
                                    } else {
                                        // Select the file in the browser and preview it
                                        self.file_browser.select_by_path(&item_path);
                                        self.current_file = Some(preview_folder.join(&item_path));
                                        self.title = item_path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| DEFAULT_TITLE.clone());
                                        // Read data from the item we captured before navigation
                                        if let Some(item) = item_for_data {
                                            if let Some(data) = item.read_data_blocking() {
                                                return Task::done(Message::DataLoaded(preview_folder.join(&item_path), data));
                                            }
                                        }
                                    }
                                } else {
                                    // File mode - use PathBuf operations
                                    self.file_browser.navigate_to(preview_folder.clone());
                                    self.navigation_bar.set_path_input(preview_folder.to_string_lossy().to_string());

                                    // Now construct full path for the item inside the previewed folder
                                    let full_path = preview_folder.join(&item_path);

                                    if is_container {
                                        // Navigate into the subfolder and select first item
                                        self.file_browser.navigate_to(full_path.clone());
                                        self.navigation_bar.set_path_input(full_path.to_string_lossy().to_string());
                                        self.file_browser.list_view.selected_index = Some(0);
                                        // Record navigation
                                        let point = self.current_history_point();
                                        self.history.navigate_to(point);
                                    } else {
                                        // Select the file in the browser and preview it
                                        self.file_browser.select_by_path(&item_path);
                                        self.current_file = Some(full_path.clone());
                                        self.title = item_path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| DEFAULT_TITLE.clone());
                                        // Read data from the item we captured before navigation
                                        if let Some(item) = item_for_data {
                                            if let Some(data) = item.read_data_blocking() {
                                                return Task::done(Message::DataLoaded(full_path, data));
                                            }
                                        }
                                        // Fallback to filesystem read
                                        if let Ok(data) = std::fs::read(&full_path) {
                                            return Task::done(Message::DataLoaded(full_path, data));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    TileGridMessage::OpenSelected if should_open => {
                        if let Some((item_path, _label, is_container)) = self.folder_preview.get_selected_info() {
                            // Get the item BEFORE navigating (items will change after navigation)
                            let item_for_data = self.folder_preview.get_selected_item();

                            // Check if we're in web mode
                            let is_web_mode = self.file_browser.is_web_mode();

                            // First navigate to the folder being previewed
                            if let Some(preview_folder) = self.folder_preview_path.take() {
                                if is_web_mode {
                                    // For web mode, use path string without PathBuf operations
                                    let preview_path_str = preview_folder.to_string_lossy();
                                    let item_path_str = item_path.to_string_lossy();

                                    self.file_browser.navigate_to_web_path(&preview_path_str);
                                    self.navigation_bar.set_path_input(self.file_browser.get_display_path());

                                    // Construct path for item
                                    let full_path_str = format!("{}/{}", preview_path_str, item_path_str);

                                    if is_container {
                                        // Navigate into the subfolder
                                        self.file_browser.navigate_to_web_path(&full_path_str);
                                        self.navigation_bar.set_path_input(self.file_browser.get_display_path());
                                        self.file_browser.list_view.selected_index = Some(0);
                                        // Record navigation
                                        let point = self.current_history_point();
                                        self.history.navigate_to(point);
                                    } else {
                                        // Select the file in the browser and preview it
                                        self.file_browser.select_by_path(&item_path);
                                        self.current_file = Some(preview_folder.join(&item_path));
                                        self.title = item_path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| DEFAULT_TITLE.clone());
                                        // Read data from the item we captured before navigation
                                        if let Some(item) = item_for_data {
                                            if let Some(data) = item.read_data_blocking() {
                                                return Task::done(Message::DataLoaded(preview_folder.join(&item_path), data));
                                            }
                                        }
                                    }
                                } else {
                                    // File mode - use PathBuf operations
                                    self.file_browser.navigate_to(preview_folder.clone());
                                    self.navigation_bar.set_path_input(preview_folder.to_string_lossy().to_string());

                                    // Now construct full path for the item inside the previewed folder
                                    let full_path = preview_folder.join(&item_path);

                                    if is_container {
                                        // Navigate into the subfolder and select first item
                                        self.file_browser.navigate_to(full_path.clone());
                                        self.navigation_bar.set_path_input(full_path.to_string_lossy().to_string());
                                        self.file_browser.list_view.selected_index = Some(0);
                                        // Record navigation
                                        let point = self.current_history_point();
                                        self.history.navigate_to(point);
                                    } else {
                                        // Select the file in the browser and preview it
                                        self.file_browser.select_by_path(&item_path);
                                        self.current_file = Some(full_path.clone());
                                        self.title = item_path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| DEFAULT_TITLE.clone());
                                        // Read data from the item we captured before navigation
                                        if let Some(item) = item_for_data {
                                            if let Some(data) = item.read_data_blocking() {
                                                return Task::done(Message::DataLoaded(full_path, data));
                                            }
                                        }
                                        // Fallback to filesystem read
                                        if let Ok(data) = std::fs::read(&full_path) {
                                            return Task::done(Message::DataLoaded(full_path, data));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
                Task::none()
            }
            Message::SettingsDialog(msg) => {
                if let Some(ref mut dialog) = self.settings_dialog {
                    if let Some(result_msg) = dialog.update(msg) {
                        return self.update(result_msg);
                    }
                }
                Task::none()
            }
            Message::ShowSettings => {
                // Create temp options from current options
                let temp_options = Arc::new(Mutex::new(self.options.lock().clone()));
                self.settings_dialog = Some(SettingsDialogState::new(self.options.clone(), temp_options));
                Task::none()
            }
            Message::CloseSettingsDialog => {
                self.settings_dialog = None;
                Task::none()
            }
            Message::AnimationTick => {
                // Calculate delta time since last tick
                let now = Instant::now();
                let delta = now.duration_since(self.last_tick);
                self.last_tick = now;
                let delta_seconds = delta.as_secs_f32();

                // Forward tick to file browser's list view
                self.file_browser.update(FileBrowserMessage::ListView(FileListViewMessage::Tick));
                // Poll tile grid results if in tiles mode
                if self.view_mode == ViewMode::Tiles {
                    let _ = self.tile_grid.poll_results();
                    self.tile_grid.tick(delta_seconds);
                }

                // Poll folder preview results if showing folder preview in list mode
                if self.view_mode == ViewMode::List && self.folder_preview_path.is_some() {
                    let _ = self.folder_preview.poll_results();
                    self.folder_preview.tick(delta_seconds);
                }

                // println!("Animation tick: delta_seconds={}", delta_seconds);

                // Forward tick to preview with delta time
                self.preview.update(PreviewMessage::AnimationTick(delta_seconds)).map(Message::Preview)
            }
            Message::ExecuteExternalCommand(index) => {
                let cmd = self.options.lock().external_commands.get(index).cloned();
                if let Some(cmd) = cmd {
                    if !cmd.is_configured() {
                        log::info!("External command F{} not configured", index + 5);
                        let toast = Toast::warning(fl!(crate::LANGUAGE_LOADER, "toast-command-not-configured", key = format!("F{}", index + 5)));
                        return Task::done(Message::ShowToast(toast));
                    }

                    // Get selected item
                    let item: Option<Box<dyn Item>> = if self.view_mode == ViewMode::Tiles {
                        self.tile_grid.get_selected_item().map(|i| i.clone_box())
                    } else {
                        self.file_browser.selected_item().map(|i| i.clone_box())
                    };

                    if let Some(item) = item {
                        if item.is_container() {
                            log::info!("Cannot execute external command on folder");
                            return Task::none();
                        }

                        // Prepare file (copy to temp if virtual)
                        if let Some(file_path) = Options::prepare_file_for_external(item.as_ref()) {
                            if let Some((program, args)) = cmd.build_command(&file_path) {
                                log::info!("Executing: {} {:?}", program, args);
                                match std::process::Command::new(&program).args(&args).spawn() {
                                    Ok(_) => log::info!("Started external command"),
                                    Err(e) => {
                                        let error_msg = format!("{}", e);
                                        let cmd_display = if args.is_empty() {
                                            program.clone()
                                        } else {
                                            format!("{} {}", program, args.join(" "))
                                        };
                                        return Task::done(Message::ShowErrorDialog {
                                            title: fl!(crate::LANGUAGE_LOADER, "error-external-command-title"),
                                            message: fl!(
                                                crate::LANGUAGE_LOADER,
                                                "error-external-command-message",
                                                command = cmd_display,
                                                error = error_msg.clone()
                                            ),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::ShowErrorDialog { title, message } => {
                self.error_dialog = Some(ConfirmationDialog::new(title, message).dialog_type(DialogType::Error).buttons(ButtonSet::Close));
                Task::none()
            }
            Message::CloseErrorDialog => {
                self.error_dialog = None;
                Task::none()
            }
            Message::ShowToast(toast) => {
                self.toasts.push(toast);
                Task::none()
            }
            Message::CloseToast(index) => {
                if index < self.toasts.len() {
                    self.toasts.remove(index);
                }
                Task::none()
            }
            Message::ShowExportDialog => {
                // Export the file that is currently being previewed (shown in status bar)
                // This is the same file whose SAUCE info is displayed
                if let Some(ref path) = self.current_file {
                    // Get buffer type from preview if available
                    let buffer_type = self.preview.get_buffer_type().unwrap_or(icy_engine::BufferType::CP437);

                    // Get the configured export path (or default to documents)
                    let export_dir = self.options.lock().export_path();

                    // Get just the filename from the current file (without extension)
                    let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("export").to_string();

                    // Combine export directory with filename
                    let export_path = export_dir.join(&file_name);

                    // Clone options for the closure
                    let options = self.options.clone();

                    self.export_dialog = Some(
                        ExportDialogState::new(export_path.to_string_lossy().to_string(), buffer_type)
                            .with_default_directory_fn(move || options.lock().export_path()),
                    );
                } else {
                    // Show toast that no file is being previewed
                    self.toasts.push(Toast::warning(fl!(crate::LANGUAGE_LOADER, "export-no-file-selected")));
                }
                Task::none()
            }
            Message::ExportDialog(msg) => {
                if let Some(ref mut dialog) = self.export_dialog {
                    // Get the screen from preview for export
                    let screen = self.preview.get_screen();

                    let result = dialog.update(msg, |state| {
                        if let Some(screen) = screen.as_ref() {
                            state.export_buffer(screen.clone())
                        } else {
                            Err("No screen available for export".to_string())
                        }
                    });

                    match result {
                        Some(true) => {
                            // Export successful
                            let path = dialog.get_full_path();
                            self.toasts
                                .push(Toast::success(fl!(crate::LANGUAGE_LOADER, "export-success", path = path.display().to_string())));
                            self.export_dialog = None;
                        }
                        Some(false) => {
                            // Cancelled
                            self.export_dialog = None;
                        }
                        None => {
                            // Dialog stays open (error or just updating)
                        }
                    }
                }
                Task::none()
            }
            Message::Copy => {
                // Get screen for copy operation
                if let Some(screen_arc) = self.preview.get_screen() {
                    let mut screen = screen_arc.lock();
                    if let Err(err) = icy_engine_gui::copy_selection_to_clipboard(&mut **screen, &*crate::CLIPBOARD_CONTEXT) {
                        log::error!("Failed to copy: {err}");
                    }
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        // Get current theme for passing to components
        let theme = self.theme();

        // Set background color for tile grids from theme
        let bg_color = theme.extended_palette().background.weaker.color;
        self.tile_grid.set_background_color(bg_color);
        self.folder_preview.set_background_color(bg_color);

        // Navigation bar at top
        let current_path = self.file_browser.current_path();

        let nav_bar = self
            .navigation_bar
            .view(current_path.as_ref(), self.history.can_go_back(), self.history.can_go_forward())
            .map(Message::Navigation);

        // Main content depends on view mode
        let content_area: Element<'_, Message> = match self.view_mode {
            ViewMode::List => {
                // File browser on left
                let file_browser = self.file_browser.view(&theme).map(Message::FileBrowser);

                // Preview area on right - show folder preview if folder selected, file preview if file selected
                let preview_area: Element<'_, Message> = if self.folder_preview_path.is_some() {
                    // Show folder contents as thumbnail grid - wrap in focusable
                    let folder_preview = self.folder_preview.view().map(Message::FolderPreview);
                    focus(folder_preview)
                        .on_event(|event, _id| {
                            if let Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event {
                                match key {
                                    Key::Named(Named::ArrowUp) => Some(Message::FolderPreview(TileGridMessage::SelectPrevious)),
                                    Key::Named(Named::ArrowDown) => Some(Message::FolderPreview(TileGridMessage::SelectNext)),
                                    Key::Named(Named::ArrowLeft) => Some(Message::FolderPreview(TileGridMessage::SelectLeft)),
                                    Key::Named(Named::ArrowRight) => Some(Message::FolderPreview(TileGridMessage::SelectRight)),
                                    Key::Named(Named::PageUp) => Some(Message::FolderPreview(TileGridMessage::PageUp)),
                                    Key::Named(Named::PageDown) => Some(Message::FolderPreview(TileGridMessage::PageDown)),
                                    Key::Named(Named::Home) => Some(Message::FolderPreview(TileGridMessage::Home)),
                                    Key::Named(Named::End) => Some(Message::FolderPreview(TileGridMessage::End)),
                                    Key::Named(Named::Enter) => Some(Message::FolderPreview(TileGridMessage::OpenSelected)),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        })
                        .style(list_focus_style)
                        .into()
                } else if self.current_file.is_some() {
                    // Show the terminal preview - wrap in focusable for scroll keys
                    let monitor_settings = self.get_current_monitor_settings();
                    let preview = self.preview.view_with_settings(Some(&monitor_settings)).map(Message::Preview);
                    focus(preview)
                        .on_event(|event, _id| {
                            if let Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event {
                                match key {
                                    Key::Named(Named::ArrowUp) => Some(Message::Preview(PreviewMessage::ScrollViewport(0.0, -50.0))),
                                    Key::Named(Named::ArrowDown) => Some(Message::Preview(PreviewMessage::ScrollViewport(0.0, 50.0))),
                                    Key::Named(Named::PageUp) => Some(Message::Preview(PreviewMessage::ScrollViewport(0.0, -400.0))),
                                    Key::Named(Named::PageDown) => Some(Message::Preview(PreviewMessage::ScrollViewport(0.0, 400.0))),
                                    Key::Named(Named::Home) => Some(Message::Preview(PreviewMessage::ScrollViewportTo(0.0, 0.0))),
                                    Key::Named(Named::End) => Some(Message::Preview(PreviewMessage::ScrollViewportTo(0.0, f32::MAX))),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        })
                        .style(list_focus_style)
                        .into()
                } else {
                    // Show placeholder text
                    let preview_content = text(
                        "Select a file to preview\n\n• Single click to select\n• Double click to open folders/files\n• Use ↑/↓ keys to navigate\n• Press Enter to open",
                    )
                    .size(14)
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.palette().text.scale_alpha(0.5)),
                    });

                    container(preview_content)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill)
                        .style(|theme: &Theme| {
                            let palette = theme.extended_palette();
                            container::Style {
                                background: Some(iced::Background::Color(palette.background.weaker.color)),
                                ..Default::default()
                            }
                        })
                        .into()
                };

                // Main content area (file browser + preview)
                row![file_browser, preview_area].into()
            }
            ViewMode::Tiles => {
                // Full-width tile grid view wrapped in focusable container for keyboard handling
                let tile_grid = self.tile_grid.view().map(Message::TileGrid);

                focus(tile_grid)
                    .on_event(|event, _id| {
                        // Handle keyboard events when focused
                        if let Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event {
                            match key {
                                Key::Named(Named::ArrowUp) => Some(Message::TileGrid(TileGridMessage::SelectPrevious)),
                                Key::Named(Named::ArrowDown) => Some(Message::TileGrid(TileGridMessage::SelectNext)),
                                Key::Named(Named::ArrowLeft) => Some(Message::TileGrid(TileGridMessage::SelectLeft)),
                                Key::Named(Named::ArrowRight) => Some(Message::TileGrid(TileGridMessage::SelectRight)),
                                Key::Named(Named::PageUp) => Some(Message::TileGrid(TileGridMessage::PageUp)),
                                Key::Named(Named::PageDown) => Some(Message::TileGrid(TileGridMessage::PageDown)),
                                Key::Named(Named::Home) => Some(Message::TileGrid(TileGridMessage::Home)),
                                Key::Named(Named::End) => Some(Message::TileGrid(TileGridMessage::End)),
                                Key::Named(Named::Enter) => Some(Message::TileGrid(TileGridMessage::OpenSelected)),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .style(list_focus_style)
                    .into()
            }
        };

        // Status bar at bottom
        let status_info = self.build_status_info();
        let status_bar = StatusBar::view(&status_info, &theme).map(Message::StatusBar);

        // Main layout: nav bar, content, status bar
        let main_layout = column![nav_bar, row![content_area, Space::new().width(1)], status_bar,];

        let base_view: Element<'_, Message> = container(main_layout).width(Length::Fill).height(Length::Fill).into();

        // Wrap with error dialog if active (highest priority)
        if let Some(ref dialog) = self.error_dialog {
            return dialog.clone().view(base_view, |_result| Message::CloseErrorDialog);
        }

        // Wrap with Settings dialog if active (takes priority)
        if let Some(ref dialog) = self.settings_dialog {
            return dialog.view(base_view);
        }

        // Wrap with Export dialog if active
        if let Some(ref dialog) = self.export_dialog {
            let dialog_view = dialog.view(|msg| Message::ExportDialog(msg));
            return icy_engine_gui::ui::modal(base_view, dialog_view, Message::ExportDialog(ExportDialogMessage::Cancel));
        }

        // Wrap with SAUCE dialog if active
        let view_with_sauce = if let Some(ref dialog) = self.sauce_dialog {
            dialog.view(base_view, Message::SauceDialog)
        } else {
            base_view
        };

        // Wrap with toast notifications
        ToastManager::new(view_with_sauce, &self.toasts, Message::CloseToast).into()
    }

    fn build_status_info(&self) -> StatusInfo {
        let view_mode = self.options.lock().view_mode;

        // Use appropriate item count based on view mode
        let item_count = if view_mode == ViewMode::Tiles {
            self.tile_grid.visible_count()
        } else {
            self.file_browser.visible_file_count()
        };

        let mut info = StatusInfo::new()
            .with_item_count(item_count)
            .with_baud_emulation(self.preview.get_baud_emulation())
            .with_viewing_file(self.current_file.is_some())
            .with_buffer_size(self.preview.get_buffer_size())
            .with_content_size(self.preview.get_content_size())
            .with_auto_scroll_enabled(self.preview.is_auto_scroll_enabled());

        // Get selected/hovered item info - from tile grid in tiles mode, from folder preview or file browser otherwise
        if view_mode == ViewMode::Tiles {
            // Use get_status_info which prefers hovered tile over selected
            if let Some((path, _label, is_container, sauce_info)) = self.tile_grid.get_status_info() {
                info.file_name = Some(
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string()),
                );
                // Try to get file size - for local files
                let file_size = std::fs::metadata(&path).ok().map(|m| m.len());
                info.file_size = file_size;
                info.sauce_info = sauce_info;
                info.selected_count = 1;

                // Check if it's an archive
                if is_container {
                    if let Some(FileFormat::Archive(archive_format)) = FileFormat::from_path(&path) {
                        let archive_name = FileFormat::Archive(archive_format).name().to_string();
                        if let Some(size) = file_size {
                            info.archive_info = Some((archive_name, size));
                        }
                    }
                }
            }
        } else if self.folder_preview_path.is_some() {
            // In list mode with folder preview, use folder_preview like tile mode
            // This shows hovered item, or selected item if nothing is hovered
            if let Some((path, _label, is_container, sauce_info)) = self.folder_preview.get_status_info() {
                info.file_name = Some(
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string()),
                );
                let file_size = std::fs::metadata(&path).ok().map(|m| m.len());
                info.file_size = file_size;
                info.sauce_info = sauce_info;
                info.selected_count = 1;

                // Check if it's an archive
                if is_container {
                    if let Some(FileFormat::Archive(archive_format)) = FileFormat::from_path(&path) {
                        let archive_name = FileFormat::Archive(archive_format).name().to_string();
                        if let Some(size) = file_size {
                            info.archive_info = Some((archive_name, size));
                        }
                    }
                }
            }
        } else {
            // In list mode with file preview, use preview sauce info
            info.sauce_info = self.preview.get_sauce_info().cloned();

            if let Some(item) = self.file_browser.selected_item() {
                let path = item.get_file_path();
                info.file_name = Some(
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string()),
                );
                info.file_size = std::fs::metadata(&path).ok().map(|m| m.len());
                info.selected_count = 1;
            }
        }

        info
    }

    /// Get the current theme based on settings
    /// When settings dialog is open, use the temp options (for live preview)
    /// Otherwise use the saved options
    pub fn theme(&self) -> Theme {
        if let Some(ref dialog) = self.settings_dialog {
            dialog.temp_options.lock().monitor_settings.get_theme()
        } else {
            self.options.lock().monitor_settings.get_theme()
        }
    }

    /// Get the current monitor settings
    /// When settings dialog is open, use the temp options (for live preview)
    /// Otherwise use the saved options
    fn get_current_monitor_settings(&self) -> icy_engine_gui::MonitorSettings {
        if let Some(ref dialog) = self.settings_dialog {
            dialog.temp_options.lock().monitor_settings.clone()
        } else {
            self.options.lock().monitor_settings.clone()
        }
    }

    /// Check if animation is needed
    pub fn needs_animation(&self) -> bool {
        self.file_browser.needs_animation()
            || self.preview.needs_animation()
            || (self.view_mode == ViewMode::Tiles && self.tile_grid.needs_animation())
            || (self.view_mode == ViewMode::List && self.folder_preview_path.is_some() && self.folder_preview.needs_animation())
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<Message> {
        // Handle mouse back/forward buttons (Button 4 = Back, Button 5 = Forward)
        if let Event::Mouse(iced::mouse::Event::ButtonPressed(button)) = event {
            match button {
                iced::mouse::Button::Back => return Some(Message::Navigation(NavigationBarMessage::Back)),
                iced::mouse::Button::Forward => return Some(Message::Navigation(NavigationBarMessage::Forward)),
                _ => {}
            }
        }

        // In tile mode, forward mouse events to the tile grid for scroll handling
        if self.view_mode == ViewMode::Tiles {
            // Estimate bounds: x=0, y=nav_bar_height (~40px), width from tile_grid, height from viewport
            // The tile grid takes the full width and height below the nav bar and above the status bar
            let nav_bar_height = 40.0;
            let bounds = Rectangle {
                x: 0.0,
                y: nav_bar_height,
                width: self.tile_grid.get_bounds_width().max(800.0),
                height: self.tile_grid.get_viewport_height().max(400.0),
            };

            // Get cursor position from event if available
            let cursor_position = match event {
                Event::Mouse(iced::mouse::Event::CursorMoved { position }) => Some(*position),
                Event::Mouse(iced::mouse::Event::ButtonPressed(_)) => self.tile_grid.last_cursor_position,
                _ => None,
            };

            // Handle scroll wheel, cursor tracking, and click/double-click
            if self.tile_grid.handle_mouse_event(event, bounds, cursor_position) {
                // Check if a double-click was detected
                if self.tile_grid.take_pending_double_click() {
                    return Some(Message::TileGrid(TileGridMessage::OpenSelected));
                }
                // Check if a click happened (selection changed)
                if let Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) = event {
                    if let Some(index) = self.tile_grid.selected_index {
                        return Some(Message::TileGrid(TileGridMessage::TileClicked(index)));
                    }
                }
                // Tile grid handled the event, request redraw
                return Some(Message::TileGrid(TileGridMessage::AnimationTick));
            }
        }

        // Handle folder preview mouse events in list mode
        if self.view_mode == ViewMode::List && self.folder_preview_path.is_some() {
            // Estimate bounds: folder preview is on the right side after the file browser (300px)
            let nav_bar_height = 40.0;
            let file_browser_width = 300.0;
            let bounds = Rectangle {
                x: file_browser_width,
                y: nav_bar_height,
                width: self.folder_preview.get_bounds_width().max(500.0),
                height: self.folder_preview.get_viewport_height().max(400.0),
            };

            // Get cursor position from event if available
            let cursor_position = match event {
                Event::Mouse(iced::mouse::Event::CursorMoved { position }) => Some(*position),
                Event::Mouse(iced::mouse::Event::ButtonPressed(_)) => self.folder_preview.last_cursor_position,
                _ => None,
            };

            // Handle scroll wheel, cursor tracking, and click/double-click
            if self.folder_preview.handle_mouse_event(event, bounds, cursor_position) {
                // Check if a double-click was detected
                if self.folder_preview.take_pending_double_click() {
                    return Some(Message::FolderPreview(TileGridMessage::OpenSelected));
                }
                // Check if a click happened (selection changed)
                if let Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) = event {
                    if let Some(index) = self.folder_preview.selected_index {
                        return Some(Message::FolderPreview(TileGridMessage::TileClicked(index)));
                    }
                }
                return Some(Message::FolderPreview(TileGridMessage::AnimationTick));
            }
        }

        match event {
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                // Alt+Left = Back, Alt+Right = Forward
                if modifiers.alt() {
                    match key {
                        Key::Named(Named::ArrowLeft) => return Some(Message::Navigation(NavigationBarMessage::Back)),
                        Key::Named(Named::ArrowRight) => return Some(Message::Navigation(NavigationBarMessage::Forward)),
                        _ => {}
                    }
                }

                // Tab cycles through focusable controls using iced's focus system
                if let Key::Named(Named::Tab) = key {
                    if modifiers.shift() {
                        return Some(Message::FocusPrevious);
                    } else {
                        return Some(Message::FocusNext);
                    }
                }

                // Handle Enter key for dialogs
                if let Key::Named(Named::Enter) = key {
                    // Settings dialog: Enter = Save (apply and close)
                    if self.settings_dialog.is_some() {
                        return Some(Message::SettingsDialog(SettingsMessage::Save));
                    }
                    // Sauce dialog: Enter = Close
                    if self.sauce_dialog.is_some() {
                        return Some(Message::SauceDialog(SauceDialogMessage::Close));
                    }
                    // Error dialog: Enter = Close
                    if self.error_dialog.is_some() {
                        return Some(Message::CloseErrorDialog);
                    }
                }

                // Common keys (not view-mode dependent)
                match key {
                    Key::Named(Named::Escape) => return Some(Message::Escape),
                    Key::Named(Named::F2) => {
                        if modifiers.shift() {
                            // Shift+F2: Cycle scroll speed
                            return Some(Message::StatusBar(StatusBarMessage::CycleScrollSpeed));
                        } else {
                            // F2: Toggle auto-scroll
                            return Some(Message::StatusBar(StatusBarMessage::ToggleAutoScroll));
                        }
                    }
                    Key::Named(Named::F3) => {
                        if modifiers.control() {
                            // Ctrl+F3: Set baud rate to max (Off/0)
                            return Some(Message::StatusBar(StatusBarMessage::SetBaudRateOff));
                        } else if modifiers.shift() {
                            // Shift+F3: Cycle baud rate backward
                            return Some(Message::StatusBar(StatusBarMessage::CycleBaudRateBackward));
                        } else {
                            // F3: Cycle baud rate forward
                            return Some(Message::StatusBar(StatusBarMessage::CycleBaudRate));
                        }
                    }
                    Key::Named(Named::F4) => {
                        // F4: Show SAUCE dialog
                        return Some(Message::StatusBar(StatusBarMessage::ShowSauceInfo));
                    }
                    Key::Named(Named::F5) => return Some(Message::ExecuteExternalCommand(0)),
                    Key::Named(Named::F6) => return Some(Message::ExecuteExternalCommand(1)),
                    Key::Named(Named::F7) => return Some(Message::ExecuteExternalCommand(2)),
                    Key::Named(Named::F8) => return Some(Message::ExecuteExternalCommand(3)),
                    Key::Named(Named::F11) => return Some(Message::ToggleFullscreen),
                    _ => {}
                }

                // Ctrl+I: Show export dialog, Ctrl+C: Copy
                if modifiers.control() {
                    if let Key::Character(c) = key {
                        if c.as_str().eq_ignore_ascii_case("i") {
                            return Some(Message::ShowExportDialog);
                        }
                        if c.as_str().eq_ignore_ascii_case("c") {
                            return Some(Message::Copy);
                        }
                    }
                }

                // macOS: Cmd+C for copy
                #[cfg(target_os = "macos")]
                if modifiers.logo() {
                    if let Key::Character(c) = key {
                        if c.as_str().eq_ignore_ascii_case("c") {
                            return Some(Message::Copy);
                        }
                    }
                }

                // View-mode dependent navigation keys
                // Both List and Tiles views handle their own keyboard events via Focus widget
                None
            }
            _ => None,
        }
    }

    /// Create a HistoryPoint for the current state
    fn current_history_point(&self) -> HistoryPoint {
        let provider = if self.navigation_bar.is_16colors_mode {
            ProviderType::Web
        } else {
            ProviderType::File
        };

        let path = self.file_browser.get_display_path();

        let selected_item = self.file_browser.selected_item().map(|item| item.get_label());
        HistoryPoint::new(provider, self.view_mode, path, selected_item)
    }

    /// Navigate to a history point - restores provider, path, mode, and selection
    fn navigate_to_history_point(&mut self, point: &HistoryPoint) {
        // Cancel any ongoing loading operation
        self.preview.cancel_loading();
        self.current_file = None;
        self.folder_preview_path = None;

        // 1. Switch provider if needed
        let is_web = point.provider == ProviderType::Web;
        if self.navigation_bar.is_16colors_mode != is_web {
            self.navigation_bar.set_16colors_mode(is_web);
            if is_web {
                // Switch to 16colors mode - get root items via get_subitems_blocking
                let root: Box<dyn crate::Item> = Box::new(SixteenColorsRoot::new());
                let cancel_token = CancellationToken::new();
                if let Some(items) = root.get_subitems_blocking(&cancel_token) {
                    self.file_browser.set_16colors_mode(items);
                }
            }
        }

        // 2. Navigate to the path
        if is_web {
            // For web, use the path as web path
            self.file_browser.navigate_to_web_path(&point.path);
        } else {
            // For file, navigate to filesystem path
            self.file_browser.navigate_to(PathBuf::from(&point.path));
        }

        // 3. Update navigation bar
        self.navigation_bar.set_path_input(self.file_browser.get_display_path());

        // 4. Switch view mode if needed
        if self.view_mode != point.view_mode {
            self.view_mode = point.view_mode;
            self.navigation_bar.set_view_mode(point.view_mode);
            {
                let mut locked = self.options.lock();
                locked.view_mode = point.view_mode;
            }
        }

        // 5. Update tile grid if in tile mode
        if self.view_mode == ViewMode::Tiles {
            let items = self.file_browser.get_items();
            self.tile_grid.set_items_from_items(items);
        }

        // 6. Select the item if specified
        if let Some(ref item_name) = point.selected_item {
            if self.view_mode == ViewMode::Tiles {
                self.tile_grid.select_by_label(item_name);
            } else {
                self.file_browser.select_by_label(item_name);
            }
        }
    }
}

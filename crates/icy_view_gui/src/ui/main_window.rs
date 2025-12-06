use std::{path::PathBuf, sync::Arc, time::Instant};

use i18n_embed_fl::fl;
use iced::{
    Element, Event, Length, Task, Theme,
    keyboard::{Key, key::Named},
    widget::{Space, column, container, image as iced_image, mouse_area, row, text},
};
use icy_engine_gui::{
    ButtonSet, ConfirmationDialog, DialogType, Toast, ToastManager,
    command_handler,
    ui::{ExportDialogMessage, ExportDialogState},
    version_helper::replace_version_marker,
};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::{
    DEFAULT_TITLE, Item, VERSION,
    commands::{cmd, create_icy_view_commands},
    items::{ProviderType, SixteenColorsProvider, SixteenColorsRoot},
};
use icy_engine::formats::FileFormat;

use super::{
    FileBrowser, FileBrowserMessage, FileListToolbar, FileListToolbarMessage, FileListViewMessage, FilterPopup, FilterPopupMessage, HistoryPoint,
    NavigationBar, NavigationBarMessage, NavigationHistory, Options, PreviewMessage, PreviewView, SauceLoader, SauceRequest, SauceResult, StatusBar,
    StatusBarMessage, StatusInfo, TileGridMessage, TileGridView,
    dialogs::about_dialog::AboutDialog,
    dialogs::help_dialog::HelpDialog,
    dialogs::sauce_dialog::{SauceDialog, SauceDialogMessage},
    dialogs::settings_dialog::{SettingsDialogState, SettingsMessage},
    file_list_toolbar::TOOLBAR_HOVER_ZONE_WIDTH,
    focus::{focus, list_focus_style},
    options::{SortOrder, ViewMode},
};

// Include the welcome logo at compile time
const WELCOME_LOGO: &[u8] = include_bytes!("../../data/welcome.xb");

// Command handler for MainWindow keyboard shortcuts
command_handler!(MainWindowCommands, create_icy_view_commands(), => Message {
    // View
    cmd::VIEW_FULLSCREEN => Message::ToggleFullscreen,
    // Dialogs
    cmd::HELP_SHOW => Message::ShowHelp,
    cmd::HELP_ABOUT => Message::ShowAbout,
    cmd::DIALOG_EXPORT => Message::ShowExportDialog,
    cmd::DIALOG_FILTER => Message::ToggleFilterPopup,
    // Edit
    cmd::EDIT_COPY => Message::Copy,
    // External commands
    cmd::EXTERNAL_COMMAND_0 => Message::ExecuteExternalCommand(0),
    cmd::EXTERNAL_COMMAND_1 => Message::ExecuteExternalCommand(1),
    cmd::EXTERNAL_COMMAND_2 => Message::ExecuteExternalCommand(2),
    cmd::EXTERNAL_COMMAND_3 => Message::ExecuteExternalCommand(3),
});

/// Static welcome logo image handle
static WELCOME_IMAGE: Lazy<iced_image::Handle> = Lazy::new(|| {
    use icy_engine::{Rectangle, RenderOptions, Selection, TextBuffer, TextPane};
    use icy_parser_core::MusicOption;
    use std::path::Path;

    // Load the XBin file
    let mut buffer = TextBuffer::from_bytes(Path::new("welcome.xb"), true, WELCOME_LOGO, Some(MusicOption::Off), None).expect("Failed to load welcome logo");

    // Replace version marker
    replace_version_marker(&mut buffer, &VERSION, None);

    // Render to RGBA
    let rect = Selection::from(Rectangle::from(0, 0, buffer.get_width(), buffer.get_height()));
    let opts = RenderOptions {
        rect,
        blink_on: true,
        selection: None,
        selection_fg: None,
        selection_bg: None,
        override_scan_lines: Some(false),
    };

    let (size, rgba) = buffer.render_to_rgba(&opts, false);
    iced_image::Handle::from_rgba(size.width as u32, size.height as u32, rgba)
});

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
    /// File list toolbar messages
    FileListToolbar(FileListToolbarMessage),
    /// Filter popup messages
    FilterPopup(FilterPopupMessage),
    /// Toggle filter popup visibility
    ToggleFilterPopup,
    /// Toggle fullscreen mode
    ToggleFullscreen,
    /// Escape key pressed
    Escape,
    /// Data was loaded for preview (path for display, data for content)
    DataLoaded(String, Vec<u8>),
    /// Data loading failed for preview
    DataLoadError(String, String),
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
    /// Skip to next file in shuffle mode
    ShuffleNext,
    /// Show help dialog
    ShowHelp,
    /// Close help dialog
    CloseHelp,
    /// Show about dialog
    ShowAbout,
    /// Close about dialog
    CloseAbout,
    /// Open a hyperlink
    OpenLink(String),
    /// No-op message (for ignored events)
    None,
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
    /// Status bar
    status_bar: StatusBar,
    /// Navigation history
    pub history: NavigationHistory,
    /// File list toolbar
    pub file_list_toolbar: FileListToolbar,
    /// Filter popup
    pub filter_popup: FilterPopup,
    /// Options
    pub options: Arc<Mutex<Options>>,
    /// Command handler for keyboard shortcuts
    commands: MainWindowCommands,
    /// Fullscreen mode
    pub fullscreen: bool,
    /// Currently loaded file for preview
    pub current_file: Option<String>,
    /// Preview view for ANSI files
    pub preview: PreviewView,
    /// Tile grid view for thumbnail preview
    pub tile_grid: TileGridView,
    /// Folder preview tile grid (for list mode when folder is selected)
    pub folder_preview: TileGridView,
    /// Path of the folder being previewed (None means show file preview)
    pub folder_preview_path: Option<String>,
    /// Last animation tick time for delta calculation
    last_tick: Instant,
    /// SAUCE dialog (shown as modal when Some)
    sauce_dialog: Option<SauceDialog>,
    /// Help dialog (shown as modal when Some)
    help_dialog: Option<HelpDialog>,
    /// About dialog (shown as modal when Some)
    about_dialog: Option<AboutDialog>,
    /// Settings dialog (shown as modal when Some)
    settings_dialog: Option<SettingsDialogState>,
    /// Error dialog (shown as modal when Some)
    error_dialog: Option<ConfirmationDialog>,
    /// Export dialog (shown as modal when Some)
    export_dialog: Option<ExportDialogState>,
    /// Toast notifications
    toasts: Vec<Toast>,
    /// SAUCE loader for async loading of SAUCE info
    sauce_loader: Option<SauceLoader>,
    /// Receiver for SAUCE load results
    sauce_rx: Option<tokio::sync::mpsc::UnboundedReceiver<SauceResult>>,
    /// Shuffle mode state
    shuffle_mode: super::ShuffleMode,
}
impl MainWindow {
    /// Creates a new MainWindow.
    /// Returns (Self, Option<Message>) where the second value is an initial message to process
    /// (e.g., to load a file preview when started with a file path)
    pub fn new(id: usize, initial_path: Option<PathBuf>, options: Arc<Mutex<Options>>, auto_scroll: bool, bps: Option<u32>) -> (Self, Option<Message>) {
        let mut opts = Options::default();
        let view_mode;
        let sort_order;
        {
            let locked = options.lock();
            opts.auto_scroll_enabled = locked.auto_scroll_enabled;
            opts.scroll_speed = locked.scroll_speed.clone();
            opts.show_settings = locked.show_settings;
            view_mode = locked.view_mode;
            sort_order = locked.sort_order;
        }
        if auto_scroll {
            opts.auto_scroll_enabled = true;
        }

        let (mut file_browser, file_to_preview) = FileBrowser::new(initial_path.clone());
        // Apply saved sort order
        file_browser.set_sort_order(sort_order);

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

        let mut navigation_bar: NavigationBar = NavigationBar::new();
        // Initialize path input with current path (normalized to forward slashes)
        if let Some(path) = file_browser.current_path() {
            navigation_bar.set_path_input(path.to_string_lossy().replace('\\', "/"));
        }

        let mut file_list_toolbar = FileListToolbar::new();
        // Check if we can go up (not at filesystem root)
        if let Some(path) = file_browser.current_path() {
            file_list_toolbar.set_can_go_up(path.parent().is_some());
        }
        // Apply initial sauce_mode to file browser
        file_browser.set_sauce_mode(opts.sauce_mode);

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
            // Read the file data and create a DataLoaded message (convert PathBuf to String)
            let file_path_str = file_path.to_string_lossy().replace('\\', "/");
            let msg = std::fs::read(file_path).ok().map(|data| Message::DataLoaded(file_path_str.clone(), data));
            (Some(file_path_str), title, msg)
        } else {
            (None, DEFAULT_TITLE.clone(), None)
        };

        // Create SAUCE loader
        let (sauce_loader, sauce_rx, sauce_cache) = SauceLoader::spawn();

        // Share the sauce cache with file browser
        file_browser.set_sauce_cache(sauce_cache.clone());

        (
            Self {
                id,
                title,
                file_browser,
                navigation_bar,
                status_bar: StatusBar::new(),
                history,
                file_list_toolbar,
                filter_popup: FilterPopup::new(),
                options,
                commands: MainWindowCommands::new(),
                fullscreen: false,
                current_file,
                preview,
                tile_grid,
                folder_preview: TileGridView::new(),
                folder_preview_path: None,
                last_tick: Instant::now(),
                sauce_dialog: None,
                help_dialog: None,
                about_dialog: None,
                settings_dialog: None,
                error_dialog: None,
                export_dialog: None,
                toasts: Vec::new(),
                sauce_loader: Some(sauce_loader),
                sauce_rx: Some(sauce_rx),
                shuffle_mode: super::ShuffleMode::new(),
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
                    // Reset SAUCE loader for new directory
                    self.reset_sauce_loader_for_navigation();
                    // Record the navigation as a new history point
                    let point = self.current_history_point();
                    self.history.navigate_to(point);
                    // Update path input to show current display path
                    self.navigation_bar.set_path_input(new_display_path.clone());
                    // Update can_go_up state for toolbar
                    self.file_list_toolbar.set_can_go_up(self.file_browser.can_go_parent());
                    // Also update tile grid when directory changes
                    if self.view_mode() == ViewMode::Tiles {
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
                                format!("{}/{}", current.to_string_lossy().replace('\\', "/"), new_selection.replace('\\', "/"))
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
                                format!("{}/{}", current.to_string_lossy().replace('\\', "/"), new_selection.replace('\\', "/"))
                            } else {
                                new_selection.clone()
                            };
                            self.current_file = Some(full_path.clone());
                            // Extract filename from path
                            self.title = new_selection.split('/').last().map(|s| s.to_string()).unwrap_or_else(|| DEFAULT_TITLE.clone());

                            // Read the data from the item (works for both local and virtual files)
                            if let Some(item_mut) = self.file_browser.selected_item_mut() {
                                if let Some(data) = item_mut.read_data_blocking() {
                                    return Task::done(Message::DataLoaded(full_path, data));
                                } else {
                                    return Task::done(Message::DataLoadError(full_path.clone(), fl!(crate::LANGUAGE_LOADER, "error-read-file-data")));
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
                        // Reset SAUCE loader after navigation (so new files are available)
                        self.reset_sauce_loader_for_navigation();
                        // Update path input with the new display path
                        let display_path = self.file_browser.get_display_path();
                        self.navigation_bar.set_path_input(display_path);
                        // Update can_go_up state for toolbar
                        self.file_list_toolbar.set_can_go_up(self.file_browser.can_go_parent());
                        if self.view_mode() == ViewMode::Tiles {
                            let items = self.file_browser.get_items();
                            self.tile_grid.set_items_from_items(items);
                        }
                    }
                    NavigationBarMessage::Refresh => {
                        // Cancel any ongoing loading operation on refresh
                        self.preview.cancel_loading();

                        self.file_browser.update(FileBrowserMessage::Refresh);
                        // Reset SAUCE loader after refresh (directory contents may have changed)
                        self.reset_sauce_loader_for_navigation();
                        // Update can_go_up state for toolbar
                        self.file_list_toolbar.set_can_go_up(self.file_browser.can_go_parent());
                        if self.view_mode() == ViewMode::Tiles {
                            let items = self.file_browser.get_items();
                            self.tile_grid.set_items_from_items(items);
                        }
                    }
                    NavigationBarMessage::OpenFilter => {
                        // Toggle filter popup
                        if self.filter_popup.is_visible() {
                            self.filter_popup.hide();
                            return Task::none();
                        } else {
                            self.filter_popup.show();
                            return self.filter_popup.focus_input();
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
                            self.navigation_bar.set_path_input(home.to_string_lossy().replace('\\', "/"));
                        }
                        // Reset SAUCE loader after navigation
                        self.reset_sauce_loader_for_navigation();
                        // Update can_go_up state for toolbar
                        self.file_list_toolbar.set_can_go_up(self.file_browser.can_go_parent());
                        if self.view_mode() == ViewMode::Tiles {
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
                                // Reset SAUCE loader after navigation
                                self.reset_sauce_loader_for_navigation();
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
                                    // Reset SAUCE loader after navigation
                                    self.reset_sauce_loader_for_navigation();
                                    self.navigation_bar.set_path_input(path.to_string_lossy().replace('\\', "/"));
                                    let point = self.current_history_point();
                                    self.history.navigate_to(point);
                                } else if path.is_file() {
                                    // Check if it's an archive (Scenario 2)
                                    if let Some(FileFormat::Archive(_)) = FileFormat::from_path(&path) {
                                        // Scenario 2: Archive - treat as directory, navigate into it
                                        self.navigation_bar.set_16colors_mode(false);
                                        self.file_browser.navigate_to(path.clone());
                                        // Reset SAUCE loader after navigation
                                        self.reset_sauce_loader_for_navigation();
                                        self.navigation_bar.set_path_input(path.to_string_lossy().replace('\\', "/"));
                                        let point = self.current_history_point();
                                        self.history.navigate_to(point);
                                    } else {
                                        // Scenario 3: Regular file - navigate to parent and select the file
                                        if let Some(parent) = path.parent() {
                                            self.navigation_bar.set_16colors_mode(false);
                                            self.file_browser.navigate_to(parent.to_path_buf());
                                            // Reset SAUCE loader after navigation
                                            self.reset_sauce_loader_for_navigation();
                                            self.navigation_bar.set_path_input(parent.to_string_lossy().replace('\\', "/"));

                                            // Select the file by its name
                                            if let Some(file_name) = path.file_name() {
                                                self.file_browser.select_by_label(&file_name.to_string_lossy());
                                            }

                                            // Load preview for the selected file
                                            let path_str = path.to_string_lossy().replace('\\', "/");
                                            self.current_file = Some(path_str.clone());
                                            self.title = path
                                                .file_name()
                                                .map(|n| n.to_string_lossy().to_string())
                                                .unwrap_or_else(|| DEFAULT_TITLE.clone());
                                            match std::fs::read(&path) {
                                                Ok(data) => {
                                                    return Task::done(Message::DataLoaded(path_str, data));
                                                }
                                                Err(e) => {
                                                    log::error!("Failed to read file {:?}: {}", path, e);
                                                }
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
                        // Update can_go_up state for toolbar after any navigation
                        self.file_list_toolbar.set_can_go_up(self.file_browser.can_go_parent());
                        if self.view_mode() == ViewMode::Tiles {
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
                        if let Some((path_buf, _label, is_container)) = self.tile_grid.get_selected_info() {
                            if !is_container {
                                // path_buf is already a String
                                let path = path_buf.replace('\\', "/");
                                self.current_file = Some(path.clone());
                                self.title = path_buf.split('/').last().unwrap_or(&path_buf).to_string();

                                // Read data for preview from file (use PathBuf for fs operation)
                                match std::fs::read(&path_buf) {
                                    Ok(data) => {
                                        return Task::done(Message::DataLoaded(path, data));
                                    }
                                    Err(e) => {
                                        log::error!("Failed to read file {:?}: {}", path_buf, e);
                                    }
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
                                        item_path.clone()
                                    } else {
                                        format!("{}/{}", current_path, item_path)
                                    };
                                    self.file_browser.navigate_to_web_path(&new_path);
                                    // Update navigation bar with display path (includes leading /)
                                    self.navigation_bar.set_path_input(self.file_browser.get_display_path());
                                } else {
                                    // Build full path from current browser path + item name
                                    let current_path = self.file_browser.get_display_path();
                                    let item_path_str = item_path.replace('\\', "/");
                                    let full_path = format!("{}/{}", current_path.replace('\\', "/"), item_path_str);
                                    self.file_browser.navigate_to(PathBuf::from(&full_path));
                                    // Update navigation bar with normalized forward slashes
                                    self.navigation_bar.set_path_input(full_path);
                                }
                                // Refresh tile grid with new items
                                let items = self.file_browser.get_items();
                                self.tile_grid.set_items_from_items(items);
                                // Update can_go_up state for toolbar
                                self.file_list_toolbar.set_can_go_up(self.file_browser.can_go_parent());
                                // Record navigation
                                let point = self.current_history_point();
                                self.history.navigate_to(point);
                            } else {
                                // For files, switch to list view and select the item
                                self.set_view_mode(ViewMode::List);
                                // Build full path for the file
                                let current_path = self.file_browser.get_display_path();
                                let item_path_str = item_path.replace('\\', "/");
                                let full_path = format!("{}/{}", current_path.replace('\\', "/"), item_path_str);
                                // Select the item in the file browser
                                self.file_browser.select_by_path(&PathBuf::from(&item_path));
                                // Load preview
                                self.current_file = Some(full_path.clone());
                                self.title = item_path.split('/').last().unwrap_or(&item_path).to_string();
                                // Read data - prefer using Item for virtual files, fall back to fs::read
                                if let Some(item) = self.tile_grid.get_item_at(*index) {
                                    if let Some(data) = item.read_data_blocking() {
                                        return Task::done(Message::DataLoaded(full_path.clone(), data));
                                    }
                                }
                                match std::fs::read(PathBuf::from(&full_path)) {
                                    Ok(data) => {
                                        return Task::done(Message::DataLoaded(full_path, data));
                                    }
                                    Err(e) => {
                                        return Task::done(Message::DataLoadError(
                                            full_path,
                                            fl!(crate::LANGUAGE_LOADER, "error-read-file", error = e.to_string()),
                                        ));
                                    }
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
                                            item_path.clone()
                                        } else {
                                            format!("{}/{}", current_path, item_path)
                                        };
                                        self.file_browser.navigate_to_web_path(&new_path);
                                        // Update navigation bar with display path (includes leading /)
                                        self.navigation_bar.set_path_input(self.file_browser.get_display_path());
                                    } else {
                                        // Build full path from current browser path + item name
                                        let current_path = self.file_browser.get_display_path();
                                        let item_path_str = item_path.replace('\\', "/");
                                        let full_path = format!("{}/{}", current_path.replace('\\', "/"), item_path_str);
                                        self.file_browser.navigate_to(PathBuf::from(&full_path));
                                        // Update navigation bar with normalized forward slashes
                                        self.navigation_bar.set_path_input(full_path);
                                    }
                                    // Refresh tile grid with new items
                                    let items = self.file_browser.get_items();
                                    self.tile_grid.set_items_from_items(items);
                                    // Update can_go_up state for toolbar
                                    self.file_list_toolbar.set_can_go_up(self.file_browser.can_go_parent());
                                    // Record navigation
                                    let point = self.current_history_point();
                                    self.history.navigate_to(point);
                                } else {
                                    // For files, switch to list view and select the item
                                    self.set_view_mode(ViewMode::List);
                                    // Build full path for the file
                                    let current_path = self.file_browser.get_display_path();
                                    let item_path_str = item_path.replace('\\', "/");
                                    let full_path = format!("{}/{}", current_path.replace('\\', "/"), item_path_str);
                                    // Select the item in the file browser
                                    self.file_browser.select_by_path(&PathBuf::from(&item_path));
                                    // Load preview
                                    self.current_file = Some(full_path.clone());
                                    self.title = item_path.split('/').last().unwrap_or(&item_path).to_string();
                                    // Read data - prefer using Item for virtual files, fall back to fs::read
                                    if let Some(item) = self.tile_grid.get_selected_item() {
                                        if let Some(data) = item.read_data_blocking() {
                                            return Task::done(Message::DataLoaded(full_path.clone(), data));
                                        }
                                    }
                                    match std::fs::read(PathBuf::from(&full_path)) {
                                        Ok(data) => {
                                            return Task::done(Message::DataLoaded(full_path, data));
                                        }
                                        Err(e) => {
                                            return Task::done(Message::DataLoadError(
                                                full_path.clone(),
                                                fl!(crate::LANGUAGE_LOADER, "error-read-file", error = e.to_string()),
                                            ));
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
                // First check if shuffle mode is active - exit it
                if self.shuffle_mode.is_active {
                    self.shuffle_mode.stop();
                    return Task::none();
                }
                // Close dialogs in priority order
                if self.error_dialog.is_some() {
                    self.error_dialog = None;
                } else if self.export_dialog.is_some() {
                    self.export_dialog = None;
                } else if self.about_dialog.is_some() {
                    self.about_dialog = None;
                } else if self.help_dialog.is_some() {
                    self.help_dialog = None;
                } else if self.settings_dialog.is_some() {
                    self.settings_dialog = None;
                } else if self.sauce_dialog.is_some() {
                    self.sauce_dialog = None;
                } else if self.filter_popup.is_visible() {
                    self.filter_popup.hide();
                } else if self.fullscreen {
                    self.fullscreen = false;
                }
                Task::none()
            }
            Message::DataLoaded(path, data) => {
                // Reset timer when loading new file to prevent animation jumps
                self.last_tick = Instant::now();

                // If in shuffle mode, extract SAUCE info for overlay
                if self.shuffle_mode.is_active {
                    if let Some(sauce) = icy_sauce::SauceRecord::from_bytes(&data).ok().flatten() {
                        let comments: Vec<String> = sauce.comments().iter().map(|s| s.to_string()).collect();
                        self.shuffle_mode.set_sauce_info(
                            Some(sauce.title().to_string()),
                            Some(sauce.author().to_string()),
                            Some(sauce.group().to_string()),
                            comments,
                        );
                    }
                }

                // Load data in preview (convert String path to PathBuf for preview API)
                self.preview.load_data(PathBuf::from(path), data).map(Message::Preview)
            }
            Message::DataLoadError(path, message) => {
                // Set preview to error state
                log::error!("Failed to load file {:?}: {}", path, message);
                self.preview.set_error(PathBuf::from(path), message);
                Task::none()
            }
            Message::Preview(msg) => {
                // Check for reset timer message
                if matches!(msg, PreviewMessage::ResetAnimationTimer) {
                    self.last_tick = Instant::now();
                    return Task::none();
                }
                // Check if this is a zoom change message
                let is_zoom_change = matches!(
                    &msg,
                    PreviewMessage::TerminalMessage(icy_engine_gui::Message::Zoom(_)) | PreviewMessage::Zoom(_)
                );
                let result = self.preview.update(msg).map(Message::Preview);
                // Sync scaling_mode from preview to options after zoom changes
                if is_zoom_change {
                    self.options.lock().monitor_settings.scaling_mode = self.preview.monitor_settings.scaling_mode.clone();
                }
                result
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
                    StatusBarMessage::CycleScrollSpeedBackward => {
                        // Cycle scroll speed backward: Slow -> Fast -> Medium -> Slow
                        use super::options::ScrollSpeed;
                        let current = self.preview.get_scroll_speed();
                        let next = match current {
                            ScrollSpeed::Slow => ScrollSpeed::Fast,
                            ScrollSpeed::Medium => ScrollSpeed::Slow,
                            ScrollSpeed::Fast => ScrollSpeed::Medium,
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
                            match self.view_mode() {
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
                        let sauce_info = if self.view_mode() == ViewMode::Tiles {
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
                                let item_path_str = item_path.replace('\\', "/");
                                let full_path = format!("{}/{}", preview_folder.replace('\\', "/"), item_path_str);
                                self.current_file = Some(full_path.clone());
                                self.title = label;

                                // Load preview data
                                if let Some(item) = self.folder_preview.get_item_at(*index) {
                                    if let Some(data) = item.read_data_blocking() {
                                        return Task::done(Message::DataLoaded(full_path.clone(), data));
                                    }
                                }
                                // Fallback to filesystem read
                                match std::fs::read(PathBuf::from(&full_path)) {
                                    Ok(data) => {
                                        return Task::done(Message::DataLoaded(full_path.clone(), data));
                                    }
                                    Err(e) => {
                                        return Task::done(Message::DataLoadError(
                                            full_path,
                                            fl!(crate::LANGUAGE_LOADER, "error-read-file", error = e.to_string()),
                                        ));
                                    }
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
                                    let preview_path_str = &preview_folder;
                                    let item_path_str = item_path.replace('\\', "/");

                                    self.file_browser.navigate_to_web_path(preview_path_str);
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
                                        self.file_browser.select_by_path(&PathBuf::from(&item_path));
                                        let item_path_str = item_path.replace('\\', "/");
                                        let full_path = format!("{}/{}", preview_folder.replace('\\', "/"), item_path_str);
                                        self.current_file = Some(full_path.clone());
                                        self.title = item_path.split('/').last().unwrap_or(&item_path).to_string();
                                        // Read data from the item we captured before navigation
                                        if let Some(item) = item_for_data {
                                            if let Some(data) = item.read_data_blocking() {
                                                return Task::done(Message::DataLoaded(full_path, data));
                                            }
                                        }
                                    }
                                } else {
                                    // File mode - use PathBuf operations
                                    self.file_browser.navigate_to(PathBuf::from(&preview_folder));
                                    self.navigation_bar.set_path_input(preview_folder.clone());

                                    // Now construct full path for the item inside the previewed folder
                                    let item_path_str = item_path.replace('\\', "/");
                                    let full_path_str = format!("{}/{}", preview_folder.replace('\\', "/"), item_path_str);

                                    if is_container {
                                        // Navigate into the subfolder and select first item
                                        self.file_browser.navigate_to(PathBuf::from(&full_path_str));
                                        self.navigation_bar.set_path_input(full_path_str);
                                        self.file_browser.list_view.selected_index = Some(0);
                                        // Record navigation
                                        let point = self.current_history_point();
                                        self.history.navigate_to(point);
                                    } else {
                                        // Select the file in the browser and preview it
                                        self.file_browser.select_by_path(&PathBuf::from(&item_path));
                                        self.current_file = Some(full_path_str.clone());
                                        self.title = item_path.split('/').last().unwrap_or(&item_path).to_string();
                                        // Read data from the item we captured before navigation
                                        if let Some(item) = item_for_data {
                                            if let Some(data) = item.read_data_blocking() {
                                                return Task::done(Message::DataLoaded(full_path_str, data));
                                            }
                                        }
                                        // Fallback to filesystem read
                                        match std::fs::read(PathBuf::from(&full_path_str)) {
                                            Ok(data) => {
                                                return Task::done(Message::DataLoaded(full_path_str.clone(), data));
                                            }
                                            Err(e) => {
                                                return Task::done(Message::DataLoadError(
                                                    full_path_str,
                                                    fl!(crate::LANGUAGE_LOADER, "error-read-file", error = e.to_string()),
                                                ));
                                            }
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
                                    let preview_path_str = preview_folder;
                                    let item_path_str = item_path.clone();

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
                                        self.file_browser.select_by_path(&PathBuf::from(&item_path));
                                        self.current_file = Some(format!("{}/{}", preview_path_str, item_path_str));
                                        self.title = item_path_str.split('/').last().unwrap_or(&item_path_str).to_string();
                                        // Read data from the item we captured before navigation
                                        if let Some(item) = item_for_data {
                                            if let Some(data) = item.read_data_blocking() {
                                                return Task::done(Message::DataLoaded(format!("{}/{}", preview_path_str, item_path_str), data));
                                            }
                                        }
                                    }
                                } else {
                                    // File mode - use PathBuf operations
                                    self.file_browser.navigate_to(PathBuf::from(&preview_folder));
                                    self.navigation_bar.set_path_input(preview_folder.clone());

                                    // Now construct full path for the item inside the previewed folder
                                    let full_path = format!("{}/{}", preview_folder, &item_path);

                                    if is_container {
                                        // Navigate into the subfolder and select first item
                                        self.file_browser.navigate_to(PathBuf::from(&full_path));
                                        self.navigation_bar.set_path_input(full_path.clone());
                                        self.file_browser.list_view.selected_index = Some(0);
                                        // Record navigation
                                        let point = self.current_history_point();
                                        self.history.navigate_to(point);
                                    } else {
                                        // Select the file in the browser and preview it
                                        self.file_browser.select_by_path(&PathBuf::from(&item_path));
                                        self.current_file = Some(full_path.clone());
                                        self.title = item_path.split('/').last().unwrap_or(&item_path).to_string();
                                        // Read data from the item we captured before navigation
                                        if let Some(item) = item_for_data {
                                            if let Some(data) = item.read_data_blocking() {
                                                return Task::done(Message::DataLoaded(full_path, data));
                                            }
                                        }
                                        // Fallback to filesystem read
                                        match std::fs::read(&full_path) {
                                            Ok(data) => {
                                                return Task::done(Message::DataLoaded(full_path, data));
                                            }
                                            Err(e) => {
                                                return Task::done(Message::DataLoadError(
                                                    full_path,
                                                    fl!(crate::LANGUAGE_LOADER, "error-read-file", error = e.to_string()),
                                                ));
                                            }
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
            Message::FileListToolbar(msg) => {
                match msg {
                    FileListToolbarMessage::Up => {
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
                        // Update can_go_up state
                        if let Some(path) = self.file_browser.current_path() {
                            self.file_list_toolbar.set_can_go_up(path.parent().is_some());
                        }
                        if self.view_mode() == ViewMode::Tiles {
                            let items = self.file_browser.get_items();
                            self.tile_grid.set_items_from_items(items);
                        }
                    }
                    FileListToolbarMessage::ToggleViewMode => {
                        // Cancel any ongoing loading operation when switching modes
                        self.preview.cancel_loading();
                        self.current_file = None;
                        self.folder_preview_path = None;

                        self.set_view_mode(self.view_mode().toggle());
                        // When switching to tile mode, populate the tile grid with current items
                        if self.view_mode() == ViewMode::Tiles {
                            let items = self.file_browser.get_items();
                            self.tile_grid.set_items_from_items(items);
                            // Apply current filter to tile grid
                            let filter = self.filter_popup.get_filter().to_string();
                            if !filter.is_empty() {
                                self.tile_grid.apply_filter(&filter);
                            }
                            // Reset toolbar for slide-in behavior
                            self.file_list_toolbar.reset_for_tiles_mode();
                        }
                    }
                    FileListToolbarMessage::CycleSortOrder => {
                        let new_order = self.sort_order().next();
                        self.set_sort_order(new_order);
                        // Refresh the file browser to apply new sort order
                        self.file_browser.set_sort_order(new_order);
                        // If in tile mode, refresh tile grid
                        if self.view_mode() == ViewMode::Tiles {
                            let items = self.file_browser.get_items();
                            self.tile_grid.set_items_from_items(items);
                            let filter = self.filter_popup.get_filter().to_string();
                            if !filter.is_empty() {
                                self.tile_grid.apply_filter(&filter);
                            }
                        }
                    }
                    FileListToolbarMessage::ToggleSauceMode => {
                        let new_mode = !self.sauce_mode();
                        self.set_sauce_mode(new_mode);
                        self.file_browser.set_sauce_mode(new_mode);

                        if new_mode {
                            // SAUCE mode enabled - start loading SAUCE info for visible items
                            self.start_sauce_loading();
                        } else {
                            // SAUCE mode disabled - cancel pending loads
                            if let Some(ref loader) = self.sauce_loader {
                                loader.cancel_all();
                            }
                            // Reset the loader for next time
                            if let Some(ref mut loader) = self.sauce_loader {
                                loader.reset();
                            }
                        }
                    }
                    FileListToolbarMessage::MouseEntered => {
                        self.file_list_toolbar.on_mouse_enter();
                    }
                    FileListToolbarMessage::MouseLeft => {
                        self.file_list_toolbar.on_mouse_leave();
                    }
                    FileListToolbarMessage::HideTick => {
                        // Check if toolbar should auto-hide
                        self.file_list_toolbar.check_auto_hide();
                    }
                    FileListToolbarMessage::StartShuffleMode => {
                        // Collect indices of all displayable files from current container
                        let indices = self.collect_shuffle_indices();
                        if !indices.is_empty() {
                            self.shuffle_mode.start(indices);
                            // Enable fullscreen for better experience
                            if !self.fullscreen {
                                self.fullscreen = true;
                            }
                            // Enable auto-scroll for shuffle mode
                            self.preview.enable_auto_scroll();
                            // Load the first item
                            if let Some(index) = self.shuffle_mode.current_item_index() {
                                return self.load_shuffle_item(index);
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::FilterPopup(msg) => {
                if let Some(filter) = self.filter_popup.update(msg) {
                    // Remember old selection before applying filter
                    let old_selection = self.file_browser.selected_item().map(|i| i.get_file_path());

                    // Apply filter to file browser and tile grids
                    self.file_browser.update(FileBrowserMessage::FilterChanged(filter.clone()));
                    self.tile_grid.apply_filter(&filter);
                    self.folder_preview.apply_filter(&filter);

                    // Check if selection changed due to filtering
                    let new_selection = self.file_browser.selected_item().map(|i| i.get_file_path());
                    if old_selection != new_selection {
                        // Selection changed - update preview
                        self.preview.cancel_loading();

                        if new_selection.is_none() {
                            self.current_file = None;
                            self.folder_preview_path = None;
                            self.title = DEFAULT_TITLE.clone();
                        } else if let Some(item) = self.file_browser.selected_item() {
                            let item_path = item.get_file_path();
                            let is_container = item.is_container();

                            if is_container {
                                // For folders, show thumbnail preview
                                self.current_file = None;
                                let full_path = if let Some(current) = self.file_browser.current_path() {
                                    format!("{}/{}", current.to_string_lossy().replace('\\', "/"), item_path.replace('\\', "/"))
                                } else {
                                    item_path.clone()
                                };
                                self.folder_preview_path = Some(full_path);
                                self.folder_preview.load_subitems_async(item.clone_box());
                            } else {
                                // For files, show file preview
                                self.folder_preview_path = None;
                                let full_path = if let Some(current) = self.file_browser.current_path() {
                                    format!("{}/{}", current.to_string_lossy().replace('\\', "/"), item_path.replace('\\', "/"))
                                } else {
                                    item_path.clone()
                                };
                                self.current_file = Some(full_path.clone());
                                self.title = item_path.split('/').last().map(|s| s.to_string()).unwrap_or_else(|| DEFAULT_TITLE.clone());

                                // Read and load data
                                if let Some(item_mut) = self.file_browser.selected_item_mut() {
                                    if let Some(data) = item_mut.read_data_blocking() {
                                        return Task::done(Message::DataLoaded(full_path, data));
                                    } else {
                                        return Task::done(Message::DataLoadError(full_path.clone(), fl!(crate::LANGUAGE_LOADER, "error-read-file-data")));
                                    }
                                }
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::ToggleFilterPopup => {
                if self.filter_popup.is_visible() {
                    self.filter_popup.hide();
                    Task::none()
                } else {
                    self.filter_popup.show();
                    self.filter_popup.focus_input()
                }
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
            Message::ShowHelp => {
                self.help_dialog = Some(HelpDialog::new());
                Task::none()
            }
            Message::CloseHelp => {
                self.help_dialog = None;
                Task::none()
            }
            Message::ShowAbout => {
                icy_engine_gui::set_default_auto_scaling_xy(true);
                use super::dialogs::about_dialog::ABOUT_ANSI;
                self.about_dialog = Some(AboutDialog::new(ABOUT_ANSI));
                Task::none()
            }
            Message::CloseAbout => {
                icy_engine_gui::set_default_auto_scaling_xy(false);
                self.about_dialog = None;
                Task::none()
            }
            Message::OpenLink(url) => {
                if let Err(e) = open::that(&url) {
                    log::error!("Failed to open URL {}: {}", url, e);
                }
                Task::none()
            }
            Message::None => Task::none(),
            Message::AnimationTick => {
                // Calculate delta time since last tick
                let now = Instant::now();
                let delta = now.duration_since(self.last_tick);
                self.last_tick = now;
                let delta_seconds = delta.as_secs_f32();

                // Shuffle mode handling
                if self.shuffle_mode.is_active {
                    // Update screen height for proper comment positioning
                    let screen_height = self.preview.get_visible_height();
                    self.shuffle_mode.set_screen_height(screen_height);

                    // Update shuffle mode animations (comments)
                    self.shuffle_mode.tick(delta_seconds);

                    // Check if scroll is complete (preview reached bottom)
                    if self.preview.is_scroll_complete() {
                        self.shuffle_mode.notify_scroll_complete();
                    }

                    // Check if we should advance to next item
                    if self.shuffle_mode.should_advance() {
                        if let Some(index) = self.shuffle_mode.next_item() {
                            // Reset preview for new file and enable auto-scroll
                            self.preview.enable_auto_scroll();
                            return self.load_shuffle_item(index);
                        }
                    }
                }

                // Forward tick to file browser's list view
                self.file_browser.update(FileBrowserMessage::ListView(FileListViewMessage::Tick));
                // Poll tile grid results if in tiles mode
                if self.view_mode() == ViewMode::Tiles {
                    let _ = self.tile_grid.poll_results();
                    self.tile_grid.tick(delta_seconds);
                    // Check toolbar auto-hide
                    self.file_list_toolbar.check_auto_hide();
                }

                // Poll folder preview results if showing folder preview in list mode
                if self.view_mode() == ViewMode::List && self.folder_preview_path.is_some() {
                    let _ = self.folder_preview.poll_results();
                    self.folder_preview.tick(delta_seconds);
                }

                // Poll SAUCE loader results if in SAUCE mode
                if self.sauce_mode() {
                    if let Some(ref mut rx) = self.sauce_rx {
                        while let Ok(result) = rx.try_recv() {
                            // SAUCE result received - invalidate the list view to show new data
                            log::debug!("SAUCE loaded for {:?}: {:?}", result.path, result.sauce);
                            self.file_browser.list_view.invalidate();
                        }
                    }
                }

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
                    let item: Option<Box<dyn Item>> = if self.view_mode() == ViewMode::Tiles {
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
                    let file_name = std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("export").to_string();

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
            Message::ShuffleNext => {
                // Skip to next item in shuffle mode
                if self.shuffle_mode.is_active {
                    if let Some(index) = self.shuffle_mode.next_item() {
                        self.preview.enable_auto_scroll();
                        return self.load_shuffle_item(index);
                    }
                }
                Task::none()
            }
        }
    }

    /// Start loading SAUCE info for all visible files
    fn start_sauce_loading(&self) {
        if let Some(ref loader) = self.sauce_loader {
            // Queue all files for SAUCE loading
            for item in &self.file_browser.files {
                if !item.is_container() && !item.is_parent() {
                    loader.load(SauceRequest {
                        item: Arc::from(item.clone_box()),
                    });
                }
            }
        }
    }

    /// Reset the SAUCE loader when navigating to a new directory
    /// Cancels pending loads, clears cache, resets the loader, and restarts loading if SAUCE mode is on
    fn reset_sauce_loader_for_navigation(&mut self) {
        // Cancel any pending SAUCE loads
        if let Some(ref loader) = self.sauce_loader {
            loader.cancel_all();
        }
        // Clear the SAUCE cache (via FileBrowser's shared reference)
        self.file_browser.clear_sauce_cache();
        // Reset the loader for new loads
        if let Some(ref mut loader) = self.sauce_loader {
            loader.reset();
        }
        // Restart SAUCE loading if SAUCE mode is enabled
        if self.sauce_mode() {
            self.start_sauce_loading();
        }
    }

    /// Collect all displayable files from current container for shuffle mode
    /// Collect indices of all displayable items for shuffle mode
    fn collect_shuffle_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();
        for (i, item) in self.file_browser.files.iter().enumerate() {
            if !item.is_container() && !item.is_parent() {
                // Check if it's a supported file format
                if let Some(format) = FileFormat::from_path(&PathBuf::from(&item.get_file_path())) {
                    if format.is_supported() || format.is_image() {
                        indices.push(i);
                    }
                }
            }
        }
        indices
    }

    /// Load an item for shuffle mode display (by index into file_browser.files)
    fn load_shuffle_item(&mut self, index: usize) -> Task<Message> {
        if index >= self.file_browser.files.len() {
            log::error!("Shuffle item index {} out of range", index);
            return Task::none();
        }

        // Check if we have preloaded data for this index
        if let Some(preloaded) = self.shuffle_mode.take_preloaded_if_matches(index) {
            log::debug!("Using preloaded data for shuffle item {}", index);
            // Convert PathBuf to String
            let path_str = preloaded.path.to_string_lossy().replace('\\', "/").to_string();
            self.current_file = Some(path_str.clone());

            // Extract SAUCE info for overlay
            if let Some(sauce) = icy_sauce::SauceRecord::from_bytes(&preloaded.data).ok().flatten() {
                let comments: Vec<String> = sauce.comments().iter().map(|s| s.to_string()).collect();
                self.shuffle_mode.set_sauce_info(
                    Some(sauce.title().to_string()),
                    Some(sauce.author().to_string()),
                    Some(sauce.group().to_string()),
                    comments,
                );
            }

            // Start preloading the next item
            self.start_shuffle_preload();

            return Task::done(Message::DataLoaded(path_str, preloaded.data));
        }

        // Get the item
        let item = &self.file_browser.files[index];
        let path = if let Some(current) = self.file_browser.current_path() {
            format!("{}/{}", current.to_string_lossy().replace('\\', "/"), item.get_file_path().replace('\\', "/"))
        } else {
            item.get_file_path().replace('\\', "/")
        };

        self.current_file = Some(path.clone());

        // Use the data model's read_data_blocking - works for both local and virtual files
        // We need to get mutable access, so we'll index again
        if let Some(data) = self.file_browser.files[index].read_data_blocking() {
            // Extract SAUCE info for overlay
            if let Some(sauce) = icy_sauce::SauceRecord::from_bytes(&data).ok().flatten() {
                let comments: Vec<String> = sauce.comments().iter().map(|s| s.to_string()).collect();
                self.shuffle_mode.set_sauce_info(
                    Some(sauce.title().to_string()),
                    Some(sauce.author().to_string()),
                    Some(sauce.group().to_string()),
                    comments,
                );
            }

            // Start preloading the next item
            self.start_shuffle_preload();

            return Task::done(Message::DataLoaded(path, data));
        }
        log::debug!("read_data_blocking returned None for shuffle item {:?}, trying next", path);
        // Try next item
        if let Some(next_index) = self.shuffle_mode.next_item() {
            return self.load_shuffle_item(next_index);
        }

        Task::none()
    }

    /// Start preloading the next shuffle item in background
    fn start_shuffle_preload(&mut self) {
        // Only preload if shuffle mode is active and not already preloading
        if !self.shuffle_mode.is_active || self.shuffle_mode.is_preloading() {
            return;
        }

        // Get the next item index
        let Some(next_index) = self.shuffle_mode.peek_next_index() else {
            return;
        };

        if next_index >= self.file_browser.files.len() {
            return;
        }

        // Get item info for the background task
        let item = self.file_browser.files[next_index].clone_box();
        let path = if let Some(current) = self.file_browser.current_path() {
            format!("{}/{}", current.to_string_lossy().replace('\\', "/"), item.get_file_path().replace('\\', "/"))
        } else {
            item.get_file_path().replace('\\', "/")
        };

        // Create channel and cancellation token
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cancel_token = CancellationToken::new();
        let cancel_clone = cancel_token.clone();

        // Register with shuffle mode
        self.shuffle_mode.start_preload(rx, cancel_token);

        // Spawn background task
        let preload_index = next_index;
        tokio::spawn(async move {
            tokio::select! {
                _ = cancel_clone.cancelled() => {
                    log::debug!("Shuffle preload cancelled for index {}", preload_index);
                    let _ = tx.send(None);
                }
                result = tokio::task::spawn_blocking(move || {
                    item.read_data_blocking().map(|data| {
                        super::PreloadedItem {
                            index: preload_index,
                            path: PathBuf::from(&path),
                            data,
                        }
                    })
                }) => {
                    match result {
                        Ok(preloaded) => {
                            log::debug!("Shuffle preload completed for index {}", preload_index);
                            let _ = tx.send(preloaded);
                        }
                        Err(e) => {
                            log::error!("Shuffle preload task failed: {}", e);
                            let _ = tx.send(None);
                        }
                    }
                }
            }
        });

        log::debug!("Started preloading shuffle item index {}", next_index);
    }

    pub fn view(&self) -> Element<'_, Message> {
        // Shuffle mode - show only preview with overlay, hide everything else
        if self.shuffle_mode.is_active {
            return self.view_shuffle_mode();
        }

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
        let content_area: Element<'_, Message> = match self.view_mode() {
            ViewMode::List => {
                // Toolbar for file list
                let options = self.options.lock();
                let toolbar = self.file_list_toolbar.view_for_list(&options).map(Message::FileListToolbar);
                drop(options);

                // File browser on left
                let file_browser = self.file_browser.view(&theme).map(Message::FileBrowser);

                // Combine toolbar and file browser in a column with fixed width
                // Width is larger in SAUCE mode to show additional columns (286+280+160+160=886)
                let list_width = if self.sauce_mode() { 886.0 } else { 286.0 };
                let file_list_column = column![toolbar, file_browser].width(Length::Fixed(list_width));

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
                                    Key::Named(Named::PageUp) => Some(Message::Preview(PreviewMessage::ScrollViewportSmooth(0.0, -400.0))),
                                    Key::Named(Named::PageDown) => Some(Message::Preview(PreviewMessage::ScrollViewportSmooth(0.0, 400.0))),
                                    Key::Named(Named::Home) => Some(Message::Preview(PreviewMessage::ScrollViewportToSmooth(0.0, 0.0))),
                                    Key::Named(Named::End) => Some(Message::Preview(PreviewMessage::ScrollViewportToSmooth(0.0, f32::MAX))),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        })
                        .style(list_focus_style)
                        .into()
                } else {
                    // Show welcome logo with message
                    let welcome_logo = iced_image::Image::new(WELCOME_IMAGE.clone()).content_fit(iced::ContentFit::None);
                    let welcome_title = text(fl!(crate::LANGUAGE_LOADER, "welcome-select-file"))
                        .size(18)
                        .style(|theme: &Theme| text::Style {
                            color: Some(theme.palette().text.scale_alpha(0.7)),
                        });
                    let welcome_tip = text(fl!(crate::LANGUAGE_LOADER, "welcome-tip")).size(13).style(|theme: &Theme| text::Style {
                        color: Some(theme.palette().text.scale_alpha(0.5)),
                    });
                    let preview_content = column![welcome_logo, welcome_title, welcome_tip].spacing(12).align_x(iced::Alignment::Center);

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

                // Main content area (toolbar + file browser + preview)
                row![file_list_column, preview_area].into()
            }
            ViewMode::Tiles => {
                // Full-width tile grid view wrapped in focusable container for keyboard handling
                let tile_grid = self.tile_grid.view().map(Message::TileGrid);

                let tile_content: Element<'_, Message> = focus(tile_grid)
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
                    .into();

                // Build toolbar with mouse detection for auto-hide
                let is_toolbar_visible = self.file_list_toolbar.is_visible;

                // Create hover zone that's always at the left edge
                let hover_zone_width = TOOLBAR_HOVER_ZONE_WIDTH;

                // Create the content to show in the overlay
                let toolbar_overlay: Element<'_, Message> = if is_toolbar_visible {
                    // Toolbar visible: show toolbar with mouse area
                    let options = self.options.lock();
                    let toolbar = self.file_list_toolbar.view(&options).map(Message::FileListToolbar);
                    drop(options);
                    let toolbar_with_hover = mouse_area(container(toolbar).style(|_| container::Style::default()))
                        .on_enter(Message::FileListToolbar(FileListToolbarMessage::MouseEntered))
                        .on_exit(Message::FileListToolbar(FileListToolbarMessage::MouseLeft));

                    container(toolbar_with_hover)
                        .align_x(iced::alignment::Horizontal::Left)
                        .align_y(iced::alignment::Vertical::Top)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into()
                } else {
                    // Toolbar hidden: show invisible hover zone to trigger showing
                    let hover_zone = mouse_area(container(Space::new()).width(Length::Fixed(hover_zone_width)).height(Length::Fixed(40.0)))
                        .on_enter(Message::FileListToolbar(FileListToolbarMessage::MouseEntered));

                    container(hover_zone)
                        .align_x(iced::alignment::Horizontal::Left)
                        .align_y(iced::alignment::Vertical::Top)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into()
                };

                iced::widget::stack![tile_content, toolbar_overlay].into()
            }
        };

        // Status bar at bottom
        let status_info = self.build_status_info();
        let status_bar = StatusBar::view(&status_info, &theme).map(Message::StatusBar);

        // Main layout: nav bar, content, status bar
        let main_layout = column![nav_bar, row![content_area, Space::new().width(1)], status_bar,];

        let base_view: Element<'_, Message> = container(main_layout).width(Length::Fill).height(Length::Fill).into();

        // Wrap with filter popup if visible
        let view_with_filter = super::filter_popup_overlay(&self.filter_popup, base_view, Message::FilterPopup);

        // Wrap with error dialog if active (highest priority)
        if let Some(ref dialog) = self.error_dialog {
            return dialog.clone().view(view_with_filter, |_result| Message::CloseErrorDialog);
        }

        // Wrap with Settings dialog if active (takes priority)
        if let Some(ref dialog) = self.settings_dialog {
            return dialog.view(view_with_filter);
        }

        // Wrap with Export dialog if active
        if let Some(ref dialog) = self.export_dialog {
            let dialog_view = dialog.view(|msg| Message::ExportDialog(msg));
            return icy_engine_gui::ui::modal(view_with_filter, dialog_view, Message::ExportDialog(ExportDialogMessage::Cancel));
        }

        // Wrap with SAUCE dialog if active
        let view_with_sauce = if let Some(ref dialog) = self.sauce_dialog {
            dialog.view(view_with_filter, Message::SauceDialog)
        } else {
            view_with_filter
        };

        // Wrap with Help dialog if active
        let view_with_help = if let Some(ref dialog) = self.help_dialog {
            dialog.view(view_with_sauce, Message::CloseHelp)
        } else {
            view_with_sauce
        };

        // Wrap with About dialog if active
        let view_with_about = if let Some(ref dialog) = self.about_dialog {
            icy_engine_gui::ui::modal(view_with_help, dialog.view(), Message::CloseAbout)
        } else {
            view_with_help
        };

        // Wrap with toast notifications
        ToastManager::new(view_with_about, &self.toasts, Message::CloseToast).into()
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
            .with_auto_scroll_enabled(self.preview.is_auto_scroll_enabled())
            .with_zoom_level(Some(self.preview.terminal.get_zoom()));

        // Get selected/hovered item info - from tile grid in tiles mode, from folder preview or file browser otherwise
        if view_mode == ViewMode::Tiles {
            // Use get_status_info which prefers hovered tile over selected
            if let Some((path, _label, is_container, sauce_info)) = self.tile_grid.get_status_info() {
                info.file_name = Some(path.split('/').last().unwrap_or(&path).to_string());
                // Try to get file size - for local files
                let file_size = std::fs::metadata(&path).ok().map(|m| m.len());
                info.file_size = file_size;
                info.sauce_info = sauce_info;
                info.selected_count = 1;

                // Check if it's an archive
                if is_container {
                    if let Some(FileFormat::Archive(archive_format)) = FileFormat::from_path(&PathBuf::from(&path)) {
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
                info.file_name = Some(path.split('/').last().unwrap_or(&path).to_string());
                let file_size = std::fs::metadata(&path).ok().map(|m| m.len());
                info.file_size = file_size;
                info.sauce_info = sauce_info;
                info.selected_count = 1;

                // Check if it's an archive
                if is_container {
                    if let Some(FileFormat::Archive(archive_format)) = FileFormat::from_path(&PathBuf::from(&path)) {
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
                info.file_name = Some(path.split('/').last().map(|s| s.to_string()).unwrap_or_else(|| path.clone()));
                // Try to get file size from the actual filesystem path if it exists
                if let Some(full_path) = item.get_full_path() {
                    info.file_size = std::fs::metadata(&full_path).ok().map(|m| m.len());
                }
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

    /// Get a string representing the current zoom level for display in title bar
    pub fn get_zoom_info_string(&self) -> String {
        let opts = self.options.lock();
        opts.monitor_settings.scaling_mode.format_zoom_string()
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

    /// Get the current view mode from options
    fn view_mode(&self) -> ViewMode {
        self.options.lock().view_mode
    }

    /// Set the view mode in options
    fn set_view_mode(&self, mode: ViewMode) {
        self.options.lock().view_mode = mode;
    }

    /// Get the current sort order from options
    fn sort_order(&self) -> SortOrder {
        self.options.lock().sort_order
    }

    /// Set the sort order in options
    fn set_sort_order(&self, order: SortOrder) {
        self.options.lock().sort_order = order;
    }

    /// Get the SAUCE mode from options
    fn sauce_mode(&self) -> bool {
        self.options.lock().sauce_mode
    }

    /// Set the SAUCE mode in options
    fn set_sauce_mode(&self, mode: bool) {
        self.options.lock().sauce_mode = mode;
    }

    /// Check if animation is needed
    pub fn needs_animation(&self) -> bool {
        self.file_browser.needs_animation()
            || self.preview.needs_animation()
            || (self.view_mode() == ViewMode::Tiles && self.tile_grid.needs_animation())
            || (self.view_mode() == ViewMode::List && self.folder_preview_path.is_some() && self.folder_preview.needs_animation())
            || self.shuffle_mode.needs_animation()
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<Message> {
        // Try the command handler first for both keyboard and mouse events
        if let Some(msg) = self.commands.handle(event) {
            return Some(msg);
        }

        // Delegate to component handlers
        if let Some(msg) = self.preview.handle_event(event) {
            return Some(Message::Preview(msg));
        }
        if let Some(msg) = self.navigation_bar.handle_event(event) {
            return Some(Message::Navigation(msg));
        }
        if let Some(msg) = self.status_bar.handle_event(event) {
            return Some(Message::StatusBar(msg));
        }

        // In tile mode, forward mouse events to the tile grid for scroll handling
        if self.view_mode() == ViewMode::Tiles {
            if let Some(msg) = self.tile_grid.handle_event(event, 0.0, 40.0) {
                return Some(Message::TileGrid(msg));
            }
        }

        // Handle folder preview mouse events in list mode
        if self.view_mode() == ViewMode::List && self.folder_preview_path.is_some() {
            if let Some(msg) = self.folder_preview.handle_event(event, 300.0, 40.0) {
                return Some(Message::FolderPreview(msg));
            }
        }

        match event {
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                // Context-dependent keys that can't be in command handler
                // Space: shuffle mode or auto-scroll toggle
                if let Key::Named(Named::Space) = key {
                    if self.shuffle_mode.is_active {
                        return Some(Message::ShuffleNext);
                    }
                    return Some(Message::StatusBar(StatusBarMessage::ToggleAutoScroll));
                }

                // Handle Enter key for dialogs
                if let Key::Named(Named::Enter) = key {
                    // First check if shuffle mode is active - exit it
                    if self.shuffle_mode.is_active {
                        return Some(Message::ShuffleNext);
                    }

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
        HistoryPoint::new(provider, self.view_mode(), path, selected_item)
    }

    /// Navigate to a history point - restores provider, path, mode, and selection
    fn navigate_to_history_point(&mut self, point: &HistoryPoint) {
        // Cancel any ongoing loading operation
        self.preview.cancel_loading();
        self.current_file = None;
        self.folder_preview_path = None;

        // Reset SAUCE loader for new directory
        self.reset_sauce_loader_for_navigation();

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

        // 4. Update can_go_up state for toolbar
        self.file_list_toolbar.set_can_go_up(self.file_browser.can_go_parent());

        // 5. Switch view mode if needed
        if self.view_mode() != point.view_mode {
            self.set_view_mode(point.view_mode);
        }

        // 6. Update tile grid if in tile mode
        if self.view_mode() == ViewMode::Tiles {
            let items = self.file_browser.get_items();
            self.tile_grid.set_items_from_items(items);
        }

        // 7. Select the item if specified
        if let Some(ref item_name) = point.selected_item {
            if self.view_mode() == ViewMode::Tiles {
                self.tile_grid.select_by_label(item_name);
            } else {
                self.file_browser.select_by_label(item_name);
            }
        }
    }

    /// View for shuffle mode - fullscreen preview with SAUCE info overlay
    fn view_shuffle_mode(&self) -> Element<'_, Message> {
        use iced::Event;
        use iced::keyboard::{Key, key::Named};
        use iced::widget::stack;

        // Preview takes full screen - use monitor settings for proper display
        let monitor_settings = self.get_current_monitor_settings();
        let preview = self.preview.view_with_settings(Some(&monitor_settings)).map(Message::Preview);

        // Create mouse area to catch clicks for exiting shuffle mode
        let clickable_preview = mouse_area(preview).on_press(Message::Escape);

        // Get actual screen height from preview for comment scrolling
        let screen_height = self.preview.get_visible_height();

        // Build shuffle mode overlay (title/author/group at top, comments at bottom)
        let shuffle_overlay = self.shuffle_mode.overlay_view(screen_height).map(|msg| {
            match msg {
                super::ShuffleModeMessage::Exit => Message::Escape,
                super::ShuffleModeMessage::NextFile => Message::AnimationTick, // Trigger advance check
                super::ShuffleModeMessage::Tick(_) => Message::AnimationTick,
            }
        });

        // Stack preview with overlay
        let content = stack![clickable_preview, shuffle_overlay];

        // Wrap in focus to handle keyboard events (Space/Enter for next, Escape to exit)
        let focusable_content = focus(content).on_event(|event, _id| {
            if let Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event {
                match key {
                    Key::Named(Named::Space) | Key::Named(Named::Enter) => Some(Message::ShuffleNext),
                    Key::Named(Named::Escape) => Some(Message::Escape),
                    _ => None,
                }
            } else {
                None
            }
        });

        container(focusable_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(iced::Color::BLACK)),
                ..Default::default()
            })
            .into()
    }
}

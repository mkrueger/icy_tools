//! File list view with shader-based rendering
//!
//! This module provides a high-performance file list view using GPU shaders
//! for rendering icons and text efficiently.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use icy_engine_gui::ui::FileIcon;
use icy_ui::{
    mouse,
    widget::{column, container, operation::ensure_visible, row, scroll_area, scrollable, shader, text, Id},
    Element, Length, Rectangle, Size, Task,
};
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::items::Item;

use super::file_list_shader::{
    invalidate_gpu_cache, render_list_item, render_list_item_with_sauce, FileListShaderPrimitive, FileListThemeColors, ListItemRenderData,
};
use super::sauce_loader::{SauceInfo, SharedSauceCache};

/// Height of each item row in pixels
pub const ITEM_HEIGHT: f32 = 24.0;

/// Time window for double-click detection (in milliseconds)
const DOUBLE_CLICK_MS: u128 = 400;

/// Scroll area ID for programmatic scrolling
pub static FILE_LIST_SCROLL_ID: Lazy<Id> = Lazy::new(Id::unique);

/// Messages for the file list view
#[derive(Debug, Clone)]
pub enum FileListViewMessage {
    /// Mouse scroll (handled by scroll_area, but kept for external use)
    Scroll(f32),
    /// Mouse click on item at index
    Click(usize),
    /// Animation tick
    Tick,
    /// Keyboard: select previous item
    SelectPrevious,
    /// Keyboard: select next item
    SelectNext,
    /// Keyboard: page up
    PageUp,
    /// Keyboard: page down
    PageDown,
    /// Keyboard: home
    Home,
    /// Keyboard: end
    End,
    /// Open selected item (Enter/double-click)
    OpenSelected,
    /// Set viewport size (from responsive layout)
    SetViewportSize(f32, f32),
}

/// Cache entry for pre-rendered list items
struct CachedItem {
    rgba_data: Arc<Vec<u8>>,
    width: u32,
    height: u32,
}

/// Custom file list view with smooth scrolling and native scrollbar
pub struct FileListView {
    /// Total item count for content height calculation
    item_count: usize,
    /// Currently selected index (into visible items)
    pub selected_index: Option<usize>,
    /// Last click time and index for double-click detection
    last_click: Option<(Instant, usize)>,
    /// Render cache invalidation flag
    needs_redraw: bool,
    /// Content version number (incremented on each invalidation for cache detection)
    content_version: u32,
    /// Shared hover state for shader
    shared_hovered_index: Arc<Mutex<Option<usize>>>,
    /// Cache for pre-rendered items (keyed by a hash of label + icon + is_folder)
    item_cache: RefCell<HashMap<u64, CachedItem>>,
    /// Current viewport width for rendering
    current_width: RefCell<f32>,
    /// Current viewport height for page calculations
    current_height: RefCell<f32>,
    /// Whether SAUCE mode is enabled (show SAUCE columns)
    sauce_mode: bool,
}

impl Default for FileListView {
    fn default() -> Self {
        Self::new()
    }
}

impl FileListView {
    pub fn new() -> Self {
        Self {
            item_count: 0,
            selected_index: None,
            last_click: None,
            needs_redraw: true,
            content_version: 0,
            shared_hovered_index: Arc::new(Mutex::new(None)),
            item_cache: RefCell::new(HashMap::new()),
            current_width: RefCell::new(300.0),
            current_height: RefCell::new(400.0),
            sauce_mode: false,
        }
    }

    /// Set the content size based on item count
    pub fn set_item_count(&mut self, count: usize) {
        self.item_count = count;
        self.invalidate();
    }

    /// Set SAUCE mode (show/hide SAUCE columns)
    pub fn set_sauce_mode(&mut self, sauce_mode: bool) {
        if self.sauce_mode != sauce_mode {
            self.sauce_mode = sauce_mode;
            self.invalidate();
        }
    }

    /// Invalidate the render cache - clears both CPU and GPU caches
    /// Use this for content changes (theme, width, item list changes)
    pub fn invalidate(&mut self) {
        self.needs_redraw = true;
        self.content_version = self.content_version.wrapping_add(1);
        // Clear CPU-side item cache
        self.item_cache.borrow_mut().clear();
        // Invalidate GPU texture cache
        invalidate_gpu_cache();
    }

    /// Light invalidation - marks redraw needed but keeps caches intact
    /// Use this for scroll/selection changes where content hasn't changed
    fn invalidate_visual(&mut self) {
        self.needs_redraw = true;
    }

    /// Calculate scroll offset to make an item visible
    /// Returns Some(offset) if scrolling is needed, None if already visible
    fn scroll_offset_for_item(&self, index: usize, current_scroll: f32, visible_height: f32) -> Option<f32> {
        let item_top = index as f32 * ITEM_HEIGHT;
        let item_bottom = item_top + ITEM_HEIGHT;
        let visible_top = current_scroll;
        let visible_bottom = visible_top + visible_height;

        if item_top < visible_top {
            // Need to scroll up
            Some(item_top)
        } else if item_bottom > visible_bottom {
            // Need to scroll down
            Some(item_bottom - visible_height)
        } else {
            None // Already visible
        }
    }

    /// Create a task to ensure the item at index is visible (animated scroll)
    fn ensure_item_visible<Message: 'static>(&self, index: usize) -> Task<Message> {
        let y = index as f32 * ITEM_HEIGHT;
        let target_rect = Rectangle::new(icy_ui::Point::new(0.0, y), Size::new(1.0, ITEM_HEIGHT));
        ensure_visible(FILE_LIST_SCROLL_ID.clone(), target_rect)
    }

    /// Update with a message
    /// Returns (should_open, scroll_task)
    /// - should_open: true if an item should be opened (Enter/double-click)
    /// - scroll_task: Task to ensure selected item is visible
    pub fn update<Message: 'static>(&mut self, message: FileListViewMessage, item_count: usize) -> (bool, Task<Message>) {
        match message {
            FileListViewMessage::Scroll(_delta_y) => {
                // Scroll is now handled by scroll_area, no invalidation needed
                (false, Task::none())
            }
            FileListViewMessage::Click(index) => {
                // index is the hovered item index from the shader
                if index < item_count {
                    // Check for double-click
                    let now = Instant::now();
                    if let Some((last_time, last_index)) = self.last_click {
                        if last_index == index && now.duration_since(last_time).as_millis() < DOUBLE_CLICK_MS {
                            // Double-click detected
                            self.last_click = None;
                            self.selected_index = Some(index);
                            self.invalidate_visual();
                            return (true, Task::none());
                        }
                    }
                    self.selected_index = Some(index);
                    self.last_click = Some((now, index));
                    self.invalidate_visual();
                }
                (false, Task::none())
            }
            FileListViewMessage::Tick => {
                // Animation is now handled by scroll_area
                (false, Task::none())
            }
            FileListViewMessage::SelectPrevious => {
                if item_count > 0 {
                    let new_index = match self.selected_index {
                        Some(i) if i > 0 => i - 1,
                        Some(i) => i,
                        None => 0,
                    };
                    self.selected_index = Some(new_index);
                    self.invalidate_visual();
                    return (false, self.ensure_item_visible(new_index));
                }
                (false, Task::none())
            }
            FileListViewMessage::SelectNext => {
                if item_count > 0 {
                    let new_index = match self.selected_index {
                        Some(i) if i < item_count - 1 => i + 1,
                        Some(i) => i,
                        None => 0,
                    };
                    self.selected_index = Some(new_index);
                    self.invalidate_visual();
                    return (false, self.ensure_item_visible(new_index));
                }
                (false, Task::none())
            }
            FileListViewMessage::PageUp => {
                if item_count > 0 {
                    let visible_height = *self.current_height.borrow();
                    let visible_items = (visible_height / ITEM_HEIGHT).max(1.0) as usize;
                    let new_index = match self.selected_index {
                        Some(i) => i.saturating_sub(visible_items),
                        None => 0,
                    };
                    self.selected_index = Some(new_index);
                    self.invalidate_visual();
                    return (false, self.ensure_item_visible(new_index));
                }
                (false, Task::none())
            }
            FileListViewMessage::PageDown => {
                if item_count > 0 {
                    let visible_height = *self.current_height.borrow();
                    let visible_items = (visible_height / ITEM_HEIGHT).max(1.0) as usize;
                    let new_index = match self.selected_index {
                        Some(i) => (i + visible_items).min(item_count - 1),
                        None => (visible_items - 1).min(item_count - 1),
                    };
                    self.selected_index = Some(new_index);
                    self.invalidate_visual();
                    return (false, self.ensure_item_visible(new_index));
                }
                (false, Task::none())
            }
            FileListViewMessage::Home => {
                if item_count > 0 {
                    self.selected_index = Some(0);
                    self.invalidate_visual();
                    return (false, self.ensure_item_visible(0));
                }
                (false, Task::none())
            }
            FileListViewMessage::End => {
                if item_count > 0 {
                    let last_index = item_count - 1;
                    self.selected_index = Some(last_index);
                    self.invalidate_visual();
                    return (false, self.ensure_item_visible(last_index));
                }
                (false, Task::none())
            }
            FileListViewMessage::OpenSelected => (self.selected_index.is_some(), Task::none()),
            FileListViewMessage::SetViewportSize(width, height) => {
                let current_width = *self.current_width.borrow();
                let current_height = *self.current_height.borrow();
                let width_changed = (current_width - width).abs() > 1.0;
                let height_changed = (current_height - height).abs() > 1.0;

                if width_changed {
                    *self.current_width.borrow_mut() = width;
                    // Clear cache when width changes - items need re-rendering
                    self.item_cache.borrow_mut().clear();
                    invalidate_gpu_cache();
                }
                if height_changed {
                    *self.current_height.borrow_mut() = height;
                    // Height change only affects pagination, no cache invalidation needed
                }
                // Don't trigger full invalidate - just update stored values
                // Rendering will pick up new dimensions on next draw
                (false, Task::none())
            }
        }
    }

    /// Generate a cache key for an item (includes theme colors and filter for proper invalidation)
    fn cache_key(label: &str, icon: FileIcon, is_folder: bool, width: u32, theme_colors: &FileListThemeColors, filter: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        label.hash(&mut hasher);
        (icon as u8).hash(&mut hasher);
        is_folder.hash(&mut hasher);
        width.hash(&mut hasher);
        // Include theme colors in cache key so items are re-rendered on theme change
        theme_colors.text_color.hash(&mut hasher);
        theme_colors.folder_color.hash(&mut hasher);
        // Include filter in cache key so items are re-rendered with highlighting
        filter.to_lowercase().hash(&mut hasher);
        hasher.finish()
    }

    /// Generate a cache key for a SAUCE mode item (includes SAUCE fields)
    fn cache_key_with_sauce(
        label: &str,
        icon: FileIcon,
        is_folder: bool,
        width: u32,
        theme_colors: &FileListThemeColors,
        filter: &str,
        sauce_info: Option<&SauceInfo>,
    ) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        label.hash(&mut hasher);
        (icon as u8).hash(&mut hasher);
        is_folder.hash(&mut hasher);
        width.hash(&mut hasher);
        theme_colors.text_color.hash(&mut hasher);
        theme_colors.folder_color.hash(&mut hasher);
        filter.to_lowercase().hash(&mut hasher);
        // Include SAUCE fields
        if let Some(info) = sauce_info {
            info.title.hash(&mut hasher);
            info.author.hash(&mut hasher);
            info.group.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Get or create a cached rendered item
    fn get_or_render_item(
        &self,
        label: &str,
        icon: FileIcon,
        is_folder: bool,
        width: u32,
        theme_colors: &FileListThemeColors,
        filter: &str,
    ) -> (Arc<Vec<u8>>, u32, u32) {
        let key = Self::cache_key(label, icon, is_folder, width, theme_colors, filter);

        if let Some(cached) = self.item_cache.borrow().get(&key) {
            return (cached.rgba_data.clone(), cached.width, cached.height);
        }

        // Render new item with theme colors and filter highlighting
        let (rgba, w, h) = render_list_item(icon, label, is_folder, width, theme_colors, filter);
        let rgba_arc = Arc::new(rgba);

        self.item_cache.borrow_mut().insert(
            key,
            CachedItem {
                rgba_data: rgba_arc.clone(),
                width: w,
                height: h,
            },
        );

        (rgba_arc, w, h)
    }

    /// Get or create a cached rendered item with SAUCE info
    fn get_or_render_item_with_sauce(
        &self,
        label: &str,
        icon: FileIcon,
        is_folder: bool,
        width: u32,
        theme_colors: &FileListThemeColors,
        filter: &str,
        sauce_info: Option<&SauceInfo>,
    ) -> (Arc<Vec<u8>>, u32, u32) {
        let key = Self::cache_key_with_sauce(label, icon, is_folder, width, theme_colors, filter, sauce_info);

        if let Some(cached) = self.item_cache.borrow().get(&key) {
            return (cached.rgba_data.clone(), cached.width, cached.height);
        }

        // Render new item with theme colors, filter highlighting and SAUCE info
        let (rgba, w, h) = render_list_item_with_sauce(
            icon,
            label,
            is_folder,
            width,
            theme_colors,
            filter,
            sauce_info.map(|s| &*s.title),
            sauce_info.map(|s| &*s.author),
            sauce_info.map(|s| &*s.group),
        );
        let rgba_arc = Arc::new(rgba);

        self.item_cache.borrow_mut().insert(
            key,
            CachedItem {
                rgba_data: rgba_arc.clone(),
                width: w,
                height: h,
            },
        );

        (rgba_arc, w, h)
    }

    /// Create the view with native scroll_area scrollbar
    pub fn view<'a, Message: Clone + 'static>(
        &'a self,
        files: &'a [Box<dyn Item>],
        visible_indices: &'a [usize],
        filter: &'a str,
        theme_colors: FileListThemeColors,
        sauce_cache: Option<&SharedSauceCache>,
        on_message: impl Fn(FileListViewMessage) -> Message + 'static,
    ) -> Element<'a, Message> {
        let on_message = Arc::new(on_message);

        let current_width = *self.current_width.borrow();
        let width = current_width.max(100.0) as u32;
        let item_count = visible_indices.len();
        let content_height = item_count as f32 * ITEM_HEIGHT;
        let content_width = current_width - 12.0; // Account for scrollbar

        // Clone data for the closure - extract all needed data upfront
        let files_data: Vec<_> = visible_indices
            .iter()
            .filter_map(|&idx| {
                files.get(idx).map(|item| {
                    let item_path = item.get_full_path().unwrap_or_else(|| item.get_file_path());
                    // Get SAUCE info if in sauce mode
                    let sauce_info = if self.sauce_mode {
                        sauce_cache.and_then(|c| c.read().get(&item_path).and_then(|opt| opt.clone()))
                    } else {
                        None
                    };
                    (item.get_label(), item.get_file_icon(), item.is_container(), sauce_info)
                })
            })
            .collect();

        let selected_index = self.selected_index;
        let shared_hovered_index = self.shared_hovered_index.clone();
        let sauce_mode = self.sauce_mode;

        let on_message_click = on_message.clone();

        // Use scroll_area with show_viewport
        let scroll_content = scroll_area()
            .id(FILE_LIST_SCROLL_ID.clone())
            .auto_scroll(true)
            .width(Length::Fill)
            .height(Length::Fill)
            .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::new().width(8).scroller_width(6)))
            .show_viewport(Size::new(content_width, content_height), move |viewport| {
                let scroll_y = viewport.y;
                let viewport_height = viewport.height;

                // Calculate visible range
                let first_visible = (scroll_y / ITEM_HEIGHT) as usize;
                let visible_count = (viewport_height / ITEM_HEIGHT).ceil() as usize + 2;
                let last_visible = (first_visible + visible_count).min(files_data.len());

                // Build list items for visible range only
                let mut items: Vec<ListItemRenderData> = Vec::with_capacity(visible_count);

                for visible_index in first_visible..last_visible {
                    if let Some((label, file_icon, is_folder, sauce_info)) = files_data.get(visible_index) {
                        let is_selected = selected_index == Some(visible_index);

                        // Render item (simplified - no caching in closure for now)
                        let (rgba_data, w, h) = if sauce_mode && sauce_info.is_some() {
                            let si = sauce_info.as_ref().unwrap();
                            render_list_item_with_sauce(
                                *file_icon,
                                label,
                                *is_folder,
                                width,
                                &theme_colors,
                                "",
                                Some(&si.title),
                                Some(&si.author),
                                Some(&si.group),
                            )
                        } else {
                            render_list_item(*file_icon, label, *is_folder, width, &theme_colors, "")
                        };

                        items.push(ListItemRenderData {
                            id: visible_index as u64,
                            rgba_data: Arc::new(rgba_data),
                            width: w,
                            height: h,
                            is_selected,
                            is_hovered: false,
                            y_position: visible_index as f32 * ITEM_HEIGHT,
                        });
                    }
                }

                let on_message_shader = on_message_click.clone();

                let program: FileListShaderWrapper<Message> = FileListShaderWrapper {
                    items,
                    scroll_y,
                    content_height,
                    _viewport_width: content_width,
                    selected_index,
                    shared_hovered_index: shared_hovered_index.clone(),
                    on_message: Arc::new(move |msg| on_message_shader(msg)),
                    theme_colors: theme_colors.clone(),
                };

                container(shader(program).width(Length::Fill).height(Length::Fill))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            });

        let list_content = container(scroll_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &icy_ui::Theme| container::Style {
                background: Some(icy_ui::Background::Color(theme.background.base)),
                border: icy_ui::Border {
                    color: theme.primary.divider,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            });

        // Add header row if in SAUCE mode
        if self.sauce_mode {
            column![self.render_header_row(), list_content].into()
        } else {
            list_content.into()
        }
    }

    /// Render header row for SAUCE mode
    fn render_header_row<Message: 'static>(&self) -> Element<'_, Message> {
        use i18n_embed_fl::fl;

        let header_style = |theme: &icy_ui::Theme| container::Style {
            background: Some(icy_ui::Background::Color(theme.secondary.base)),
            border: icy_ui::Border {
                color: theme.primary.divider,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        };

        let header_text_style = |theme: &icy_ui::Theme| text::Style {
            color: Some(theme.background.on.scale_alpha(0.7)),
        };

        // Column widths for SAUCE mode - matching constants in file_list_shader.rs
        // Name: 200px, Title: 280px (35 chars), Author: 160px (20 chars), Group: 160px (20 chars)
        use super::file_list_shader::{SAUCE_AUTHOR_WIDTH, SAUCE_GROUP_WIDTH, SAUCE_NAME_WIDTH, SAUCE_TITLE_WIDTH};

        let name_header = container(text(fl!(crate::LANGUAGE_LOADER, "header-name")).size(11).style(header_text_style))
            .width(Length::Fixed(SAUCE_NAME_WIDTH as f32))
            .padding([2, 4]);
        let title_header = container(text(fl!(crate::LANGUAGE_LOADER, "header-title")).size(11).style(header_text_style))
            .width(Length::Fixed(SAUCE_TITLE_WIDTH as f32))
            .padding([2, 4]);
        let author_header = container(text(fl!(crate::LANGUAGE_LOADER, "header-author")).size(11).style(header_text_style))
            .width(Length::Fixed(SAUCE_AUTHOR_WIDTH as f32))
            .padding([2, 4]);
        let group_header = container(text(fl!(crate::LANGUAGE_LOADER, "header-group")).size(11).style(header_text_style))
            .width(Length::Fixed(SAUCE_GROUP_WIDTH as f32))
            .padding([2, 4]);

        container(row![name_header, title_header, author_header, group_header].spacing(0))
            .width(Length::Fill)
            .style(header_style)
            .into()
    }
}

// ============================================================================
// Shader Program Wrapper
// ============================================================================

/// Wrapper for the shader program that implements shader::Program
struct FileListShaderWrapper<Message> {
    items: Vec<ListItemRenderData>,
    scroll_y: f32,
    content_height: f32,
    _viewport_width: f32,
    selected_index: Option<usize>,
    shared_hovered_index: Arc<Mutex<Option<usize>>>,
    on_message: Arc<dyn Fn(FileListViewMessage) -> Message>,
    theme_colors: FileListThemeColors,
}

/// State for the shader program
#[derive(Debug, Default)]
struct FileListShaderState {
    hovered_index: Option<usize>,
    last_bounds: Option<(f32, f32)>,
}

impl<Message> shader::Program<Message> for FileListShaderWrapper<Message>
where
    Message: Clone + 'static,
{
    type State = FileListShaderState;
    type Primitive = FileListShaderPrimitive;

    fn draw(&self, _state: &Self::State, _cursor: mouse::Cursor, bounds: icy_ui::Rectangle) -> Self::Primitive {
        // Apply hover state from shared state
        let hovered = *self.shared_hovered_index.lock();
        let items: Vec<ListItemRenderData> = self
            .items
            .iter()
            .map(|item: &ListItemRenderData| {
                let mut item = item.clone();
                item.is_hovered = hovered == Some(item.id as usize);
                item.is_selected = self.selected_index == Some(item.id as usize);
                item
            })
            .collect();

        FileListShaderPrimitive {
            items,
            scroll_y: self.scroll_y,
            viewport_height: bounds.height,
            viewport_width: bounds.width,
            theme_colors: self.theme_colors,
        }
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &icy_ui::Event,
        bounds: icy_ui::Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<icy_ui::widget::Action<Message>> {
        // Check if viewport size changed significantly (debounce threshold of 2px)
        // Only send message if we have a previous size AND it changed significantly
        let current_bounds = (bounds.width, bounds.height);
        let size_changed_significantly = match state.last_bounds {
            Some((last_w, last_h)) => (bounds.width - last_w).abs() > 2.0 || (bounds.height - last_h).abs() > 2.0,
            None => false, // Don't trigger on first call - just record the initial size
        };

        // Always update last_bounds to track the current size
        if state.last_bounds.is_none() || size_changed_significantly {
            state.last_bounds = Some(current_bounds);
        }

        if size_changed_significantly {
            let msg = (self.on_message)(FileListViewMessage::SetViewportSize(bounds.width, bounds.height));
            return Some(icy_ui::widget::Action::publish(msg));
        }

        // Handle hover detection
        let found_index = if let Some(cursor_pos) = cursor.position_in(bounds) {
            let item_y = cursor_pos.y + self.scroll_y;
            let index = (item_y / ITEM_HEIGHT) as usize;
            let item_count = (self.content_height / ITEM_HEIGHT) as usize;
            if index < item_count {
                Some(index)
            } else {
                None
            }
        } else {
            None
        };

        // Handle events
        let is_over = cursor.is_over(bounds);

        match event {
            icy_ui::Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::WheelScrolled { .. } => {
                    // Scroll is handled by scroll_area, don't publish redundant messages
                }
                mouse::Event::ButtonPressed {
                    button: mouse::Button::Left, ..
                } => {
                    // Use the hover index (found_index) which already accounts for scroll
                    if is_over {
                        if let Some(index) = found_index {
                            let msg = (self.on_message)(FileListViewMessage::Click(index));
                            return Some(icy_ui::widget::Action::publish(msg));
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }

        // Only update shared and local state if hover actually changed
        if state.hovered_index != found_index {
            *self.shared_hovered_index.lock() = found_index;
            state.hovered_index = found_index;
            // Request redraw to update hover highlighting
            return Some(icy_ui::widget::Action::request_redraw());
        }

        None
    }

    fn mouse_interaction(&self, state: &Self::State, _bounds: icy_ui::Rectangle, _cursor: mouse::Cursor) -> mouse::Interaction {
        if state.hovered_index.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

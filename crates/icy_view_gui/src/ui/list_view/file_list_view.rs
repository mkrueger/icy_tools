//! File list view with shader-based rendering
//!
//! This module provides a high-performance file list view using GPU shaders
//! for rendering icons and text efficiently.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use iced::{
    Element, Length, mouse,
    widget::{column, container, row, shader, stack, text},
};
use icy_engine_gui::{ScrollbarOverlay, ScrollbarState, Viewport, ui::FileIcon};
use parking_lot::Mutex;

use crate::Item;

use super::file_list_shader::{
    FileListShaderPrimitive, FileListThemeColors, ListItemRenderData, invalidate_gpu_cache, render_list_item, render_list_item_with_sauce,
};
use super::sauce_loader::{SauceInfo, SharedSauceCache};

/// Height of each item row in pixels
pub const ITEM_HEIGHT: f32 = 24.0;

/// Time window for double-click detection (in milliseconds)
const DOUBLE_CLICK_MS: u128 = 400;

/// Messages for the file list view
#[derive(Debug, Clone)]
pub enum FileListViewMessage {
    /// Mouse scroll
    Scroll(f32, f32),
    /// Mouse click at Y position
    Click(f32),
    /// Scrollbar hover state changed
    ScrollbarHovered(bool),
    /// Scroll to absolute position (from scrollbar drag)
    ScrollTo(f32, f32),
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
    /// Selection changed (for filter-triggered updates)
    SelectionChanged,
}

/// Cache entry for pre-rendered list items
struct CachedItem {
    rgba_data: Arc<Vec<u8>>,
    width: u32,
    height: u32,
}

/// Custom file list view with smooth scrolling and overlay scrollbar
pub struct FileListView {
    /// Viewport for scroll management
    pub viewport: Viewport,
    /// Scrollbar state for animation
    pub scrollbar: ScrollbarState,
    /// Shared hover state for scrollbar
    pub scrollbar_hover_state: Arc<AtomicBool>,
    /// Currently selected index (into visible items)
    pub selected_index: Option<usize>,
    /// Last click time and Y position for double-click detection
    last_click: Option<(Instant, f32)>,
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
        let mut viewport = Viewport::default();
        // Smooth scroll animation speed
        // At 60fps (delta_time=0.016), this gives lerp_factor of ~0.25 = 25% per frame
        viewport.scroll_animation_speed = 15.0;

        Self {
            viewport,
            scrollbar: ScrollbarState::new(),
            scrollbar_hover_state: Arc::new(AtomicBool::new(false)),
            selected_index: None,
            last_click: None,
            needs_redraw: true,
            content_version: 0,
            shared_hovered_index: Arc::new(Mutex::new(None)),
            item_cache: RefCell::new(HashMap::new()),
            current_width: RefCell::new(300.0),
            sauce_mode: false,
        }
    }

    /// Set the content size based on item count
    pub fn set_item_count(&mut self, count: usize) {
        let content_height = count as f32 * ITEM_HEIGHT;
        self.viewport.set_content_size(300.0, content_height);
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

    /// Update scrollbar position from viewport
    fn update_scrollbar_position(&mut self) {
        let max_scroll = self.viewport.max_scroll_y();
        if max_scroll > 0.0 {
            let position = self.viewport.scroll_y / max_scroll;
            self.scrollbar.set_scroll_position(position);
        } else {
            self.scrollbar.set_scroll_position(0.0);
        }
    }

    /// Ensure the selected item is visible (scroll if needed) - uses immediate scroll
    fn ensure_visible(&mut self, index: usize) {
        let item_top = index as f32 * ITEM_HEIGHT;
        let item_bottom = item_top + ITEM_HEIGHT;
        let visible_top = self.viewport.scroll_y;
        let visible_bottom = visible_top + self.viewport.visible_height;

        if item_top < visible_top {
            // Scroll up to show item (immediate, no animation)
            self.viewport.scroll_y_to_immediate(item_top);
        } else if item_bottom > visible_bottom {
            // Scroll down to show item (immediate, no animation)
            self.viewport.scroll_y_to_immediate(item_bottom - self.viewport.visible_height);
        }
    }

    /// Ensure the currently selected item is visible (public wrapper)
    pub fn ensure_selected_visible(&mut self) {
        if let Some(index) = self.selected_index {
            self.ensure_visible(index);
        }
    }

    /// Update with a message, returns true if an item should be opened
    pub fn update(&mut self, message: FileListViewMessage, item_count: usize) -> bool {
        match message {
            FileListViewMessage::Scroll(_, delta_y) => {
                // delta_y is already in pixels (Lines are converted to pixels in canvas)
                // Negative delta = scroll up (content moves down), positive = scroll down
                self.viewport.scroll_y_by(-delta_y);
                self.update_scrollbar_position();
                self.invalidate_visual();
                false
            }
            FileListViewMessage::Click(y) => {
                // Calculate which item was clicked
                let click_y = y + self.viewport.scroll_y;
                let index = (click_y / ITEM_HEIGHT) as usize;

                if index < item_count {
                    // Check for double-click
                    let now = Instant::now();
                    if let Some((last_time, last_y)) = self.last_click {
                        let same_item = ((last_y + self.viewport.scroll_y) / ITEM_HEIGHT) as usize == index;
                        if same_item && now.duration_since(last_time).as_millis() < DOUBLE_CLICK_MS {
                            // Double-click detected
                            self.last_click = None;
                            self.selected_index = Some(index);
                            self.invalidate_visual();
                            return true;
                        }
                    }
                    self.selected_index = Some(index);
                    self.last_click = Some((now, y));
                    self.invalidate_visual();
                }
                false
            }
            FileListViewMessage::ScrollbarHovered(hovered) => {
                self.scrollbar.set_hovered(hovered);
                false
            }
            FileListViewMessage::ScrollTo(_, y) => {
                self.viewport.scroll_y_to_immediate(y);
                self.update_scrollbar_position();
                self.invalidate_visual();
                false
            }
            FileListViewMessage::Tick => {
                self.viewport.update_animation();
                self.scrollbar.update_animation();
                self.update_scrollbar_position();
                if self.viewport.is_animating() || self.scrollbar.is_animating() {
                    self.invalidate_visual();
                }
                false
            }
            FileListViewMessage::SelectPrevious => {
                if item_count > 0 {
                    self.selected_index = Some(match self.selected_index {
                        Some(i) if i > 0 => i - 1,
                        Some(i) => i, // Stay at start, no wrap
                        None => 0,
                    });
                    if let Some(idx) = self.selected_index {
                        self.ensure_visible(idx);
                    }
                    self.invalidate_visual();
                }
                false
            }
            FileListViewMessage::SelectNext => {
                if item_count > 0 {
                    self.selected_index = Some(match self.selected_index {
                        Some(i) if i < item_count - 1 => i + 1,
                        Some(i) => i, // Stay at end, no wrap
                        None => 0,
                    });
                    if let Some(idx) = self.selected_index {
                        self.ensure_visible(idx);
                    }
                    self.invalidate_visual();
                }
                false
            }
            FileListViewMessage::PageUp => {
                if item_count > 0 {
                    let visible_items = (self.viewport.visible_height / ITEM_HEIGHT) as usize;
                    self.selected_index = Some(match self.selected_index {
                        Some(i) => i.saturating_sub(visible_items.max(1)),
                        None => 0,
                    });
                    if let Some(idx) = self.selected_index {
                        self.ensure_visible(idx);
                    }
                    // Also scroll viewport
                    self.viewport.scroll_y_by(-(self.viewport.visible_height - ITEM_HEIGHT));
                    self.update_scrollbar_position();
                    self.invalidate_visual();
                }
                false
            }
            FileListViewMessage::PageDown => {
                if item_count > 0 {
                    let visible_items = (self.viewport.visible_height / ITEM_HEIGHT) as usize;
                    self.selected_index = Some(match self.selected_index {
                        Some(i) => (i + visible_items.max(1)).min(item_count - 1),
                        None => (visible_items.max(1) - 1).min(item_count - 1),
                    });
                    if let Some(idx) = self.selected_index {
                        self.ensure_visible(idx);
                    }
                    // Also scroll viewport
                    self.viewport.scroll_y_by(self.viewport.visible_height - ITEM_HEIGHT);
                    self.update_scrollbar_position();
                    self.invalidate_visual();
                }
                false
            }
            FileListViewMessage::Home => {
                if item_count > 0 {
                    self.selected_index = Some(0);
                    self.viewport.scroll_y_to(0.0);
                    self.update_scrollbar_position();
                    self.invalidate_visual();
                }
                false
            }
            FileListViewMessage::End => {
                if item_count > 0 {
                    self.selected_index = Some(item_count - 1);
                    let max_scroll = self.viewport.max_scroll_y();
                    self.viewport.scroll_y_to(max_scroll);
                    self.update_scrollbar_position();
                    self.invalidate_visual();
                }
                false
            }
            FileListViewMessage::OpenSelected => self.selected_index.is_some(),
            FileListViewMessage::SelectionChanged => {
                // Just signal that selection changed (for filter-triggered updates)
                // The actual selection is already set, this just triggers the preview update
                false
            }
            FileListViewMessage::SetViewportSize(width, height) => {
                self.viewport.set_visible_size(width, height);
                let current = *self.current_width.borrow();
                if (current - width).abs() > 1.0 {
                    *self.current_width.borrow_mut() = width;
                    // Clear cache when width changes significantly
                    self.item_cache.borrow_mut().clear();
                }
                self.invalidate();
                false
            }
        }
    }

    /// Check if animation is needed
    pub fn needs_animation(&self) -> bool {
        self.viewport.is_animating() || self.scrollbar.needs_animation()
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
            sauce_info.map(|s| s.title.as_str()),
            sauce_info.map(|s| s.author.as_str()),
            sauce_info.map(|s| s.group.as_str()),
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

    /// Create the view with overlay scrollbar
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
        let on_message_scroll = on_message.clone();
        let on_message_hover = on_message.clone();

        let current_width = *self.current_width.borrow();
        let width = current_width.max(100.0) as u32;
        let scroll_y = self.viewport.scroll_y;
        let viewport_height = self.viewport.visible_height;

        // Calculate visible range
        let item_count = visible_indices.len();
        let first_visible = (scroll_y / ITEM_HEIGHT) as usize;
        let visible_count = (viewport_height / ITEM_HEIGHT).ceil() as usize + 2;
        let last_visible = (first_visible + visible_count).min(item_count);

        // Build list items for visible range only
        let mut items: Vec<ListItemRenderData> = Vec::with_capacity(visible_count);

        // Lock sauce cache once if in sauce mode (use read lock for shared access)
        let sauce_cache_guard = if self.sauce_mode { sauce_cache.map(|c| c.read()) } else { None };

        for visible_index in first_visible..last_visible {
            if let Some(&file_index) = visible_indices.get(visible_index) {
                if let Some(item) = files.get(file_index) {
                    let is_folder = item.is_container();
                    let is_selected = self.selected_index == Some(visible_index);

                    // Get appropriate icon based on state
                    let file_icon = item.get_file_icon();

                    let label = item.get_label();

                    // Choose render method based on sauce mode
                    let (rgba_data, w, h) = if self.sauce_mode {
                        // Get SAUCE info from cache if available
                        // SauceCache.get() returns Option<Option<SauceInfo>> (resolved from interned strings)
                        // Use get_full_path() or fall back to get_file_path() (same as SauceLoader)
                        let item_path = item.get_full_path().unwrap_or_else(|| item.get_file_path());
                        let sauce_info = sauce_cache_guard.as_ref().and_then(|cache| cache.get(&item_path).flatten());
                        self.get_or_render_item_with_sauce(&label, file_icon, is_folder, width, &theme_colors, filter, sauce_info.as_ref())
                    } else {
                        self.get_or_render_item(&label, file_icon, is_folder, width, &theme_colors, filter)
                    };

                    items.push(ListItemRenderData {
                        id: visible_index as u64,
                        rgba_data,
                        width: w,
                        height: h,
                        is_selected,
                        is_hovered: false,
                        y_position: visible_index as f32 * ITEM_HEIGHT,
                    });
                }
            }
        }

        // Create shader program wrapper with theme colors
        let program = FileListShaderWrapper {
            items,
            scroll_y,
            content_height: item_count as f32 * ITEM_HEIGHT,
            _viewport_width: current_width,
            selected_index: self.selected_index,
            shared_hovered_index: self.shared_hovered_index.clone(),
            on_message: on_message.clone(),
            theme_colors,
        };

        let shader_widget = shader(program).width(Length::Fill).height(Length::Fill);

        // Create scrollbar overlay
        let scrollbar_visibility = self.scrollbar.visibility;
        let scrollbar_height_ratio = self.viewport.visible_height / self.viewport.content_height.max(1.0);
        let scrollbar_position = self.scrollbar.scroll_position;
        let max_scroll_y = self.viewport.max_scroll_y();

        let show_scrollbar = self.viewport.content_height > self.viewport.visible_height;

        if show_scrollbar {
            let scrollbar_view = ScrollbarOverlay::new(
                scrollbar_visibility,
                scrollbar_position,
                scrollbar_height_ratio,
                max_scroll_y,
                self.scrollbar_hover_state.clone(),
                move |x, y| on_message_scroll(FileListViewMessage::ScrollTo(x, y)),
                move |is_hovered| on_message_hover(FileListViewMessage::ScrollbarHovered(is_hovered)),
            )
            .view();

            // Stack shader with scrollbar overlay
            let list_content = container(stack![
                container(shader_widget).width(Length::Fill).height(Length::Fill),
                container(scrollbar_view)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Center)
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &iced::Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(iced::Background::Color(palette.background.base.color)),
                    border: iced::Border {
                        color: palette.background.strong.color,
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                }
            });

            // Add header row if in SAUCE mode
            if self.sauce_mode {
                column![self.render_header_row(), list_content].into()
            } else {
                list_content.into()
            }
        } else {
            let list_content = container(shader_widget).width(Length::Fill).height(Length::Fill).style(|theme: &iced::Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(iced::Background::Color(palette.background.base.color)),
                    border: iced::Border {
                        color: palette.background.strong.color,
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                }
            });

            // Add header row if in SAUCE mode
            if self.sauce_mode {
                column![self.render_header_row(), list_content].into()
            } else {
                list_content.into()
            }
        }
    }

    /// Render header row for SAUCE mode
    fn render_header_row<Message: 'static>(&self) -> Element<'_, Message> {
        use i18n_embed_fl::fl;

        let header_style = |theme: &iced::Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background.weak.color)),
                border: iced::Border {
                    color: palette.background.strong.color,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            }
        };

        let header_text_style = |theme: &iced::Theme| text::Style {
            color: Some(theme.extended_palette().background.base.text.scale_alpha(0.7)),
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

    fn draw(&self, _state: &Self::State, _cursor: mouse::Cursor, bounds: iced::Rectangle) -> Self::Primitive {
        // Apply hover state from shared state
        let hovered = *self.shared_hovered_index.lock();
        let items: Vec<ListItemRenderData> = self
            .items
            .iter()
            .map(|item| {
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

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: iced::Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::Action<Message>> {
        // Check if viewport size changed significantly (debounce threshold of 2px)
        let current_bounds = (bounds.width, bounds.height);
        let size_changed_significantly = match state.last_bounds {
            Some((last_w, last_h)) => (bounds.width - last_w).abs() > 2.0 || (bounds.height - last_h).abs() > 2.0,
            None => true,
        };
        if size_changed_significantly {
            state.last_bounds = Some(current_bounds);
            let msg = (self.on_message)(FileListViewMessage::SetViewportSize(bounds.width, bounds.height));
            return Some(iced::widget::Action::publish(msg));
        }

        // Handle hover detection
        let found_index = if let Some(cursor_pos) = cursor.position_in(bounds) {
            let item_y = cursor_pos.y + self.scroll_y;
            let index = (item_y / ITEM_HEIGHT) as usize;
            let item_count = (self.content_height / ITEM_HEIGHT) as usize;
            if index < item_count { Some(index) } else { None }
        } else {
            None
        };

        // Update shared and local state
        *self.shared_hovered_index.lock() = found_index;
        state.hovered_index = found_index;

        // Handle events
        let is_over = cursor.is_over(bounds);

        match event {
            iced::Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::WheelScrolled { delta } => {
                    if is_over {
                        let (dx, dy) = match delta {
                            mouse::ScrollDelta::Lines { x, y } => (*x * 20.0, *y * 20.0),
                            mouse::ScrollDelta::Pixels { x, y } => (*x, *y),
                        };
                        let msg = (self.on_message)(FileListViewMessage::Scroll(dx, dy));
                        return Some(iced::widget::Action::publish(msg));
                    }
                }
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if is_over {
                        if let Some(pos) = cursor.position_in(bounds) {
                            let msg = (self.on_message)(FileListViewMessage::Click(pos.y));
                            return Some(iced::widget::Action::publish(msg));
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn mouse_interaction(&self, state: &Self::State, _bounds: iced::Rectangle, _cursor: mouse::Cursor) -> mouse::Interaction {
        if state.hovered_index.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

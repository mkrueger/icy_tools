//! Right panel component
//!
//! Contains Minimap and Layers panels with a resizable split.

use std::sync::Arc;

use iced::{
    widget::{container, pane_grid},
    Element, Length, Task, Theme,
};

use icy_engine::Screen;
use icy_engine_edit::EditState;
use icy_engine_gui::SharedRenderCacheHandle;
use parking_lot::Mutex;

use crate::ui::{LayerMessage, LayerView, MinimapMessage, MinimapView, ViewportInfo};

/// Base width for the right panel (matches 80-char buffer display)
pub const RIGHT_PANEL_BASE_WIDTH: f32 = 320.0;

/// Gap between minimap and layer list panes (in logical pixels)
pub const RIGHT_PANEL_PANE_SPACING: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RightPane {
    Minimap,
    Layers,
}

/// Messages for the right panel
#[derive(Clone, Debug)]
pub enum RightPanelMessage {
    /// Minimap messages
    Minimap(MinimapMessage),
    /// Layer view messages
    Layers(LayerMessage),
    /// Pane grid resized
    PaneResized(pane_grid::ResizeEvent),
}

/// Right panel state
pub struct RightPanel {
    /// Minimap view
    pub minimap: MinimapView,
    /// Layer view
    pub layers: LayerView,

    panes: pane_grid::State<RightPane>,
}

impl Default for RightPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl RightPanel {
    pub fn new() -> Self {
        let (mut panes, minimap_pane) = pane_grid::State::new(RightPane::Minimap);
        let _ = panes.split(pane_grid::Axis::Horizontal, minimap_pane, RightPane::Layers);

        Self {
            minimap: MinimapView::new(),
            layers: LayerView::new(),
            panes,
        }
    }

    /// Update the right panel state
    pub fn update(&mut self, message: RightPanelMessage) -> Task<RightPanelMessage> {
        match message {
            RightPanelMessage::Minimap(msg) => self.minimap.update(msg).map(RightPanelMessage::Minimap),
            RightPanelMessage::Layers(msg) => self.layers.update(msg).map(RightPanelMessage::Layers),
            RightPanelMessage::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Task::none()
            }
        }
    }

    /// Render the right panel
    /// The panel has a fixed width of RIGHT_PANEL_BASE_WIDTH (320pt at 100% scale)
    /// `paste_mode` indicates whether we're in paste mode (affects layer view behavior)
    /// `network_mode` indicates collaboration mode (hides layers - not compatible with Moebius)
    pub fn view<'a>(
        &'a self,
        theme: &Theme,
        screen: &'a Arc<Mutex<Box<dyn Screen>>>,
        viewport_info: &ViewportInfo,
        render_cache: Option<&'a SharedRenderCacheHandle>,
        paste_mode: bool,
        network_mode: bool,
    ) -> Element<'a, RightPanelMessage> {
        // Determine current font page for consistent glyph rendering.
        let current_font_page: Option<usize> = {
            let mut screen_guard = screen.lock();
            let state = screen_guard
                .as_any_mut()
                .downcast_mut::<EditState>()
                .expect("AnsiEditor screen should always be EditState");
            Some(state.get_caret().font_page() as usize)
        };

        // Use pane_grid for resizable split view (minimap + layers)
        // In network mode, only show minimap (layers not compatible with Moebius)
        if network_mode {
            let minimap = self.minimap.view(theme, screen, viewport_info, render_cache).map(RightPanelMessage::Minimap);
            return container(minimap).width(Length::Fill).height(Length::Fill).into();
        }

        let pane_grid: Element<'a, RightPanelMessage> = pane_grid::PaneGrid::new(&self.panes, |_id, pane, _is_maximized| {
            let content: Element<'a, RightPanelMessage> = match pane {
                RightPane::Minimap => self.minimap.view(theme, screen, viewport_info, render_cache).map(RightPanelMessage::Minimap),
                RightPane::Layers => self.layers.view(theme, screen, current_font_page, paste_mode).map(RightPanelMessage::Layers),
            };
            pane_grid::Content::new(content)
        })
        .on_resize(10, RightPanelMessage::PaneResized)
        .spacing(RIGHT_PANEL_PANE_SPACING)
        .into();

        container(pane_grid).width(Length::Fill).height(Length::Fill).into()
        /*
        container(text("Right panel placeholder"))
           .width(Length::Fill)
           .height(Length::Fill)
           .into()*/
    }
}

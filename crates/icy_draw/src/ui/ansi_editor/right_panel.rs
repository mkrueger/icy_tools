//! Right panel component
//!
//! Contains Minimap (top, fills available space) and Layers (bottom, fixed height) panels.

use std::sync::Arc;

use iced::{
    Element, Length, Task,
    widget::{column, container, rule, text},
};

use icy_engine::Screen;
use parking_lot::Mutex;

use crate::ui::{LayerMessage, LayerView, MinimapMessage, MinimapView, ViewportInfo};

/// Base width for the right panel (matches 80-char buffer display)
pub const RIGHT_PANEL_BASE_WIDTH: f32 = 320.0;

/// Fixed height for the layer view section
const LAYER_VIEW_HEIGHT: f32 = 220.0;

/// Messages for the right panel
#[derive(Clone, Debug)]
pub enum RightPanelMessage {
    /// Minimap messages
    Minimap(MinimapMessage),
    /// Layer view messages
    Layers(LayerMessage),
}

/// Right panel state
pub struct RightPanel {
    /// Minimap view
    pub minimap: MinimapView,
    /// Layer view
    pub layers: LayerView,
}

impl Default for RightPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl RightPanel {
    pub fn new() -> Self {
        Self {
            minimap: MinimapView::new(),
            layers: LayerView::new(),
        }
    }

    /// Update the right panel state
    pub fn update(&mut self, message: RightPanelMessage) -> Task<RightPanelMessage> {
        match message {
            RightPanelMessage::Minimap(msg) => self.minimap.update(msg).map(RightPanelMessage::Minimap),
            RightPanelMessage::Layers(msg) => self.layers.update(msg).map(RightPanelMessage::Layers),
        }
    }

    /// Render the right panel
    /// The panel has a fixed width of RIGHT_PANEL_BASE_WIDTH (320pt at 100% scale)
    pub fn view<'a>(&'a self, screen: &'a Arc<Mutex<Box<dyn Screen>>>, viewport_info: &ViewportInfo) -> Element<'a, RightPanelMessage> {
        // Minimap fills available space above the layer view (no padding)
        let minimap = self.minimap.view(screen, viewport_info).map(RightPanelMessage::Minimap);

        // Layer view with fixed height at bottom
        let layers = self.layers.view(screen).map(RightPanelMessage::Layers);
        //let layers = text("foo");

        let content = column![
            // Minimap section - fills all available space above layer view
            container(minimap).width(Length::Fill).height(Length::Fill),
            rule::horizontal(1),
            // Layers section - fixed height at bottom
            container(column![text("Layers").size(12), layers].spacing(4))
                .padding(4)
                .width(Length::Fill)
                .height(Length::Fixed(LAYER_VIEW_HEIGHT)),
        ]
        .spacing(0);

        container(content).width(Length::Fill).height(Length::Fill).into()
    }
}

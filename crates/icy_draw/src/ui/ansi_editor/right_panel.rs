//! Right panel component
//!
//! Contains Tool Options, Minimap, Layers, and Channels panels in a vertical stack.

use std::sync::Arc;

use iced::{
    Element, Length, Task,
    widget::{column, container, rule, scrollable, text},
};

use icy_engine_edit::tools::Tool;
use icy_engine_edit::EditState;
use parking_lot::Mutex;

use crate::ui::{ChannelsMessage, ChannelsView, LayerMessage, LayerView, MinimapMessage, MinimapView};

/// Messages for the right panel
#[derive(Clone, Debug)]
pub enum RightPanelMessage {
    /// Minimap messages
    Minimap(MinimapMessage),
    /// Layer view messages
    Layers(LayerMessage),
    /// Channels view messages
    Channels(ChannelsMessage),
    /// Toggle panel collapse
    ToggleCollapse,
}

/// Right panel state
pub struct RightPanel {
    /// Whether the panel is collapsed
    pub is_collapsed: bool,
    /// Minimap view
    pub minimap: MinimapView,
    /// Layer view
    pub layers: LayerView,
    /// Channels view
    pub channels: ChannelsView,
}

impl Default for RightPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl RightPanel {
    pub fn new() -> Self {
        Self {
            is_collapsed: false,
            minimap: MinimapView::new(),
            layers: LayerView::new(),
            channels: ChannelsView::new(),
        }
    }

    /// Update the right panel state
    pub fn update(&mut self, message: RightPanelMessage) -> Task<RightPanelMessage> {
        match message {
            RightPanelMessage::Minimap(msg) => self.minimap.update(msg).map(RightPanelMessage::Minimap),
            RightPanelMessage::Layers(msg) => self.layers.update(msg).map(RightPanelMessage::Layers),
            RightPanelMessage::Channels(msg) => self.channels.update(msg).map(RightPanelMessage::Channels),
            RightPanelMessage::ToggleCollapse => {
                self.is_collapsed = !self.is_collapsed;
                Task::none()
            }
        }
    }

    /// Render the right panel
    pub fn view<'a>(&'a self, edit_state: &'a Arc<Mutex<EditState>>, current_tool: Tool) -> Element<'a, RightPanelMessage> {
        if self.is_collapsed {
            // Show minimal collapsed bar
            return container(text("▶").size(16)).width(Length::Fixed(20.0)).height(Length::Fill).into();
        }

        // Tool info section (show current tool name)
        let tool_info: Element<'_, RightPanelMessage> =
            container(column![text(current_tool.name()).size(12), text(current_tool.tooltip()).size(10),].spacing(2))
        .padding(4)
        .height(Length::Shrink)
        .into();
        // Full panel with Tool Info, Minimap, Layers, Channels
        let minimap = self.minimap.view(edit_state).map(RightPanelMessage::Minimap);

        let layers = self.layers.view(edit_state).map(RightPanelMessage::Layers);

        let channels = self.channels.view(edit_state).map(RightPanelMessage::Channels);

        let content = column![
            // Tool info section
            tool_info,
            rule::horizontal(1),
            // Minimap section
            container(column![text("Minimap").size(12), minimap,].spacing(4))
                .padding(4)
                .height(Length::Fixed(150.0)),
            rule::horizontal(1),
            // Layers section (scrollable, takes remaining space)
            container(column![text("Layers").size(12), scrollable(layers).height(Length::Fill),].spacing(4))
                .padding(4)
                .height(Length::Fill),
            rule::horizontal(1),
            // Channels section
            container(column![text("Channels").size(12), channels,].spacing(4))
                .padding(4)
                .height(Length::Fixed(60.0)),
        ]
        .spacing(0);

        container(content).width(Length::Fill).height(Length::Fill).into()
    }
}

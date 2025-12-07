//! Layer view component
//!
//! Shows the layer stack with visibility toggles and layer management controls.

use std::sync::Arc;

use iced::{Element, Length, Task, widget::{button, column, container, row, text}};

use icy_engine_edit::EditState;
use parking_lot::Mutex;

/// Messages for the layer view
#[derive(Clone, Debug)]
pub enum LayerMessage {
    /// Select a layer
    Select(usize),
    /// Toggle layer visibility
    ToggleVisibility(usize),
    /// Add new layer
    Add,
    /// Remove layer
    Remove(usize),
    /// Move layer up
    MoveUp(usize),
    /// Move layer down
    MoveDown(usize),
    /// Rename layer
    Rename(usize, String),
}

/// Layer view state
pub struct LayerView {
    // No additional state needed for now
}

impl Default for LayerView {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerView {
    pub fn new() -> Self {
        Self {}
    }

    /// Update the layer view state
    pub fn update(&mut self, _message: LayerMessage) -> Task<LayerMessage> {
        // Most messages are handled by the parent AnsiEditor
        Task::none()
    }

    /// Render the layer view
    pub fn view<'a>(&'a self, edit_state: &'a Arc<Mutex<EditState>>) -> Element<'a, LayerMessage> {
        let state = edit_state.lock();
        let buffer = state.get_buffer();
        let current_layer = state.get_current_layer().unwrap_or(0);

        let layers: Vec<Element<'_, LayerMessage>> = buffer.layers
            .iter()
            .enumerate()
            .rev() // Show top layer first
            .map(|(idx, layer)| {
                let is_selected = idx == current_layer;
                let is_visible = layer.get_is_visible();
                let title = layer.get_title();

                let layer_row = row![
                    // Visibility toggle (using button instead of checkbox)
                    button(
                        text(if is_visible { "👁" } else { "○" }).size(12)
                    )
                    .on_press(LayerMessage::ToggleVisibility(idx))
                    .width(Length::Fixed(24.0))
                    .padding(2),
                    
                    // Layer name (clickable to select)
                    button(
                        text(if title.is_empty() { 
                            format!("Layer {}", idx + 1) 
                        } else { 
                            title.to_string() 
                        })
                        .size(12)
                    )
                    .on_press(LayerMessage::Select(idx))
                    .style(if is_selected {
                        iced::widget::button::primary
                    } else {
                        iced::widget::button::text
                    })
                    .width(Length::Fill),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center);

                container(layer_row)
                    .padding(2)
                    .into()
            })
            .collect();

        // Layer list
        let layer_list = column(layers).spacing(2);

        // Control buttons at bottom
        let controls = row![
            button(text("+").size(14))
                .on_press(LayerMessage::Add)
                .padding(4),
            button(text("▲").size(14))
                .on_press(LayerMessage::MoveUp(current_layer))
                .padding(4),
            button(text("▼").size(14))
                .on_press(LayerMessage::MoveDown(current_layer))
                .padding(4),
            button(text("−").size(14))
                .on_press(LayerMessage::Remove(current_layer))
                .padding(4),
        ]
        .spacing(4);

        column![
            layer_list,
            controls,
        ]
        .spacing(8)
        .into()
    }
}

//! Canvas markers / selection / overlay updates for `AnsiEditorCore`.
//!
//! Split out of `ansi_editor.rs` so that the marker rebuilding,
//! selection-mask uploading and tag-overlay generation live in one place.

use std::path::PathBuf;

use icy_engine::{Screen, TextPane};
use icy_engine_edit::EditState;

use super::ansi_editor::AnsiEditorCore;
use super::tools;

impl AnsiEditorCore {
    /// Update the canvas markers based on current guide/raster settings
    pub(super) fn update_markers(&mut self) {
        // Get font dimensions from screen for pixel conversion
        let (font_width, font_height) = {
            let screen = self.screen.lock();
            let size = screen.font_dimensions();
            (size.width as f32, size.height as f32)
        };

        // Update raster grid in pixel coordinates
        if self.show_raster {
            if let Some((cols, rows)) = self.raster {
                // Convert character spacing to pixel spacing
                let pixel_width = cols * font_width;
                let pixel_height = rows * font_height;
                self.canvas.set_raster(Some((pixel_width, pixel_height)));
            } else {
                self.canvas.set_raster(None);
            }
        } else {
            self.canvas.set_raster(None);
        }

        // Update guide crosshair in pixel coordinates
        if self.show_guide {
            if let Some((col, row)) = self.guide {
                // Convert character position to pixel position
                let pixel_x = col * font_width;
                let pixel_y = row * font_height;
                self.canvas.set_guide(Some((pixel_x, pixel_y)));
            } else {
                self.canvas.set_guide(None);
            }
        } else {
            self.canvas.set_guide(None);
        }
    }

    /// Build EditorMarkers from current editor state.
    ///
    /// This collects all marker data (layer bounds, selection, etc.) in one place
    /// and returns it as an EditorMarkers struct to be passed to the view.
    pub(crate) fn build_editor_markers(&self) -> icy_engine_gui::EditorMarkers {
        let mut markers = icy_engine_gui::EditorMarkers::default();

        // Read current marker settings from canvas (which still holds them for now)
        // TODO: Eventually move all marker state out of canvas.terminal.markers
        {
            let term_markers = self.canvas.terminal.markers.read();
            markers.raster = term_markers.raster;
            markers.guide = term_markers.guide;
            markers.reference_image = term_markers.reference_image.clone();
            markers.marker_settings = term_markers.marker_settings.clone();
            markers.selection_rect = term_markers.selection_rect;
            markers.selection_color = term_markers.selection_color;
            markers.selection_mask_data = term_markers.selection_mask_data.clone();
            markers.tool_overlay_mask_data = term_markers.tool_overlay_mask_data.clone();
            markers.tool_overlay_rect = term_markers.tool_overlay_rect;
            markers.tool_overlay_cell_height_scale = term_markers.tool_overlay_cell_height_scale;
            markers.brush_preview_rect = term_markers.brush_preview_rect;
        }

        // Compute layer bounds and caret origin directly
        let is_paste = self.is_paste_mode();
        let show_borders = self.show_layer_borders || is_paste;
        markers.show_layer_bounds = show_borders;
        markers.paste_mode = is_paste;
        // Animate layer border when in paste mode (marching ants)
        markers.layer_border_animated = is_paste;

        // Get layer bounds from screen
        let mut screen = self.screen.lock();

        // Get font dimensions for pixel conversion
        let size = screen.font_dimensions();
        let (font_width, font_height) = (size.width as f32, size.height as f32);

        // Access the EditState to get buffer and current layer
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            let buffer = edit_state.get_buffer();

            // Caret should be rendered relative to the *current* layer.
            markers.caret_origin_px = edit_state
                .get_current_layer()
                .ok()
                .and_then(|idx| buffer.layers.get(idx))
                .map(|layer| {
                    let offset = layer.offset();
                    (offset.x as f32 * font_width, offset.y as f32 * font_height)
                })
                .unwrap_or((0.0, 0.0));

            // In paste mode, find the floating layer instead of current layer
            let target_layer = edit_state.get_current_layer().ok();

            if let Some(layer_idx) = target_layer {
                if let Some(layer) = buffer.layers.get(layer_idx) {
                    // Use offset() which respects preview_offset during drag
                    let offset = layer.offset();
                    let size = layer.size();
                    let width = size.width;
                    let height = size.height;

                    // Convert to pixels
                    let x = offset.x as f32 * font_width;
                    let y = offset.y as f32 * font_height;
                    let w = width as f32 * font_width;
                    let h = height as f32 * font_height;

                    markers.layer_bounds = Some((x, y, w, h));
                }
            }
        }

        markers
    }

    /// Update tag rectangle overlays when Tag tool is active
    pub(super) fn update_tag_overlays(&mut self) {
        // Snapshot selection from TagTool state (only when Tag tool is active).
        let selection: Vec<usize> = self
            .current_tool
            .as_any()
            .downcast_ref::<tools::TagTool>()
            .map(|t| t.state().selection.clone())
            .unwrap_or_default();

        // First, get all the data we need (position, length, is_selected)
        let (font_width, font_height, tag_data): (f32, f32, Vec<(icy_engine::Position, usize, bool)>) = {
            let mut screen = self.screen.lock();

            // Get font dimensions for pixel conversion
            let size = screen.font_dimensions();
            let (fw, fh) = (size.width as f32, size.height as f32);

            // Access EditState to get tags and update overlay mask
            let tags = if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                let tag_info = edit_state
                    .get_buffer()
                    .tags
                    .iter()
                    .enumerate()
                    .map(|(idx, tag)| (tag.position, tag.len(), selection.contains(&idx)))
                    .collect();
                tools::TagTool::update_overlay_mask_in_state(edit_state);
                tag_info
            } else {
                vec![]
            };

            (fw, fh, tags)
        };

        // Now render overlay to canvas (no longer holding screen lock)
        let (mask, rect) = tools::TagTool::overlay_mask_for_tags(font_width, font_height, &tag_data);
        self.canvas.set_tool_overlay_mask(mask, rect);
    }

    /// Lightweight update: only update the selection rectangle (no mask regeneration).
    /// Use this during drag operations for better performance.
    pub(super) fn update_selection_rect_only(&mut self) {
        use icy_engine::AddType;
        use icy_engine_gui::selection_colors;

        let (selection_rect, selection_color) = {
            let mut screen = self.screen.lock();
            let size = screen.font_dimensions();
            let font_width = size.width as f32;
            let font_height = size.height as f32;

            if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                let selection = edit_state.selection();

                let selection_color = match selection.map(|s| s.add_type) {
                    Some(AddType::Add) => selection_colors::ADD,
                    Some(AddType::Subtract) => selection_colors::SUBTRACT,
                    _ => selection_colors::DEFAULT,
                };

                if let Some(sel) = selection {
                    let rect = sel.as_rectangle();
                    let x = rect.left() as f32 * font_width;
                    let y = rect.top() as f32 * font_height;
                    let w = rect.width() as f32 * font_width;
                    let h = rect.height() as f32 * font_height;
                    (Some((x, y, w, h)), selection_color)
                } else {
                    (None, selection_colors::DEFAULT)
                }
            } else {
                (None, selection_colors::DEFAULT)
            }
        };

        self.canvas.set_selection(selection_rect);
        self.canvas.set_selection_color(selection_color);
        // Note: We intentionally do NOT update the selection mask here for performance
    }

    /// Update the selection display in the shader
    pub(super) fn update_selection_mask_display(&mut self) {
        use icy_engine::AddType;
        use icy_engine_gui::selection_colors;

        // Get selection from EditState and convert to pixel coordinates
        let (selection_rect, selection_color, selection_mask_data) = {
            let mut screen = self.screen.lock();

            // Get font dimensions for pixel conversion
            let size = screen.font_dimensions();
            let font_width = size.width as f32;
            let font_height = size.height as f32;

            // Access the EditState to get selection
            if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
                // Get the selection mask
                let selection_mask = edit_state.selection_mask();
                let selection = edit_state.selection();

                // Determine selection color based on add_type
                let selection_color = match selection.map(|s| s.add_type) {
                    Some(AddType::Add) => selection_colors::ADD,
                    Some(AddType::Subtract) => selection_colors::SUBTRACT,
                    _ => selection_colors::DEFAULT,
                };

                // Check if selection mask has content
                if !selection_mask.is_empty() {
                    // Generate texture data from selection mask.
                    // IMPORTANT: the shader samples this mask in *document cell coordinates* (0..buffer_w/0..buffer_h),
                    // so the texture must cover the full document size (no cropping/bounding-rect).
                    let buffer = edit_state.get_buffer();
                    let width = buffer.width().max(1) as u32;
                    let height = buffer.height().max(1) as u32;

                    // Create RGBA texture data (4 bytes per pixel)
                    let mut rgba_data = vec![0u8; (width * height * 4) as usize];

                    for y in 0..height {
                        for x in 0..width {
                            let doc_x = x as i32;
                            let doc_y = y as i32;
                            let is_selected = selection_mask.is_selected(icy_engine::Position::new(doc_x, doc_y));

                            let pixel_idx = ((y * width + x) * 4) as usize;
                            if is_selected {
                                // White = selected
                                rgba_data[pixel_idx] = 255;
                                rgba_data[pixel_idx + 1] = 255;
                                rgba_data[pixel_idx + 2] = 255;
                                rgba_data[pixel_idx + 3] = 255;
                            } else {
                                // Black = not selected
                                rgba_data[pixel_idx] = 0;
                                rgba_data[pixel_idx + 1] = 0;
                                rgba_data[pixel_idx + 2] = 0;
                                rgba_data[pixel_idx + 3] = 255;
                            }
                        }
                    }

                    // Selection rect is the *active* rectangular selection only (if present), not the mask bounds.
                    let selection_rect = selection.map(|sel| {
                        let rect = sel.as_rectangle();
                        let x = rect.left() as f32 * font_width;
                        let y = rect.top() as f32 * font_height;
                        // Selection.size() already returns inclusive dimensions (+1), no need to add again
                        let w = rect.width() as f32 * font_width;
                        let h = rect.height() as f32 * font_height;
                        (x, y, w, h)
                    });

                    (selection_rect, selection_color, Some((rgba_data, width, height)))
                } else if let Some(sel) = selection {
                    // No mask, but have selection rectangle
                    let rect = sel.as_rectangle();
                    let x = rect.left() as f32 * font_width;
                    let y = rect.top() as f32 * font_height;
                    // Selection.size() already returns inclusive dimensions (+1), no need to add again
                    let w = rect.width() as f32 * font_width;
                    let h = rect.height() as f32 * font_height;

                    (Some((x, y, w, h)), selection_color, None)
                } else {
                    (None, selection_colors::DEFAULT, None)
                }
            } else {
                (None, selection_colors::DEFAULT, None)
            }
        };

        self.canvas.set_selection(selection_rect);
        self.canvas.set_selection_color(selection_color);
        self.canvas.set_selection_mask(selection_mask_data);
    }

    /// Set or update the reference image
    pub fn set_reference_image(&mut self, path: Option<PathBuf>, alpha: f32) {
        self.canvas.set_reference_image(path, alpha);
    }

    /// Toggle reference image visibility
    pub fn toggle_reference_image(&mut self) {
        self.canvas.toggle_reference_image();
    }
}

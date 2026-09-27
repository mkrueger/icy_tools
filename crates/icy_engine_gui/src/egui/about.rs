//! About dialog shared by the icy tools: the tool's `about.icy` artwork, scaled to fit the
//! window, with clickable hyperlinks and a close button.

use std::sync::Arc;

use ::egui;
use icy_engine::{formats::FileFormat, Position, Screen};
use parking_lot::Mutex;

use super::dialog::{labels, Dialog, DialogButton};
use crate::{
    terminal::egui::TerminalCallback, version_helper::replace_version_marker, CRTShaderProgram, CRTShaderState, MonitorSettings, ScalingMode, Terminal,
};

pub struct AboutDialog {
    terminal: Terminal,
    shader_state: CRTShaderState,
}

impl AboutDialog {
    /// Loads the artwork of an `.icy` file and fills in its version marker.
    pub fn new(artwork: &[u8], version: &semver::Version, build_date: Option<String>) -> Result<Self, String> {
        let mut screen = FileFormat::IcyDraw
            .from_bytes(artwork, Some(icy_engine::formats::LoadData::new(Some(icy_parser_core::MusicOption::Off), None)))
            .map_err(|error| error.to_string())?
            .screen;
        replace_version_marker(&mut screen.buffer, version, build_date);
        screen.caret.visible = false;
        let shader_state = CRTShaderState::from_screen(&screen);
        let terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen) as Box<dyn Screen>)));
        Ok(Self { terminal, shader_state })
    }

    /// Shows the dialog. Returns the URL of a clicked hyperlink in `link`, and false once closed.
    pub fn show_with_link(&mut self, context: &egui::Context, link: &mut Option<String>) -> bool {
        let artwork = self.terminal.screen.lock().resolution();
        let response = Dialog::new("icy-about")
            .max_width(artwork.width as f32 + 48.0)
            .scroll(false)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    let available = egui::vec2(ui.available_width(), (context.content_rect().height() - 190.0).max(120.0));
                    let zoom = ScalingMode::Auto.compute_zoom(artwork.width as f32, artwork.height as f32, available.x, available.y, false);
                    let size = egui::vec2(self.terminal.content_width() * zoom, self.terminal.content_height() * zoom);
                    let offset = ((ui.available_width() - size.x) / 2.0).max(0.0);
                    ui.horizontal(|ui| {
                        ui.add_space(offset);
                        let (bounds, response) = ui.allocate_exact_size(size, egui::Sense::click());
                        self.terminal.update_scroll_viewport([0.0, 0.0, bounds.width(), bounds.height()], zoom);
                        let mut settings = MonitorSettings::neutral();
                        settings.use_integer_scaling = false;
                        let frame = CRTShaderProgram::new(&self.terminal, Arc::new(settings), None).frame(
                            &self.shader_state,
                            [bounds.width(), bounds.height()],
                            ui.ctx().pixels_per_point(),
                        );
                        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                            bounds,
                            TerminalCallback {
                                frame,
                                bounds,
                                context: ui.ctx().clone(),
                            },
                        ));
                        if let Some(url) = self.hyperlink_at(response.hover_pos()) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            if response.clicked() && !ui.ctx().will_discard() {
                                *link = Some(url);
                            }
                        }
                    });
                });
                dialog.buttons([DialogButton::primary(labels::close(), ()).cancels()]);
            });
        response.action.is_none() && !response.dismissed
    }

    /// Shows the dialog and opens clicked hyperlinks in the browser. Returns false once closed.
    pub fn show(&mut self, context: &egui::Context) -> bool {
        let mut link = None;
        let open = self.show_with_link(context, &mut link);
        if let Some(url) = link {
            context.open_url(egui::OpenUrl::new_tab(url));
        }
        open
    }

    fn hyperlink_at(&self, pointer: Option<egui::Pos2>) -> Option<String> {
        let pointer = pointer?;
        let (column, row) = self.terminal.render_info.read().screen_to_cell(pointer.x, pointer.y)?;
        let position = Position::new(column, row);
        let screen = self.terminal.screen.lock();
        screen
            .hyperlinks()
            .iter()
            .find(|link| screen.is_position_in_range(position, link.position, link.length))
            .map(|link| link.url(&**screen))
    }
}

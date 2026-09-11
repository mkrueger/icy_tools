//! About screen: shows the shared ANSI artwork in a terminal view, like the legacy client.

use std::sync::Arc;

use eframe::{egui, egui_wgpu};
use icy_engine::{formats::FileFormat, Screen};
use icy_engine_gui::{terminal::egui::TerminalCallback, CRTShaderProgram, CRTShaderState, MonitorSettings, ScalingMode, Terminal};
use parking_lot::Mutex;

use super::{appearance, navigation};

const ABOUT_ANSI: &[u8] = include_bytes!("../../../data/about.icy");

pub struct About {
    terminal: Terminal,
    shader_state: CRTShaderState,
}

impl About {
    pub fn load() -> Result<Self, String> {
        let mut screen = FileFormat::IcyDraw
            .from_bytes(
                ABOUT_ANSI,
                Some(icy_engine::formats::LoadData::new(Some(icy_parser_core::MusicOption::Off), None)),
            )
            .map_err(|error| error.to_string())?
            .screen;
        if let Ok(version) = semver::Version::parse(env!("CARGO_PKG_VERSION")) {
            icy_engine_gui::version_helper::replace_version_marker(&mut screen.buffer, &version, option_env!("ICY_BUILD_DATE").map(String::from));
        }
        screen.caret.visible = false;
        let shader_state = CRTShaderState::from_screen(&screen);
        let terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen) as Box<dyn Screen>)));
        Ok(Self { terminal, shader_state })
    }

    /// Returns a URL when one of the artwork's hyperlinks was clicked.
    pub fn show(&mut self, context: &egui::Context, open: &mut bool) -> Option<String> {
        let mut link = None;
        let mut close = false;
        let artwork = {
            let screen = self.terminal.screen.lock();
            screen.resolution()
        };
        let response = appearance::Dialog::new("about", "Icy Term")
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
                                link = Some(url);
                            }
                        }
                    });
                });
                dialog.actions(|ui| {
                    close |= ui.add(appearance::primary_button(tr!("egui-close"))).clicked();
                });
            });
        if close || response.closed {
            *open = false;
        }
        link
    }

    fn hyperlink_at(&self, pointer: Option<egui::Pos2>) -> Option<String> {
        let pointer = pointer?;
        let (column, row) = self.terminal.render_info.read().screen_to_cell(pointer.x, pointer.y)?;
        let screen = self.terminal.screen.lock();
        match navigation::click_action(&**screen, icy_engine::Position::new(column, row)) {
            Some(navigation::ClickAction::Link(url)) => Some(url),
            _ => None,
        }
    }
}

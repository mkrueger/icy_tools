use std::sync::Arc;

use ::egui::{self, Vec2};
use icy_engine::{Screen, TextScreen};
use parking_lot::Mutex;

use crate::{terminal::egui::TerminalCallback, CRTShaderProgram, CRTShaderState, MonitorSettings, Terminal};

pub struct ScreenView {
    pub terminal: Terminal,
    pub shader_state: CRTShaderState,
    pub scroll_to: Option<Vec2>,
    pub offset: Vec2,
    pub max_offset: Vec2,
    pub zoom: f32,
}

impl ScreenView {
    pub fn new(screen: TextScreen) -> Self {
        let shader_state = CRTShaderState::from_screen(&screen);
        let terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen) as Box<dyn Screen>)));
        Self {
            terminal,
            shader_state,
            scroll_to: None,
            offset: Vec2::ZERO,
            max_offset: Vec2::ZERO,
            zoom: 1.0,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, settings: &MonitorSettings) -> egui::Response {
        schedule_frame(ui.ctx(), &self.terminal, &mut self.shader_state, settings);
        let available = ui.available_size().max(Vec2::splat(1.0));
        let resolution = self.terminal.screen.lock().resolution();
        self.zoom = settings
            .scaling_mode
            .compute_zoom(
                resolution.width as f32,
                resolution.height as f32,
                available.x,
                available.y,
                settings.use_integer_scaling,
            )
            .max(0.01);
        let content = egui::vec2(self.terminal.content_width(), self.terminal.content_height()) * self.zoom;
        let size = if settings.scaling_mode.is_fit_width() {
            egui::vec2(available.x, content.y)
        } else if settings.scaling_mode.is_auto() {
            available
        } else {
            content
        }
        .max(available);
        self.max_offset = (size - available).max(Vec2::ZERO);
        let mut scroll = egui::ScrollArea::both()
            .id_salt(self.shader_state.instance_id)
            .auto_shrink([false, false])
            .scroll_source(egui::scroll_area::ScrollSource {
                drag: false,
                ..Default::default()
            });
        if let Some(offset) = self.scroll_to.take() {
            scroll = scroll.scroll_offset(offset.max(Vec2::ZERO).min(self.max_offset));
        }
        scroll
            .show_viewport(ui, |ui, viewport| {
                let origin = ui.cursor().min;
                ui.allocate_space(size);
                self.offset = viewport.min.to_vec2();
                let bounds = egui::Rect::from_min_size(origin + self.offset, viewport.size());
                let response = ui.interact(bounds, ui.id().with("screen"), egui::Sense::click_and_drag());
                self.terminal
                    .update_scroll_viewport([self.offset.x, self.offset.y, bounds.width(), bounds.height()], self.zoom);
                let mut settings = settings.clone();
                if self.zoom < 1.0 {
                    settings.use_integer_scaling = false;
                }
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
                response
            })
            .inner
    }
}

pub fn schedule_frame(context: &egui::Context, terminal: &Terminal, state: &mut CRTShaderState, settings: &MonitorSettings) {
    let now = crate::Blink::now_ms();
    let screen = terminal.screen.lock();
    let buffer_type = screen.buffer_type();
    let caret = screen.caret();
    let mut delay = screen.terminal_state().synchronized_output_remaining();
    for (blink, enabled, rate) in [
        (
            &mut state.caret_blink,
            caret.visible && caret.blinking && terminal.has_focus,
            buffer_type.caret_blink_rate(),
        ),
        (&mut state.character_blink, screen.ice_mode().has_blink(), buffer_type.blink_rate()),
    ] {
        if enabled {
            blink.set_rate(rate as u128);
            blink.update(now);
            let remaining = std::time::Duration::from_millis(blink.time_until_next(now) as u64 + 1);
            delay = Some(delay.map_or(remaining, |current| current.min(remaining)));
        }
    }
    if settings.use_noise {
        let noise = std::time::Duration::from_millis(33);
        delay = Some(delay.map_or(noise, |current| current.min(noise)));
    }
    if let Some(delay) = delay {
        context.request_repaint_after(delay);
    }
}

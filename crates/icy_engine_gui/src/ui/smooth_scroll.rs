use egui::{Color32, EventFilter, Id, Pos2, Rect, Response, Sense, Ui, Vec2};

use crate::{TerminalCalc, TerminalOptions};

pub struct SmoothScroll {
    /// Current scroll position in terminal pixels (not screen pixels)
    char_scroll_position: Vec2,
    /// used to determine if the buffer should auto scroll to the bottom.
    last_char_height: f32,
    drag_horiz_start: bool,
    drag_vert_start: bool,
    lock_focus: bool,
    hide_scrollbars: bool,
    stick_to_bottom: bool,
    interactive: bool,
    scroll_offset_x: Option<f32>,
    scroll_offset_y: Option<f32>,
    /// Scroll position set by the user
    set_scroll_position: bool,
}

impl Default for SmoothScroll {
    fn default() -> Self {
        Self::new()
    }
}

impl SmoothScroll {
    pub fn new() -> Self {
        Self {
            char_scroll_position: Vec2::ZERO,
            last_char_height: 0.0,
            drag_horiz_start: false,
            drag_vert_start: false,
            lock_focus: true,
            stick_to_bottom: true,
            scroll_offset_x: None,
            scroll_offset_y: None,
            set_scroll_position: false,
            hide_scrollbars: false,
            interactive: true,
        }
    }

    pub fn with_interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    pub fn with_lock_focus(mut self, lock_focus: bool) -> Self {
        self.lock_focus = lock_focus;
        self
    }

    pub fn with_hide_scrollbars(mut self, hide_scrollbars: bool) -> Self {
        self.hide_scrollbars = hide_scrollbars;
        self
    }

    pub(crate) fn with_stick_to_bottom(mut self, stick_to_bottom: bool) -> Self {
        self.stick_to_bottom = stick_to_bottom;
        self
    }

    pub(crate) fn with_scroll_x_offset(mut self, scroll_offset: Option<f32>) -> Self {
        self.scroll_offset_x = scroll_offset;
        self
    }
    pub(crate) fn with_scroll_y_offset(mut self, scroll_offset: Option<f32>) -> Self {
        self.scroll_offset_y = scroll_offset;
        self
    }

    fn persist_data(&mut self, ui: &Ui, id: Id) {
        ui.ctx().memory_mut(|mem: &mut egui::Memory| {
            mem.data.insert_persisted(
                id,
                (self.char_scroll_position, self.last_char_height, self.drag_horiz_start, self.drag_vert_start),
            );
        });
    }

    fn load_data(&mut self, ui: &Ui, id: Id) {
        if let Some(scroll) = ui.ctx().memory_mut(|mem| mem.data.get_persisted::<(Vec2, f32, bool, bool)>(id)) {
            self.char_scroll_position = scroll.0;
            if self.char_scroll_position.x.is_nan() {
                self.char_scroll_position.x = 0.0;
            }
            if self.char_scroll_position.y.is_nan() {
                self.char_scroll_position.y = 0.0;
            }
            self.last_char_height = scroll.1;
            self.drag_horiz_start = scroll.2;
            self.drag_vert_start = scroll.3;
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        options: &TerminalOptions,
        calc_contents: impl FnOnce(Rect, &TerminalOptions) -> TerminalCalc,
        add_contents: impl FnOnce(&mut Ui, &mut TerminalCalc, &TerminalOptions),
    ) -> (Response, TerminalCalc) {
        let desired_size = if let Some(terminal_size) = options.terminal_size {
            terminal_size
        } else {
            ui.available_size()
        };
        let (id, rect) = ui.allocate_space(desired_size);
        self.load_data(ui, id);

        let mut response = ui.interact(rect, id, Sense::click_and_drag());
        response.intrinsic_size = Some(desired_size);

        if self.interactive && response.clicked() {
            response.request_focus();
        }
        if self.interactive && ui.memory(|mem| mem.has_focus(id)) {
            ui.memory_mut(|mem: &mut egui::Memory| {
                mem.set_focus_lock_filter(
                    id,
                    EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                )
            });
        }

        let rect = response.rect;

        let mut calc = calc_contents(rect, options);
        calc.char_scroll_position = self.char_scroll_position;

        if self.stick_to_bottom && (calc.char_height - self.last_char_height).abs() > 0.1 {
            self.char_scroll_position = Vec2::new(
                calc.font_width * (calc.char_width - calc.buffer_char_width).max(0.0),
                calc.font_height * (calc.char_height - calc.buffer_char_height).max(0.0),
            );
        }
        self.last_char_height = calc.char_height;

        if let Some(sp) = self.scroll_offset_x {
            if sp.is_nan() {
                log::error!("scroll_offset_x is NaN");
            } else {
                self.char_scroll_position.x = sp.floor();
            }
        }
        if let Some(sp) = self.scroll_offset_y {
            if sp.is_nan() {
                log::error!("scroll_offset_y is NaN");
            } else {
                self.char_scroll_position.y = sp.floor();
            }
        }
        self.clamp_scroll_position(&mut calc);

        let scrollbar_width = ui.style().spacing.scroll.bar_width;
        let x = rect.right() - scrollbar_width;
        let mut scrollbar_rect: Rect = rect;
        scrollbar_rect.set_left(x);
        calc.vert_scrollbar_rect = scrollbar_rect;

        let scrollbar_height = ui.style().spacing.scroll.bar_width;
        let y = rect.bottom() - scrollbar_height;
        let mut scrollbar_rect: Rect = rect;
        scrollbar_rect.set_top(y);
        calc.horiz_scrollbar_rect = scrollbar_rect;

        calc.has_focus |= response.has_focus();
        add_contents(ui, &mut calc, options);

        let has_horiz_scollbar = calc.char_width > calc.buffer_char_width;
        let has_vert_scrollbar = calc.char_height > calc.buffer_char_height;
        if has_vert_scrollbar && !self.hide_scrollbars {
            self.clamp_scroll_position(&mut calc);
            self.show_vertical_scrollbar(ui, &response, &mut calc, has_horiz_scollbar);
        }
        if response.has_focus() {
            calc.has_focus = true;
        }

        if response.clicked() || options.request_focus {
            response.request_focus();
        }

        if has_horiz_scollbar && !self.hide_scrollbars {
            self.clamp_scroll_position(&mut calc);
            self.show_horizontal_scrollbar(ui, &response, &mut calc, has_vert_scrollbar);
        }
        if response.has_focus() {
            calc.has_focus = true;
        }
        self.persist_data(ui, id);
        calc.set_scroll_position_set_by_user = self.set_scroll_position;

        self.clamp_scroll_position(&mut calc);

        (response, calc)
    }

    fn clamp_scroll_position(&mut self, calc: &mut TerminalCalc) {
        self.char_scroll_position.y = self.char_scroll_position.y.clamp(0.0, calc.max_y_scroll()).floor();
        self.char_scroll_position.x = self.char_scroll_position.x.clamp(0.0, calc.max_x_scroll()).floor();

        calc.char_scroll_position = self.char_scroll_position;
    }

    fn show_vertical_scrollbar(&mut self, ui: &Ui, response: &Response, calc: &mut TerminalCalc, has_horiz_scrollbar: bool) {
        let scrollbar_width = ui.style().spacing.scroll.bar_width;
        let x = calc.terminal_rect.right() - scrollbar_width;
        let mut bg_rect: Rect = calc.terminal_rect;
        bg_rect.set_left(x);

        let max_y_scroll = calc.max_y_scroll();
        let term_height = calc.terminal_rect.height() - if has_horiz_scrollbar { scrollbar_width } else { 0.0 };
        let bar_height = term_height * calc.buffer_char_height / calc.char_height;

        let bar_offset = -bar_height / 2.0;
        let how_on = if ui.is_enabled() {
            let (dragged, hovered) = self.handle_user_input_vert(ui, &response, x, bar_offset, calc, bg_rect);
            self.clamp_scroll_position(calc);
            ui.ctx().animate_bool(response.id.with("_vert"), hovered || dragged)
        } else {
            0.0
        };

        let x_size = egui::lerp(2.0..=scrollbar_width, how_on);

        // draw bg
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(calc.terminal_rect.right() - x_size, bg_rect.top()), Vec2::new(x_size, term_height)),
            0.,
            Color32::from_rgba_unmultiplied(0x3F, 0x3F, 0x3F, 32),
        );

        // draw bar
        let bar_top = calc.terminal_rect.top() + term_height * self.char_scroll_position.y / (max_y_scroll + calc.buffer_char_height * calc.font_height);
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(calc.terminal_rect.right() - x_size, bar_top), Vec2::new(x_size, bar_height)),
            4.,
            Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x5F + (127.0 * how_on) as u8),
        );
    }

    fn show_horizontal_scrollbar(&mut self, ui: &Ui, response: &Response, calc: &mut TerminalCalc, has_vert_scrollbar: bool) {
        let scrollbar_height = ui.style().spacing.scroll.bar_width;
        let y = calc.terminal_rect.bottom() - scrollbar_height;
        let mut bg_rect: Rect = calc.terminal_rect;
        bg_rect.set_top(y);

        let max_x_scroll = calc.max_x_scroll();
        let term_width = calc.terminal_rect.width() - if has_vert_scrollbar { scrollbar_height } else { 0.0 };
        let bar_width = term_width * calc.buffer_char_width / calc.char_width;
        let bar_offset = -bar_width / 2.0;

        let how_on = if ui.is_enabled() {
            let (dragged, hovered) = self.handle_user_input_horiz(ui, response, y, bar_offset, calc, bg_rect);
            self.clamp_scroll_position(calc);
            ui.ctx().animate_bool(response.id.with("_horiz"), hovered || dragged)
        } else {
            0.0
        };

        let y_size = egui::lerp(2.0..=scrollbar_height, how_on);

        // draw bg
        ui.painter().rect_filled(
            Rect::from_min_size(
                Pos2::new(calc.terminal_rect.left(), bg_rect.bottom() - y_size),
                Vec2::new(calc.terminal_rect.width(), y_size),
            ),
            0.,
            Color32::from_rgba_unmultiplied(0x3F, 0x3F, 0x3F, 32),
        );

        // draw bar
        let bar_left = calc.terminal_rect.left() + term_width * self.char_scroll_position.x / (max_x_scroll + calc.buffer_char_width * calc.font_width);
        ui.painter().rect_filled(
            Rect::from_min_size(Pos2::new(bar_left, calc.terminal_rect.bottom() - y_size), Vec2::new(bar_width, y_size)),
            4.,
            Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x5F + (127.0 * how_on) as u8),
        );
    }

    fn handle_user_input_vert(&mut self, ui: &Ui, response: &Response, x: f32, bar_offset: f32, calc: &TerminalCalc, bg_rect: Rect) -> (bool, bool) {
        if response.clicked() {
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                if mouse_pos.x > x {
                    let my = mouse_pos.y + bar_offset;
                    self.char_scroll_position = Vec2::new(
                        self.char_scroll_position.x,
                        calc.char_height * calc.font_height * (my - bg_rect.top()) / bg_rect.height().max(1.0),
                    );
                    self.set_scroll_position = true;
                }
            }
        }

        let mut dragged: bool = false;

        if self.drag_vert_start && response.dragged() {
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                dragged = true;
                let my = mouse_pos.y + bar_offset;
                self.char_scroll_position = Vec2::new(
                    self.char_scroll_position.x,
                    calc.char_height * calc.font_height * (my - bg_rect.top()) / bg_rect.height().max(1.0),
                );
                self.set_scroll_position = true;
            }
        }
        let mut hovered = false;
        if response.hovered() {
            let events: Vec<egui::Event> = ui.input(|i| i.events.clone());
            for e in events {
                if let egui::Event::MouseWheel { unit, delta, modifiers, .. } = e {
                    if !(modifiers.ctrl || modifiers.mac_cmd) {
                        let modifier = match unit {
                            egui::MouseWheelUnit::Point => 1.0,
                            egui::MouseWheelUnit::Line => calc.char_height,
                            egui::MouseWheelUnit::Page => calc.char_height * calc.buffer_char_height,
                        };
                        self.char_scroll_position.y -= delta.y * modifier;
                        self.set_scroll_position = true;
                    }
                }
            }

            if let Some(mouse_pos) = response.hover_pos() {
                if mouse_pos.x > x {
                    hovered = true;
                }
            }
        }

        if hovered && response.drag_started() {
            self.drag_vert_start = true;
        }

        if response.drag_stopped() {
            self.drag_vert_start = false;
        }
        (dragged, hovered)
    }

    fn handle_user_input_horiz(&mut self, ui: &Ui, response: &Response, y: f32, bar_offset: f32, calc: &TerminalCalc, bg_rect: Rect) -> (bool, bool) {
        if response.clicked() {
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                if mouse_pos.y > y {
                    let mx = mouse_pos.x + bar_offset;
                    self.char_scroll_position = Vec2::new(
                        calc.char_width * calc.font_width * (mx - bg_rect.left()) / bg_rect.width().max(1.0),
                        self.char_scroll_position.y,
                    );
                    self.set_scroll_position = true;
                }
            }
        }

        let mut dragged: bool = false;

        if self.drag_horiz_start && response.dragged() {
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                dragged = true;
                let mx = mouse_pos.x + bar_offset;
                self.char_scroll_position = Vec2::new(
                    calc.char_width * calc.font_width * (mx - bg_rect.left()) / bg_rect.width().max(1.0),
                    self.char_scroll_position.y,
                );
                self.set_scroll_position = true;
            }
        }
        let mut hovered = false;
        if response.hovered() {
            let events: Vec<egui::Event> = ui.input(|i| i.events.clone());
            for e in events {
                if let egui::Event::MouseWheel { unit, delta, .. } = e {
                    let modifier = match unit {
                        egui::MouseWheelUnit::Point => 1.0,
                        egui::MouseWheelUnit::Line => calc.char_width,
                        egui::MouseWheelUnit::Page => calc.char_width * calc.buffer_char_width,
                    };
                    self.char_scroll_position.x -= delta.x * modifier;
                    self.set_scroll_position = true;
                }
            }

            if let Some(mouse_pos) = response.hover_pos() {
                if mouse_pos.y > y {
                    hovered = true;
                }
            }
        }

        if hovered && response.drag_started() {
            self.drag_horiz_start = true;
        }

        if response.drag_stopped() {
            self.drag_horiz_start = false;
        }
        (dragged, hovered)
    }
}

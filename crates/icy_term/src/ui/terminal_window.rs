#![allow(clippy::float_cmp)]
use eframe::{
    egui::{self, CursorIcon, PointerButton},
    epaint::Vec2,
};
use egui::{ImageButton, Margin, Modifiers, RichText};
use i18n_embed_fl::fl;
use icy_engine::{Position, Selection, TextPane};
use web_time::Duration;

use crate::{
    icons::{CALL, DOWNLOAD, KEY, LOGOUT, MENU, UPLOAD},
    LATEST_VERSION, VERSION,
};

use super::{dialogs, MainWindow, MainWindowMode};

fn encode_mouse_button(button: i32) -> char {
    unsafe { char::from_u32_unchecked(b' '.saturating_add(button as u8) as u32) }
}
fn encode_mouse_position(pos: i32) -> char {
    unsafe { char::from_u32_unchecked(b'!'.saturating_add(pos as u8) as u32) }
}

impl MainWindow {
    pub fn update_terminal_window(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame, show_dialing_directory: bool) {
        let toolbar_bg_color = ctx.style().visuals.extreme_bg_color;
        let button_frame = egui::containers::Frame::none().fill(toolbar_bg_color).inner_margin(Margin::same(6.0));

        let enable_ui = matches!(self.get_mode(), MainWindowMode::ShowTerminal);

        if !self.is_fullscreen_mode {
            egui::TopBottomPanel::top("button_bar").frame(button_frame).show(ctx, |ui| {
                if !enable_ui {
                    ui.disable();
                }
                ui.horizontal(|ui| {
                    let r = ui.add(ImageButton::new(UPLOAD.clone().tint(crate::ui::button_tint(ui)))).on_hover_ui(|ui| {
                        ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "terminal-upload")).small());
                    });

                    if r.clicked() {
                        self.set_mode(ctx, MainWindowMode::SelectProtocol(false));
                    }

                    let r = ui.add(ImageButton::new(DOWNLOAD.clone().tint(crate::ui::button_tint(ui)))).on_hover_ui(|ui| {
                        ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "terminal-download")).small());
                    });

                    if r.clicked() {
                        self.set_mode(ctx, MainWindowMode::SelectProtocol(true));
                    }
                    let mut send_login = false;
                    if let Some(auto_login) = &mut self.terminal_thread.lock().auto_login {
                        if !auto_login.logged_in {
                            let r = ui.add(ImageButton::new(KEY.clone().tint(crate::ui::button_tint(ui)))).on_hover_ui(|ui| {
                                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "terminal-autologin")).small());
                            });

                            if r.clicked() {
                                send_login = true;
                                auto_login.logged_in = true;
                            }
                        }
                    }
                    if send_login {
                        self.send_login();
                    }

                    let r: egui::Response = ui.add(ImageButton::new(CALL.clone().tint(crate::ui::button_tint(ui)))).on_hover_ui(|ui| {
                        ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "terminal-dialing_directory")).small());
                    });

                    if r.clicked() {
                        self.set_mode(ctx, MainWindowMode::ShowDialingDirectory);
                    }

                    let mut mode = None;
                    if let Some(auto_login) = &mut self.terminal_thread.lock().auto_login {
                        if auto_login.iemsi.isi.is_some() {
                            if self.get_mode() == MainWindowMode::ShowIEMSI {
                                let r: egui::Response = ui.add(egui::Button::new(RichText::new(fl!(crate::LANGUAGE_LOADER, "toolbar-hide-iemsi"))));

                                if r.clicked() {
                                    mode = Some(MainWindowMode::ShowTerminal);
                                }
                            } else {
                                let r: egui::Response = ui.add(egui::Button::new(RichText::new(fl!(crate::LANGUAGE_LOADER, "toolbar-show-iemsi"))));

                                if r.clicked() {
                                    mode = Some(MainWindowMode::ShowIEMSI);
                                }
                            }
                        }
                    }

                    if let Some(mode) = mode {
                        self.set_mode(ctx, mode);
                    }

                    if self.terminal_thread.lock().sound_thread.lock().is_playing() {
                        let button_text = match self.terminal_thread.lock().sound_thread.lock().stop_button {
                            0 => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing1"),
                            1 => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing2"),
                            2 => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing3"),
                            3 => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing4"),
                            4 => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing5"),
                            _ => fl!(crate::LANGUAGE_LOADER, "toolbar-stop-playing6"),
                        };

                        let r: egui::Response = ui.add(egui::Button::new(RichText::new(button_text)));
                        if r.clicked() {
                            self.terminal_thread.lock().sound_thread.lock().clear();
                        }
                    }

                    if self.terminal_thread.lock().capture_dialog.capture_session {
                        let r: egui::Response = ui.add(egui::Button::new(RichText::new(fl!(crate::LANGUAGE_LOADER, "toolbar-stop-capture"))));

                        if r.clicked() {
                            self.terminal_thread.lock().capture_dialog.capture_session = false;
                        }
                    }
                    if *VERSION < *LATEST_VERSION {
                        ui.hyperlink_to(
                            fl!(crate::LANGUAGE_LOADER, "menu-upgrade_version", version = LATEST_VERSION.to_string()),
                            "https://github.com/mkrueger/icy_tools/releases",
                        );
                    }

                    let size = ui.available_size_before_wrap();
                    ui.add_space(size.x - 70.0);

                    let r = ui.add(ImageButton::new(LOGOUT.clone().tint(crate::ui::button_tint(ui)))).on_hover_ui(|ui| {
                        ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "terminal-hangup")).small());
                    });
                    if r.clicked() {
                        self.hangup(ctx);
                    }
                    ui.menu_image_button(MENU.clone().tint(crate::ui::button_tint(ui)), |ui| {
                        let r = ui.hyperlink_to(
                            fl!(crate::LANGUAGE_LOADER, "menu-item-discuss"),
                            "https://github.com/mkrueger/icy_tools/discussions",
                        );
                        if r.clicked() {
                            ui.close_menu();
                        }
                        let r = ui.hyperlink_to(
                            fl!(crate::LANGUAGE_LOADER, "menu-item-report-bug"),
                            "https://github.com/mkrueger/icy_tools/issues/new",
                        );
                        if r.clicked() {
                            ui.close_menu();
                        }
                        let r = ui.hyperlink_to(
                            fl!(crate::LANGUAGE_LOADER, "menu-item-check-releases"),
                            "https://github.com/mkrueger/icy_tools/releases",
                        );
                        if r.clicked() {
                            ui.close_menu();
                        }
                        ui.separator();
                        #[cfg(not(target_arch = "wasm32"))]
                        if ui.button(fl!(crate::LANGUAGE_LOADER, "menu-item-capture-dialog")).clicked() {
                            self.set_mode(ctx, MainWindowMode::ShowCaptureDialog);
                            ui.close_menu();
                        }

                        if ui.button(fl!(crate::LANGUAGE_LOADER, "menu-item-settings")).clicked() {
                            self.set_mode(ctx, MainWindowMode::ShowSettings);
                            ui.close_menu();
                        }
                    });
                });
            });
        }
        let frame_no_margins = egui::containers::Frame::none().outer_margin(Margin::same(0.0)).inner_margin(Margin::same(0.0));

        egui::CentralPanel::default().frame(frame_no_margins).show(ctx, |ui| {
            if !enable_ui {
                ui.disable();
            }
            let rect = ui.available_rect_before_wrap();

            self.show_terminal_area(ui);
            let msg = if self.show_find_dialog { self.find_dialog.show_ui(ui, rect) } else { None };

            match msg {
                Some(dialogs::find_dialog::Message::ChangePattern(pattern)) => {
                    self.find_dialog.pattern = pattern.chars().collect();
                    let lock = &mut self.buffer_view.lock();
                    let (buffer, _, parser) = lock.get_edit_state_mut().get_buffer_and_caret_mut();
                    self.find_dialog.search_pattern(buffer, (*parser).as_ref());
                    self.find_dialog.update_pattern(lock);
                }
                Some(dialogs::find_dialog::Message::FindNext) => {
                    self.find_dialog.find_next(&mut self.buffer_view.lock());
                }
                Some(dialogs::find_dialog::Message::FindPrev) => {
                    self.find_dialog.find_prev(&mut self.buffer_view.lock());
                }
                Some(dialogs::find_dialog::Message::CloseDialog) => {
                    self.show_find_dialog = false;
                }
                Some(dialogs::find_dialog::Message::SetCasing(case_sensitive)) => {
                    self.find_dialog.case_sensitive = case_sensitive;
                    let lock = &mut self.buffer_view.lock();
                    let (buffer, _, parser) = lock.get_edit_state_mut().get_buffer_and_caret_mut();
                    self.find_dialog.search_pattern(buffer, (*parser).as_ref());
                    self.find_dialog.update_pattern(lock);
                }

                None => {}
            }
        });

        if show_dialing_directory {
            dialogs::dialing_directory_dialog::view_dialing_directory(self, ctx);
        }

        let take = self.terminal_thread.lock().auto_transfer.take();
        if let Some((protocol_type, download, file_name)) = take {
            self.initiate_file_transfer(ctx, protocol_type, download, file_name);
        }

        if self.terminal_thread_handle.is_some() && self.terminal_thread_handle.as_ref().unwrap().is_finished() {
            let handle = self.terminal_thread_handle.take().unwrap();
            if let Err(err) = handle.join() {
                let err = format!("Error update thread crashed: {:?}", err.downcast_ref::<&str>());
                log::error!("{err}");
                self.output_string(&err);
            }
        }
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn show_terminal_area(&mut self, ui: &mut egui::Ui) {
        let mut monitor_settings = self.get_options().monitor_settings.clone();

        monitor_settings.selection_fg = self.screen_mode.get_selection_fg();
        monitor_settings.selection_bg = self.screen_mode.get_selection_bg();
        /*  if ui.input(|i| i.key_down(egui::Key::W)) {
            let enabled = self.buffer_update_thread.lock().enabled;
            self.buffer_update_thread.lock().enabled = !enabled;
        }*/

        let opt = icy_engine_gui::TerminalOptions {
            filter: self.get_options().scaling.get_filter(),
            monitor_settings,
            stick_to_bottom: true,
            use_terminal_height: true,
            ..Default::default()
        };
        let (mut response, calc) = icy_engine_gui::show_terminal_area(ui, self.buffer_view.clone(), opt);
        let inner_response = response.context_menu(|ui| terminal_context_menu(ui, self));
        if let Some(inner_response) = inner_response {
            response = inner_response.response;
        }

        if matches!(self.get_mode(), MainWindowMode::ShowTerminal) && ui.is_enabled() && !self.show_find_dialog {
            let events = ui.input(|i| i.events.clone());
            for e in events {
                match e {
                    egui::Event::PointerButton {
                        button: PointerButton::Middle,
                        pressed: true,
                        ..
                    } => {
                        self.copy_to_clipboard();
                    }
                    egui::Event::Paste(text) => {
                        self.output_string(&text);
                    }
                    /*egui::Event::CompositionEnd(text) |*/
                    egui::Event::Text(text) => {
                        for c in text.chars() {
                            self.output_char(c);
                        }
                    }

                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed: true,
                        modifiers,
                    } => {
                        if calc.buffer_rect.contains(pos - calc.terminal_rect.left_top().to_vec2()) && !calc.vert_scrollbar_rect.contains(pos) {
                            let buffer_view = self.buffer_view.clone();
                            let click_pos = calc.calc_click_pos(pos);
                            let mode: icy_engine::MouseMode = buffer_view.lock().get_buffer().terminal_state.mouse_mode;

                            match mode {
                                icy_engine::MouseMode::VT200 | icy_engine::MouseMode::VT200_Highlight => {
                                    let mut modifier_mask = 0;
                                    if matches!(button, PointerButton::Secondary) {
                                        modifier_mask |= 1;
                                    }
                                    if modifiers.shift {
                                        modifier_mask |= 4;
                                    }
                                    if modifiers.alt {
                                        modifier_mask |= 8;
                                    }
                                    if modifiers.ctrl || modifiers.mac_cmd {
                                        modifier_mask |= 16;
                                    }
                                    self.output_string(
                                        format!(
                                            "\x1b[M{}{}{}",
                                            encode_mouse_button(modifier_mask),
                                            encode_mouse_position(click_pos.x as i32),
                                            encode_mouse_position(click_pos.y as i32 - calc.first_line as i32)
                                        )
                                        .as_str(),
                                    );
                                }
                                icy_engine::MouseMode::X10 => {
                                    self.output_string(
                                        format!(
                                            "\x1b[M{}{}{}",
                                            encode_mouse_button(0),
                                            encode_mouse_position(click_pos.x as i32),
                                            encode_mouse_position(click_pos.y as i32)
                                        )
                                        .as_str(),
                                    );
                                }
                                _ => {} /*
                                        icy_engine::MouseMode::ButtonEvents => todo!(),
                                        icy_engine::MouseMode::AnyEvents => todo!(),
                                        icy_engine::MouseMode::FocusEvent => todo!(),
                                        icy_engine::MouseMode::AlternateScroll => todo!(),
                                        icy_engine::MouseMode::ExtendedMode => todo!(),
                                        icy_engine::MouseMode::SGRExtendedMode => todo!(),
                                        icy_engine::MouseMode::URXVTExtendedMode => todo!(),
                                        icy_engine::MouseMode::PixelPosition => todo!(),*/
                            }
                        }
                    }
                    egui::Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers,
                        ..
                    } => {
                        if calc.buffer_rect.contains(pos - calc.terminal_rect.left_top().to_vec2()) && !calc.vert_scrollbar_rect.contains(pos) {
                            let mode: icy_engine::MouseMode = self.buffer_view.lock().get_buffer().terminal_state.mouse_mode;
                            match mode {
                                icy_engine::MouseMode::VT200 | icy_engine::MouseMode::VT200_Highlight => {
                                    if calc.buffer_rect.contains(pos) {
                                        let click_pos = calc.calc_click_pos(pos);
                                        let mut modifier_mask = 3; // 3 means realease
                                        if modifiers.shift {
                                            modifier_mask |= 4;
                                        }
                                        if modifiers.alt {
                                            modifier_mask |= 8;
                                        }
                                        if modifiers.ctrl || modifiers.mac_cmd {
                                            modifier_mask |= 16;
                                        }
                                        self.output_string(
                                            format!(
                                                "\x1b[M{}{}{}",
                                                encode_mouse_button(modifier_mask),
                                                encode_mouse_position(click_pos.x as i32),
                                                encode_mouse_position(click_pos.y as i32)
                                            )
                                            .as_str(),
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    egui::Event::PointerMoved(pos) => {
                        if calc.buffer_rect.contains(pos - calc.terminal_rect.left_top().to_vec2()) && !calc.vert_scrollbar_rect.contains(pos) {
                            // Dev feature in debug mode - print char under cursor
                            // when shift is pressed
                            if cfg!(debug_assertions) && ui.input(|i| i.modifiers.shift_only()) {
                                let click_pos: Vec2 = calc.calc_click_pos(pos);
                                let buffer_view = self.buffer_view.clone();

                                let ch = buffer_view.lock().get_buffer().get_char((click_pos.x as usize, click_pos.y as usize));
                                println!("Char under cursor: {ch:?}");
                            }
                        }
                    }

                    egui::Event::Cut => {
                        self.handle_key_press(ui, &response, egui::Key::X, Modifiers::CTRL);
                    }
                    egui::Event::Copy => {
                        self.handle_key_press(ui, &response, egui::Key::C, Modifiers::CTRL);
                    }
                    egui::Event::Key {
                        key, pressed: true, modifiers, ..
                    } => {
                        if self.handle_key_press(ui, &response, key, modifiers) {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if self.use_rip {
                if response.clicked_by(PointerButton::Primary) {
                    if let Some(mouse_pos) = response.hover_pos() {
                        let mouse_pos = mouse_pos.to_vec2() - calc.buffer_rect.left_top().to_vec2();

                        let x = (mouse_pos.x / calc.buffer_rect.width() * 640.0) as i32;
                        let y = (mouse_pos.y / calc.buffer_rect.height() * 350.0) as i32;
                        let mut found_field = None;
                        for mouse_field in &self.terminal_thread.lock().mouse_field {
                            if !mouse_field.style.is_mouse_button() {
                                continue;
                            }
                            if mouse_field.contains(x, y) {
                                if let Some(found_field) = &found_field {
                                    if mouse_field.contains_field(found_field) {
                                        continue;
                                    }
                                }
                                found_field = Some(mouse_field.clone());
                            }
                        }

                        if let Some(mouse_field) = &found_field {
                            if let Some(cmd) = &mouse_field.host_command {
                                if mouse_field.style.reset_screen_after_click() {
                                    let mut buffer = self.buffer_view.lock();
                                    buffer.get_buffer_mut().terminal_state.clear_margins_left_right();
                                    buffer.get_buffer_mut().terminal_state.clear_margins_top_bottom();
                                    buffer.clear_buffer_screen();
                                }
                                self.output_string(cmd);
                                return;
                            }
                        }
                    }
                }

                if response.hovered() {
                    let hover_pos_opt = ui.input(|i| i.pointer.hover_pos());
                    if let Some(hover_pos) = hover_pos_opt {
                        let hover_pos = hover_pos.to_vec2() - calc.buffer_rect.left_top().to_vec2();

                        let x = (hover_pos.x / calc.buffer_rect.width() * 640.0) as i32;
                        let y = (hover_pos.y / calc.buffer_rect.height() * 350.0) as i32;
                        let fields = &self.terminal_thread.lock().mouse_field;
                        for mouse_field in fields {
                            if !mouse_field.style.is_mouse_button() {
                                continue;
                            }
                            if mouse_field.contains(x, y) {
                                ui.output_mut(|o: &mut egui::PlatformOutput| o.cursor_icon = CursorIcon::PointingHand);
                                break;
                            }
                        }
                    }
                }
                return;
            }

            if response.clicked_by(PointerButton::Primary) {
                if let Some(mouse_pos) = response.hover_pos() {
                    if calc.buffer_rect.contains(mouse_pos) && !calc.vert_scrollbar_rect.contains(mouse_pos) {
                        self.buffer_view.lock().clear_selection();
                    }
                }
            }

            if response.drag_started_by(PointerButton::Primary) {
                self.drag_start = None;
                if let Some(mouse_pos) = response.hover_pos() {
                    if calc.buffer_rect.contains(mouse_pos) && !calc.vert_scrollbar_rect.contains(mouse_pos) {
                        let click_pos = calc.calc_click_pos(mouse_pos);
                        self.last_pos = Position::new(click_pos.x as i32, click_pos.y as i32);
                        self.drag_start = Some(click_pos);
                        self.buffer_view.lock().get_edit_state_mut().set_mask_size();
                        self.buffer_view.lock().set_selection(Selection::new((click_pos.x, click_pos.y)));
                        self.buffer_view.lock().get_selection().as_mut().unwrap().shape = if response.ctx.input(|i| i.modifiers.alt) {
                            icy_engine::Shape::Rectangle
                        } else {
                            icy_engine::Shape::Lines
                        };
                    }
                }
                self.last_pos = Position::new(-1, -1);
            }

            if response.dragged_by(PointerButton::Primary) && self.drag_start.is_some() {
                if let Some(mouse_pos) = response.hover_pos() {
                    let click_pos = calc.calc_click_pos(mouse_pos);
                    let cur = Position::new(click_pos.x as i32, click_pos.y as i32);

                    if cur != self.last_pos {
                        self.last_pos = cur;
                        let mut l = self.buffer_view.lock();
                        l.get_edit_state_mut().set_mask_size();

                        if let Some(sel) = &mut l.get_selection() {
                            if !sel.locked {
                                sel.lead = Position::new(click_pos.x as i32, click_pos.y as i32);
                                sel.shape = if ui.input(|i| i.modifiers.alt) {
                                    icy_engine::Shape::Rectangle
                                } else {
                                    icy_engine::Shape::Lines
                                };
                                l.clear_selection();
                                l.set_selection(*sel);
                                let _ = l.get_edit_state_mut().add_selection_to_mask();
                                l.redraw_view();
                            }
                        }
                    }
                }
            }

            if response.drag_stopped_by(PointerButton::Primary) && self.drag_start.is_some() {
                self.shift_pressed_during_selection = ui.input(|i| i.modifiers.shift);
                if response.hover_pos().is_some() {
                    let l = self.buffer_view.lock();
                    if let Some(sel) = &mut l.get_selection() {
                        sel.locked = true;
                    }
                }
                self.last_pos = Position::new(-1, -1);

                self.drag_start = None;
            }

            if response.hovered() {
                let hover_pos_opt = ui.input(|i| i.pointer.hover_pos());
                if let Some(hover_pos) = hover_pos_opt {
                    if calc.buffer_rect.contains(hover_pos) {
                        let click_pos = calc.calc_click_pos(hover_pos);
                        let mut hovered_link = false;
                        let lock = self.buffer_view.lock();
                        let buffer = lock.get_buffer();
                        for hyper_link in buffer.layers[0].hyperlinks() {
                            if buffer.is_position_in_range(Position::new(click_pos.x as i32, click_pos.y as i32), hyper_link.position, hyper_link.length) {
                                ui.output_mut(|o: &mut egui::PlatformOutput| o.cursor_icon = CursorIcon::PointingHand);
                                let url = hyper_link.get_url(buffer);
                                response = response.on_hover_ui_at_pointer(|ui| {
                                    ui.hyperlink(url.clone());
                                });
                                hovered_link = true;

                                if response.clicked_by(PointerButton::Primary)
                                /* && response.is_pointer_button_down_on() */
                                {
                                    ui.ctx().output_mut(|o| {
                                        o.open_url = Some(egui::output::OpenUrl { url, new_tab: false });
                                    });
                                }
                                break;
                            }
                        }
                        if !hovered_link && !calc.vert_scrollbar_rect.contains(hover_pos) {
                            ui.output_mut(|o| o.cursor_icon = CursorIcon::Text);
                        }
                    }
                }
            }
        }
    }

    fn handle_key_press(&mut self, ui: &mut egui::Ui, response: &egui::Response, key: egui::Key, modifiers: egui::Modifiers) -> bool {
        let im = self.screen_mode.get_input_mode();
        let key_map = im.cur_map();
        let mut key_code = key as u32;
        if modifiers.ctrl || modifiers.command {
            key_code |= icy_engine_gui::ui::CTRL_MOD;
        }
        if modifiers.shift {
            key_code |= icy_engine_gui::ui::SHIFT_MOD;
        }
        for (k, m) in key_map {
            if *k == key_code {
                if self.terminal_thread.lock().is_connected {
                    self.send_vec(m.to_vec());
                } else {
                    for c in *m {
                        self.print_char(*c);
                    }
                }
                response.request_focus();
                ui.input_mut(|i| {
                    i.consume_key(modifiers, key);
                });
                return true;
            }
        }
        false
    }

    fn copy_to_clipboard(&mut self) {
        let buffer_view = self.buffer_view.clone();
        let mut l = buffer_view.lock();
        if self.shift_pressed_during_selection {
            if let Some(data) = l.get_edit_state().get_clipboard_data() {
                if let Err(err) = icy_engine::util::push_data(icy_engine::util::BUFFER_DATA, &data) {
                    log::error!("error while copy:{err}");
                }
                return;
            }
        }

        if let Some(txt) = l.get_copy_text() {
            let mut clipboard = arboard::Clipboard::new().unwrap();
            clipboard.set_text(txt).unwrap();
        }
        l.clear_selection();
    }
}

fn terminal_context_menu(ui: &mut egui::Ui, window: &mut MainWindow) {
    ui.input_mut(|i| i.events.clear());

    if ui.button(fl!(crate::LANGUAGE_LOADER, "terminal-menu-copy")).clicked() {
        window.copy_to_clipboard();
        ui.close_menu();
    }

    if ui.button(fl!(crate::LANGUAGE_LOADER, "terminal-menu-paste")).clicked() {
        let mut clipboard = arboard::Clipboard::new().unwrap();
        if let Ok(text) = clipboard.get_text() {
            let im = window.screen_mode.get_input_mode();
            let key_map = im.cur_map();
            let mut first = true;
            let mut txt = String::new();
            text.lines().for_each(|line| {
                if first {
                    first = false;
                } else {
                    for (k, m) in key_map {
                        if *k == eframe::egui::Key::Enter as u32 {
                            for c in *m {
                                txt.push(*c as char);
                            }
                        }
                    }
                }
                txt.push_str(line);
            });
            window.output_string(&txt);
        }
        ui.close_menu();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        ui.separator();
        if ui
            .add(egui::Button::new(fl!(crate::LANGUAGE_LOADER, "terminal-menu-export")).wrap_mode(egui::TextWrapMode::Truncate))
            .clicked()
        {
            window.init_export_dialog(ui.ctx());
            ui.close_menu();
        }
    }
}

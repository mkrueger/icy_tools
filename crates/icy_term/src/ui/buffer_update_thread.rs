use crate::{
    features::{AutoFileTransfer, AutoLogin},
    protocol::TransferType,
    util::SoundThread,
    Terminal, TerminalResult,
};
use egui::mutex::Mutex;
use icy_engine::{
    ansi::{self, MusicOption},
    rip::bgi::MouseField,
    BufferParser, Caret,
};
use icy_engine_gui::BufferView;
use std::{mem, path::PathBuf, sync::Arc, thread};
use web_time::Duration;

use super::{
    connect::{Connection, DataConnection},
    dialogs,
};

pub struct BufferUpdateThread {
    pub capture_dialog: dialogs::capture_dialog::DialogState,

    pub buffer_view: Arc<Mutex<BufferView>>,
    pub connection: Arc<Mutex<Option<Box<Connection>>>>,

    pub auto_file_transfer: AutoFileTransfer,
    pub auto_login: Option<AutoLogin>,
    pub sound_thread: Arc<Mutex<SoundThread>>,

    pub auto_transfer: Option<(TransferType, bool)>,
    pub enabled: bool,

    pub terminal_type: Option<(Terminal, MusicOption)>,

    pub mouse_field: Vec<MouseField>,

    pub cache_directory: PathBuf,
}

impl BufferUpdateThread {
    pub fn get_data(&self) -> TerminalResult<Vec<u8>> {
        let data = if let Some(con) = self.connection.lock().as_mut() {
            con.update_state()?;
            if con.is_disconnected() {
                return Ok(Vec::new());
            }
            if con.is_data_available()? {
                con.read_buffer()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        Ok(data)
    }
    pub fn update_state(&mut self, ctx: &egui::Context, buffer_parser: &mut dyn BufferParser, data: &[u8]) -> TerminalResult<(u64, usize)> {
        self.sound_thread.lock().update_state()?;
        Ok(self.update_buffer(ctx, buffer_parser, data))
    }

    fn update_buffer(&mut self, ctx: &egui::Context, buffer_parser: &mut dyn BufferParser, data: &[u8]) -> (u64, usize) {
        let has_data = !data.is_empty();
        if !self.enabled {
            return (10, 0);
        }

        {
            let mut caret: Caret = Caret::default();
            mem::swap(&mut caret, self.buffer_view.lock().get_caret_mut());

            loop {
                let Some(act) = buffer_parser.get_next_action(self.buffer_view.lock().get_buffer_mut(), &mut caret, 0) else {
                    break;
                };
                let (p, ms) = self.handle_action(act, &mut self.buffer_view.lock());
                if p {
                    self.buffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();
                    ctx.request_repaint();
                    mem::swap(&mut caret, self.buffer_view.lock().get_caret_mut());

                    return (ms as u64, 0);
                }
            }
            mem::swap(&mut caret, self.buffer_view.lock().get_caret_mut());
        }

        let mut idx = 0;
        for ch in data {
            let ch = *ch;
            if let Some(autologin) = &mut self.auto_login {
                if let Some(con) = self.connection.lock().as_mut() {
                    if let Err(err) = autologin.try_login(con, ch) {
                        log::error!("{err}");
                    }

                    if let Err(err) = autologin.run_autologin(con) {
                        log::error!("{err}");
                    }
                }
            }
            /*
            match ch {
                b'\\' => print!("\\\\"),
                b'\n' => println!("\\n"),
                b'\r' => print!("\\r"),
                b'\"' => print!("\\\""),
                _ => {
                    if ch < b' ' || ch == b'\x7F' {
                        print!("\\x{ch:02X}");
                    } else if ch > b'\x7F' {
                        print!("\\u{{{ch:02X}}}");
                    } else {
                        print!("{}", ch as char);
                    }
                }
            }*/
            self.capture_dialog.append_data(ch);
            let (p, ms) = self.print_char(&mut self.buffer_view.lock(), buffer_parser, ch);
            idx += 1;

            if p {
                self.buffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();
                ctx.request_repaint();
                return (ms as u64, idx);
            }
            if let Some((protocol_type, download)) = self.auto_file_transfer.try_transfer(ch) {
                self.auto_transfer = Some((protocol_type, download));
            }
        }

        if has_data {
            self.buffer_view.lock().get_buffer_mut().update_hyperlinks();
            (0, data.len())
        } else {
            (10, data.len())
        }
    }

    pub fn print_char(&self, buffer_view: &mut BufferView, buffer_parser: &mut dyn BufferParser, c: u8) -> (bool, u32) {
        let mut caret: Caret = Caret::default();
        mem::swap(&mut caret, buffer_view.get_caret_mut());
        let buffer = buffer_view.get_buffer_mut();
        let result = buffer_parser.print_char(buffer, 0, &mut caret, c as char);
        mem::swap(&mut caret, buffer_view.get_caret_mut());

        match result {
            Ok(action) => {
                return self.handle_action(action, buffer_view);
            }

            Err(err) => {
                log::error!("print_char: {err}");
            }
        }
        (false, 0)
    }

    fn handle_action(&self, result: icy_engine::CallbackAction, buffer_view: &mut BufferView) -> (bool, u32) {
        match result {
            icy_engine::CallbackAction::SendString(result) => {
                if let Some(con) = self.connection.lock().as_mut() {
                    if con.is_connected() {
                        let r = con.send(result.as_bytes().to_vec());
                        if let Err(r) = r {
                            log::error!("callbackaction::SendString: {r}");
                        }
                    }
                }
            }
            icy_engine::CallbackAction::PlayMusic(music) => {
                let r = self.sound_thread.lock().play_music(music);
                if let Err(r) = r {
                    log::error!("callbackaction::PlayMusic: {r}");
                }
            }
            icy_engine::CallbackAction::Beep => {
                let r = self.sound_thread.lock().beep();
                if let Err(r) = r {
                    log::error!("callbackaction::Beep: {r}");
                }
            }
            icy_engine::CallbackAction::ChangeBaudEmulation(baud_emulation) => {
                if let Some(con) = self.connection.lock().as_mut() {
                    let r = con.set_baud_rate(baud_emulation.get_baud_rate());
                    if let Err(r) = r {
                        log::error!("callbackaction::ChangeBaudEmulation: {r}");
                    }
                }
            }
            icy_engine::CallbackAction::ResizeTerminal(_, _) => {
                buffer_view.redraw_view();
            }

            icy_engine::CallbackAction::NoUpdate => {
                return (false, 0);
            }

            icy_engine::CallbackAction::Update => {
                return (true, 0);
            }
            icy_engine::CallbackAction::Pause(ms) => {
                // note: doesn't block the UI thread
                return (true, ms);
            }
        }
        (false, 0)
    }
}

pub fn run_update_thread(ctx: &egui::Context, update_thread: Arc<Mutex<BufferUpdateThread>>) -> thread::JoinHandle<()> {
    let ctx = ctx.clone();
    thread::spawn(move || {
        let mut data = Vec::new();
        let mut idx = 0;
        let mut buffer_parser: Box<dyn BufferParser> = Box::<ansi::Parser>::default();
        loop {
            if idx >= data.len() {
                let lock = &update_thread.lock();
                match lock.get_data() {
                    Ok(d) => {
                        data = d;
                    }
                    Err(err) => {
                        log::error!("run_update_thread: {err}");
                        for ch in format!("{err}").chars() {
                            let _ = lock.buffer_view.lock().print_char(ch);
                        }
                        lock.buffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();
                        data.clear();
                    }
                }
                idx = 0;
            }
            if idx < data.len() {
                {
                    let lock = &mut update_thread.lock();
                    if let Some((te, b)) = lock.terminal_type.take() {
                        buffer_parser = te.get_parser(b, lock.cache_directory.clone());
                    }
                }
                let update_state = update_thread.lock().update_state(&ctx, &mut *buffer_parser, &data[idx..]);
                match update_state {
                    Err(err) => {
                        log::error!("run_update_thread::update_state: {err}");
                        idx = data.len();
                    }
                    Ok((sleep_ms, parsed_data)) => {
                        let data = buffer_parser.get_picture_data();
                        if data.is_some() {
                            update_thread.lock().mouse_field = buffer_parser.get_mouse_fields();
                            update_thread.lock().buffer_view.lock().set_reference_image(data);
                        }
                        if sleep_ms > 0 {
                            thread::sleep(Duration::from_millis(sleep_ms));
                        }
                        idx += parsed_data;
                    }
                }
            } else {
                thread::sleep(Duration::from_millis(10));
            }
        }
    })
}

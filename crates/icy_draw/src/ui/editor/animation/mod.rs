use std::{
    path::{Path, PathBuf},
    sync::{mpsc::Receiver, Arc},
    time::Instant,
};

use crate::{model::Tool, AnsiEditor, ClipboardHandler, Document, DocumentOptions, Message, TerminalResult, UndoHandler};
use eframe::{
    egui::{self, Id, ImageButton, RichText, Slider, TextEdit, TopBottomPanel},
    epaint::Vec2,
};
use egui::{Image, ProgressBar};
use egui_code_editor::{CodeEditor, Syntax};
use i18n_embed_fl::fl;
use icy_engine::{ascii, AttributedChar, Buffer, EngineResult, Size, TextAttribute, UnicodeConverter};
use icy_engine_gui::{animations::Animator, show_terminal_area, BufferView, MonitorSettings};

use self::encoding::{start_encoding_thread, ENCODERS};
mod asciicast_encoder;
mod encoding;
mod gif_encoder;
mod highlighting;
//mod mp4_encoder;

pub struct AnimationEditor {
    gl: Arc<glow::Context>,
    id: usize,

    undostack: usize,

    txt: String,
    buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,
    animator: Arc<std::sync::Mutex<Animator>>,
    next_animator: Option<Arc<std::sync::Mutex<Animator>>>,
    set_frame: usize,

    parent_path: Option<PathBuf>,
    export_path: PathBuf,
    export_type: usize,

    first_frame: bool,

    shedule_update: bool,
    last_update: Instant,
    cursor_index: usize,
    scale: f32,

    rx: Option<Receiver<usize>>,
    thread: Option<std::thread::JoinHandle<TerminalResult<()>>>,
    cur_encoding_frame: usize,
    encoding_frames: usize,
    encoding_error: String,
}

impl AnimationEditor {
    pub fn new(gl: Arc<glow::Context>, id: usize, path: &Path, txt: String) -> Self {
        let mut buffer = Buffer::new(Size::new(80, 25));
        buffer.is_terminal_buffer = false;
        let mut buffer_view = BufferView::from_buffer(&gl, buffer);
        buffer_view.interactive = false;
        let buffer_view = Arc::new(eframe::epaint::mutex::Mutex::new(buffer_view));
        let parent_path = path.parent().map(|p| p.to_path_buf());
        let animator = Animator::run(&parent_path, txt.clone());
        let export_path = path.with_extension("gif");
        Self {
            gl,
            id,
            buffer_view,
            animator,
            txt,
            undostack: 0,
            export_path,
            export_type: 0,
            parent_path,
            set_frame: 0,
            scale: 1.0,
            next_animator: None,
            shedule_update: false,
            last_update: Instant::now(),
            first_frame: true,
            rx: None,
            thread: None,
            cur_encoding_frame: 0,
            encoding_frames: 0,
            cursor_index: 0,
            encoding_error: String::new(),
        }
    }

    fn export(&mut self) -> TerminalResult<()> {
        if let Some((rx, handle)) = start_encoding_thread(self.export_type, self.gl.clone(), self.export_path.clone(), self.animator.clone())? {
            self.rx = Some(rx);
            self.thread = Some(handle);
            self.encoding_frames = self.animator.lock().unwrap().frames.len();
        }
        Ok(())
    }
}

impl ClipboardHandler for AnimationEditor {
    fn can_copy(&self) -> bool {
        false
    }

    fn copy(&mut self) -> EngineResult<()> {
        Ok(())
    }

    fn can_paste(&self) -> bool {
        false
    }

    fn paste(&mut self) -> EngineResult<()> {
        Ok(())
    }
}

impl UndoHandler for AnimationEditor {
    fn undo_description(&self) -> Option<String> {
        None
    }

    fn can_undo(&self) -> bool {
        false
    }

    fn undo(&mut self) -> EngineResult<Option<Message>> {
        Ok(None)
    }

    fn redo_description(&self) -> Option<String> {
        None
    }

    fn can_redo(&self) -> bool {
        false
    }

    fn redo(&mut self) -> EngineResult<Option<Message>> {
        Ok(None)
    }
}

impl Document for AnimationEditor {
    fn default_extension(&self) -> &'static str {
        "icyanim"
    }

    fn undo_stack_len(&self) -> usize {
        self.undostack
    }

    fn can_paste_char(&self) -> bool {
        true
    }

    fn paste_char(&mut self, _ui: &mut eframe::egui::Ui, ch: char) {
        let ch = ascii::CP437Converter::default().convert_to_unicode(AttributedChar::new(ch, TextAttribute::default()));
        self.txt.insert(self.cursor_index, ch);
        if let Some((i, _)) = self.txt.char_indices().nth(self.cursor_index + 1) {
            self.cursor_index = i;
        }
    }

    fn show_ui(&mut self, ui: &mut eframe::egui::Ui, _cur_tool: &mut Box<dyn Tool>, _selected_tool: usize, _options: &DocumentOptions) -> Option<Message> {
        let mut message = None;

        if self.first_frame && self.animator.lock().unwrap().success() {
            let animator = &mut self.animator.lock().unwrap();
            let frame_count = animator.frames.len();
            if frame_count > 0 {
                animator.set_cur_frame(self.set_frame);
                animator.display_frame(self.buffer_view.clone());
            }
            self.first_frame = false;
        }
        if let Some(next) = &self.next_animator {
            if next.lock().unwrap().success() || !next.lock().unwrap().error.is_empty() {
                self.animator = next.clone();
                self.next_animator = None;
                let animator = &mut self.animator.lock().unwrap();
                animator.set_cur_frame(self.set_frame);
                animator.display_frame(self.buffer_view.clone());
            }
        }

        egui::SidePanel::right("movie_panel")
            .default_width(ui.available_width() / 2.0)
            .min_width(660.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    if !self.animator.lock().unwrap().error.is_empty() {
                        ui.set_enabled(false);
                    }

                    if self.animator.lock().unwrap().success() {
                        let animator = &mut self.animator.lock().unwrap();
                        let frame_count = animator.frames.len();
                        if animator.is_playing() {
                            if ui.add(ImageButton::new(crate::PAUSE_SVG.clone())).clicked() {
                                animator.set_is_playing(false);
                            }
                        } else {
                            let image: &Image<'static> = if animator.get_cur_frame() + 1 < frame_count {
                                &crate::PLAY_SVG
                            } else {
                                &crate::REPLAY_SVG
                            };
                            if ui.add(ImageButton::new(image.clone())).clicked() {
                                if animator.get_cur_frame() + 1 >= frame_count {
                                    animator.set_cur_frame(0);
                                }
                                animator.start_playback(self.buffer_view.clone());
                            }
                        }
                        if ui
                            .add_enabled(animator.get_cur_frame() + 1 < frame_count, ImageButton::new(crate::SKIP_NEXT_SVG.clone()))
                            .clicked()
                        {
                            animator.set_cur_frame(frame_count - 1);
                            animator.display_frame(self.buffer_view.clone());
                        }
                        let is_loop = animator.get_is_loop();
                        if ui.add(ImageButton::new(crate::REPEAT_SVG.clone()).selected(is_loop)).clicked() {
                            animator.set_is_loop(!is_loop);
                        }

                        let mut cf = animator.get_cur_frame() + 1;

                        if frame_count > 0
                            && ui
                                .add(Slider::new(&mut cf, 1..=frame_count).text(fl!(crate::LANGUAGE_LOADER, "animation_of_frame_count", total = frame_count)))
                                .changed()
                        {
                            animator.set_cur_frame(cf - 1);
                            animator.display_frame(self.buffer_view.clone());
                        }

                        if ui
                            .add_enabled(animator.get_cur_frame() > 0, ImageButton::new(crate::NAVIGATE_PREV.clone()))
                            .clicked()
                        {
                            let cf = animator.get_cur_frame() - 1;
                            animator.set_cur_frame(cf);
                            animator.display_frame(self.buffer_view.clone());
                        }

                        if ui
                            .add_enabled(
                                animator.get_cur_frame() + 1 < animator.frames.len(),
                                ImageButton::new(crate::NAVIGATE_NEXT.clone()),
                            )
                            .clicked()
                        {
                            let cf = animator.get_cur_frame() + 1;
                            animator.set_cur_frame(cf);
                            animator.display_frame(self.buffer_view.clone());
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(if self.scale < 2.0 { "2x" } else { "1x" }).clicked() {
                                if self.scale < 2.0 {
                                    self.scale = 2.0;
                                } else {
                                    self.scale = 1.0;
                                }
                            }
                        });
                    }
                });

                if self.animator.lock().unwrap().success() {
                    let cur_frame = self.animator.lock().unwrap().get_cur_frame();

                    let monitor_settings = if let Some((_, settings, _)) = self.animator.lock().unwrap().frames.get(cur_frame) {
                        settings.clone()
                    } else {
                        MonitorSettings::default()
                    };
                    let mut scale = Vec2::splat(self.scale);
                    if self.buffer_view.lock().get_buffer().use_aspect_ratio() {
                        scale.y *= 1.35;
                    }
                    let opt = icy_engine_gui::TerminalOptions {
                        stick_to_bottom: false,
                        scale: Some(scale),
                        monitor_settings,
                        id: Some(Id::new(self.id + 20000)),
                        ..Default::default()
                    };
                    ui.allocate_ui(Vec2::new(ui.available_width(), ui.available_height() - 100.0), |ui| {
                        self.buffer_view.lock().get_caret_mut().set_is_visible(false);
                        let (_, _) = show_terminal_area(ui, self.buffer_view.clone(), opt);
                    });
                    ui.add_space(8.0);
                }

                if let Some(rx) = &self.rx {
                    if let Ok(x) = rx.recv() {
                        self.cur_encoding_frame = x;
                    }

                    ui.label(fl!(
                        crate::LANGUAGE_LOADER,
                        "animation_encoding_frame",
                        cur = self.cur_encoding_frame,
                        total = self.encoding_frames
                    ));
                    ui.add(ProgressBar::new(self.cur_encoding_frame as f32 / self.encoding_frames as f32));
                    if self.cur_encoding_frame >= self.encoding_frames {
                        if let Some(thread) = self.thread.take() {
                            if let Ok(Err(err)) = thread.join() {
                                log::error!("Error during encoding: {err}");
                                self.encoding_error = format!("{err}");
                            }
                        }
                        self.rx = None;
                    } else if let Some(thread) = &self.thread {
                        if thread.is_finished() {
                            if let Err(err) = self.thread.take().unwrap().join() {
                                let msg = if let Some(msg) = err.downcast_ref::<&'static str>() {
                                    msg.to_string()
                                } else if let Some(msg) = err.downcast_ref::<String>() {
                                    msg.clone()
                                } else {
                                    format!("?{:?}", err)
                                };
                                log::error!("Error during encoding: {:?}", msg);
                                self.encoding_error = format!("Thread aborted: {:?}", msg);
                            }
                            self.rx = None;
                        }
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "animation_editor_path_label"));
                        let mut path_edit = self.export_path.to_str().unwrap().to_string();
                        let response = ui.add(
                            //    ui.available_size(),
                            TextEdit::singleline(&mut path_edit).desired_width(f32::INFINITY),
                        );
                        if response.changed() {
                            self.export_path = path_edit.into();
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        for (i, enc) in ENCODERS.iter().enumerate() {
                            if ui.selectable_label(self.export_type == i, enc.label()).clicked() {
                                self.export_type = i;
                                self.export_path.set_extension(enc.extension());
                            }
                        }

                        if ui.button(fl!(crate::LANGUAGE_LOADER, "animation_editor_export_button")).clicked() {
                            if let Err(err) = self.export() {
                                message = Some(Message::ShowError(format!("Could not export: {}", err)));
                            }
                        }
                    });

                    if !self.encoding_error.is_empty() {
                        ui.colored_label(ui.style().visuals.error_fg_color, RichText::new(&self.encoding_error));
                    } else {
                        ui.horizontal(|ui| {
                            ui.small(fl!(crate::LANGUAGE_LOADER, "animation_icy_play_note"));
                            ui.hyperlink_to(RichText::new("Icy Play").small(), "https://github.com/mkrueger/icy_play");
                        });
                    }
                }
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            TopBottomPanel::bottom("code_error_bottom_panel").exact_height(200.).show_inside(ui, |ui| {
                if !self.animator.lock().unwrap().error.is_empty() {
                    ui.colored_label(ui.style().visuals.error_fg_color, RichText::new(&self.animator.lock().unwrap().error).small());
                } else {
                    egui::ScrollArea::vertical().max_width(f32::INFINITY).show(ui, |ui| {
                        self.animator.lock().unwrap().log.iter().for_each(|line| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("Frame {}:", line.frame)).strong());
                                ui.label(RichText::new(&line.text));
                                ui.add_space(ui.available_width());
                            });
                        });
                    });
                }
            });

            let r = CodeEditor::default()
                .id_source("code editor")
                .with_rows(12)
                .with_fontsize(14.0)
                .with_theme(if ui.style().visuals.dark_mode {
                    egui_code_editor::ColorTheme::GITHUB_DARK
                } else {
                    egui_code_editor::ColorTheme::GITHUB_LIGHT
                })
                .with_syntax(highlighting::lua())
                .with_numlines(true)
                .show(ui, &mut self.txt);
            if self.shedule_update && self.last_update.elapsed().as_millis() > 1000 {
                self.shedule_update = false;

                let path = self.parent_path.clone();
                let txt = self.txt.clone();
                self.set_frame = self.animator.lock().unwrap().get_cur_frame();
                self.next_animator = Some(Animator::run(&path, txt));
            }

            if let Some(range) = r.cursor_range {
                if let Some((i, _)) = self.txt.char_indices().nth(range.as_sorted_char_range().start) {
                    self.cursor_index = i;
                } else {
                    self.cursor_index = 0;
                }
            }
            if r.response.changed {
                self.shedule_update = true;
                self.last_update = Instant::now();
                self.undostack += 1;
            }
        });

        let buffer_view = self.buffer_view.clone();
        if self.animator.lock().unwrap().success() {
            self.animator.lock().unwrap().update_frame(buffer_view);
        }
        message
    }

    fn get_bytes(&mut self, _path: &Path) -> TerminalResult<Vec<u8>> {
        Ok(self.txt.as_bytes().to_vec())
    }

    fn get_ansi_editor_mut(&mut self) -> Option<&mut AnsiEditor> {
        None
    }

    fn get_ansi_editor(&self) -> Option<&AnsiEditor> {
        None
    }

    fn destroy(&self, gl: &glow::Context) -> Option<Message> {
        self.buffer_view.lock().destroy(gl);
        None
    }
}

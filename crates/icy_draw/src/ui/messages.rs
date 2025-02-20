use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

use eframe::{
    egui::{self},
    epaint::Vec2,
};
use egui::mutex::Mutex;
use icy_engine::{util::pop_data, AnsiFont, BitFont, EngineResult, IceMode, PaletteMode, Size, Tag, TextPane};

use crate::{util::autosave, AnsiEditor, Document, MainWindow, NewFileDialog, SaveFileDialog, SelectCharacterDialog, SelectOutlineDialog, Settings, SETTINGS};

#[derive(Clone)]
pub enum Message {
    NewFileDialog,
    OpenFileDialog,
    SaveFile,
    SaveFileAs,
    ExportFile,
    ShowOutlineDialog,
    CloseWindow,

    AddNewLayer(usize),
    EditLayer(usize),
    RemoveLayer(usize),
    RaiseLayer(usize),
    LowerLayer(usize),
    ToggleLayerVisibility(usize),
    SelectLayer(usize),
    DuplicateLayer(usize),
    MergeLayerDown(usize),

    Undo,
    Redo,
    EditSauce,
    SetCanvasSize,
    SelectAll,
    SelectNothing,
    DeleteSelection,

    ShowAboutDialog,
    ShowCharacterSelectionDialog(Rc<RefCell<char>>),
    SelectFontDialog(Arc<Mutex<Vec<Box<dyn AnsiFont>>>>, Arc<Mutex<i32>>),
    ShowError(String),
    SetFontPage(usize),
    CharTable(char),
    ResizeLayer(usize),
    SelectTool(usize),
    AnchorLayer,
    AddFloatingLayer,
    JustifyLeft,
    JustifyRight,
    Center,
    FlipX,
    FlipY,
    Crop,
    Paste,
    ResizeBuffer(bool, i32, i32),
    PasteAsNewImage,
    PasteAsBrush,
    Copy,
    Cut,
    RemoveFloatingLayer,
    SelectOutline(usize),

    CenterLine,
    JustifyLineLeft,
    JustifyLineRight,

    InsertRow,
    DeleteRow,
    InsertColumn,
    DeleteColumn,

    EraseRow,
    EraseRowToStart,
    EraseRowToEnd,

    EraseColumn,
    EraseColumnToStart,
    EraseColumnToEnd,

    ScrollAreaUp,
    ScrollAreaDown,
    ScrollAreaLeft,
    ScrollAreaRight,

    SetReferenceImage,
    ToggleReferenceImage,
    ClearReferenceImage,

    PickAttributeUnderCaret,
    SwitchToDefaultColor,
    ToggleColor,

    StampLayerDown,
    RotateLayer,
    MakeLayerTransparent,

    ToggleFullScreen,

    ZoomReset,
    ZoomIn,
    ZoomOut,

    OpenFontSelector,
    OpenAddFonts,
    OpenFontManager,
    OpenFontDirectory,
    OpenPalettesDirectory,
    ToggleMirrorMode,
    ClearRecentOpenFiles,
    SetGuide(i32, i32),
    SetRaster(i32, i32),
    LoadFile(PathBuf, bool),
    TryLoadFile(PathBuf),
    ClearLayer(usize),
    InverseSelection,

    SetForeground(u32),
    SetForegroundRgb(u8, u8, u8),

    SetBackground(u32),
    SetBackgroundRgb(u8, u8, u8),
    ClearSelection,
    UpdateFont(Box<(BitFont, BitFont)>),

    SelectPalette,
    ToggleLayerBorders,
    ToggleLineNumbers,
    RunPlugin(usize),
    OpenPluginDirectory,
    SelectPreviousTool,
    NextFgColor,
    PreviousFgColor,
    NextBgColor,
    PreviousBgColor,
    ShowSettings,
    ToggleLGAFont,
    ToggleAspectRatio,
    SwitchToFontPage(usize),
    SetAnsiFont(usize),
    SetFont(Box<BitFont>),
    SwitchPaletteMode(PaletteMode),
    SwitchIceMode(IceMode),
    AddFont(Box<BitFont>),
    AddAnsiFont(usize),
    SetSauceFont(String),
    ToggleGrid,
    KeySwitchForeground(usize),
    KeySwitchBackground(usize),
    AddNewTag(Box<Tag>),
    UpdateTag(Box<Tag>, usize),
    ShowTags(bool),
    RemoveTag(usize),
    SelectCurrentTag(usize),
    ShowEditTagDialog(Box<Tag>, i32),
    ToggleLeftPane,
    ToggleRightPane,
}

pub const CTRL_SHIFT: egui::Modifiers = egui::Modifiers {
    alt: false,
    ctrl: true,
    shift: true,
    mac_cmd: false,
    command: false,
};

impl<'a> MainWindow<'a> {
    pub fn handle_message(&mut self, msg_opt: Option<Message>) {
        let Some(msg) = msg_opt else {
            return;
        };
        match msg {
            Message::NewFileDialog => {
                self.open_dialog(NewFileDialog::default());
            }
            Message::OpenFileDialog => {
                let mut initial_directory = if let Some(d) = self.get_active_pane_mut() { d.get_path() } else { None };
                set_default_initial_directory_opt(&mut initial_directory);
                if let Some(mut path) = initial_directory {
                    while path.parent().is_some() && !path.is_dir() {
                        path = path.parent().unwrap().to_path_buf();
                    }
                    self.open_file_window.file_view.set_path(path, true);
                }
                self.open_file_window.reset();
                self.in_open_file_mode = true;
            }

            Message::TryLoadFile(path) => {
                let auto_save = autosave::get_autosave_file(&path);
                if auto_save.exists() {
                    self.open_dialog(crate::AutoSaveDialog::new(path));
                    return;
                }

                self.open_file(&path, false, None);
            }
            Message::LoadFile(path, load_autosave) => {
                self.open_file(&path, load_autosave, None);
            }

            Message::SaveFile => {
                let msg = if let Some(pane) = self.get_active_pane_mut() {
                    if pane.is_untitled() || pane.doc.lock().get_ansi_editor().is_some() && !is_icy_file(pane.get_path()) {
                        Some(Message::SaveFileAs)
                    } else {
                        pane.save()
                    }
                } else {
                    None
                };

                self.handle_message(msg);
            }
            Message::SaveFileAs => {
                if let Some((_, tab)) = self.get_active_pane() {
                    let path = tab.get_path();
                    if let Some(path) = &path {
                        self.mru_files.add_recent_file(path);
                    }

                    self.open_dialog(SaveFileDialog::new(path));
                }
            }
            Message::ExportFile => {
                let path = if let Some((_, tab)) = self.get_active_pane() {
                    tab.get_path()
                } else {
                    return;
                };
                self.run_editor_command(path, |window, editor, path| {
                    let view = editor.buffer_view.clone();
                    window.open_dialog(crate::ExportFileDialog::new(path, view.lock().get_buffer()));
                    None
                });
                /*
                if let Some(doc) = self.get_active_document() {
                    if let Some(editor) = doc.lock().get_ansi_editor() {
                        let view = editor.buffer_view.clone();
                        self.open_dialog(crate::ExportFileDialog::new(path, view.lock().get_buffer()));
                    }
                }*/
            }
            Message::ShowOutlineDialog => {
                self.open_dialog(SelectOutlineDialog::default());
            }
            Message::Undo => {
                let mut msg = None;
                if let Some(editor) = self.get_active_document() {
                    msg = self.handle_result(editor.lock().undo()).unwrap_or(None);
                }
                self.handle_message(msg);
            }
            Message::Redo => {
                let mut msg = None;
                if let Some(editor) = self.get_active_document() {
                    msg = self.handle_result(editor.lock().redo()).unwrap_or(None);
                }
                self.handle_message(msg);
            }

            Message::SelectAll => {
                self.run_editor_command(0, |_, editor, _| {
                    let buf = &mut editor.buffer_view.lock();
                    let w = buf.get_buffer().get_width();
                    let h = buf.get_buffer().get_height();

                    buf.set_selection(icy_engine::Rectangle::from(0, 0, w, h));
                    None
                });
            }
            Message::SelectNothing => {
                self.run_editor_command(0, |_, editor, _| to_message(editor.buffer_view.lock().get_edit_state_mut().clear_selection()));
            }
            Message::DeleteSelection => {
                self.run_editor_command(0, |_, editor: &mut AnsiEditor, _| {
                    to_message(editor.buffer_view.lock().get_edit_state_mut().erase_selection())
                });
            }
            Message::ShowCharacterSelectionDialog(ch) => {
                self.run_editor_command(ch, |window, editor: &mut AnsiEditor, ch| {
                    let buf = editor.buffer_view.clone();
                    window.open_dialog(SelectCharacterDialog::new(buf, ch));
                    None
                });
            }
            Message::SelectFontDialog(fonts, selected_font) => {
                self.open_dialog(crate::SelectFontDialog::new(fonts, selected_font));
            }
            Message::EditSauce => {
                if let Some(doc) = self.get_active_document() {
                    if let Some(editor) = doc.lock().get_ansi_editor() {
                        let view = editor.buffer_view.clone();
                        self.open_dialog(crate::EditSauceDialog::new(view.lock().get_buffer()));
                    }
                }
            }

            Message::SetCanvasSize => {
                if let Some(doc) = self.get_active_document() {
                    if let Some(editor) = doc.lock().get_ansi_editor() {
                        let view = editor.buffer_view.clone();
                        self.open_dialog(crate::SetCanvasSizeDialog::new(view.lock().get_buffer()));
                    }
                }
            }

            Message::EditLayer(i) => {
                if let Some(doc) = self.get_active_document() {
                    if let Some(editor) = doc.lock().get_ansi_editor() {
                        let view = editor.buffer_view.clone();
                        self.open_dialog(crate::EditLayerDialog::new(view.lock().get_buffer(), i));
                    }
                }
            }
            Message::ResizeLayer(i) => {
                if let Some(doc) = self.get_active_document() {
                    if let Some(editor) = doc.lock().get_ansi_editor_mut() {
                        let view = editor.buffer_view.clone();
                        self.open_dialog(crate::ResizeLayerDialog::new(view.lock().get_buffer(), i));
                    }
                }
            }

            Message::AddNewTag(tag) => {
                self.run_editor_command(tag, |_, editor, tag| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().add_new_tag(*tag))
                });
            }

            Message::RemoveTag(index) => {
                self.run_editor_command(index, |_, editor, index| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().remove_tag(index))
                });
            }

            Message::UpdateTag(tag, index) => {
                self.run_editor_command((tag, index), |_, editor, (tag, index)| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().update_tag(*tag, index))
                });
            }
            Message::ShowTags(tags_enabled) => {
                self.run_editor_command(tags_enabled, |_, editor, tags_enabled| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().show_tags(tags_enabled))
                });
            }
            Message::SelectCurrentTag(tag) => {
                self.run_editor_command(tag, |_, editor, tag| {
                    let mut lock = editor.buffer_view.lock();
                    lock.get_edit_state_mut().set_current_tag(tag);
                    None
                });
            }
            Message::ShowEditTagDialog(tag, index) => {
                self.open_dialog(crate::EditTagDialog::new(*tag, index));
            }

            Message::AddNewLayer(cur_layer) => {
                self.run_editor_command(cur_layer, |_, editor, cur_layer| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().add_new_layer(cur_layer))
                });
            }
            Message::RaiseLayer(cur_layer) => {
                self.run_editor_command(cur_layer, |_, editor, cur_layer| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().raise_layer(cur_layer))
                });
            }
            Message::LowerLayer(cur_layer) => {
                self.run_editor_command(cur_layer, |_, editor, cur_layer| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().lower_layer(cur_layer))
                });
            }
            Message::RemoveLayer(cur_layer) => {
                self.run_editor_command(cur_layer, |_, editor: &mut crate::AnsiEditor, cur_layer| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().remove_layer(cur_layer))
                });

                self.run_editor_command(0, |_, editor: &mut crate::AnsiEditor, _| {
                    let mut lock = editor.buffer_view.lock();

                    if lock.get_buffer().layers.is_empty() {
                        to_message(lock.get_edit_state_mut().add_new_layer(0))
                    } else {
                        None
                    }
                });
            }
            Message::ClearLayer(cur_layer) => {
                self.run_editor_command(cur_layer, |_, editor, cur_layer| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().clear_layer(cur_layer))
                });
            }
            Message::RemoveFloatingLayer => {
                self.run_editor_command(0, |_, editor: &mut crate::AnsiEditor, _| {
                    let mut lock = editor.buffer_view.lock();
                    if let Ok(layer) = lock.get_edit_state().get_current_layer() {
                        to_message(lock.get_edit_state_mut().remove_layer(layer))
                    } else {
                        Some(Message::ShowError("No floating layer to remove".to_string()))
                    }
                });
            }
            Message::ClearSelection => {
                self.run_editor_command(0, |_, editor: &mut crate::AnsiEditor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().clear_selection())
                });
            }
            Message::DuplicateLayer(cur_layer) => {
                self.run_editor_command(cur_layer, |_, editor: &mut crate::AnsiEditor, cur_layer| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().duplicate_layer(cur_layer))
                });
            }
            Message::MergeLayerDown(cur_layer) => {
                self.run_editor_command(cur_layer, |_, editor: &mut crate::AnsiEditor, cur_layer| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().merge_layer_down(cur_layer))
                });
            }

            Message::ToggleLayerVisibility(cur_layer) => {
                self.run_editor_command(cur_layer, |_, editor: &mut crate::AnsiEditor, cur_layer| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().toggle_layer_visibility(cur_layer))
                });
            }

            Message::SelectLayer(cur_layer) => {
                self.run_editor_command(cur_layer, |_, editor, cur_layer| {
                    editor.set_cur_layer_index(cur_layer);
                    None
                });
            }

            Message::AnchorLayer => {
                self.run_editor_command(0, |_, editor: &mut crate::AnsiEditor, _| {
                    to_message(editor.buffer_view.lock().get_edit_state_mut().anchor_layer())
                });
            }

            Message::AddFloatingLayer => {
                self.run_editor_command(0, |_, editor: &mut crate::AnsiEditor, _| {
                    to_message(editor.buffer_view.lock().get_edit_state_mut().add_floating_layer())
                });
            }

            Message::SetFontPage(page) => {
                self.run_editor_command(page, |_, editor, page| {
                    editor.buffer_view.lock().get_caret_mut().set_font_page(page);

                    let lock = &mut editor.buffer_view.lock();
                    let buf = &mut lock.get_buffer_mut();
                    if buf.get_font(page).is_none() {
                        match BitFont::from_ansi_font_page(page) {
                            Ok(font) => {
                                buf.set_font(page, font);
                            }
                            Err(err) => {
                                log::error!("Failed to load font: {err}");
                            }
                        }
                    }
                    None
                });
            }

            Message::CharTable(ch) => {
                self.run_editor_command(ch, |_, editor, ch| {
                    editor.type_cp437_key(ch);
                    None
                });
            }

            Message::SelectTool(tool) => {
                self.run_editor_command((), |_, editor, _| {
                    editor.set_tool_switch(true);
                    None
                });
                self.document_behavior.set_selected_tool(tool);
            }

            Message::SelectPreviousTool => {
                self.document_behavior.select_prev_tool();
            }

            Message::ShowAboutDialog => {
                self.open_dialog(crate::AboutDialog::default());
            }

            Message::ShowError(msg) => {
                log::error!("{msg}");
                self.toasts.error(msg);
            }

            Message::Paste => {
                self.do_paste = true;
            }

            Message::Cut => {
                self.do_cut = true;
            }
            Message::Copy => {
                self.do_copy = true;
            }

            Message::JustifyLeft => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().justify_left())
                });
            }

            Message::JustifyRight => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().justify_right())
                });
            }

            Message::Center => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().center())
                });
            }

            Message::FlipX => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().flip_x())
                });
            }

            Message::FlipY => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().flip_y())
                });
            }

            Message::Crop => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().crop())
                });
            }
            Message::ResizeBuffer(resize_layer, w, h) => {
                self.run_editor_command((resize_layer, w, h), |_, editor, (resize_layer, w, h)| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().resize_buffer(resize_layer, Size::new(w, h)))
                });
            }

            Message::PasteAsNewImage => {
                if let Some(data) = pop_data(icy_engine::util::BUFFER_DATA) {
                    let mut buf = icy_engine::Buffer::new((80, 24));
                    buf.is_terminal_buffer = false;
                    let editor = AnsiEditor::new(&self.gl, buf);

                    {
                        let mut lock = editor.buffer_view.lock();
                        let state = lock.get_edit_state_mut();
                        if let Some(mut layer) = state.from_clipboard_data(&data) {
                            layer.set_offset((0, 0));
                            layer.role = icy_engine::Role::Normal;
                            layer.properties.has_alpha_channel = false;
                            let lc = layer.get_line_count();

                            state.get_buffer_mut().layers.clear();
                            state.get_buffer_mut().layers.push(layer);
                            state.get_buffer_mut().set_height(lc);
                        }
                    }

                    crate::add_child(&mut self.document_tree, None, Box::new(editor));
                }
            }

            Message::PasteAsBrush => {
                /*
                if let Some(data) = pop_data(icy_engine::util::BUFFER_DATA) {
                    if let Some(layer) = Layer::from_clipboard_data(&data) {
                        unsafe {
                            crate::model::brush_imp::CUSTOM_BRUSH = Some(layer);
                            self.document_behavior.set_selected_tool(crate::BRUSH_TOOL);
                        }
                    }
                }*/
            }
            Message::CloseWindow => {
                self.request_close = true;
            }
            Message::CenterLine => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    if lock.get_edit_state_mut().is_something_selected() {
                        to_message(lock.get_edit_state_mut().center())
                    } else {
                        to_message(lock.get_edit_state_mut().center_line())
                    }
                });
            }
            Message::JustifyLineLeft => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    if lock.get_edit_state_mut().is_something_selected() {
                        to_message(lock.get_edit_state_mut().justify_left())
                    } else {
                        to_message(lock.get_edit_state_mut().justify_line_left())
                    }
                });
            }
            Message::JustifyLineRight => {
                self.run_editor_command(0, |_, editor: &mut AnsiEditor, _| {
                    let mut lock = editor.buffer_view.lock();
                    if lock.get_edit_state_mut().is_something_selected() {
                        to_message(lock.get_edit_state_mut().justify_right())
                    } else {
                        to_message(lock.get_edit_state_mut().justify_line_right())
                    }
                });
            }
            Message::InsertRow => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().insert_row())
                });
            }
            Message::DeleteRow => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().delete_row())
                });
            }
            Message::InsertColumn => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().insert_column())
                });
            }
            Message::DeleteColumn => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().delete_column())
                });
            }
            Message::EraseRow => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().erase_row())
                });
            }
            Message::EraseRowToStart => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().erase_row_to_start())
                });
            }
            Message::EraseRowToEnd => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().erase_row_to_end())
                });
            }
            Message::EraseColumn => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().erase_column())
                });
            }
            Message::EraseColumnToStart => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().erase_column_to_start())
                });
            }
            Message::EraseColumnToEnd => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().erase_column_to_end())
                });
            }
            Message::ScrollAreaUp => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().scroll_area_up())
                });
            }
            Message::ScrollAreaDown => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().scroll_area_down())
                });
            }
            Message::ScrollAreaLeft => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().scroll_area_left())
                });
            }
            Message::ScrollAreaRight => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().scroll_area_right())
                });
            }

            Message::StampLayerDown => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().stamp_layer_down())
                });
            }

            Message::RotateLayer => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().rotate_layer())
                });
            }

            Message::MakeLayerTransparent => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    to_message(lock.get_edit_state_mut().make_layer_transparent())
                });
            }

            Message::SetReferenceImage => {
                self.run_editor_command(0, |window, editor, _| {
                    let mut initial_directory = if let Some(d) = editor.buffer_view.lock().get_reference_image_path() {
                        d.parent().map(|p| p.to_path_buf())
                    } else {
                        None
                    };
                    set_default_initial_directory_opt(&mut initial_directory);

                    window.open_dialog(crate::OpenReferenceImageDialog::new(initial_directory));
                    None
                });
            }
            Message::ToggleReferenceImage => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock = editor.buffer_view.lock();
                    lock.toggle_reference_image();
                    None
                });
            }
            Message::ClearReferenceImage => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut lock: eframe::epaint::mutex::MutexGuard<'_, icy_engine_gui::BufferView> = editor.buffer_view.lock();
                    lock.clear_reference_image();
                    None
                });
            }

            Message::PickAttributeUnderCaret => {
                self.run_editor_command(0, |_, editor, _| {
                    let bv = &mut editor.buffer_view.lock();
                    let pos = bv.get_caret().get_position();

                    let attr = if let Some(layer) = bv.get_edit_state().get_cur_layer() {
                        layer.get_char(pos + layer.get_offset()).attribute
                    } else {
                        bv.get_buffer().get_char(pos).attribute
                    };

                    let fg = attr.get_foreground();
                    let bg = attr.get_background();
                    let caret = bv.get_caret_mut();
                    caret.set_foreground(fg);
                    caret.set_background(bg);
                    None
                });
            }

            Message::SwitchToDefaultColor => {
                self.run_editor_command(0, |_, editor, _| {
                    let bv = &mut editor.buffer_view.lock();
                    let caret = bv.get_caret_mut();
                    caret.set_foreground(7);
                    caret.set_background(0);
                    None
                });
            }

            Message::ToggleColor => {
                self.run_editor_command(0, |_, editor, _| {
                    let mut attr = editor.buffer_view.lock().get_caret().get_attribute();
                    let fg = attr.get_foreground();
                    let bg = attr.get_background();
                    attr.set_foreground(bg);
                    attr.set_background(fg);
                    editor.buffer_view.lock().get_caret_mut().set_attr(attr);
                    None
                });
            }

            Message::SelectOutline(outline) => {
                Settings::set_character_set(outline);
            }

            Message::ToggleFullScreen => {
                self.is_fullscreen = !self.is_fullscreen;
                self.set_fullscreen_opt = Some(self.is_fullscreen);
            }

            Message::ZoomReset => unsafe {
                SETTINGS.set_scale(Vec2::splat(2.0));
            },

            Message::ZoomIn => unsafe {
                SETTINGS.set_scale(SETTINGS.get_scale() + Vec2::splat(0.5));
            },

            Message::ZoomOut => unsafe {
                SETTINGS.set_scale(SETTINGS.get_scale() - Vec2::splat(0.5));
            },

            Message::OpenFontDirectory => match Settings::get_font_diretory() {
                Ok(dir) => {
                    if let Err(err) = open::that(dir) {
                        self.handle_message(Some(Message::ShowError(format!("Can't open font directory: {err}"))));
                    }
                }
                Err(err) => {
                    self.handle_message(Some(Message::ShowError(format!("{err}"))));
                }
            },
            Message::OpenPalettesDirectory => match Settings::get_palettes_diretory() {
                Ok(dir) => {
                    if let Err(err) = open::that(dir) {
                        self.handle_message(Some(Message::ShowError(format!("Can't open font directory: {err}"))));
                    }
                }
                Err(err) => {
                    self.handle_message(Some(Message::ShowError(format!("{err}"))));
                }
            },
            Message::OpenFontSelector => {
                self.run_editor_command(0, |window, editor, _| {
                    window.open_dialog(crate::FontSelector::new(editor, false));
                    None
                });
            }

            Message::OpenAddFonts => {
                self.run_editor_command(0, |window, editor, _| {
                    window.open_dialog(crate::FontSelector::new(editor, true));
                    None
                });
            }

            Message::OpenFontManager => {
                self.run_editor_command(0, |window, editor, _| {
                    window.open_dialog(crate::FontManager::new(editor));
                    None
                });
            }
            Message::SetGuide(x, y) => {
                self.run_editor_command((x, y), |_, editor, (x, y)| {
                    if x <= 0 && y <= 0 {
                        editor.guide = None;
                    } else {
                        editor.guide = Some(Vec2::new(x as f32, y as f32));
                        editor.buffer_view.lock().set_show_guide(true);
                    }
                    None
                });
            }
            Message::SetRaster(x, y) => {
                self.run_editor_command((x, y), |_, editor, (x, y)| {
                    if x <= 0 && y <= 0 {
                        editor.raster = None;
                    } else {
                        editor.raster = Some(Vec2::new(x as f32, y as f32));
                        editor.buffer_view.lock().set_show_raster(true);
                    }
                    None
                });
            }

            Message::ToggleMirrorMode => {
                self.run_editor_command(0, |_, editor, _| {
                    let mode = editor.buffer_view.lock().get_edit_state_mut().get_mirror_mode();
                    editor.buffer_view.lock().get_edit_state_mut().set_mirror_mode(!mode);
                    None
                });
            }

            Message::ClearRecentOpenFiles => {
                self.mru_files.clear_recent_files();
            }

            Message::InverseSelection => {
                self.run_editor_command(0, |_, editor, _| to_message(editor.buffer_view.lock().get_edit_state_mut().inverse_selection()));
            }

            Message::SetForeground(color) => {
                self.run_editor_command(color, |_, editor, color| {
                    editor.buffer_view.lock().get_caret_mut().set_foreground(color);
                    None
                });
            }
            Message::SetForegroundRgb(r, g, b) => {
                self.run_editor_command((r, g, b), |_, editor, (r, g, b)| {
                    let color = editor.buffer_view.lock().get_buffer_mut().palette.insert_color_rgb(r, g, b);
                    editor.buffer_view.lock().get_caret_mut().set_foreground(color);
                    None
                });
            }

            Message::SetBackground(color) => {
                self.run_editor_command(color, |_, editor, color| {
                    editor.buffer_view.lock().get_caret_mut().set_background(color);
                    None
                });
            }
            Message::SetBackgroundRgb(r, g, b) => {
                self.run_editor_command((r, g, b), |_, editor, (r, g, b)| {
                    let color = editor.buffer_view.lock().get_buffer_mut().palette.insert_color_rgb(r, g, b);
                    editor.buffer_view.lock().get_caret_mut().set_background(color);
                    None
                });
            }

            Message::UpdateFont(font_box) => {
                let (old, new) = font_box.as_ref();
                self.enumerate_documents(|_, pane| {
                    if let Some(editor) = pane.doc.lock().get_ansi_editor() {
                        editor.buffer_view.lock().get_buffer_mut().font_iter_mut().for_each(|(_, font)| {
                            if font.glyphs == old.glyphs {
                                *font = new.clone();
                            }
                        });
                        editor.buffer_view.lock().redraw_font();
                    }
                });
            }

            Message::SelectPalette => {
                self.run_editor_command(0, |window, editor, _| {
                    let mut msg = None;

                    match crate::SelectPaletteDialog::new(editor) {
                        Ok(dialog) => {
                            window.open_dialog(dialog);
                        }
                        Err(err) => {
                            log::error!("Failed to open palette dialog: {err}");
                            msg = Some(Message::ShowError(format!("{err}")));
                        }
                    }
                    msg
                });
            }

            Message::ToggleLayerBorders => unsafe {
                SETTINGS.show_layer_borders = !SETTINGS.show_layer_borders;
            },
            Message::ToggleLineNumbers => unsafe {
                SETTINGS.show_line_numbers = !SETTINGS.show_line_numbers;
            },
            Message::RunPlugin(i) => {
                let plugin = self.plugins[i].clone();
                self.run_editor_command(i, |window, editor, _i| {
                    let mut msg = None;
                    if let Err(err) = plugin.run_plugin(window, editor) {
                        msg = Some(Message::ShowError(format!("Error running plugin: {err}")));
                    }
                    msg
                });
            }
            Message::OpenPluginDirectory => match Settings::get_plugin_directory() {
                Ok(dir) => {
                    if let Err(err) = open::that(dir) {
                        self.handle_message(Some(Message::ShowError(format!("Can't open font directory: {err}"))));
                    }
                }
                Err(err) => {
                    self.handle_message(Some(Message::ShowError(format!("{err}"))));
                }
            },

            Message::KeySwitchForeground(k) => {
                self.run_editor_command(k, |_, editor, k| {
                    let palette_len = editor.buffer_view.lock().get_buffer_mut().palette.len() as u32;
                    let mut fg = editor.buffer_view.lock().get_caret_mut().get_attribute().get_foreground();
                    if fg % 8 == k as u32 {
                        fg += 8;
                    } else {
                        fg = k as u32;
                    }
                    fg %= palette_len;
                    if fg < 8 || editor.buffer_view.lock().get_buffer().font_mode.has_high_fg_colors() || palette_len > 16 {
                        editor.buffer_view.lock().get_caret_mut().set_foreground(fg);
                    }
                    None
                });
            }

            Message::KeySwitchBackground(k) => {
                self.run_editor_command(k, |_, editor, k| {
                    let palette_len = editor.buffer_view.lock().get_buffer_mut().palette.len() as u32;
                    let mut bg = editor.buffer_view.lock().get_caret_mut().get_attribute().get_background();
                    if bg % 8 == k as u32 {
                        bg += 8;
                    } else {
                        bg = k as u32;
                    }
                    bg %= palette_len;
                    if bg < 8 || editor.buffer_view.lock().get_buffer().ice_mode.has_high_bg_colors() || palette_len > 16 {
                        editor.buffer_view.lock().get_caret_mut().set_background(bg);
                    }
                    None
                });
            }

            Message::NextFgColor => {
                self.run_editor_command(0, |_, editor, _| {
                    let palette_len = editor.buffer_view.lock().get_buffer_mut().palette.len() as u32;
                    let fg = editor.buffer_view.lock().get_caret_mut().get_attribute().get_foreground();

                    editor.buffer_view.lock().get_caret_mut().set_foreground((fg + 1) % palette_len);
                    None
                });
            }
            Message::PreviousFgColor => {
                self.run_editor_command(0, |_, editor, _| {
                    let palette_len = editor.buffer_view.lock().get_buffer_mut().palette.len() as u32;
                    let fg = editor.buffer_view.lock().get_caret_mut().get_attribute().get_foreground();

                    editor.buffer_view.lock().get_caret_mut().set_foreground((fg + palette_len - 1) % palette_len);
                    None
                });
            }
            Message::NextBgColor => {
                self.run_editor_command(0, |_, editor, _| {
                    let palette_len = editor.buffer_view.lock().get_buffer_mut().palette.len() as u32;
                    let bg = editor.buffer_view.lock().get_caret_mut().get_attribute().get_background();

                    editor.buffer_view.lock().get_caret_mut().set_background((bg + 1) % palette_len);
                    None
                });
            }
            Message::PreviousBgColor => {
                self.run_editor_command(0, |_, editor, _| {
                    let palette_len = editor.buffer_view.lock().get_buffer_mut().palette.len() as u32;
                    let bg = editor.buffer_view.lock().get_caret_mut().get_attribute().get_background();

                    editor.buffer_view.lock().get_caret_mut().set_background((bg + palette_len - 1) % palette_len);
                    None
                });
            }
            Message::ShowSettings => {
                self.show_settings = true;
                self.settings_dialog.init(self.key_bindings.clone());
            }

            Message::ToggleLGAFont => {
                self.run_editor_command(0, |_, editor, _| {
                    let use_lga = editor.buffer_view.lock().get_buffer_mut().use_letter_spacing();
                    to_message(editor.buffer_view.lock().get_edit_state_mut().set_use_letter_spacing(!use_lga))
                });
            }

            Message::ToggleAspectRatio => {
                self.run_editor_command(0, |_, editor, _| {
                    let use_ar = editor.buffer_view.lock().get_buffer_mut().use_aspect_ratio();
                    to_message(editor.buffer_view.lock().get_edit_state_mut().set_use_aspect_ratio(!use_ar))
                });
            }

            Message::SwitchToFontPage(page) => {
                self.run_editor_command(page, |_, editor, page| {
                    to_message(editor.buffer_view.lock().get_edit_state_mut().switch_to_font_page(page))
                });
            }
            Message::SetAnsiFont(page) => {
                self.run_editor_command(page, |_, editor, page| {
                    to_message(editor.buffer_view.lock().get_edit_state_mut().set_ansi_font(page))
                });
            }

            Message::SetSauceFont(name) => {
                self.run_editor_command(name, |_, editor, name| {
                    to_message(editor.buffer_view.lock().get_edit_state_mut().set_sauce_font(&name))
                });
            }

            Message::SetFont(fnt) => {
                self.run_editor_command(fnt, |_, editor, fnt| to_message(editor.buffer_view.lock().get_edit_state_mut().set_font(*fnt)));
            }
            Message::AddAnsiFont(page) => {
                self.run_editor_command(page, |_, editor, page| {
                    to_message(editor.buffer_view.lock().get_edit_state_mut().add_ansi_font(page))
                });
            }
            Message::AddFont(fnt) => {
                self.run_editor_command(fnt, |_, editor, fnt| to_message(editor.buffer_view.lock().get_edit_state_mut().add_font(*fnt)));
            }

            Message::SwitchPaletteMode(mode) => {
                self.run_editor_command(mode, |_, editor, mode| {
                    to_message(editor.buffer_view.lock().get_edit_state_mut().set_palette_mode(mode))
                });
            }

            Message::SwitchIceMode(mode) => {
                self.run_editor_command(mode, |_, editor, mode| {
                    to_message(editor.buffer_view.lock().get_edit_state_mut().set_ice_mode(mode))
                });
            }

            Message::ToggleGrid => {
                self.run_editor_command(0, |_, editor, _| {
                    let lock = &mut editor.buffer_view.lock();
                    let show_raster = lock.get_show_raster();
                    let show_guide = lock.get_show_guide();

                    if editor.raster.is_some() && editor.guide.is_some() {
                        if show_raster && show_guide {
                            lock.set_show_raster(false);
                            lock.set_show_guide(false);
                        } else if show_raster {
                            lock.set_show_guide(true);
                        } else {
                            lock.set_show_raster(true);
                        }
                    } else if editor.raster.is_some() {
                        lock.set_show_raster(!show_raster);
                    } else if editor.guide.is_some() {
                        lock.set_show_guide(!show_guide);
                    }
                    None
                });
            }
            Message::ToggleLeftPane => {
                self.left_panel = !self.left_panel;
            }
            Message::ToggleRightPane => {
                self.right_panel = !self.right_panel;
            }
        }
    }
}

fn is_icy_file(get_path: Option<PathBuf>) -> bool {
    let Some(path) = get_path else {
        return false;
    };
    let Some(ext) = path.extension() else {
        return false;
    };
    ext == "icy"
}

pub fn set_default_initial_directory_opt(initial_directory: &mut Option<PathBuf>) {
    if initial_directory.is_some() {
        return;
    }
    *initial_directory = if let Ok(cur) = std::env::current_dir() {
        Some(cur)
    } else {
        return;
    };
}

pub fn to_message<T>(result: EngineResult<T>) -> Option<Message> {
    if let Err(result) = result {
        Some(Message::ShowError(format!("{result}")))
    } else {
        None
    }
}

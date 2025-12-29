//! Font Tool (TDF/Figlet Font Rendering)
//!
//! Renders text using TDF (TheDraw Font) or Figlet fonts.
//! Each character typed is rendered as a multi-cell font glyph.
//!
//! This tool extends the Click tool's navigation and selection behavior,
//! adding font-specific character rendering, backspace, and enter handling.

use i18n_embed_fl::fl;
use iced::widget::{button, column, container, row, text, Space};
use iced::Element;
use iced::{Length, Theme};
use icy_engine::Position;
use icy_engine_edit::tools::Tool;
use icy_engine_edit::{OperationType, TdfEditStateRenderer};
use icy_engine_gui::TerminalMessage;

use super::{handle_navigation_key, SelectionMouseState, ToolContext, ToolHandler, ToolId, ToolMessage, ToolResult, ToolViewContext, UiAction};

use crate::ui::editor::ansi::widget::font_tool::FontToolState;
use crate::ui::editor::ansi::widget::outline_selector::OutlineSelectorMessage;
use crate::Settings;
use crate::SharedFontLibrary;
use crate::LANGUAGE_LOADER;

/// Font tool state
pub struct FontTool {
    /// Currently selected font slot (0-9)
    font_slot: usize,

    // === Font Tool UI/State (moved from AnsiEditor) ===
    pub font_tool: FontToolState,
    outline_selector_open: bool,
    outline_style_cache: usize,

    // === Selection/Mouse State (shared with ClickTool) ===
    selection_mouse: SelectionMouseState,
}

impl FontTool {
    pub fn new(font_library: SharedFontLibrary) -> Self {
        Self {
            font_slot: 0,
            font_tool: FontToolState::new(font_library.clone()),
            outline_selector_open: false,
            outline_style_cache: 0,
            selection_mouse: SelectionMouseState::new(),
        }
    }

    pub fn build_font_panel_info(&self, options: &Settings) -> crate::ui::editor::ansi::widget::toolbar::top::FontPanelInfo {
        let font_name = self.font_tool.with_selected_font(|f| f.name().to_string()).unwrap_or_default();

        let has_fonts = self.font_tool.has_fonts();

        let char_availability: Vec<(char, bool)> = ('!'..='~').map(|ch| (ch, self.font_tool.has_char(ch))).collect();

        let outline_style = options.font_outline_style;

        crate::ui::editor::ansi::widget::toolbar::top::FontPanelInfo {
            font_name,
            has_fonts,
            char_availability,
            outline_style,
        }
    }

    pub fn open_outline_selector(&mut self) {
        self.outline_selector_open = true;
    }

    pub fn is_outline_selector_open(&self) -> bool {
        self.outline_selector_open
    }

    pub fn handle_outline_selector_message(&mut self, options: &std::sync::Arc<parking_lot::RwLock<Settings>>, msg: OutlineSelectorMessage) {
        match msg {
            OutlineSelectorMessage::SelectOutline(style) => {
                options.write().font_outline_style = style;
                self.outline_selector_open = false;
                self.outline_style_cache = style;
            }
            OutlineSelectorMessage::Cancel => {
                self.outline_selector_open = false;
            }
        }
    }

    pub fn select_font(&mut self, index: i32) {
        self.font_tool.select_font(index);
        self.font_tool.prev_char = '\0';
    }

    fn outline_style_from_ctx(ctx: &ToolContext) -> usize {
        ctx.options.and_then(|opts| Some(opts.read().font_outline_style)).unwrap_or(0)
    }

    fn render_char(&mut self, ctx: &mut ToolContext, ch: char) -> ToolResult {
        // Check if we have a selected font
        let font_idx = self.font_tool.selected_font;
        if font_idx < 0 || (font_idx as usize) >= self.font_tool.font_count() {
            log::warn!("No font selected for Font tool");
            return ToolResult::None;
        }

        // Check if character is supported
        let has_char = self.font_tool.with_font_at(font_idx as usize, |font| font.has_char(ch));
        if !has_char.unwrap_or(false) {
            return ToolResult::None;
        }

        let outline_style = Self::outline_style_from_ctx(ctx);

        // Begin atomic undo with RenderCharacter operation type for backspace support
        let _undo_guard = ctx.state.begin_typed_atomic_undo("Render font character", OperationType::RenderCharacter);

        // Save caret position for undo - this allows backspace to restore position
        let _ = ctx.state.undo_caret_position();

        let caret_pos = ctx.state.get_caret().position();
        let start_y = caret_pos.y;

        let result: Result<Position, icy_engine::EngineError> = match TdfEditStateRenderer::new(ctx.state, caret_pos.x, start_y) {
            Ok(mut renderer) => {
                let render_options = retrofont::RenderOptions {
                    outline_style,
                    ..Default::default()
                };

                let lib = self.font_tool.font_library.read();
                if let Some(font) = lib.get_font(font_idx as usize) {
                    match font.render_glyph(&mut renderer, ch, &render_options) {
                        Ok(_) => Ok(Position::new(renderer.max_x(), start_y)),
                        Err(e) => Err(icy_engine::EngineError::Generic(format!("Font render error: {}", e))),
                    }
                } else {
                    Err(icy_engine::EngineError::Generic("Font not found".to_string()))
                }
            }
            Err(e) => Err(e),
        };

        match result {
            Ok(new_pos) => {
                self.font_tool.prev_char = ch;
                ctx.state.set_caret_position(new_pos);
                ToolResult::Commit("Render font character".to_string())
            }
            Err(e) => {
                log::warn!("Failed to render font character: {}", e);
                ToolResult::None
            }
        }
    }

    fn backspace(&mut self, ctx: &mut ToolContext) -> ToolResult {
        // Try to find and reverse the last RenderCharacter operation in the undo stack
        let mut use_backspace = true;

        {
            let undo_stack = ctx.state.get_undo_stack();
            let Ok(stack) = undo_stack.lock() else {
                return ToolResult::None;
            };

            let mut reverse_count = 0;
            let mut found_index = None;

            for i in (0..stack.len()).rev() {
                match stack[i].get_operation_type() {
                    OperationType::RenderCharacter => {
                        if reverse_count == 0 {
                            found_index = Some(i);
                            break;
                        }
                        reverse_count -= 1;
                    }
                    OperationType::ReversedRenderCharacter => {
                        reverse_count += 1;
                    }
                    OperationType::Unknown => {
                        break;
                    } // Other operation types are irrelevant here.
                }
            }

            if let Some(idx) = found_index {
                if let Some(op) = stack[idx].try_clone() {
                    drop(stack);
                    match ctx.state.push_reverse_undo("Undo font character", op, OperationType::ReversedRenderCharacter) {
                        Ok(_) => {
                            use_backspace = false;
                        }
                        Err(e) => {
                            log::warn!("Failed to push reverse undo for font character: {}", e);
                        }
                    }
                }
            }
        }

        self.font_tool.prev_char = '\0';

        if use_backspace {
            let _ = if ctx.state.is_something_selected() {
                ctx.state.erase_selection()
            } else {
                ctx.state.backspace()
            };
        }

        ToolResult::Commit("Font backspace".to_string())
    }
}

impl ToolHandler for FontTool {
    fn id(&self) -> ToolId {
        ToolId::Tool(Tool::Font)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn handle_message(&mut self, ctx: &mut ToolContext<'_>, msg: &ToolMessage) -> ToolResult {
        // Keep cached outline style in sync for toolbar label.
        self.outline_style_cache = Self::outline_style_from_ctx(ctx);

        match *msg {
            ToolMessage::FontSelectSlot(slot) => {
                self.font_slot = slot.min(9);
                ToolResult::Status(format!("Font slot: {}", self.font_slot))
            }
            ToolMessage::FontOpenSelector => ToolResult::Ui(UiAction::OpenTdfFontSelector),
            ToolMessage::FontOpenDirectory => ToolResult::Ui(UiAction::OpenFontDirectory),
            ToolMessage::FontOpenOutlineSelector => {
                self.open_outline_selector();
                ToolResult::None
            }
            ToolMessage::FontSetOutline(style) => {
                ctx.options.as_ref().expect("FontTool requires options").write().font_outline_style = style;
                self.outline_style_cache = style;
                ToolResult::None
            }
            _ => ToolResult::None,
        }
    }

    fn cancel_capture(&mut self) {
        self.selection_mouse.cancel();
    }

    fn handle_terminal_message(&mut self, ctx: &mut ToolContext, msg: &TerminalMessage) -> ToolResult {
        match msg {
            TerminalMessage::Move(evt) => {
                if self.selection_mouse.is_dragging() {
                    return ToolResult::None;
                }

                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                // Hover: update cursor interaction for selection resize handles.
                self.selection_mouse.handle_move(ctx.state.selection(), pos);
                ToolResult::None
            }

            TerminalMessage::Press(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                if evt.button == icy_engine::MouseButton::Left {
                    self.selection_mouse.handle_press(ctx, pos);
                    return ToolResult::StartCapture.and(ToolResult::Redraw);
                }

                ToolResult::None
            }

            TerminalMessage::Drag(evt) => {
                let Some(pos) = evt.text_position else {
                    return ToolResult::None;
                };

                if self.selection_mouse.handle_drag(ctx, pos) {
                    return ToolResult::Redraw;
                }

                ToolResult::None
            }

            TerminalMessage::Release(evt) => {
                if self.selection_mouse.handle_release(ctx, evt.text_position) {
                    return ToolResult::EndCapture.and(ToolResult::Redraw);
                }

                ToolResult::None
            }

            _ => ToolResult::None,
        }
    }

    fn handle_event(&mut self, ctx: &mut ToolContext, event: &iced::Event) -> ToolResult {
        match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                use iced::keyboard::key::Named;

                // Font-specific keys: Backspace, Enter, Space
                if let iced::keyboard::Key::Named(named) = key {
                    match named {
                        Named::Backspace => {
                            return self.backspace(ctx);
                        }
                        Named::Enter => {
                            let font_height = self.font_tool.max_height().max(1) as i32;
                            let pos = ctx.state.get_caret().position();
                            ctx.state.set_caret_position(Position::new(0, pos.y + font_height));
                            self.font_tool.prev_char = '\0';
                            return ToolResult::Redraw;
                        }
                        Named::Space => {
                            return self.render_char(ctx, ' ');
                        }
                        _ => {}
                    }
                }

                // Common navigation keys (arrows, home, end, page up/down, delete, tab, insert)
                let nav_result = handle_navigation_key(ctx, key, modifiers);
                if nav_result.is_handled() {
                    return nav_result.to_tool_result();
                }

                // Handle font slot switching (0-9)
                if let iced::keyboard::Key::Character(ch) = key {
                    if modifiers.control() {
                        if let Some(digit) = ch.chars().next().and_then(|c| c.to_digit(10)) {
                            self.font_slot = digit as usize;
                            return ToolResult::Status(format!("Font slot: {}", self.font_slot));
                        }
                    }
                }

                // Character input - render font glyph
                if let iced::keyboard::Key::Character(s) = key {
                    if !modifiers.control() && !modifiers.alt() {
                        if let Some(ch) = s.chars().next() {
                            return self.render_char(ctx, ch);
                        }
                    }
                }

                ToolResult::None
            }
            _ => ToolResult::None,
        }
    }

    fn view_toolbar(&self, _ctx: &ToolViewContext) -> Element<'_, ToolMessage> {
        // Outline style labels (localized)
        let outline_labels: [String; 19] = [
            fl!(LANGUAGE_LOADER, "font-tool-outline_normal"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_round"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_square"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_shadow"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_3d"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_block1"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_block2"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_block3"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_block4"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_fancy1"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_fancy2"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_fancy3"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_fancy4"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_fancy5"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_fancy6"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_fancy7"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_fancy8"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_fancy9"),
            fl!(LANGUAGE_LOADER, "font-tool-outline_fancy10"),
        ];

        let has_fonts = self.font_tool.has_fonts();
        if !has_fonts {
            let content = row![
                text(fl!(LANGUAGE_LOADER, "font-tool-no_fonts")).size(14),
                Space::new().width(Length::Fixed(16.0)),
                button(text(fl!(LANGUAGE_LOADER, "font-tool-open_directory")).size(14))
                    .padding([4, 12])
                    .style(button::primary)
                    .on_press(ToolMessage::FontOpenDirectory),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center);

            return container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        }

        let font_label = self
            .font_tool
            .with_selected_font(|f| f.name().to_string())
            .unwrap_or_else(|| fl!(LANGUAGE_LOADER, "font-tool-select_font"));

        let font_button = button(text(font_label).size(14))
            .padding([4, 12])
            .style(button::primary)
            .on_press(ToolMessage::FontOpenSelector);

        let row1: Element<'_, ToolMessage> = ('!'..='O')
            .fold(row![].spacing(1), |r, ch| {
                let ok = self.font_tool.has_char(ch);
                r.push(text(ch.to_string()).size(12).style(move |theme: &Theme| iced::widget::text::Style {
                    color: Some(if ok { theme.background.on } else { theme.button.on }),
                }))
            })
            .into();

        let row2: Element<'_, ToolMessage> = ('P'..='~')
            .fold(row![].spacing(1), |r, ch| {
                let ok = self.font_tool.has_char(ch);
                r.push(text(ch.to_string()).size(12).style(move |theme: &Theme| iced::widget::text::Style {
                    color: Some(if ok { theme.background.on } else { theme.button.on }),
                }))
            })
            .into();

        let char_preview: Element<'_, ToolMessage> = container(column![row1, row2].spacing(2))
            .padding([2, 6])
            .style(|theme: &Theme| container::Style {
                background: Some(iced::Background::Color(theme.secondary.base)),
                border: iced::Border::default().rounded(4).width(1).color(theme.primary.divider),
                ..Default::default()
            })
            .into();

        let outline_label = outline_labels
            .get(self.outline_style_cache)
            .cloned()
            .unwrap_or_else(|| fl!(LANGUAGE_LOADER, "font-tool-outline_normal"));
        let outline_button = button(text(outline_label).size(14))
            .padding([4, 12])
            .style(button::secondary)
            .on_press(ToolMessage::FontOpenOutlineSelector);

        let content = row![
            Space::new().width(Length::Fill),
            font_button,
            Space::new().width(Length::Fixed(8.0)),
            char_preview,
            Space::new().width(Length::Fixed(16.0)),
            text(fl!(LANGUAGE_LOADER, "font-tool-outline")).size(14),
            outline_button,
            Space::new().width(Length::Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        content.into()
    }

    fn cursor(&self) -> iced::mouse::Interaction {
        self.selection_mouse.cursor().unwrap_or(iced::mouse::Interaction::Text)
    }

    fn show_caret(&self) -> bool {
        true
    }

    fn show_selection(&self) -> bool {
        true
    }
}

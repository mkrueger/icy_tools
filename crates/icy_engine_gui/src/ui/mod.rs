pub mod buffer_view;
use std::sync::Arc;

pub use buffer_view::*;

pub mod smooth_scroll;
use egui::{FontFamily, FontId, Pos2, Rect, Response, Vec2, WidgetText};
use icy_engine::TextPane;
pub use smooth_scroll::*;

pub mod keymaps;
pub use keymaps::*;

pub mod settings;
pub use settings::*;

use crate::{MarkerSettings, MonitorSettings};

#[derive(Clone, Debug)]
pub struct TerminalCalc {
    /// The height of the buffer in chars
    pub char_height: f32,
    pub char_width: f32,

    /// The height of the visible area in chars
    pub buffer_char_height: f32,
    pub buffer_char_width: f32,

    /// Size of a single terminal pixel in screen pixels
    pub scale: Vec2,

    pub char_size: Vec2,
    pub font_width: f32,
    pub font_height: f32,
    pub first_column: f32,
    pub first_line: f32,
    pub terminal_rect: egui::Rect,
    pub buffer_rect: egui::Rect,
    pub vert_scrollbar_rect: egui::Rect,
    pub horiz_scrollbar_rect: egui::Rect,
    pub char_scroll_position: Vec2,
    pub forced_width: i32,
    pub forced_height: i32,
    pub real_width: i32,
    pub real_height: i32,

    pub set_scroll_position_set_by_user: bool,

    pub has_focus: bool,

    pub screen_shot: Option<Vec<u8>>,
}

impl Default for TerminalCalc {
    fn default() -> Self {
        Self {
            char_height: Default::default(),
            char_width: Default::default(),
            buffer_char_height: Default::default(),
            buffer_char_width: Default::default(),
            scale: Default::default(),
            char_size: Default::default(),
            font_width: Default::default(),
            font_height: Default::default(),
            first_column: Default::default(),
            first_line: Default::default(),
            terminal_rect: egui::Rect::NOTHING,
            buffer_rect: egui::Rect::NOTHING,
            vert_scrollbar_rect: egui::Rect::NOTHING,
            horiz_scrollbar_rect: egui::Rect::NOTHING,
            char_scroll_position: Default::default(),
            forced_width: Default::default(),
            forced_height: Default::default(),
            set_scroll_position_set_by_user: Default::default(),
            has_focus: Default::default(),
            real_width: 0,
            real_height: 0,
            screen_shot: None,
        }
    }
}

impl TerminalCalc {
    pub fn from_buffer(buf: &icy_engine::Buffer) -> Self {
        let dims = buf.get_font_dimensions();
        let buffer_rect = Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(buf.get_width() as f32 * dims.width as f32, buf.get_height() as f32 * dims.height as f32),
        );
        Self {
            char_height: buf.get_height() as f32,
            char_width: buf.get_width() as f32,
            buffer_char_height: buf.get_height() as f32,
            buffer_char_width: buf.get_width() as f32,
            scale: Vec2::new(1.0, 1.0),
            char_size: Vec2::new(dims.width as f32, dims.height as f32),
            font_width: dims.width as f32,
            font_height: dims.height as f32,
            first_column: 0.0,
            first_line: 0.0,
            terminal_rect: buffer_rect,
            buffer_rect,
            vert_scrollbar_rect: egui::Rect::NOTHING,
            horiz_scrollbar_rect: egui::Rect::NOTHING,
            char_scroll_position: Vec2::ZERO,
            forced_width: buf.get_width(),
            forced_height: buf.get_height(),
            set_scroll_position_set_by_user: Default::default(),
            has_focus: false,
            real_width: buf.get_width(),
            real_height: buf.get_height(),
            screen_shot: None,
        }
    }

    /// Returns the char position of the cursor in the buffer
    pub fn calc_click_pos(&self, click_pos: Pos2) -> Vec2 {
        (click_pos.to_vec2() - self.buffer_rect.left_top().to_vec2()) / self.char_size + Vec2::new(self.first_column, self.first_line)
    }

    pub fn calc_click_pos_half_block(&self, click_pos: Pos2) -> Vec2 {
        (click_pos.to_vec2() - self.buffer_rect.left_top().to_vec2()) / Vec2::new(self.char_size.x, self.char_size.y / 2.0)
            + Vec2::new(self.first_column, self.first_line * 2.0)
    }

    pub fn viewport_top(&self) -> Vec2 {
        self.char_scroll_position * self.scale
    }

    pub fn max_y_scroll(&self) -> f32 {
        if self.char_height <= self.buffer_char_height {
            return 0.0;
        }
        let y_remainder = (self.char_size.y - self.terminal_rect.height() % self.char_size.y) / self.scale.y;
        (self.font_height * (self.char_height - self.buffer_char_height).max(0.0) + y_remainder).floor()
    }

    pub fn max_x_scroll(&self) -> f32 {
        if self.char_width <= self.buffer_char_width {
            return 0.0;
        }
        let x_remainder = (self.char_size.x - self.terminal_rect.width() % self.char_size.x) / self.scale.x;
        (self.font_width * (self.char_width - self.buffer_char_width).max(0.0) + x_remainder).floor()
    }
}

#[derive(Default, Clone, Copy)]
pub enum CaretShape {
    #[default]
    Underline,
    Block,
}

#[derive(Clone)]
pub struct TerminalOptions {
    pub filter: i32,
    pub monitor_settings: MonitorSettings,
    pub marker_settings: MarkerSettings,
    pub stick_to_bottom: bool,
    pub scale: Option<Vec2>,
    pub fit_width: bool,
    pub render_real_height: bool,
    pub use_terminal_height: bool,
    pub scroll_offset_x: Option<f32>,
    pub scroll_offset_y: Option<f32>,
    pub id: Option<egui::Id>,

    pub show_layer_borders: bool,
    pub show_line_numbers: bool,
    pub force_focus: bool,
    pub request_focus: bool,

    pub hide_scrollbars: bool,
    pub terminal_size: Option<Vec2>,
    pub guide: Option<Vec2>,
    pub raster: Option<Vec2>,
    pub clip_rect: Option<Rect>,
    pub caret_shape: CaretShape,
}

impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            filter: glow::NEAREST as i32,
            monitor_settings: Default::default(),
            marker_settings: Default::default(),
            stick_to_bottom: Default::default(),
            scale: Default::default(),
            fit_width: false,
            render_real_height: false,
            use_terminal_height: true,
            show_layer_borders: false,
            show_line_numbers: false,
            hide_scrollbars: false,
            force_focus: false,
            scroll_offset_x: None,
            scroll_offset_y: None,
            id: None,
            guide: None,
            raster: None,
            terminal_size: None,
            clip_rect: None,
            request_focus: false,
            caret_shape: CaretShape::Underline,
        }
    }
}

pub fn show_terminal_area(ui: &mut egui::Ui, buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>, options: TerminalOptions) -> (Response, TerminalCalc) {
    let mut forced_height = buffer_view.lock().get_buffer().get_height();
    let mut forced_width = buffer_view.lock().get_buffer().get_width();

    if buffer_view.lock().get_buffer().is_terminal_buffer {
        forced_width = buffer_view.lock().get_buffer().terminal_state.get_width();
        forced_height = buffer_view.lock().get_buffer().terminal_state.get_height();
    }

    let mut buf_h = forced_height as f32;
    let real_height = if options.use_terminal_height {
        buffer_view.lock().get_buffer().get_height().max(forced_height)
    } else {
        forced_height
    };
    let real_width = forced_width;

    let mut buf_w = real_width as f32;

    let font_dimensions = buffer_view.lock().get_buffer().get_font_dimensions();
    let buffer_view2: Arc<egui::mutex::Mutex<BufferView>> = buffer_view.clone();

    let mut scroll = SmoothScroll::new()
        .with_stick_to_bottom(options.stick_to_bottom)
        .with_scroll_y_offset(options.scroll_offset_y)
        .with_scroll_x_offset(options.scroll_offset_x)
        .with_hide_scrollbars(options.hide_scrollbars);

    if let Some(id) = options.id {
        scroll = scroll.with_id(id);
    }
    let caret_pos = buffer_view.lock().get_edit_state().get_caret().get_position();
    let selected_rect = buffer_view.lock().get_edit_state().get_selection();
    let show_line_numbers = options.show_line_numbers;
    let (response, calc) = scroll.show(
        ui,
        &options,
        |rect, options: &TerminalOptions| {
            let size = rect.size();

            let font_width = font_dimensions.width as f32 + if buffer_view2.lock().get_buffer().use_letter_spacing() { 1.0 } else { 0.0 };

            let mut scale_x = size.x / font_width / buf_w;
            let mut scale_y = size.y / font_dimensions.height as f32 / buf_h;
            let mut forced_scale = options.scale;
            if options.fit_width {
                forced_scale = Some(Vec2::new(scale_x, scale_x));
            }

            if scale_x < scale_y {
                scale_y = scale_x;
            } else {
                scale_x = scale_y;
            }

            if let Some(scale) = forced_scale {
                scale_x = scale.x;
                scale_y = scale.y;

                let h = size.y / (font_dimensions.height as f32 * scale_y);
                buf_h = h.ceil().min(real_height as f32);

                forced_height = (buf_h as i32).min(real_height);

                let w = size.x / (font_dimensions.width as f32 * scale_x);
                buf_w = w.ceil().min(real_width as f32);

                forced_width = (buf_w as i32).min(real_width);
            }

            let char_size = Vec2::new(font_width * scale_x, font_dimensions.height as f32 * scale_y);

            let rect_w = buf_w * char_size.x;
            let rect_h = buf_h * char_size.y;
            let buffer_rect = Rect::from_min_size(
                Pos2::new(
                    (rect.left() + (rect.width() - rect_w).max(0.0) / 2.).floor(),
                    rect.top() + ((rect.height() - rect_h) / 2.).max(0.0).floor(),
                ),
                Vec2::new(rect_w.floor(), rect_h.floor()),
            );

            // Set the scrolling height.
            TerminalCalc {
                char_height: real_height as f32,
                char_width: real_width as f32,
                buffer_char_width: buf_w,
                buffer_char_height: buf_h,
                scale: Vec2::new(scale_x, scale_y),
                char_size: Vec2::new(font_width * scale_x, font_dimensions.height as f32 * scale_y),
                font_width: font_dimensions.width as f32,
                font_height: font_dimensions.height as f32,
                first_column: 0.,
                first_line: 0.,
                terminal_rect: rect,
                buffer_rect,
                vert_scrollbar_rect: Rect::NOTHING,
                horiz_scrollbar_rect: Rect::NOTHING,
                char_scroll_position: Vec2::ZERO,
                set_scroll_position_set_by_user: false,
                forced_width,
                forced_height,
                real_width,
                real_height,
                has_focus: false,
                screen_shot: None,
            }
        },
        |ui, calc, options: &TerminalOptions| {
            let viewport_top_y = calc.char_scroll_position.y * calc.scale.y;
            calc.first_line = viewport_top_y / calc.char_size.y;
            let viewport_top_x = calc.char_scroll_position.x * calc.scale.x;
            calc.first_column = viewport_top_x / calc.char_size.x;

            /*
            {
                let buffer_view = &mut buffer_view.lock();
                buffer_view.char_size = calc.char_size;
                if buffer_view.viewport_top != viewport_top {
                    buffer_view.viewport_top = viewport_top;
                    buffer_view.redraw_view();
                }
            }*/
            buffer_view.lock().calc = calc.clone();
            let options = options.clone();
            let callback = egui::PaintCallback {
                rect: calc.terminal_rect,
                callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |info, painter| {
                    buffer_view.lock().render_contents(painter.gl(), &info, &options);
                })),
            };
            ui.painter().add(callback);

            if show_line_numbers {
                let font_size = 12.0 * calc.font_height / 16.0 * calc.scale.y;
                ui.set_clip_rect(calc.terminal_rect);
                let painter = ui.painter();
                if calc.char_width <= calc.buffer_char_width {
                    for y in 0..if calc.forced_height < calc.char_height as i32 {
                        calc.forced_height + 1
                    } else {
                        calc.forced_height
                    } {
                        let font_id = FontId::new(font_size, FontFamily::Proportional);
                        let text: WidgetText = format!("{}", 1 + y + calc.first_line as i32).into();
                        let galley = text.into_galley(ui, Some(false), f32::INFINITY, font_id);
                        let rect = Rect::from_min_size(
                            Pos2::new(
                                calc.buffer_rect.left() - galley.size().x - 4.0 - (calc.char_scroll_position.x % calc.font_height) * calc.scale.y,
                                calc.buffer_rect.top() + y as f32 * calc.char_size.y - (calc.char_scroll_position.y % calc.font_height) * calc.scale.y,
                            ),
                            Vec2::new(galley.size().x, calc.char_height),
                        );
                        let is_selected = if let Some(sel) = selected_rect {
                            sel.min().y <= y + calc.first_line as i32 && y + (calc.first_line as i32) < sel.max().y
                        } else {
                            caret_pos.y == y + calc.first_line as i32
                        };
                        let color = if is_selected {
                            ui.visuals().strong_text_color()
                        } else {
                            ui.visuals().text_color()
                        };
                        painter.galley_with_override_text_color(egui::Align2::RIGHT_TOP.align_size_within_rect(galley.size(), rect).min, galley.clone(), color);

                        let rect = Rect::from_min_size(
                            Pos2::new(
                                calc.buffer_rect.left() + calc.buffer_char_width * calc.char_size.x + 4.0
                                    - (calc.char_scroll_position.x % calc.font_width) * calc.scale.x,
                                calc.buffer_rect.top() + y as f32 * calc.char_size.y - (calc.char_scroll_position.y % calc.font_height) * calc.scale.y,
                            ),
                            Vec2::new(galley.size().x, calc.char_height),
                        );
                        painter.galley_with_override_text_color(egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), rect).min, galley, color);
                    }
                }
                let buf_w = calc.buffer_char_width;
                if calc.char_height <= calc.buffer_char_height {
                    for x in 0..buf_w as i32 {
                        let font_id = FontId::new(font_size, FontFamily::Proportional);
                        let text: WidgetText = format!("{}", (1 + x) % 10).into();
                        let galley = text.into_galley(ui, Some(false), f32::INFINITY, font_id);
                        let rect = Rect::from_min_size(
                            Pos2::new(
                                calc.buffer_rect.left() - galley.size().x - 4.0 + x as f32 * calc.char_size.x + calc.char_size.x,
                                calc.buffer_rect.top() - calc.char_size.y,
                            ),
                            Vec2::new(galley.size().x, calc.char_height),
                        );
                        let is_selected = if let Some(sel) = selected_rect {
                            sel.min().x <= x && x < sel.max().x
                        } else {
                            caret_pos.x == x
                        };
                        let color = if is_selected {
                            ui.visuals().strong_text_color()
                        } else {
                            ui.visuals().text_color()
                        };
                        painter.galley_with_override_text_color(egui::Align2::RIGHT_TOP.align_size_within_rect(galley.size(), rect).min, galley.clone(), color);
                        let rect = Rect::from_min_size(
                            Pos2::new(
                                calc.buffer_rect.left() - galley.size().x - 4.0 + x as f32 * calc.char_size.x + calc.char_size.x,
                                calc.buffer_rect.bottom() + 4.0,
                            ),
                            Vec2::new(galley.size().x, calc.char_height),
                        );

                        painter.galley_with_override_text_color(egui::Align2::RIGHT_TOP.align_size_within_rect(galley.size(), rect).min, galley, color);
                    }
                }
            }
        },
    );

    (response, calc)
}

use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    DesktopLanguageRequester,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n"] // path to the compiled localization resources
struct Localizations;

use once_cell::sync::Lazy;
static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});

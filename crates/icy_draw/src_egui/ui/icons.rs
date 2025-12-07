use egui::{Image, Vec2};

lazy_static::lazy_static! {
    pub static ref SWAP_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/swap.svg"));
}

lazy_static::lazy_static! {
    pub static ref ADD_LAYER_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/add_layer.svg"));
}

lazy_static::lazy_static! {
    pub static ref MOVE_DOWN_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/move_down.svg"));
}

lazy_static::lazy_static! {
    pub static ref MOVE_UP_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/move_up.svg"));
}

const SIZE: Vec2 = Vec2::splat(22.0);

lazy_static::lazy_static! {
    pub static ref DELETE_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/delete.svg"));
    pub static ref VISIBLE_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/visible.svg"));
    pub static ref INVISIBLE_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/invisible.svg"));
    pub static ref ANCHOR_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/anchor.svg"));

    pub static ref PLAY_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/play.svg")).fit_to_exact_size(SIZE);
    pub static ref REPLAY_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/replay.svg")).fit_to_exact_size(SIZE);
    pub static ref PAUSE_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/pause.svg")).fit_to_exact_size(SIZE);
    pub static ref PLAY_PAUSE_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/play_pause.svg")).fit_to_exact_size(SIZE);
    pub static ref SKIP_NEXT_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/skip_next.svg")).fit_to_exact_size(SIZE);
    pub static ref REPEAT_SVG: Image<'static> = Image::new(egui::include_image!("../../data/icons/repeat.svg")).fit_to_exact_size(SIZE);
    pub static ref NAVIGATE_NEXT: Image<'static> = Image::new(egui::include_image!("../../data/icons/navigate_next.svg")).fit_to_exact_size(SIZE);
    pub static ref NAVIGATE_PREV: Image<'static> = Image::new(egui::include_image!("../../data/icons/navigate_prev.svg")).fit_to_exact_size(SIZE);

    pub static ref ANSI_TEMPLATE_IMG: Image<'static> = Image::new(egui::include_image!("../../data/file_template_icons/ansi.png"));
    pub static ref ANSIMATION_TEMPLATE_IMG: Image<'static> = Image::new(egui::include_image!("../../data/file_template_icons/ansimation.png"));
    pub static ref FILE_ID_TEMPLATE_IMG: Image<'static> = Image::new(egui::include_image!("../../data/file_template_icons/file_id.png"));
    pub static ref BITFONT_TEMPLATE_IMG: Image<'static> = Image::new(egui::include_image!("../../data/file_template_icons/bit_font.png"));
    pub static ref BLOCKFONT_TEMPLATE_IMG: Image<'static> = Image::new(egui::include_image!("../../data/file_template_icons/block_font.png"));
    pub static ref COLORFONT_TEMPLATE_IMG: Image<'static> = Image::new(egui::include_image!("../../data/file_template_icons/color_font.png"));
    pub static ref OUTLINEFONT_TEMPLATE_IMG: Image<'static> = Image::new(egui::include_image!("../../data/file_template_icons/outline_font.png"));
}

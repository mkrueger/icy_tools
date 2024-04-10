use egui::{Image, Vec2};
const SIZE: Vec2 = Vec2::splat(22.0);

lazy_static::lazy_static! {
    pub static ref ADD: Image<'static> = Image::new(egui::include_image!("../data/icons/add.svg")).fit_to_exact_size(SIZE);
    pub static ref CALL: Image<'static> = Image::new(egui::include_image!("../data/icons/call.svg")).fit_to_exact_size(SIZE);
    pub static ref CALL_MADE: Image<'static> = Image::new(egui::include_image!("../data/icons/call_made.svg")).fit_to_exact_size(SIZE);
    pub static ref DELETE: Image<'static> = Image::new(egui::include_image!("../data/icons/delete.svg")).fit_to_exact_size(SIZE);
    pub static ref DOWNLOAD: Image<'static> = Image::new(egui::include_image!("../data/icons/download.svg")).fit_to_exact_size(SIZE);
    pub static ref LOGOUT: Image<'static> = Image::new(egui::include_image!("../data/icons/logout.svg")).fit_to_exact_size(SIZE);
    pub static ref MENU: Image<'static> = Image::new(egui::include_image!("../data/icons/menu.svg")).fit_to_exact_size(SIZE);
    pub static ref STAR: Image<'static> = Image::new(egui::include_image!("../data/icons/star.svg")).fit_to_exact_size(SIZE);
    pub static ref UNSTAR: Image<'static> = Image::new(egui::include_image!("../data/icons/unstar.svg")).fit_to_exact_size(SIZE);
    pub static ref UPLOAD: Image<'static> = Image::new(egui::include_image!("../data/icons/upload.svg")).fit_to_exact_size(SIZE);
    pub static ref CLOSE: Image<'static> = Image::new(egui::include_image!("../data/icons/close.svg")).fit_to_exact_size(SIZE);
    pub static ref VISIBILITY: Image<'static> = Image::new(egui::include_image!("../data/icons/visibility.svg")).fit_to_exact_size(SIZE);
    pub static ref VISIBILITY_OFF: Image<'static> = Image::new(egui::include_image!("../data/icons/visibility_off.svg")).fit_to_exact_size(SIZE);
    pub static ref KEY: Image<'static> = Image::new(egui::include_image!("../data/icons/key.svg")).fit_to_exact_size(SIZE);

}

use egui::Image;

lazy_static::lazy_static! {
    pub static ref ADD_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/add.svg"));
    pub static ref PENCIL_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/pencil.svg"));
    pub static ref BRUSH_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/paint_brush.svg"));
    pub static ref TEXT_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/text.svg"));
    pub static ref CURSOR_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/cursor.svg"));
    pub static ref DROPPER_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/dropper.svg"));
    pub static ref ELLIPSE_FILLED_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/ellipse_filled.svg"));
    pub static ref ELLIPSE_OUTLINE_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/ellipse_outline.svg"));
    pub static ref ERASER_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/eraser.svg"));
    pub static ref FILL_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/fill.svg"));
    pub static ref LINE_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/line.svg"));
    pub static ref FONT_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/font.svg"));
    pub static ref MOVE_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/move.svg"));
    pub static ref RECTANGLE_FILLED_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/rectangle_filled.svg"));
    pub static ref RECTANGLE_OUTLINE_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/rectangle_outline.svg"));
    pub static ref SELECT_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/select.svg"));
    pub static ref FLIP_TOOL_SVG: Image<'static> = Image::new(egui::include_image!("../../../data/icons/flip_tool.svg"));
}

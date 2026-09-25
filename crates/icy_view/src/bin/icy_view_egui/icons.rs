use eframe::egui;
use icy_engine_gui::file_icons::FileIcon;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Back,
    Forward,
    Up,
    Down,
    Refresh,
    Web,
    List,
    Tiles,
    Shuffle,
    Menu,
    Search,
    SortName,
    SortSize,
    SortDate,
    Star,
    StarOutline,
    Places,
    Close,
    Home,
    History,
    Chevron,
    Play,
    Pause,
    Replay,
    Bolt,
    File(FileIcon),
}

#[derive(Default)]
pub struct Icons(HashMap<String, egui::TextureHandle>);

impl Icons {
    pub fn image(&mut self, context: &egui::Context, icon: Icon, size: f32) -> egui::Image<'static> {
        let (name, bytes): (&str, &[u8]) = match icon {
            Icon::Back => ("back", include_bytes!("../../../data/icons/arrow_back.svg")),
            Icon::Forward => ("forward", include_bytes!("../../../data/icons/arrow_forward.svg")),
            Icon::Up => ("up", include_bytes!("../../../data/icons/arrow_upward.svg")),
            Icon::Down => ("down", include_bytes!("../../../data/icons/arrow_downward.svg")),
            Icon::Refresh => ("refresh", include_bytes!("../../../data/icons/refresh.svg")),
            Icon::Web => ("web", include_bytes!("../../../data/icons/language.svg")),
            Icon::List => ("list", include_bytes!("../../../data/icons/view_list.svg")),
            Icon::Tiles => ("tiles", include_bytes!("../../../data/icons/grid_view.svg")),
            Icon::Shuffle => ("shuffle", include_bytes!("../../../data/icons/shuffle.svg")),
            Icon::Menu => ("menu", include_bytes!("../../../../icy_engine_gui/data/icons/menu.svg")),
            Icon::Search => ("search", include_bytes!("../../../data/icons/search.svg")),
            Icon::SortName => ("sort-name", include_bytes!("../../../data/icons/sort_by_alpha.svg")),
            Icon::SortSize => ("sort-size", include_bytes!("../../../data/icons/straighten.svg")),
            Icon::SortDate => ("sort-date", include_bytes!("../../../data/icons/calendar_today.svg")),
            Icon::Star => ("star", include_bytes!("../../../data/icons/star.svg")),
            Icon::StarOutline => ("star-outline", include_bytes!("../../../data/icons/star_outline.svg")),
            Icon::Places => ("places", include_bytes!("../../../data/icons/bookmarks.svg")),
            Icon::Close => ("close", include_bytes!("../../../data/icons/close.svg")),
            Icon::Home => ("home", include_bytes!("../../../data/icons/home.svg")),
            Icon::History => ("history", include_bytes!("../../../data/icons/history.svg")),
            Icon::Chevron => ("chevron", include_bytes!("../../../data/icons/chevron_right.svg")),
            Icon::Play => ("play", include_bytes!("../../../data/icons/play_arrow.svg")),
            Icon::Pause => ("pause", include_bytes!("../../../data/icons/pause.svg")),
            Icon::Replay => ("replay", include_bytes!("../../../data/icons/replay.svg")),
            Icon::Bolt => ("bolt", include_bytes!("../../../data/icons/bolt.svg")),
            Icon::File(kind) => {
                let name = match kind {
                    FileIcon::Folder | FileIcon::FolderOpen | FileIcon::FolderData => "folder",
                    FileIcon::Archive => "archive",
                    FileIcon::Image => "image",
                    FileIcon::Ansi => "ansi",
                    FileIcon::Native => "native",
                    FileIcon::Music => "music",
                    FileIcon::Movie => "movie",
                    FileIcon::Binary => "binary",
                    FileIcon::Graphics | FileIcon::Game => "graphics",
                    _ => "text",
                };
                let bytes: &[u8] = match name {
                    "folder" => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_folder.svg"),
                    "archive" => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/folder_zip.svg"),
                    "image" => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_image.svg"),
                    "ansi" => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_ansi.svg"),
                    "native" => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_native.svg"),
                    "music" => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_music.svg"),
                    "movie" => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_movie.svg"),
                    "binary" => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_binary.svg"),
                    "graphics" => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_graphics.svg"),
                    _ => include_bytes!("../../../../icy_engine_gui/src/ui/icons/files/file_text.svg"),
                };
                (name, bytes)
            }
        };
        let texture = self.0.entry(name.to_string()).or_insert_with(|| {
            let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default()).expect("bundled icon");
            let mut pixels = resvg::tiny_skia::Pixmap::new(48, 48).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(48.0 / tree.size().width(), 48.0 / tree.size().height()),
                &mut pixels.as_mut(),
            );
            let image = egui::ColorImage::from_rgba_premultiplied([48, 48], pixels.data());
            context.load_texture(name, image, egui::TextureOptions::LINEAR)
        });
        egui::Image::new((texture.id(), egui::Vec2::splat(size))).tint(context.style().visuals.text_color())
    }

    /// Flat toolbar button: only hovered, pressed or selected buttons get a background.
    pub fn button(&mut self, ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool, selected: bool) -> egui::Response {
        let image = self.image(ui.ctx(), icon, 18.0);
        ui.scope(|ui| {
            compact(ui);
            ui.add_enabled(enabled, tool_button(image, selected))
        })
        .inner
        .on_hover_text(label)
    }
}

/// Button padding that keeps icon buttons square instead of the wide text button default.
pub fn compact(ui: &mut egui::Ui) {
    ui.spacing_mut().button_padding = egui::vec2(6.0, 5.0);
}

pub const TOOL_SIZE: egui::Vec2 = egui::vec2(30.0, 28.0);

pub fn tool_button<'a>(image: egui::Image<'a>, selected: bool) -> egui::Button<'a> {
    egui::Button::image(image)
        .selected(selected)
        .frame_when_inactive(selected)
        .image_tint_follows_text_color(true)
        .stroke(egui::Stroke::NONE)
        .corner_radius(6)
        .min_size(TOOL_SIZE)
}

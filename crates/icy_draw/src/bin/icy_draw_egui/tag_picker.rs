//! Browser of the tag replacement lists (PCBoard, IcyBoard and the user's own TOML lists)
//! shown in the tag properties dialog.

use std::path::PathBuf;

use eframe::egui;
use icy_draw::{
    fl,
    tag_replacements::{self, TagReplacement, TagReplacementList, TaglistInfo},
};
use icy_engine_gui::egui::appearance;

const TAG_COLUMN: f32 = 150.0;
const LIST_HEIGHT: f32 = 320.0;

/// What the user did in the browser that the dialog owner has to carry out.
pub enum Action {
    Pick(TagReplacement),
    /// Another list was chosen; its id is remembered in the settings.
    SelectList(String),
    Import,
    Create,
    OpenFolder,
    Close,
}

pub struct ReplacementPicker {
    dir: Option<PathBuf>,
    lists: Vec<TaglistInfo>,
    list: TagReplacementList,
    filter: String,
    /// Frames left in which the filter asks for the keyboard; the dialog may resize and
    /// relayout first.
    focus_filter: u8,
    /// Why the last import, list creation or folder opening failed.
    error: Option<String>,
}

impl ReplacementPicker {
    /// Loads the available lists fresh from `dir`, so edited or added files show up.
    pub fn new(selected: &str, dir: Option<PathBuf>) -> Self {
        let lists = tag_replacements::get_available_taglists(dir.as_deref());
        let mut picker = Self {
            list: tag_replacements::load_taglist("", None),
            dir,
            lists,
            filter: String::new(),
            focus_filter: 3,
            error: None,
        };
        picker.select(selected);
        picker
    }

    #[cfg(test)]
    pub fn selected(&self) -> &str {
        &self.list.id
    }

    /// Whether there is a folder for own lists to import, create and open.
    pub fn has_folder(&self) -> bool {
        self.dir.is_some()
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    /// Reloads the lists, e.g. after one was imported or created, and shows `id`.
    pub fn reload(&mut self, id: &str) {
        self.lists = tag_replacements::get_available_taglists(self.dir.as_deref());
        self.select(id);
    }

    fn select(&mut self, id: &str) {
        self.error = None;
        let id = self
            .lists
            .iter()
            .find(|list| list.id.eq_ignore_ascii_case(id))
            .or_else(|| self.lists.first())
            .map(|list| list.id.clone())
            .unwrap_or_default();
        self.list = tag_replacements::load_taglist(&id, self.dir.as_deref());
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        appearance::group(ui, &fl!("tag-replacements-title"), |ui| {
            ui.horizontal(|ui| {
                let name = self.lists.iter().find(|list| list.id == self.list.id).map(|list| list.name.clone()).unwrap_or_default();
                egui::ComboBox::from_id_salt("tag-replacement-list")
                    .selected_text(name)
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        for list in &self.lists {
                            if ui.selectable_label(list.id == self.list.id, &list.name).clicked() {
                                action = Some(Action::SelectList(list.id.clone()));
                            }
                        }
                    });
                let filter = ui.add(
                    appearance::text_edit(&mut self.filter)
                        .hint_text(fl!("tag-edit-filter"))
                        .desired_width(f32::INFINITY),
                );
                if filter.has_focus() {
                    self.focus_filter = 0;
                } else if self.focus_filter > 0 {
                    self.focus_filter -= 1;
                    filter.request_focus();
                }
                if filter.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    if let Some(entry) = tag_replacements::filter_taglist(&self.list, &self.filter).first() {
                        action = Some(Action::Pick((*entry).clone()));
                    }
                }
            });
            if !self.list.description.trim().is_empty() {
                ui.add(egui::Label::new(egui::RichText::new(&self.list.description).weak()).wrap());
            }
            ui.add_space(4.0);
            let entries = tag_replacements::filter_taglist(&self.list, &self.filter);
            egui::ScrollArea::vertical().id_salt("tag-replacement-entries").min_scrolled_height(LIST_HEIGHT).max_height(LIST_HEIGHT).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.spacing_mut().interact_size.y = 18.0;
                if entries.is_empty() {
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| ui.weak(fl!("tag-replacements-none")));
                }
                for (index, entry) in entries.iter().enumerate() {
                    if entry_row(ui, index, entry).clicked() {
                        action = Some(Action::Pick((*entry).clone()));
                    }
                }
            });
            if !self.list.comments.trim().is_empty() {
                ui.add_space(4.0);
                egui::CollapsingHeader::new(fl!("tag-replacements-notes"))
                    .id_salt("tag-replacement-notes")
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(self.list.comments.trim()).small()).wrap());
                    });
            }
            if let Some(error) = &self.error {
                ui.add_space(4.0);
                ui.add(egui::Label::new(egui::RichText::new(error).color(ui.visuals().error_fg_color)).wrap());
            }
        });
        if let Some(Action::SelectList(id)) = &action {
            self.select(id);
        }
        action
    }
}

/// One replacement: the tag in a fixed column and its description, highlighted on hover.
fn entry_row(ui: &mut egui::Ui, index: usize, entry: &TagReplacement) -> egui::Response {
    // Reserved before the text, so the hover highlight is painted behind it.
    let background = ui.painter().add(egui::Shape::Noop);
    let rect = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(egui::vec2(TAG_COLUMN, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(TAG_COLUMN);
                    ui.add(egui::Label::new(egui::RichText::new(&entry.tag).monospace().strong()).truncate().selectable(false));
                });
                ui.add(egui::Label::new(&entry.description).wrap().selectable(false));
            });
        })
        .response
        .rect;
    let response = ui.interact(rect, ui.id().with(("tag-replacement", index)), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .set(background, egui::Shape::rect_filled(rect, 4, ui.visuals().widgets.hovered.weak_bg_fill));
    }
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if entry.example.is_empty() {
        response
    } else {
        response.on_hover_text(fl!("tag-replacements-example", example = entry.example.as_str()))
    }
}

/// The preview a picked replacement gives the tag: its example, or the tag itself when the
/// replacement shows nothing, so the tag stays visible.
pub fn preview(entry: &TagReplacement) -> String {
    if entry.example.is_empty() {
        entry.tag.clone()
    } else {
        entry.example.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Dialog, DrawApp};
    use eframe::egui;

    fn frame(context: &egui::Context, app: &mut DrawApp, events: Vec<egui::Event>) -> egui::FullOutput {
        let time = context.input(|input| input.time) + 0.05;
        context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
                events,
                time: Some(time),
                ..Default::default()
            },
            |context| app.show(context),
        )
    }

    fn text_position(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.size() / 2.0),
            _ => None,
        })
    }

    fn click(context: &egui::Context, app: &mut DrawApp, label: &str) {
        // The dialog settles its size and position over a few frames.
        for _ in 0..8 {
            frame(context, app, vec![]);
        }
        let output = frame(context, app, vec![]);
        let position = text_position(&output, label).unwrap_or_else(|| panic!("no {label:?} on screen"));
        frame(context, app, vec![egui::Event::PointerMoved(position)]);
        for pressed in [true, false] {
            frame(
                context,
                app,
                vec![egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
    }

    /// Types `text` once the replacement filter has the keyboard.
    fn type_filter(context: &egui::Context, app: &mut DrawApp, text: &str) {
        for _ in 0..4 {
            frame(context, app, vec![]);
        }
        frame(context, app, vec![egui::Event::Text(text.into())]);
    }

    fn draft(app: &DrawApp) -> icy_engine::Tag {
        match &app.dialog {
            Some(Dialog::TagProperties(_, tag)) => (**tag).clone(),
            _ => panic!("the tag dialog is open"),
        }
    }

    #[test]
    fn picking_a_replacement_fills_the_tag() {
        super::super::tests::use_english();
        let context = egui::Context::default();
        icy_engine_gui::egui::appearance::apply(&context);
        let mut app = DrawApp::new();
        frame(&context, &mut app, vec![]);
        app.open_tag_properties(None);

        click(&context, &mut app, "…");
        assert_eq!(app.tag_picker.as_ref().map(|picker| picker.selected()), Some("pcboard"), "the first list is shown");
        type_filter(&context, &mut app, "user");
        click(&context, &mut app, "@USER@");
        let tag = draft(&app);
        assert_eq!((tag.replacement_value.as_str(), tag.preview.as_str()), ("@USER@", "JOHN DOE"));
        assert!(app.tag_picker.is_none(), "picking returns to the tag");

        click(&context, &mut app, "…");
        type_filter(&context, &mut app, "beep");
        click(&context, &mut app, "@BEEP@");
        let tag = draft(&app);
        assert_eq!(
            (tag.replacement_value.as_str(), tag.preview.as_str()),
            ("@BEEP@", "@BEEP@"),
            "without an example the tag shows itself"
        );
    }

    #[test]
    fn own_lists_are_listed_after_the_built_in_ones() {
        let dir = tempfile::tempdir().unwrap();
        let (id, _) = icy_draw::tag_replacements::create_taglist(dir.path()).unwrap();
        let mut picker = super::ReplacementPicker::new(&id, Some(dir.path().to_path_buf()));
        assert_eq!(picker.selected(), "my_tags");
        assert_eq!(picker.lists.last().map(|list| list.name.as_str()), Some("My Tags"));
        picker.reload("missing");
        assert_eq!(picker.selected(), "pcboard", "a vanished list falls back to the first");
    }
}

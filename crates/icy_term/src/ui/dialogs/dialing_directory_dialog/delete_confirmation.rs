use crate::ui::dialing_directory_dialog::DialingDirectoryMsg;
use crate::ui::Message;
use i18n_embed_fl::fl;
use icy_ui::Element;
use icy_engine_gui::ui::{ButtonSet, ConfirmationDialog, DialogResult, DialogType};

impl super::DialingDirectoryState {
    pub fn delete_confirmation_modal(&self, idx: usize) -> Element<'_, Message> {
        let system_name = if let Some(addr) = self.addresses.lock().addresses.get(idx) {
            addr.system_name.clone()
        } else {
            "Unknown".to_string()
        };

        let title = fl!(crate::LANGUAGE_LOADER, "delete-bbs-title");
        let question = fl!(crate::LANGUAGE_LOADER, "delete-bbs-question", system = system_name);

        // Create background element from the main content
        let background = icy_ui::widget::Space::new().width(icy_ui::Length::Fill).height(icy_ui::Length::Fill).into();

        let dialog = ConfirmationDialog::new(title, question)
            .dialog_type(DialogType::Question)
            .buttons(ButtonSet::DeleteCancel);

        dialog.view(background, move |result| match result {
            DialogResult::Delete => Message::from(DialingDirectoryMsg::ConfirmDelete(idx)),
            _ => Message::from(DialingDirectoryMsg::Close),
        })
    }
}

use crate::ui::Message;
use crate::ui::dialing_directory_dialog::DialingDirectoryMsg;
use i18n_embed_fl::fl;
use iced::Element;
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
        let background = iced::widget::Space::new().width(iced::Length::Fill).height(iced::Length::Fill).into();

        let dialog = ConfirmationDialog::new(title, question)
            .dialog_type(DialogType::Question)
            .buttons(ButtonSet::DeleteCancel);

        dialog.view(background, move |result| match result {
            DialogResult::Delete => Message::from(DialingDirectoryMsg::ConfirmDelete(idx)),
            _ => Message::from(DialingDirectoryMsg::Close),
        })
    }
}

use crate::{Options, ui::MainWindowMode};

#[derive(Default)]
pub struct MainWindowState {
    pub mode: MainWindowMode,

    //    pub settings_dialog: dialogs::settings_dialog::DialogState,

    // don't store files in unit test mode
    #[cfg(test)]
    pub options_written: bool,
}

impl MainWindowState {
    #[cfg(test)]
    pub fn store_options(&mut self) {
        self.options_written = true;
    }

    #[cfg(not(test))]
    pub fn store_options(&mut self) {
        /*        if let Err(err) = self.options.store_options() {
            log::error!("{err}");
        }*/
    }
}

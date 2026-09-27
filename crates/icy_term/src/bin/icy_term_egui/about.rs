//! About screen: the shared icy tools about dialog with this tool's artwork.

use eframe::egui;
use icy_engine_gui::egui::about::AboutDialog;

const ABOUT_ANSI: &[u8] = include_bytes!("../../../data/about.icy");

pub struct About(AboutDialog);

impl About {
    pub fn load() -> Result<Self, String> {
        let version = semver::Version::parse(env!("CARGO_PKG_VERSION")).map_err(|error| error.to_string())?;
        AboutDialog::new(ABOUT_ANSI, &version, option_env!("ICY_BUILD_DATE").map(String::from)).map(Self)
    }

    /// Returns a URL when one of the artwork's hyperlinks was clicked.
    pub fn show(&mut self, context: &egui::Context, open: &mut bool) -> Option<String> {
        let mut link = None;
        *open = self.0.show_with_link(context, &mut link);
        link
    }
}

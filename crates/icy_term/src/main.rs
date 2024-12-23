#![warn(clippy::all, clippy::pedantic)]
#![allow(
    non_upper_case_globals,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::struct_excessive_bools,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless
)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod ui;
use std::path::PathBuf;

use directories::ProjectDirs;
use lazy_static::lazy_static;
use ui::MainWindow;
use web_time::Instant;
pub type TerminalResult<T> = Res<T>;
use i18n_embed::fluent::{fluent_language_loader, FluentLanguageLoader};
use log::LevelFilter;
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};
use semver::Version;

pub mod data;
pub use data::*;

mod features;
mod icons;
mod util;
pub type Res<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

lazy_static! {
    static ref VERSION: Version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    static ref DEFAULT_TITLE: String = format!("iCY TERM {}", *crate::VERSION);
}

lazy_static::lazy_static! {
    static ref LATEST_VERSION: Version = {
        let github = github_release_check::GitHub::new().unwrap();
        if let Ok(ver) = github.get_all_versions("mkrueger/icy_tools") {
            for v in ver {
                if v.starts_with("IcyTerm") {
                    return Version::parse(&v[7..]).unwrap();
                }
            }
        }
        VERSION.clone()
    };
}
/* RustEmbed version (not working os wasm atm)
#[derive(RustEmbed)]
#[folder = "i18n"] // path to the compiled localization resources
struct Localizations;
*/

// Hack for fixing RustEmbed on wasm
struct Localizations {}

const localization_files: [&str; 2] = ["en/icy_term.ftl", "de/icy_term.ftl"];

impl i18n_embed::I18nAssets for Localizations {
    fn get_file(&self, file_path: &str) -> Option<std::borrow::Cow<'_, [u8]>> {
        match file_path {
            "en/icy_term.ftl" => Some(std::borrow::Cow::Borrowed(include_bytes!("../i18n/en/icy_term.ftl"))),
            "de/icy_term.ftl" => Some(std::borrow::Cow::Borrowed(include_bytes!("../i18n/de/icy_term.ftl"))),
            _ => None,
        }
    }

    fn filenames_iter(&self) -> Box<dyn Iterator<Item = String>> {
        Box::new(localization_files.iter().map(std::string::ToString::to_string))
    }
}

use once_cell::sync::Lazy;
static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    #[cfg(not(target_arch = "wasm32"))]
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    #[cfg(target_arch = "wasm32")]
    let requested_languages = i18n_embed::WebLanguageRequester::requested_languages();

    let _result = i18n_embed::select(&loader, &Localizations {}, &requested_languages);
    loader
});

fn get_log_file() -> anyhow::Result<PathBuf> {
    if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_term") {
        let dir = proj_dirs.config_dir().join("icy_term.log");
        return Ok(dir);
    }
    Err(anyhow::anyhow!("Error getting log directory"))
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::fs;

    use eframe::icon_data::from_png_bytes;

    let options = match Options::load_options() {
        Ok(options) => options,
        Err(e) => {
            log::error!("Error reading dialing_directory: {e}");
            Options::default()
        }
    };

    let mut native_options = eframe::NativeOptions {
        multisampling: 0,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let icon_data = from_png_bytes(include_bytes!("../build/linux/256x256.png")).unwrap();
    if let Some(rect) = options.window_rect {
        native_options.viewport = native_options
            .viewport
            .with_inner_size(rect.max.to_vec2())
            .with_icon(icon_data)
            .with_position(rect.min);
    } else {
        native_options.viewport = native_options.viewport.with_inner_size(egui::vec2(1284. + 8., 839.)).with_icon(icon_data);
    }

    if let Ok(log_file) = get_log_file() {
        // delete log file when it is too big
        if let Ok(data) = fs::metadata(&log_file) {
            if data.len() > 1024 * 256 {
                fs::remove_file(&log_file).unwrap();
            }
        }

        let level = log::LevelFilter::Info;

        // Build a stderr logger.
        let stderr = ConsoleAppender::builder().target(Target::Stderr).build();

        // Logging to log file.
        let logfile = FileAppender::builder()
            // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
            .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
            .build(log_file)
            .unwrap();

        let config = Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .appender(
                Appender::builder()
                    .filter(Box::new(ThresholdFilter::new(level)))
                    .build("stderr", Box::new(stderr)),
            )
            .build(Root::builder().appender("logfile").appender("stderr").build(LevelFilter::Info))
            .unwrap();

        // Use this to change log levels at runtime.
        // This means you can change the default log level to trace
        // if you are trying to debug an issue and need more logs on then turn it off
        // once you are done.
        let _handle = log4rs::init_config(config);
    } else {
        eprintln!("Failed to create log file");
    }

    log::info!("Starting iCY TERM {}", *VERSION);
    icy_net::websocket::init_websocket_providers();
    if let Err(err) = eframe::run_native(&DEFAULT_TITLE, native_options, Box::new(|cc| Ok(Box::new(MainWindow::new(cc, options))))) {
        log::error!("Error returned by run_native: {}", err);
    }
    log::info!("shutting down.");
}

lazy_static! {
    static ref START_TIME: Instant = Instant::now();
}
#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();
    let web_options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start("icy_term_canvas", web_options, Box::new(|cc| Box::new(MainWindow::new(cc))))
            .await
            .expect("failed to start eframe");
    });
}
/*
pub struct TemplateApp {
    // Example stuff:
    label: String,

    // this how you opt-out of serialization of a member
    value: f32,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
        }
    }
}
impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        Default::default()
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {}

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self { label, value } = self;

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
            });
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Side Panel");

            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(label);
            });

            ui.add(egui::Slider::new(value, 0.0..=10.0).text("value"));
            if ui.button("Increment").clicked() {
                *value += 1.0;
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("powered by ");
                    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                    ui.label(" and ");
                    ui.hyperlink_to(
                        "eframe",
                        "https://github.com/emilk/egui/tree/master/crates/eframe",
                    );
                    ui.label(".");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            ui.heading("eframe template");
            ui.hyperlink("https://github.com/emilk/eframe_template");
            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/master/",
                "Source code."
            ));
            egui::warn_if_debug_build(ui);
        });

        if false {
            egui::Window::new("Window").show(ctx, |ui| {
                ui.label("Windows can be moved by dragging them.");
                ui.label("They are automatically sized based on contents.");
                ui.label("You can turn on resizing and scrolling if you like.");
                ui.label("You would normally choose either panels OR windows.");
            });
        }
    }
}
*/

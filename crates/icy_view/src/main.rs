#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;

use clap::Parser;
use icy_view_gui::{options::Options, MainWindow};
use semver::Version;

lazy_static::lazy_static! {
    static ref VERSION: Version = Version::parse( env!("CARGO_PKG_VERSION")).unwrap();
    static ref DEFAULT_TITLE: String = format!("iCY VIEW {}", *crate::VERSION);
}

lazy_static::lazy_static! {
    static ref LATEST_VERSION: Version = {
        let github = github_release_check::GitHub::new().unwrap();
        if let Ok(latest) = github.get_latest_version("mkrueger/icy_view") {
            latest
        } else {
            VERSION.clone()
        }
    };
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    path: Option<PathBuf>,

    #[clap(long, default_value_t = false, help = "Enable auto-scrolling")]
    auto: bool,
}

fn main() {
    let args = Cli::parse();
    let mut options = Options::load_options();
    if args.auto {
        options.auto_scroll_enabled = true;
    }

    let native_options = eframe::NativeOptions {
        //initial_window_size: Some(egui::vec2(1284. + 8., 839.)),
        multisampling: 0,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    //  options.viewport.icon = Some(IconData::from( &include_bytes!("../build/linux/256x256.png")[..]).unwrap());
    eframe::run_native(
        &DEFAULT_TITLE,
        native_options,
        Box::new(|cc| {
            let gl = cc.gl.as_ref().expect("You need to run eframe with the glow backend");
            egui_extras::install_image_loaders(&cc.egui_ctx);

            let mut fd = MainWindow::new(gl, args.path, options);
            fd.store_options = true;
            if *VERSION < *LATEST_VERSION {
                fd.file_view.upgrade_version = Some(LATEST_VERSION.to_string());
            }
            let cmd = fd.file_view.refresh();
            fd.handle_command(cmd);
            Box::new(fd)
        }),
    )
    .unwrap();
}

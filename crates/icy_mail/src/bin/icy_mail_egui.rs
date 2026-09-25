use clap::Parser;
use eframe::{egui, egui_wgpu};
use icy_engine_gui::{egui::appearance, TerminalShaderRenderer};
use std::path::PathBuf;

#[path = "icy_mail_egui/app.rs"]
mod app;
#[path = "icy_mail_egui/chrome.rs"]
mod chrome;
#[path = "icy_mail_egui/composer.rs"]
mod composer;
#[path = "icy_mail_egui/dialogs.rs"]
mod dialogs;
#[path = "icy_mail_egui/list.rs"]
mod list;
#[path = "icy_mail_egui/loading.rs"]
mod loading;
#[path = "icy_mail_egui/reader_view.rs"]
mod reader_view;
#[path = "icy_mail_egui/sidebar.rs"]
mod sidebar;
#[path = "icy_mail_egui/terminal_editor.rs"]
mod terminal_editor;
#[path = "icy_mail_egui/welcome.rs"]
mod welcome;
#[path = "icy_mail_egui/widgets.rs"]
mod widgets;
#[cfg(test)]
use icy_mail::{qwk, threading};
#[cfg(test)]
#[path = "../qwk/tests.rs"]
mod packet_tests;
#[cfg(test)]
#[path = "icy_mail_egui/tests.rs"]
mod tests;

#[derive(Parser)]
#[command(version, about = "An offline QWK mail reader and reply-packet composer")]
struct Args {
    #[arg(value_name = "FILE", help = "Mail package to open (QWK/ZIP)")]
    file: Option<PathBuf>,
}

fn viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title(format!("Icy Mail {}", env!("CARGO_PKG_VERSION")))
        .with_inner_size([1100.0, 760.0])
        .with_min_inner_size([360.0, 240.0])
        .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../../build/linux/256x256.png")).expect("bundled mail icon"))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing_subscriber::filter::LevelFilter::WARN)
        .try_init();
    eframe::run_native(
        "Icy Mail",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: viewport(),
            wgpu_options: egui_wgpu::WgpuConfiguration {
                present_mode: egui_wgpu::wgpu::PresentMode::AutoNoVsync,
                ..Default::default()
            },
            ..Default::default()
        },
        Box::new(move |creation| {
            let render = creation.wgpu_render_state.as_ref().ok_or("wgpu renderer unavailable")?;
            render
                .renderer
                .write()
                .callback_resources
                .insert(TerminalShaderRenderer::new(&render.device, render.target_format));
            appearance::apply(&creation.egui_ctx);
            let mut mail = app::MailApp::new(&creation.egui_ctx);
            if let Some(path) = args.file {
                mail.open(path, &creation.egui_ctx);
            }
            Ok(Box::new(mail))
        }),
    )
    .map_err(|error| anyhow::anyhow!("{error}"))
}

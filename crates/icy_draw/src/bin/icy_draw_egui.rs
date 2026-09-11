use clap::Parser;
use eframe::{egui, egui_wgpu};
use icy_engine_gui::{egui::appearance, TerminalShaderRenderer};
use std::path::PathBuf;

#[path = "icy_draw_egui/animation.rs"]
mod animation;
#[path = "icy_draw_egui/app.rs"]
mod app;
#[path = "icy_draw_egui/export.rs"]
mod export;
#[path = "icy_draw_egui/font.rs"]
mod font;
#[path = "icy_draw_egui/widgets.rs"]
mod widgets;

#[derive(Parser)]
#[command(version, about = "ANSI and ASCII art editor")]
struct Args {
    file: Option<PathBuf>,
    #[arg(long)]
    mcp_port: Option<u16>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    Host(icy_draw::host::HostArgs),
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let _ = flexi_logger::Logger::try_with_str("warn").and_then(|logger| logger.start());
    if let Some(Command::Host(host)) = args.command {
        return host.run();
    }
    eframe::run_native(
        "Icy Draw",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1280.0, 820.0])
                .with_min_inner_size([440.0, 300.0])
                .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../../build/linux/256x256.png"))?),
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
            let mut editor = app::DrawApp::new();
            editor.persist_settings = true;
            if let Some(path) = args.file {
                editor.open(path);
            }
            if let Some(port) = args.mcp_port {
                editor.enable_mcp(port, creation.egui_ctx.clone());
            }
            Ok(Box::new(editor))
        }),
    )
    .map_err(|error| anyhow::anyhow!("{error}"))
}

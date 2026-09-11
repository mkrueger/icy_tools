use clap::Parser;
use clap_i18n_richformatter::clap_i18n;
use eframe::{egui, egui_wgpu};
use icy_engine_gui::{egui::appearance, ScalingMode, TerminalShaderRenderer};
use icy_view::Options;
use std::path::PathBuf;

#[path = "icy_view_egui/app.rs"]
mod app;
#[path = "icy_view_egui/browser.rs"]
mod browser;
#[path = "icy_view_egui/colors.rs"]
mod colors;
#[path = "icy_view_egui/dialogs.rs"]
mod dialogs;
#[path = "icy_view_egui/file_list.rs"]
mod file_list;
#[path = "icy_view_egui/icons.rs"]
mod icons;
#[path = "icy_view_egui/preview.rs"]
mod preview;
#[path = "icy_view_egui/shuffle.rs"]
mod shuffle;
#[cfg(test)]
#[path = "icy_view_egui/tests.rs"]
mod tests;
#[path = "icy_view_egui/thumbnails.rs"]
mod thumbnails;
#[path = "icy_view_egui/tile_grid.rs"]
mod tile_grid;
#[path = "icy_view_egui/tile_toolbar.rs"]
mod tile_toolbar;

fn text(key: &str) -> String {
    if icy_view::LANGUAGE_LOADER.has(key) {
        icy_view::LANGUAGE_LOADER.get(key)
    } else {
        icy_engine_gui::LANGUAGE_LOADER.get(key)
    }
}

fn link_at(terminal: &icy_engine_gui::Terminal, pointer: Option<egui::Pos2>) -> Option<String> {
    let pointer = pointer?;
    let info = terminal.render_info.read();
    let (column, row) = info.screen_to_cell(pointer.x, pointer.y)?;
    let position = icy_engine::Position::new(
        column + (terminal.scroll_x() / info.font_width.max(1.0)) as i32,
        row + (terminal.scroll_y() / info.font_height.max(1.0)) as i32,
    );
    let screen = terminal.screen.lock();
    screen
        .hyperlinks()
        .iter()
        .find(|link| screen.is_position_in_range(position, link.position, link.length))
        .map(|link| link.url(&**screen))
}

#[derive(Parser)]
#[command(version, about = text("app-about"))]
#[clap_i18n]
struct Args {
    #[arg(value_name = "PATH", help = text("arg-path-help"))]
    path: Option<PathBuf>,
    #[arg(long, help = text("arg-auto-help"))]
    auto: bool,
    #[arg(long, value_name = "RATE", help = text("arg-bps-help"))]
    bps: Option<u32>,
    #[arg(long, help = text("arg-portable-help"))]
    portable: bool,
    #[arg(long, value_name = "DIR", help = text("arg-config-dir-help"))]
    config_dir: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse_i18n_or_exit();
    icy_view::init_config_dir(args.portable, args.config_dir);
    let _logger = flexi_logger::Logger::try_with_env_or_str("info,wgpu_core=error,wgpu_hal=error,i18n_embed=error")?
        .log_to_file(
            flexi_logger::FileSpec::default()
                .directory(icy_view::get_config_dir())
                .basename("icy_view")
                .suppress_timestamp(),
        )
        .rotate(
            flexi_logger::Criterion::Size(64 * 1024),
            flexi_logger::Naming::Numbers,
            flexi_logger::Cleanup::KeepLogFiles(3),
        )
        .start()?;
    let mut options = Options::load_options();
    if args.auto {
        options.auto_scroll_enabled = true;
    }
    if options.monitor_settings.scaling_mode.is_auto() {
        options.monitor_settings.scaling_mode = ScalingMode::FitWidth;
    }
    let native = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: egui_wgpu::WgpuConfiguration {
            present_mode: egui_wgpu::wgpu::PresentMode::AutoNoVsync,
            ..Default::default()
        },
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([360.0, 240.0])
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../../build/linux/128x128.png"))?),
        ..Default::default()
    };
    eframe::run_native(
        "Icy View",
        native,
        Box::new(move |creation| {
            let render = creation.wgpu_render_state.as_ref().ok_or("wgpu renderer unavailable")?;
            render
                .renderer
                .write()
                .callback_resources
                .insert(TerminalShaderRenderer::new(&render.device, render.target_format));
            appearance::apply(&creation.egui_ctx);
            let mut viewer = app::Viewer::new(
                args.path.unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
                options,
                &creation.egui_ctx,
            )?;
            if let Some(rate) = args.bps {
                viewer.preview.set_baud(rate);
            }
            Ok(Box::new(viewer))
        }),
    )
    .map_err(|error| anyhow::anyhow!("{error}"))?;
    Options::cleanup_session_temp();
    Ok(())
}

use clap::Parser;
use eframe::{egui, egui_wgpu};
use icy_engine_gui::{egui::appearance, TerminalShaderRenderer};
use std::path::PathBuf;

const DEFAULT_LOG_FILTER: &str = "warn,wgpu_hal=error,wgpu_core=error";

#[path = "icy_draw_egui/animation.rs"]
mod animation;
#[path = "icy_draw_egui/animation_export.rs"]
mod animation_export;
#[path = "icy_draw_egui/app.rs"]
mod app;
#[path = "icy_draw_egui/attribute_picker.rs"]
mod attribute_picker;
#[path = "icy_draw_egui/command_list.rs"]
mod command_list;
#[path = "icy_draw_egui/export.rs"]
mod export;
#[path = "icy_draw_egui/font.rs"]
mod font;
#[path = "icy_draw_egui/igs.rs"]
mod igs;
#[path = "icy_draw_egui/input.rs"]
mod input;
#[path = "icy_draw_egui/log_bridge.rs"]
mod log_bridge;
#[cfg(target_os = "linux")]
#[path = "icy_draw_egui/native_drop.rs"]
mod native_drop;
#[path = "icy_draw_egui/palette.rs"]
mod palette;
#[path = "icy_draw_egui/playback.rs"]
mod playback;
#[path = "icy_draw_egui/rip.rs"]
mod rip;
#[path = "icy_draw_egui/skypix.rs"]
mod skypix;
#[path = "icy_draw_egui/widgets.rs"]
mod widgets;

#[derive(Parser)]
#[command(version, about = "ANSI and ASCII art editor")]
struct Args {
    file: Option<PathBuf>,
    #[arg(long)]
    mcp_port: Option<u16>,
    /// Use X11/XWayland, including native file drag-and-drop (Linux only).
    #[cfg(target_os = "linux")]
    #[arg(long)]
    x11: bool,
    /// Restores an autosaved document; used by the recovery dialog for additional windows.
    #[arg(long, hide = true, value_name = "ID")]
    recover: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    Host(icy_draw::host::HostArgs),
}

impl Args {
    fn initial_window_size(&self) -> [f32; 2] {
        if self.file.is_none() && self.recover.is_none() {
            [1000.0, 600.0]
        } else {
            [1280.0, 820.0]
        }
    }
}

#[cfg(target_os = "linux")]
fn x11_event_loop_hook(force_x11: bool) -> Option<eframe::EventLoopBuilderHook> {
    if force_x11 {
        Some(Box::new(|builder| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_x11();
        }))
    } else {
        None
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let initial_window_size = args.initial_window_size();
    let _logger = flexi_logger::Logger::try_with_env_or_str(DEFAULT_LOG_FILTER)?.start()?;
    log_bridge::LogBridge::install();
    if let Some(Command::Host(host)) = args.command {
        return host.run();
    }
    eframe::run_native(
        "Icy Draw",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            #[cfg(target_os = "linux")]
            event_loop_builder: x11_event_loop_hook(args.x11),
            viewport: egui::ViewportBuilder::default()
                .with_inner_size(initial_window_size)
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
            #[cfg(target_os = "linux")]
            editor.configure_native_drop(creation, args.x11);
            editor.persist_settings = true;
            editor.enable_recovery(icy_draw::Settings::recovery_dir());
            if let Some(id) = &args.recover {
                editor.restore_recovered(id);
            } else {
                match args.file {
                    Some(path) => editor.open(path),
                    None => editor.show_start = true,
                }
                editor.offer_recovery();
            }
            if let Some(port) = args.mcp_port {
                editor.enable_mcp(port, creation.egui_ctx.clone());
            }
            Ok(Box::new(editor))
        }),
    )
    .map_err(|error| anyhow::anyhow!("{error}"))
}

#[cfg(test)]
mod logging_tests {
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires a running X11 display; creates an event loop without a window"]
    fn x11_hook_selects_the_real_x11_backend() {
        use winit::platform::x11::{EventLoopBuilderExtX11, EventLoopExtX11};
        assert!(super::x11_event_loop_hook(false).is_none());
        let mut builder = winit::event_loop::EventLoop::with_user_event();
        builder.with_any_thread(true);
        super::x11_event_loop_hook(true).unwrap()(&mut builder);
        let event_loop = builder.build().unwrap();
        assert!(event_loop.is_x11());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn x11_is_explicit_and_works_with_document_and_recovery_arguments() {
        use clap::Parser;
        assert!(!super::Args::parse_from(["icy_draw"]).x11);
        for arguments in [
            vec!["icy_draw", "--x11"],
            vec!["icy_draw", "--x11", "drawing.ans"],
            vec!["icy_draw", "--x11", "--recover", "document-id"],
        ] {
            assert!(super::Args::parse_from(arguments).x11);
        }
    }

    #[test]
    fn start_window_is_compact_without_shrinking_document_windows() {
        use clap::Parser;

        for arguments in [vec!["icy_draw"], vec!["icy_draw", "--mcp-port", "9000"]] {
            assert_eq!(super::Args::parse_from(arguments).initial_window_size(), [1000.0, 600.0]);
        }
        for arguments in [vec!["icy_draw", "drawing.ans"], vec!["icy_draw", "--recover", "document-id"]] {
            assert_eq!(super::Args::parse_from(arguments).initial_window_size(), [1280.0, 820.0]);
        }
    }

    #[test]
    fn default_filter_keeps_errors_and_application_warnings() {
        let spec = flexi_logger::LogSpecification::parse(super::DEFAULT_LOG_FILTER).unwrap();
        for module in ["wgpu_hal::vulkan::instance", "wgpu_hal::gles::egl", "wgpu_hal::gles::adapter", "wgpu_core"] {
            assert!(!spec.enabled(log::Level::Warn, module));
            assert!(spec.enabled(log::Level::Error, module));
        }
        assert!(spec.enabled(log::Level::Warn, "icy_draw"));
        assert!(!spec.enabled(log::Level::Info, "icy_draw"));
    }
}

use egui::Vec2;
use icy_engine::{Buffer, TextPane};
use icy_engine_gui::{animations::Animator, BufferView, TerminalCalc};
use std::{
    path::{Path, PathBuf},
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
};

use super::{asciicast_encoder::AsciiCast, gif_encoder::GifEncoder /*, mp4_encoder::Mp4Encoder*/};
use crate::TerminalResult;

pub trait AnimationEncoder {
    fn label(&self) -> String;
    fn extension(&self) -> String;
    fn encode(&self, path: &Path, frames: Vec<(Vec<u8>, u32)>, width: usize, height: usize, sender: Sender<usize>) -> TerminalResult<()>;

    fn direct_encoding(&self, _path: &Path, _animator: Arc<std::sync::Mutex<Animator>>) -> TerminalResult<bool> {
        Ok(false)
    }
}
pub const ENCODERS: &[&dyn AnimationEncoder] = &[&GifEncoder {}, /*&Mp4Encoder {},*/ &AsciiCast {}];
type EncodingThread = (Receiver<usize>, JoinHandle<TerminalResult<()>>);

pub fn start_encoding_thread(
    encoder: usize,
    gl: Arc<glow::Context>,
    path: PathBuf,
    animator: Arc<std::sync::Mutex<Animator>>,
) -> TerminalResult<Option<EncodingThread>> {
    if !animator.lock().unwrap().success() {
        return Err(anyhow::anyhow!("Animation is not finished."));
    }
    if ENCODERS[encoder].direct_encoding(&path, animator.clone())? {
        return Ok(None);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let mut buffer = Buffer::new((80, 25));
    buffer.is_terminal_buffer = false;
    let mut buffer_view = BufferView::from_buffer(&gl, buffer);
    buffer_view.interactive = false;
    let buffer_view = Arc::new(eframe::epaint::mutex::Mutex::new(buffer_view));
    animator.lock().unwrap().set_cur_frame(0);
    animator.lock().unwrap().display_frame(buffer_view.clone());
    buffer_view.lock().calc = TerminalCalc::from_buffer(&animator.lock().unwrap().frames[0].0);

    let frame_count = animator.lock().unwrap().frames.len();

    let mut data = Vec::new();

    let mut opt = icy_engine_gui::TerminalOptions {
        stick_to_bottom: false,
        scale: Some(Vec2::new(1.0, 1.0)),
        ..Default::default()
    };

    for frame in 0..frame_count {
        animator.lock().unwrap().set_cur_frame(frame);
        opt.monitor_settings = animator.lock().unwrap().display_frame(buffer_view.clone());
        let (_, frame) = buffer_view.lock().render_buffer(&gl, &opt);
        data.push((frame, animator.lock().unwrap().get_delay()));
    }

    let size = buffer_view.lock().get_buffer().get_size();
    let dim = buffer_view.lock().get_buffer().get_font_dimensions();
    let width = (size.width * dim.width) as usize;
    let height = (size.height * dim.height) as usize;
    let t = thread::Builder::new()
        .name("Encoding".into())
        .spawn(move || ENCODERS[encoder].encode(&path, data, width, height, tx))?;

    Ok(Some((rx, t)))
}

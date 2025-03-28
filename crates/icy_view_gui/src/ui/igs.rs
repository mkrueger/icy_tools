use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicBool},
    thread,
    time::Duration,
};

use eframe::egui::ColorImage;
use icy_engine::{Buffer, BufferParser, CallbackAction, Caret};
/*
use rodio::{
    cpal::SampleRate,
    source::{Function, SignalGenerator},
    OutputStream, Source,
};*/

pub struct IGS {
    is_playing: bool,
    exit_requested: Arc<AtomicBool>,

    run_thread: Option<thread::JoinHandle<()>>,
    pub texture_handle: ColorImage,
}

fn make_texture(executor: &mut icy_engine::parsers::igs::Parser) -> ColorImage {
    let Some((size, pixels)) = executor.get_picture_data() else {
        return ColorImage::example();
    };
    ColorImage::from_rgba_premultiplied([size.width as usize, size.height as usize], &pixels)
}

impl IGS {
    pub fn stop(&mut self) {
        self.exit_requested.swap(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn run(_parent: &Option<PathBuf>, in_txt: String) -> Arc<Mutex<Self>> {
        let texture_handle = ColorImage::example();
        let exit_requested = Arc::new(AtomicBool::new(false));

        let igs = Arc::new(Mutex::new(IGS {
            run_thread: None,
            is_playing: false,
            texture_handle,
            exit_requested: exit_requested.clone(),
        }));
        let result = igs.clone();

        let run_thread = thread::spawn(move || {
            let mut buffer = Buffer::new((80, 24));
            let mut caret = Caret::default();
            let vec = in_txt.chars().collect::<Vec<_>>();
            let mut i = 0;
            let mut parser = icy_engine::parsers::igs::Parser::new(icy_engine::igs::TerminalResolution::Low);
            // let (_stream, stream_handle) = OutputStream::try_default().unwrap();
            // let sample_rate = SampleRate(48000);

            while i < vec.len() {
                if exit_requested.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                if !igs.lock().unwrap().is_playing {
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
                let c = vec[i];
                i += 1;
                match parser.print_char(&mut buffer, 0, &mut caret, c) {
                    Ok(act) => run_action(&igs, &mut parser, act),
                    Err(err) => {
                        eprintln!("IGS Error: {:?}", err);
                    }
                }
                while let Some(act) = parser.get_next_action(&mut buffer, &mut caret, 0) {
                    run_action(&igs, &mut parser, act);
                }
            }
        });
        result.lock().unwrap().run_thread = Some(run_thread);
        result.lock().unwrap().is_playing = true;
        result
    }
}

fn run_action(igs: &Arc<Mutex<IGS>>, parser: &mut icy_engine::parsers::igs::Parser, act: CallbackAction) {
    match act {
        CallbackAction::Update => {
            let texture_handle = make_texture(parser);
            igs.lock().unwrap().texture_handle = texture_handle;
        }
        CallbackAction::Pause(ms) => {
            thread::sleep(Duration::from_millis(ms as u64));
        }

        CallbackAction::PlayGISTSound(_data) => {

            // TODO: Implement sound
            // May be helpful https://github.com/th-otto/gist/blob/master/src/sndsubs.c

            /*
            let dur = 5;
            for f in effect {
                if f == 0 {
                    thread::sleep(Duration::from_millis(dur));
                    continue;
                }
                let f = f as u16;
                let f = (f as f32) / 50.0;

                if let Err(err) = stream_handle.play_raw(
                    SignalGenerator::new(sample_rate, f, Function::Square)
                        .amplify(0.07)
                        .take_duration(std::time::Duration::from_millis(dur)),
                ) {
                    log::error!("Error in playing note: {}", err);
                    break;
                }
            }*/
        }

        _ => {}
    }
}

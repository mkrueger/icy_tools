use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread,
    time::Duration,
};

use eframe::egui::ColorImage;
use icy_engine::{
    igs::{CommandExecutor, DrawExecutor},
    Buffer, BufferParser, CallbackAction, Caret,
};

pub struct IGS {
    is_playing: bool,
    exit_requested: Arc<AtomicBool>,

    run_thread: Option<thread::JoinHandle<()>>,
    executor: Arc<Mutex<dyn CommandExecutor>>,
    pub texture_handle: ColorImage,
}

fn make_texture(executor: &Arc<Mutex<dyn CommandExecutor>>) -> ColorImage {
    let Some((size, pixels)) = executor.lock().unwrap().get_picture_data() else {
        return ColorImage::example();
    };
    ColorImage::from_rgba_premultiplied([size.width as usize, size.height as usize], &pixels)
}

impl IGS {
    pub fn stop(&mut self) {
        self.exit_requested.swap(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn run(_parent: &Option<PathBuf>, in_txt: String) -> Arc<Mutex<Self>> {
        let executor: Arc<Mutex<dyn CommandExecutor>> = Arc::new(Mutex::new(DrawExecutor::default()));
        let texture_handle = make_texture(&executor);
        let exit_requested = Arc::new(AtomicBool::new(false));

        let igs = Arc::new(Mutex::new(IGS {
            executor: executor.clone(),
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
            let mut parser = icy_engine::parsers::igs::Parser::new(executor);

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
                if let Ok(act) = parser.print_char(&mut buffer, 0, &mut caret, c) {
                    match act {
                        CallbackAction::Update => {
                            let texture_handle = make_texture(&igs.lock().unwrap().executor);
                            igs.lock().unwrap().texture_handle = texture_handle;
                        }
                        CallbackAction::Pause(ms) => {
                            println!("pause {}", ms);
                            thread::sleep(Duration::from_millis(ms as u64));
                        }

                        _ => {}
                    }
                }
            }
        });
        result.lock().unwrap().run_thread = Some(run_thread);
        result.lock().unwrap().is_playing = true;
        result
    }
}

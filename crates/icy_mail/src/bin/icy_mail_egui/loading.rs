use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
};

use eframe::egui;
use icy_engine::TextScreen;
use icy_mail::{qwk::QwkPackage, reader::render_body};

pub enum Event {
    Package(u64, PathBuf, Result<Arc<QwkPackage>, String>),
    Body(u64, Result<TextScreen, String>),
    Picked(Option<PathBuf>),
}

type Jobs<T> = mpsc::Sender<(T, egui::Context)>;

pub struct Loader {
    pub package_generation: u64,
    pub body_generation: u64,
    pub picking: bool,
    pub sender: mpsc::Sender<Event>,
    pub receiver: mpsc::Receiver<Event>,
    packages: Jobs<(u64, PathBuf)>,
    bodies: Jobs<(u64, Arc<QwkPackage>, usize)>,
}

impl Default for Loader {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        let packages = latest_worker(sender.clone(), |(generation, path): (u64, PathBuf)| {
            let result = QwkPackage::load_from_file(&path).map(Arc::new).map_err(|error| error.to_string());
            Event::Package(generation, path, result)
        });
        let bodies = latest_worker(sender.clone(), |(generation, package, index): (u64, Arc<QwkPackage>, usize)| {
            let result = package
                .get_message(index)
                .and_then(|message| render_body(&message.text))
                .map_err(|error| error.to_string());
            Event::Body(generation, result)
        });
        Self {
            package_generation: 0,
            body_generation: 0,
            picking: false,
            sender,
            receiver,
            packages,
            bodies,
        }
    }
}

impl Loader {
    pub fn package(&mut self, path: PathBuf, context: &egui::Context) {
        self.package_generation = self.package_generation.wrapping_add(1);
        self.body_generation = self.body_generation.wrapping_add(1);
        let _ = self.packages.send(((self.package_generation, path), context.clone()));
    }

    pub fn body(&mut self, package: Arc<QwkPackage>, index: usize, context: &egui::Context) {
        self.body_generation = self.body_generation.wrapping_add(1);
        let _ = self.bodies.send(((self.body_generation, package, index), context.clone()));
    }

    pub fn pick(&mut self, context: &egui::Context) {
        if self.picking {
            return;
        }
        self.picking = true;
        let sender = self.sender.clone();
        let context = context.clone();
        std::thread::spawn(move || {
            let path = rfd::FileDialog::new()
                .set_title("Open Mail Package")
                .add_filter("Mail Packages", &["qwk", "zip", "rep"])
                .add_filter("All Files", &["*"])
                .pick_file();
            let _ = sender.send(Event::Picked(path));
            context.request_repaint();
        });
    }
}

fn latest_worker<T: Send + 'static>(events: mpsc::Sender<Event>, load: impl Fn(T) -> Event + Send + 'static) -> Jobs<T> {
    let (sender, receiver) = mpsc::channel::<(T, egui::Context)>();
    std::thread::spawn(move || {
        while let Ok(mut job) = receiver.recv() {
            for newer in receiver.try_iter() {
                job = newer;
            }
            let (request, context) = job;
            if events.send(load(request)).is_err() {
                break;
            }
            context.request_repaint();
        }
    });
    sender
}

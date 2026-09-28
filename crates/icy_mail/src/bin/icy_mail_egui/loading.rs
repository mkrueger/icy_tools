use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
};

use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine::TextScreen;
use icy_mail::{
    drafts::DraftStore,
    qwk::QwkPackage,
    reader::{render_body, render_file_page},
    LANGUAGE_LOADER,
};
use rayon::prelude::*;

pub enum Event {
    Package(u64, PathBuf, Result<Arc<QwkPackage>, String>),
    Body(u64, Result<TextScreen, String>),
    Search(u64, String, Result<HashSet<usize>, String>),
    Picked(Option<PathBuf>),
    Exported(Option<(PathBuf, Result<(), String>)>),
}

type Jobs<T> = mpsc::Sender<(T, egui::Context)>;

/// What the content pane shows: a message body or one of the packet's bulletin files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodySource {
    Message(usize),
    File(usize, usize),
}

enum SearchJob {
    Scan(u64, Arc<QwkPackage>, String),
    Clear,
}

#[derive(Debug)]
enum SearchError {
    Cancelled,
    Message(String),
}

struct BodySearch {
    package: Arc<QwkPackage>,
    text: Vec<Option<String>>,
}

impl BodySearch {
    fn new(package: Arc<QwkPackage>) -> Self {
        Self {
            text: vec![None; package.infos.len()],
            package,
        }
    }

    fn find(&mut self, query: &str, generation: u64, current: &AtomicU64) -> Result<HashSet<usize>, SearchError> {
        let package = &self.package;
        self.text
            .par_iter_mut()
            .with_min_len(256)
            .zip(&package.infos)
            .map(|(cached, info)| {
                if current.load(Ordering::Relaxed) != generation {
                    return Err(SearchError::Cancelled);
                }
                if cached.is_none() {
                    let message = package
                        .read_message(info.index)
                        .map_err(|error| SearchError::Message(fl!(LANGUAGE_LOADER, "search-message-error", number = info.number, error = error.to_string())))?;
                    *cached = Some(icy_mail::reader::body_search_text(&message.text));
                }
                Ok(cached.as_ref().is_some_and(|text| text.contains(query)).then_some(info.index))
            })
            .filter_map(Result::transpose)
            .collect()
    }
}

pub struct Loader {
    pub package_generation: u64,
    pub body_generation: u64,
    pub picking: bool,
    pub export_picking: bool,
    pub sender: mpsc::Sender<Event>,
    pub receiver: mpsc::Receiver<Event>,
    packages: Jobs<(u64, PathBuf)>,
    bodies: Jobs<(u64, Arc<QwkPackage>, BodySource)>,
    searches: Jobs<SearchJob>,
    search_generation: Arc<AtomicU64>,
    pub search_query: Option<String>,
    pub searching: bool,
}

impl Default for Loader {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        let packages = latest_worker(sender.clone(), |(generation, path): (u64, PathBuf)| {
            let result = QwkPackage::load_from_file(&path).map(Arc::new).map_err(|error| error.to_string());
            Some(Event::Package(generation, path, result))
        });
        let bodies = latest_worker(sender.clone(), |(generation, package, source): (u64, Arc<QwkPackage>, BodySource)| {
            let result = match source {
                BodySource::Message(index) => package.get_message(index).and_then(|message| render_body(&message.text)),
                BodySource::File(index, page) => package
                    .files
                    .get(index)
                    .ok_or_else(|| format!("Packet file {index} does not exist").into())
                    .and_then(|file| render_file_page(&file.data, page)),
            }
            .map_err(|error| error.to_string());
            Some(Event::Body(generation, result))
        });
        let search_generation = Arc::new(AtomicU64::new(0));
        let current = search_generation.clone();
        let mut index: Option<BodySearch> = None;
        let searches = latest_worker(sender.clone(), move |job| {
            let SearchJob::Scan(generation, package, query) = job else {
                index = None;
                return None;
            };
            if current.load(Ordering::Relaxed) != generation {
                return None;
            }
            let index = match &mut index {
                Some(index) if Arc::ptr_eq(&index.package, &package) => index,
                slot => slot.insert(BodySearch::new(package)),
            };
            let result = match index.find(&query, generation, &current) {
                Ok(matches) => Ok(matches),
                Err(SearchError::Cancelled) => return None,
                Err(SearchError::Message(error)) => Err(error),
            };
            Some(Event::Search(generation, query, result))
        });
        Self {
            package_generation: 0,
            body_generation: 0,
            picking: false,
            export_picking: false,
            sender,
            receiver,
            packages,
            bodies,
            searches,
            search_generation,
            search_query: None,
            searching: false,
        }
    }
}

impl Loader {
    pub fn package(&mut self, path: PathBuf, context: &egui::Context) {
        self.cancel_search();
        if let Err(error) = self.searches.send((SearchJob::Clear, context.clone())) {
            log::error!("Could not clear the message search cache: {error}");
        }
        self.package_generation = self.package_generation.wrapping_add(1);
        self.body_generation = self.body_generation.wrapping_add(1);
        let _ = self.packages.send(((self.package_generation, path), context.clone()));
    }

    pub fn body(&mut self, package: Arc<QwkPackage>, source: BodySource, context: &egui::Context) {
        self.body_generation = self.body_generation.wrapping_add(1);
        let _ = self.bodies.send(((self.body_generation, package, source), context.clone()));
    }

    pub fn search_generation(&self) -> u64 {
        self.search_generation.load(Ordering::Relaxed)
    }

    pub fn cancel_search(&mut self) {
        self.search_generation.fetch_add(1, Ordering::Relaxed);
        self.search_query = None;
        self.searching = false;
    }

    pub fn search(&mut self, package: Arc<QwkPackage>, query: String, context: &egui::Context) -> Result<(), String> {
        if self.search_query.as_ref() == Some(&query) {
            return Ok(());
        }
        self.cancel_search();
        self.search_query = Some(query.clone());
        if query.is_empty() {
            return Ok(());
        }
        self.searches
            .send((SearchJob::Scan(self.search_generation(), package, query), context.clone()))
            .map_err(|error| error.to_string())?;
        self.searching = true;
        context.request_repaint();
        Ok(())
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
                .set_title(fl!(LANGUAGE_LOADER, "loading-open-title"))
                .add_filter(
                    fl!(LANGUAGE_LOADER, "loading-filter-packages"),
                    &["qwk", "zip", "arj", "lzh", "lha", "rar", "7z", "arc", "zoo", "rep"],
                )
                .add_filter(fl!(LANGUAGE_LOADER, "loading-filter-all"), &["*"])
                .pick_file();
            let _ = sender.send(Event::Picked(path));
            context.request_repaint();
        });
    }

    pub fn pick_export(&mut self, suggested: PathBuf, drafts: DraftStore, context: &egui::Context) {
        if self.export_picking {
            return;
        }
        self.export_picking = true;
        let sender = self.sender.clone();
        let context = context.clone();
        std::thread::spawn(move || {
            let mut dialog = rfd::FileDialog::new()
                .set_title(fl!(LANGUAGE_LOADER, "loading-export-title"))
                .add_filter(fl!(LANGUAGE_LOADER, "loading-filter-reply"), &["rep"])
                .set_file_name(suggested.file_name().unwrap_or_default().to_string_lossy());
            if let Some(directory) = suggested.parent() {
                dialog = dialog.set_directory(directory);
            }
            let path = dialog.save_file();
            let result = path.map(|path| {
                let result = drafts.export_rep(&path).map_err(|error| error.to_string());
                (path, result)
            });
            let _ = sender.send(Event::Exported(result));
            context.request_repaint();
        });
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.cancel_search();
    }
}

fn latest_worker<T: Send + 'static>(events: mpsc::Sender<Event>, mut load: impl FnMut(T) -> Option<Event> + Send + 'static) -> Jobs<T> {
    let (sender, receiver) = mpsc::channel::<(T, egui::Context)>();
    std::thread::spawn(move || {
        while let Ok(mut job) = receiver.recv() {
            for newer in receiver.try_iter() {
                job = newer;
            }
            let (request, context) = job;
            if let Some(event) = load(request) {
                if events.send(event).is_err() {
                    break;
                }
                context.request_repaint();
            }
        }
    });
    sender
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_search_reuses_text_and_resumes_after_cancellation() {
        let (_dir, package) = crate::packet_tests::load();
        let mut index = BodySearch::new(Arc::new(package));
        let generation = AtomicU64::new(1);
        assert!(matches!(index.find("line", 0, &generation), Err(SearchError::Cancelled)));
        assert!(index.text.iter().all(Option::is_none));
        assert_eq!(index.find("line 4", 1, &generation).unwrap(), HashSet::from([2]));
        assert!(index.text.iter().all(Option::is_some));
        let allocations: Vec<_> = index.text.iter().map(|text| text.as_ref().unwrap().as_ptr()).collect();
        generation.store(2, Ordering::Relaxed);
        assert!(matches!(index.find("line", 1, &generation), Err(SearchError::Cancelled)));
        assert_eq!(index.find("line 1", 2, &generation).unwrap(), HashSet::from([0, 2, 3]));
        assert_eq!(
            index.text.iter().map(|text| text.as_ref().unwrap().as_ptr()).collect::<Vec<_>>(),
            allocations,
            "changing the query must not decode or allocate message text again"
        );
    }

    #[test]
    fn opening_a_packet_releases_the_previous_search_cache() {
        let (dir, package) = crate::packet_tests::load();
        let package = Arc::new(package);
        let previous = Arc::downgrade(&package);
        let context = egui::Context::default();
        let mut loader = Loader::default();
        loader.search(package, "line 4".into(), &context).unwrap();
        assert!(matches!(
            loader.receiver.recv_timeout(std::time::Duration::from_secs(5)).unwrap(),
            Event::Search(_, _, Ok(_))
        ));
        assert!(previous.upgrade().is_some(), "the current packet's text cache should be retained");
        loader.package(dir.path().join("TEST.QWK"), &context);
        assert!(matches!(
            loader.receiver.recv_timeout(std::time::Duration::from_secs(5)).unwrap(),
            Event::Package(_, _, Ok(_))
        ));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while previous.upgrade().is_some() {
            assert!(std::time::Instant::now() < deadline, "old packet search cache was not released");
            std::thread::yield_now();
        }
    }

    #[test]
    fn parallel_body_search_reports_read_errors() {
        let (_dir, mut package) = crate::packet_tests::load();
        let base = package.infos[0].clone();
        let descriptor = package.descriptors[0].clone();
        for index in package.infos.len()..4096 {
            let mut info = base.clone();
            info.index = index;
            package.infos.push(info);
            package.descriptors.push(descriptor.clone());
        }
        package.descriptors.last_mut().unwrap().offset = u64::MAX;
        let mut index = BodySearch::new(Arc::new(package));
        assert!(matches!(index.find("line", 0, &AtomicU64::new(0)), Err(SearchError::Message(_))));
    }
}

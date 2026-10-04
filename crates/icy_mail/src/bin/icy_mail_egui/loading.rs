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
    drafts::{Draft, DraftStore},
    options::ReadingMode,
    qwk::{ExtractionCache, QwkPackage},
    reader::{render_body, render_body_wide, render_file_page, render_file_page_wide},
    LANGUAGE_LOADER,
};
use rayon::prelude::*;

use super::modern_view::{self, PreparedItem};

pub enum Event {
    Package(u64, PathBuf, Result<Arc<QwkPackage>, String>),
    Body(BodyRequest, Result<PreparedBody, String>),
    Preloaded(u64, u64, Result<bool, String>),
    Search(u64, String, Result<HashSet<usize>, String>),
    Picked(Option<PathBuf>),
    Exported(Option<(PathBuf, Result<(), String>)>),
    MessageSaved(Option<(PathBuf, Result<(), String>)>),
    MessagesSaved(usize, Option<(PathBuf, Result<(), String>)>),
    RepliesPicked(u64, Option<(PathBuf, Result<Vec<Draft>, String>)>),
}

type Jobs<T> = mpsc::Sender<(T, egui::Context)>;

/// What the content pane shows: a message body or one of the packet's bulletin files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodySource {
    Message(usize),
    File(usize, usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyRequest {
    pub generation: u64,
    pub source: BodySource,
    pub mode: ReadingMode,
}

pub struct PreparedBody {
    pub classic: TextScreen,
    pub modern: Option<Vec<PreparedItem>>,
}

fn prepare_body(package: &QwkPackage, source: BodySource, mode: ReadingMode) -> icy_mail::Res<PreparedBody> {
    let (classic, wide) = match source {
        BodySource::Message(index) => {
            let message = package.get_message(index)?;
            let classic = render_body(&message.text)?;
            let wide = (mode == ReadingMode::Modern).then(|| render_body_wide(&message.text)).transpose()?;
            (classic, wide)
        }
        BodySource::File(index, page) => {
            let file = package.files.get(index).ok_or_else(|| format!("Packet file {index} does not exist"))?;
            let classic = render_file_page(&file.data, page)?;
            let wide = (mode == ReadingMode::Modern).then(|| render_file_page_wide(&file.data, page)).transpose()?;
            (classic, wide)
        }
    };
    let modern = wide.map(|wide| modern_view::prepare_items(&classic, modern_view::blocks(&classic, &wide)));
    Ok(PreparedBody { classic, modern })
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
    pub import_picking: bool,
    pub save_picking: bool,
    pub sender: mpsc::Sender<Event>,
    pub receiver: mpsc::Receiver<Event>,
    packages: Jobs<(u64, PathBuf, Option<PathBuf>, u32)>,
    bodies: Jobs<(BodyRequest, Arc<QwkPackage>)>,
    pub body_request: Option<BodyRequest>,
    preloads: Jobs<(u64, u64, Arc<QwkPackage>)>,
    preload_generation: Arc<AtomicU64>,
    searches: Jobs<SearchJob>,
    search_generation: Arc<AtomicU64>,
    pub search_query: Option<String>,
    pub searching: bool,
}

impl Default for Loader {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        let packages = latest_worker(sender.clone(), |(generation, path, directory, days): (u64, PathBuf, Option<PathBuf>, u32)| {
            let directory = directory.map(Ok).unwrap_or_else(ExtractionCache::default_directory);
            let result = match directory {
                Ok(directory) => QwkPackage::load_from_file_cached(&path, &ExtractionCache::new(directory, days)),
                Err(error) => {
                    log::warn!("could not locate the packet extraction cache, loading uncached: {error}");
                    QwkPackage::load_from_file(&path)
                }
            }
            .map(Arc::new)
            .map_err(|error| error.to_string());
            Some(Event::Package(generation, path, result))
        });
        let bodies = latest_worker(sender.clone(), |(request, package): (BodyRequest, Arc<QwkPackage>)| {
            let result = prepare_body(&package, request.source, request.mode).map_err(|error| error.to_string());
            Some(Event::Body(request, result))
        });
        let preload_generation = Arc::new(AtomicU64::new(0));
        let current = preload_generation.clone();
        let preloads = latest_worker(sender.clone(), move |(package_generation, generation, package): (u64, u64, Arc<QwkPackage>)| {
            let result = package
                .preload_messages(|| current.load(Ordering::Relaxed) != generation)
                .map_err(|error| error.to_string());
            if let Err(error) = &result {
                log::warn!("could not preload packet message data: {error}");
            }
            Some(Event::Preloaded(package_generation, generation, result))
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
                Err(SearchError::Message(error)) => {
                    log::warn!("message body search failed: {error}");
                    Err(error)
                }
            };
            Some(Event::Search(generation, query, result))
        });
        Self {
            package_generation: 0,
            body_generation: 0,
            picking: false,
            export_picking: false,
            import_picking: false,
            save_picking: false,
            sender,
            receiver,
            packages,
            bodies,
            body_request: None,
            preloads,
            preload_generation,
            searches,
            search_generation,
            search_query: None,
            searching: false,
        }
    }
}

impl Loader {
    pub fn package(&mut self, path: PathBuf, cache_directory: Option<PathBuf>, retention_days: u32, context: &egui::Context) {
        self.cancel_preload();
        self.body_request = None;
        self.cancel_search();
        if let Err(error) = self.searches.send((SearchJob::Clear, context.clone())) {
            log::error!("Could not clear the message search cache: {error}");
        }
        self.package_generation = self.package_generation.wrapping_add(1);
        self.body_generation = self.body_generation.wrapping_add(1);
        let _ = self
            .packages
            .send(((self.package_generation, path, cache_directory, retention_days), context.clone()));
    }

    pub fn body(&mut self, package: Arc<QwkPackage>, source: BodySource, mode: ReadingMode, context: &egui::Context) {
        self.cancel_preload();
        self.body_generation = self.body_generation.wrapping_add(1);
        let request = BodyRequest {
            generation: self.body_generation,
            source,
            mode,
        };
        self.body_request = Some(request);
        if let Err(error) = self.bodies.send(((request, package), context.clone())) {
            log::error!("could not queue the message body: {error}");
            let _ = self.sender.send(Event::Body(request, Err(error.to_string())));
            context.request_repaint();
        }
    }

    pub fn preload_generation(&self) -> u64 {
        self.preload_generation.load(Ordering::Relaxed)
    }

    pub fn cancel_preload(&mut self) {
        self.preload_generation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn preload(&mut self, package: Arc<QwkPackage>, context: &egui::Context) {
        if !package.needs_preload() {
            return;
        }
        let generation = self.preload_generation();
        if let Err(error) = self.preloads.send(((self.package_generation, generation, package), context.clone())) {
            log::error!("could not queue packet message preloading: {error}");
            let _ = self.sender.send(Event::Preloaded(self.package_generation, generation, Err(error.to_string())));
            context.request_repaint();
        }
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

    /// Asks where to save a message's text and writes `data` there.
    pub fn pick_save_message(&mut self, name: String, data: Vec<u8>, context: &egui::Context) {
        if self.save_picking {
            return;
        }
        self.save_picking = true;
        let sender = self.sender.clone();
        let context = context.clone();
        std::thread::spawn(move || {
            let extension = std::path::Path::new(&name)
                .extension()
                .map(|extension| extension.to_string_lossy().to_string())
                .unwrap_or_default();
            let path = rfd::FileDialog::new()
                .set_title(fl!(LANGUAGE_LOADER, "loading-save-message-title"))
                .add_filter(fl!(LANGUAGE_LOADER, "loading-filter-message"), &[extension.as_str()])
                .add_filter(fl!(LANGUAGE_LOADER, "loading-filter-all"), &["*"])
                .set_file_name(name)
                .save_file();
            let result = path.map(|path| {
                let result = std::fs::write(&path, data).map_err(|error| error.to_string());
                (path, result)
            });
            let _ = sender.send(Event::MessageSaved(result));
            context.request_repaint();
        });
    }

    pub fn pick_import(&mut self, drafts: DraftStore, context: &egui::Context) {
        if self.import_picking {
            return;
        }
        self.import_picking = true;
        let generation = self.package_generation;
        let sender = self.sender.clone();
        let context = context.clone();
        std::thread::spawn(move || {
            let path = rfd::FileDialog::new()
                .set_title(fl!(LANGUAGE_LOADER, "loading-import-title"))
                .add_filter(fl!(LANGUAGE_LOADER, "loading-filter-reply"), &["rep"])
                .add_filter(fl!(LANGUAGE_LOADER, "loading-filter-all"), &["*"])
                .pick_file();
            let result = path.map(|path| {
                let result = drafts.read_rep(&path).map_err(|error| error.to_string());
                (path, result)
            });
            let _ = sender.send(Event::RepliesPicked(generation, result));
            context.request_repaint();
        });
    }

    pub fn pick_save_messages(&mut self, package: Arc<QwkPackage>, indices: Vec<usize>, source: PathBuf, context: &egui::Context) {
        if self.save_picking {
            return;
        }
        self.save_picking = true;
        let sender = self.sender.clone();
        let context = context.clone();
        std::thread::spawn(move || {
            let path = rfd::FileDialog::new()
                .set_title(fl!(LANGUAGE_LOADER, "loading-save-messages-title"))
                .add_filter(fl!(LANGUAGE_LOADER, "loading-filter-message"), &["txt"])
                .set_file_name("messages.txt")
                .save_file();
            let result = path.map(|path| {
                let result = icy_mail::transcript::save_transcript(&package, &indices, &source, &path).map_err(|error| error.to_string());
                (path, result)
            });
            let _ = sender.send(Event::MessagesSaved(indices.len(), result));
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
        self.cancel_preload();
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
    fn body_worker_prepares_modern_message_and_bulletin_content() {
        let (_dir, mut package) = crate::packet_tests::load();
        let mut data = format!("{}\r\n\r\n\x1b[31m", "long wrapped text ".repeat(10)).into_bytes();
        data.extend([219; 20]);
        data.extend(b"\x1b[0m");
        Arc::make_mut(&mut package.files)[0].data = data;
        let package = Arc::new(package);
        let context = egui::Context::default();
        let mut loader = Loader::default();
        for source in [BodySource::Message(2), BodySource::File(0, 0)] {
            for mode in [ReadingMode::Classic, ReadingMode::Modern] {
                let previous_preload = loader.preload_generation();
                loader.body(package.clone(), source, mode, &context);
                assert_ne!(
                    loader.preload_generation(),
                    previous_preload,
                    "foreground body work preempts raw preloading before it is queued"
                );
                let Event::Body(request, Ok(prepared)) = loader.receiver.recv_timeout(std::time::Duration::from_secs(10)).unwrap() else {
                    panic!("body job did not prepare successfully");
                };
                assert_eq!(Some(request), loader.body_request);
                assert_eq!(request.source, source);
                assert_eq!(request.mode, mode);
                assert_eq!(prepared.modern.is_some(), mode == ReadingMode::Modern);
                if let Some(items) = prepared.modern {
                    let wide = match source {
                        BodySource::Message(index) => render_body_wide(&package.get_message(index).unwrap().text).unwrap(),
                        BodySource::File(index, page) => render_file_page_wide(&package.files[index].data, page).unwrap(),
                    };
                    let expected = modern_view::prepare_items(&prepared.classic, modern_view::blocks(&prepared.classic, &wide));
                    assert_eq!(items.len(), expected.len());
                    for (actual, expected) in items.iter().zip(&expected) {
                        match (actual, expected) {
                            (PreparedItem::Text(actual), PreparedItem::Text(expected)) => assert_eq!(format!("{actual:?}"), format!("{expected:?}")),
                            (PreparedItem::Art(actual), PreparedItem::Art(expected)) => {
                                assert_eq!(actual.size, expected.size);
                                assert_eq!(actual.pixels, expected.pixels);
                            }
                            _ => panic!("background block analysis differs from the direct rendering"),
                        }
                    }
                    if matches!(source, BodySource::File(..)) {
                        assert!(items.iter().any(|item| matches!(item, PreparedItem::Art(_))));
                        assert!(
                            items.iter().any(|item| {
                                matches!(item, PreparedItem::Text(line) if line.spans.iter().map(|span| span.text.chars().count()).sum::<usize>() > 80)
                            }),
                            "wide preparation rejoins terminal-wrapped prose"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn preload_jobs_cancel_resume_and_leave_body_search_correct() {
        let (dir, _) = crate::packet_tests::load();
        let path = dir.path().join("TEST.QWK");
        let cache = ExtractionCache::new(dir.path().join("packet-extractions"), 30);
        QwkPackage::load_from_file_cached(&path, &cache).unwrap();
        let package = Arc::new(QwkPackage::load_from_file_cached(&path, &cache).unwrap());
        assert!(package.needs_preload());
        let context = egui::Context::default();
        let mut loader = Loader::default();
        let previous = loader.preload_generation();
        loader.cancel_preload();
        loader
            .preloads
            .send(((loader.package_generation, previous, package.clone()), context.clone()))
            .unwrap();
        assert!(matches!(
            loader.receiver.recv_timeout(std::time::Duration::from_secs(10)).unwrap(),
            Event::Preloaded(_, generation, Ok(false)) if generation == previous
        ));
        assert!(package.needs_preload(), "a cancelled job must not load the entire raw body store");
        loader.preload(package.clone(), &context);
        loader.search(package.clone(), "line 4".into(), &context).unwrap();
        let mut preloaded = false;
        let mut searched = false;
        for _ in 0..2 {
            match loader.receiver.recv_timeout(std::time::Duration::from_secs(10)).unwrap() {
                Event::Preloaded(_, generation, Ok(true)) => {
                    assert_eq!(generation, loader.preload_generation());
                    preloaded = true;
                }
                Event::Search(_, query, Ok(matches)) => {
                    assert_eq!(query, "line 4");
                    assert_eq!(matches, HashSet::from([2]));
                    searched = true;
                }
                _ => panic!("preload or body search failed"),
            }
        }
        assert!(preloaded && searched);
        assert!(!package.needs_preload());
        let generation = loader.preload_generation.clone();
        let previous = generation.load(Ordering::Relaxed);
        drop(loader);
        assert_ne!(generation.load(Ordering::Relaxed), previous, "window drop cancels outstanding preload work");
    }

    #[test]
    fn package_jobs_use_the_requested_cache_directory_and_retention() {
        let (dir, _) = crate::packet_tests::load();
        let path = dir.path().join("TEST.QWK");
        let cache = dir.path().join("packet-extractions");
        let context = egui::Context::default();
        let mut loader = Loader::default();
        for days in [30, 0] {
            loader.package(path.clone(), Some(cache.clone()), days, &context);
            match loader.receiver.recv_timeout(std::time::Duration::from_secs(10)).unwrap() {
                Event::Package(generation, loaded_path, Ok(package)) => {
                    assert_eq!(generation, loader.package_generation);
                    assert_eq!(loaded_path, path);
                    assert!(!package.infos.is_empty());
                }
                _ => panic!("packet job did not load successfully"),
            }
            let entries = std::fs::read_dir(&cache)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_dir())
                .count();
            if days == 0 {
                assert_eq!(entries, 0, "disabling the cache applies to the next job and removes existing extractions");
            } else {
                assert!(entries > 0, "the job must populate the configured cache rather than the user's cache");
            }
        }
    }

    #[test]
    fn package_job_still_opens_when_cache_storage_is_unavailable() {
        let (dir, _) = crate::packet_tests::load();
        let cache = dir.path().join("not-a-directory");
        std::fs::write(&cache, b"blocked cache path").unwrap();
        let mut loader = Loader::default();
        loader.package(dir.path().join("TEST.QWK"), Some(cache), 30, &egui::Context::default());
        assert!(matches!(
            loader.receiver.recv_timeout(std::time::Duration::from_secs(10)).unwrap(),
            Event::Package(_, _, Ok(_))
        ));
    }

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
        loader.package(dir.path().join("TEST.QWK"), Some(dir.path().join("packet-extractions")), 30, &context);
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

    #[test]
    fn file_backed_search_reports_unrecoverable_read_errors_instead_of_partial_matches() {
        for delete_source in [false, true] {
            let (dir, _) = crate::packet_tests::load();
            let path = dir.path().join("TEST.QWK");
            let cache_directory = dir.path().join("packet-extractions");
            let cache = ExtractionCache::new(cache_directory.clone(), 30);
            QwkPackage::load_from_file_cached(&path, &cache).unwrap();
            let package = Arc::new(QwkPackage::load_from_file_cached(&path, &cache).unwrap());
            assert!(package.needs_preload());
            let entries: Vec<_> = std::fs::read_dir(&cache_directory)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.is_dir())
                .collect();
            assert_eq!(entries.len(), 1);
            let raw_body = entries[0].join("1.dat");
            let mut data = std::fs::read(&raw_body).unwrap();
            data[128 + 71] ^= 1;
            std::fs::write(&raw_body, data).unwrap();
            if delete_source {
                std::fs::remove_file(&path).unwrap();
            } else {
                let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
                std::fs::File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_modified(modified + std::time::Duration::from_secs(2))
                    .unwrap();
            }
            let context = egui::Context::default();
            let mut loader = Loader::default();
            loader.search(package, "line".into(), &context).unwrap();
            let Event::Search(generation, query, Err(error)) = loader.receiver.recv_timeout(std::time::Duration::from_secs(10)).unwrap() else {
                panic!("an unreadable raw body must fail the search, not publish successful matches");
            };
            assert_eq!(generation, loader.search_generation());
            assert_eq!(query, "line");
            assert!(!error.is_empty());
            assert!(
                loader.receiver.try_recv().is_err(),
                "failed searches do not publish a second, success-shaped result"
            );
        }
    }
}

use std::{path::PathBuf, sync::mpsc};

use eframe::egui;
use icy_view::items::{get_items_at_path, sort_items, Item, ItemError, NavPoint, ProviderType, SixteenColorsProvider};
use icy_view::sort_order::SortOrder;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Location {
    pub point: NavPoint,
    pub container: Option<Box<dyn Item>>,
}

enum Event {
    Listed(u64, Result<Vec<Box<dyn Item>>, ItemError>),
    Loaded(u64, String, Result<Vec<u8>, ItemError>),
}

pub struct Browser {
    pub location: Location,
    pub items: Vec<Box<dyn Item>>,
    pub selected: Option<usize>,
    pub filter: String,
    pub sort: SortOrder,
    pub loading: bool,
    pub preview_loading: bool,
    pub error: Option<String>,
    pub back: Vec<Location>,
    pub forward: Vec<Location>,
    /// Keys of the files passing the rating filter; folders always pass.
    pub rated: Option<std::collections::HashSet<String>>,
    /// Bumped whenever `rated` changes so cached layouts refresh.
    pub filter_generation: u64,
    generation: u64,
    preview_generation: u64,
    cancel: CancellationToken,
    preview_cancel: CancellationToken,
    runtime: tokio::runtime::Runtime,
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
}

impl Browser {
    pub fn revision(&self) -> u64 {
        self.generation
    }

    pub fn new(path: PathBuf, sort: SortOrder) -> anyhow::Result<Self> {
        let path = if path.is_absolute() { path } else { std::env::current_dir()?.join(path) };
        let mut point = NavPoint::file(path.to_string_lossy().replace('\\', "/"));
        if path.is_file()
            && !matches!(
                icy_engine::formats::FileFormat::from_path(&path),
                Some(icy_engine::formats::FileFormat::Archive(_))
            )
        {
            point.selected_item = path.file_name().map(|name| name.to_string_lossy().into_owned());
            point.path = path.parent().unwrap_or(&path).to_string_lossy().replace('\\', "/");
        }
        let (sender, receiver) = mpsc::channel();
        Ok(Self {
            location: Location { point, container: None },
            items: Vec::new(),
            selected: None,
            filter: String::new(),
            sort,
            loading: false,
            preview_loading: false,
            error: None,
            back: Vec::new(),
            forward: Vec::new(),
            rated: None,
            filter_generation: 0,
            generation: 0,
            preview_generation: 0,
            cancel: CancellationToken::new(),
            preview_cancel: CancellationToken::new(),
            runtime: tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?,
            sender,
            receiver,
        })
    }

    pub fn refresh(&mut self, context: &egui::Context) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        self.preview_cancel.cancel();
        self.preview_generation += 1;
        self.generation += 1;
        self.loading = true;
        self.preview_loading = false;
        self.error = None;
        self.items.clear();
        self.selected = None;
        let generation = self.generation;
        let location = self.location.clone();
        let cancel = self.cancel.clone();
        let sender = self.sender.clone();
        let context = context.clone();
        self.runtime.spawn(async move {
            let result = tokio::select! {
                _ = cancel.cancelled() => return,
                result = async {
                    if let Some(item) = location.container {
                        item.get_subitems(&cancel).await
                    } else if location.point.provider_type == ProviderType::Web {
                        SixteenColorsProvider::new().get_items(&location.point.path).await
                    } else {
                        tokio::task::spawn_blocking(move || get_items_at_path(&location.point.path)
                            .ok_or_else(|| ItemError::NotFound(location.point.path)))
                            .await.map_err(|error| ItemError::Other(error.to_string()))?
                    }
                } => result,
            };
            let _ = sender.send(Event::Listed(generation, result));
            context.request_repaint();
        });
    }

    pub fn navigate(&mut self, location: Location, context: &egui::Context) {
        self.back.push(self.location.clone());
        self.forward.clear();
        self.location = location;
        self.filter.clear();
        self.refresh(context);
    }

    pub fn history(&mut self, forward: bool, context: &egui::Context) {
        let (source, target) = if forward {
            (&mut self.forward, &mut self.back)
        } else {
            (&mut self.back, &mut self.forward)
        };
        if let Some(location) = source.pop() {
            target.push(self.location.clone());
            self.location = location;
            self.refresh(context);
        }
    }

    pub fn up(&mut self, context: &egui::Context) {
        let mut point = self.location.point.clone();
        if point.navigate_up() {
            if self.back.last().is_some_and(|location| location.point.path == point.path) {
                self.history(false, context);
            } else {
                self.navigate(Location { point, container: None }, context);
            }
        }
    }

    pub fn select(&mut self, index: usize, context: &egui::Context) {
        let Some(item) = self.items.get(index) else {
            return;
        };
        self.selected = Some(index);
        self.location.point.selected_item = Some(item.get_label());
        self.preview_cancel.cancel();
        self.preview_cancel = CancellationToken::new();
        self.preview_generation += 1;
        self.preview_loading = !item.is_container();
        if item.is_container() {
            return;
        }
        let item = item.clone();
        let generation = self.preview_generation;
        let cancel = self.preview_cancel.clone();
        let sender = self.sender.clone();
        let context = context.clone();
        self.runtime.spawn(async move {
            let path = item.get_full_path().unwrap_or_else(|| item.get_file_path());
            let result = tokio::select! { _ = cancel.cancelled() => return, result = item.read_data() => result };
            let _ = sender.send(Event::Loaded(generation, path, result));
            context.request_repaint();
        });
    }

    pub fn enter(&mut self, index: usize, context: &egui::Context) {
        let Some(item) = self.items.get(index) else {
            return;
        };
        if !item.is_container() {
            self.select(index, context);
            return;
        }
        let path = item
            .get_full_path()
            .unwrap_or_else(|| format!("{}/{}", self.location.point.path.trim_end_matches('/'), item.get_file_path()));
        let location = Location {
            point: NavPoint {
                provider_type: self.location.point.provider_type,
                path,
                selected_item: None,
            },
            container: Some(item.clone()),
        };
        self.navigate(location, context);
    }

    pub fn visible(&self) -> Vec<usize> {
        let filter = self.filter.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.get_label().to_lowercase().contains(&filter))
            .filter(|(_, item)| {
                item.is_container()
                    || self
                        .rated
                        .as_ref()
                        .is_none_or(|rated| rated.contains(&super::library::key(&self.location.point, &***item)))
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Warm an item's data cache so the next slideshow file shows up without a download pause.
    pub fn preload(&self, index: usize) {
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        let cancel = self.cancel.clone();
        self.runtime.spawn(async move {
            tokio::select! {
                _ = cancel.cancelled() => {}
                _ = item.read_data() => {}
            }
        });
    }

    pub fn poll(&mut self, context: &egui::Context) -> Option<(String, Vec<u8>)> {
        let mut loaded = None;
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                Event::Listed(generation, result) if generation == self.generation => {
                    self.loading = false;
                    match result {
                        Ok(mut items) => {
                            sort_items(&mut items, self.sort);
                            self.items = items;
                            if let Some(index) = self.items.iter().position(|item| Some(item.get_label()) == self.location.point.selected_item) {
                                self.select(index, context);
                            }
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
                Event::Loaded(generation, path, result) if generation == self.preview_generation => {
                    self.preview_loading = false;
                    match result {
                        Ok(data) => loaded = Some((path, data)),
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
                _ => {}
            }
        }
        loaded
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.preview_cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_directory_results_populate_cached_tiles() {
        let fixture = crate::tests::Fixture::new();
        std::fs::write(fixture.0.join("first.ans"), b"FIRST").unwrap();
        std::fs::write(fixture.0.join("second.ans"), b"SECOND").unwrap();
        let context = egui::Context::default();
        let mut browser = Browser::new(fixture.0.clone(), Default::default()).unwrap();
        browser.location.point = NavPoint::web("2025/openworld-01");
        browser.loading = true;
        let mut grid = crate::tile_grid::TileGrid::default();
        let mut thumbnails = crate::thumbnails::Thumbnails::new(&context);
        let mut icons = crate::icons::Icons::default();
        let mut draw = |browser: &Browser, grid: &mut crate::tile_grid::TileGrid| {
            context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(760.0, 640.0))),
                    ..Default::default()
                },
                |context| {
                    egui::CentralPanel::default().show(context, |ui| {
                        grid.show(ui, browser, &mut thumbnails, &mut icons, &Default::default(), false);
                    });
                },
            )
        };
        for _ in 0..3 {
            let _ = draw(&browser, &mut grid);
        }
        assert!(grid.layout.as_ref().unwrap().items.is_empty());
        let generation = browser.revision();
        let items = get_items_at_path(&fixture.0.to_string_lossy()).unwrap();
        browser.sender.send(Event::Listed(generation, Ok(items))).unwrap();
        browser.poll(&context);
        assert!(!browser.loading);
        assert_eq!(browser.items.len(), 2);
        assert_eq!(browser.revision(), generation);
        let output = draw(&browser, &mut grid);
        assert_eq!(grid.layout.as_ref().unwrap().items.len(), 2);
        assert!(!output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Text(label) if label.galley.text() == crate::text("folder-empty"))));
    }

    #[test]
    #[ignore = "requires live Sixteen Colors access"]
    fn live_web_pack_populates_tiles_after_loading() {
        let fixture = crate::tests::Fixture::new();
        let context = egui::Context::default();
        let mut browser = Browser::new(fixture.0.clone(), Default::default()).unwrap();
        browser.location = Location {
            point: NavPoint::web("2025/mist1225"),
            container: Some(Box::new(icy_view::items::SixteenColorsPack::new(
                "mist1225.zip".into(),
                0,
                2025,
                "mist1225".into(),
            ))),
        };
        browser.refresh(&context);
        let mut grid = crate::tile_grid::TileGrid::default();
        let mut thumbnails = crate::thumbnails::Thumbnails::new(&context);
        let mut icons = crate::icons::Icons::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let output = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(760.0, 640.0))),
                    ..Default::default()
                },
                |context| {
                    egui::CentralPanel::default().show(context, |ui| {
                        grid.show(ui, &browser, &mut thumbnails, &mut icons, &Default::default(), false);
                    });
                },
            );
            if !browser.loading {
                if let Some(error) = &browser.error {
                    // The pack endpoint is reached over a redirect and can time out under load.
                    println!("skipped, Sixteen Colors unreachable: {error}");
                    return;
                }
                assert!(!browser.items.is_empty(), "live pack returned no files");
                assert_eq!(grid.layout.as_ref().unwrap().items.len(), browser.items.len());
                let label = browser.items[0].get_label();
                assert!(output
                    .shapes
                    .iter()
                    .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == label)));
                println!(
                    "mist1225: {} files, {} tiles; first: {label}",
                    browser.items.len(),
                    grid.layout.as_ref().unwrap().items.len()
                );
                break;
            }
            assert!(std::time::Instant::now() < deadline, "live pack request timed out");
            browser.poll(&context);
            std::thread::yield_now();
        }
    }

    #[test]
    fn stale_results_do_not_replace_current_selection_or_finish_loading() {
        let fixture = crate::tests::Fixture::new();
        std::fs::write(fixture.0.join("art.ans"), b"CURRENT").unwrap();
        let context = egui::Context::default();
        let mut browser = Browser::new(fixture.0.clone(), Default::default()).unwrap();
        browser.refresh(&context);
        crate::tests::wait_browser(&mut browser, &context);
        browser.preview_generation = 42;
        browser.preview_loading = true;
        browser.sender.send(Event::Loaded(41, "old.ans".into(), Ok(b"OLD".to_vec()))).unwrap();
        browser.sender.send(Event::Listed(0, Ok(Vec::new()))).unwrap();
        assert!(browser.poll(&context).is_none());
        assert!(browser.preview_loading);
        assert_eq!(browser.items.len(), 1);
        browser.sender.send(Event::Loaded(42, "art.ans".into(), Ok(b"CURRENT".to_vec()))).unwrap();
        assert_eq!(browser.poll(&context).unwrap().1, b"CURRENT");
        assert!(!browser.preview_loading);
    }
}

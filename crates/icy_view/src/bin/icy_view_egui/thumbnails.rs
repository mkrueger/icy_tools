use eframe::egui;
use icy_view::{
    items::Item,
    thumbnail_backend::{
        thumbnail::{ThumbnailResult, ThumbnailState},
        thumbnail_loader::{ThumbnailLoader, ThumbnailRequest},
    },
};
use std::{
    collections::HashMap,
    sync::{mpsc, Arc},
};

pub struct Thumbnails {
    loader: ThumbnailLoader,
    receiver: mpsc::Receiver<ThumbnailResult>,
    pub entries: HashMap<String, Entry>,
    pub revision: u64,
    clock: u64,
    source: Option<(String, u64)>,
}

pub struct Entry {
    pub images: Vec<ThumbnailImage>,
    pub dimensions: egui::Vec2,
    pub width_multiplier: usize,
    pub sauce: Option<icy_sauce::SauceRecord>,
    pub error: Option<String>,
    last_used: u64,
    pending: bool,
}

pub struct ThumbnailImage {
    pub size: egui::Vec2,
    pub slices: Vec<(egui::Vec2, egui::TextureHandle)>,
}

impl ThumbnailImage {
    fn new(context: &egui::Context, name: &str, rgba: &icy_view::thumbnail::RgbaData) -> Self {
        let limit = context.input(|input| input.max_texture_side).clamp(1, 2048);
        let mut slices = Vec::new();
        for top in (0..rgba.height as usize).step_by(limit) {
            for left in (0..rgba.width as usize).step_by(limit) {
                let width = limit.min(rgba.width as usize - left);
                let height = limit.min(rgba.height as usize - top);
                let mut pixels = Vec::with_capacity(width * height * 4);
                for row in top..top + height {
                    let start = (row * rgba.width as usize + left) * 4;
                    pixels.extend_from_slice(&rgba.data[start..start + width * 4]);
                }
                let texture = context.load_texture(
                    name,
                    egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels),
                    egui::TextureOptions::NEAREST,
                );
                slices.push((egui::vec2(left as f32, top as f32), texture));
            }
        }
        Self {
            size: egui::vec2(rgba.width as f32, rgba.height as f32),
            slices,
        }
    }

    pub fn paint(&self, ui: &egui::Ui, bounds: egui::Rect) {
        let scale = bounds.width() / self.size.x.max(1.0);
        for (offset, texture) in &self.slices {
            let rect = egui::Rect::from_min_size(bounds.min + *offset * scale, texture.size_vec2() * scale);
            if ui.is_rect_visible(rect) {
                egui::Image::new(texture).paint_at(ui, rect);
            }
        }
    }
}

impl Thumbnails {
    pub fn new(context: &egui::Context) -> Self {
        let (loader, mut results) = ThumbnailLoader::spawn();
        let (sender, receiver) = mpsc::channel();
        let context = context.clone();
        std::thread::spawn(move || {
            while let Some(result) = results.blocking_recv() {
                if sender.send(result).is_err() {
                    break;
                }
                context.request_repaint();
            }
        });
        Self {
            loader,
            receiver,
            entries: HashMap::new(),
            revision: 0,
            clock: 0,
            source: None,
        }
    }

    pub fn key(item: &dyn Item) -> String {
        item.get_full_path().unwrap_or_else(|| item.get_file_path())
    }

    pub fn clear(&mut self, context: &egui::Context) {
        let revision = self.revision + 1;
        *self = Self::new(context);
        self.revision = revision;
    }

    pub fn request(&mut self, item: &dyn Item) {
        let key = Self::key(item);
        self.clock += 1;
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_used = self.clock;
            if entry.pending || !entry.images.is_empty() || entry.error.is_some() {
                return;
            }
            entry.pending = true;
        } else {
            self.entries.insert(
                key,
                Entry {
                    images: Vec::new(),
                    dimensions: egui::vec2(320.0, 100.0),
                    width_multiplier: 1,
                    sauce: None,
                    error: None,
                    last_used: self.clock,
                    pending: true,
                },
            );
        }
        self.loader.load(ThumbnailRequest {
            item: Arc::from(item.clone_box()),
        });
    }

    pub fn poll(&mut self, context: &egui::Context) {
        while let Ok(result) = self.receiver.try_recv() {
            let Some(entry) = self.entries.get_mut(&result.path) else {
                continue;
            };
            entry.sauce = result.sauce_info;
            entry.pending = false;
            entry.width_multiplier = (result.width_multiplier as usize).clamp(1, 3);
            let frames = match result.state {
                ThumbnailState::Ready { rgba } => vec![rgba],
                ThumbnailState::Animated { frames, .. } => frames,
                ThumbnailState::Error { message, .. } => {
                    entry.error = Some(message);
                    Vec::new()
                }
                _ => Vec::new(),
            };
            if let Some(rgba) = frames.first() {
                entry.dimensions = egui::vec2(rgba.width as f32, rgba.height as f32);
            }
            entry.images = frames.iter().take(2).map(|rgba| ThumbnailImage::new(context, &result.path, rgba)).collect();
            self.revision += 1;
        }
    }

    pub fn trim(&mut self) {
        let mut loaded: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| !entry.images.is_empty())
            .map(|(path, entry)| {
                let bytes: usize = entry
                    .images
                    .iter()
                    .flat_map(|image| &image.slices)
                    .map(|(_, texture)| texture.byte_size())
                    .sum();
                (path.clone(), entry.last_used, bytes)
            })
            .collect();
        let mut bytes: usize = loaded.iter().map(|(_, _, size)| size).sum();
        let mut count = loaded.len();
        loaded.sort_by_key(|(_, used, _)| *used);
        for (path, _, size) in loaded {
            if count <= 500 && bytes <= 256 * 1024 * 1024 {
                break;
            }
            self.entries.get_mut(&path).unwrap().images.clear();
            bytes = bytes.saturating_sub(size);
            count -= 1;
        }
    }

    pub fn set_source(&mut self, context: &egui::Context, path: &str, revision: u64) {
        let source = (path.to_owned(), revision);
        if self.source.as_ref() != Some(&source) {
            self.clear(context);
            self.source = Some(source);
        }
    }
}

impl Drop for Thumbnails {
    fn drop(&mut self) {
        self.loader.cancel_loading();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eviction_preserves_geometry_and_changing_source_clears_it() {
        let context = egui::Context::default();
        let mut thumbnails = Thumbnails::new(&context);
        thumbnails.set_source(&context, "archive-a", 1);
        for index in 0..501 {
            thumbnails.entries.insert(
                index.to_string(),
                Entry {
                    images: vec![ThumbnailImage::new(
                        &context,
                        "cached-thumbnail",
                        &icy_view::thumbnail::RgbaData::new(vec![255; 4], 1, 1),
                    )],
                    dimensions: egui::vec2(640.0, 1800.0),
                    width_multiplier: 2,
                    sauce: None,
                    error: None,
                    last_used: index,
                    pending: false,
                },
            );
        }
        thumbnails.trim();
        let evicted = &thumbnails.entries["0"];
        assert!(evicted.images.is_empty());
        assert_eq!(evicted.dimensions, egui::vec2(640.0, 1800.0));
        assert_eq!(evicted.width_multiplier, 2);
        assert_eq!(thumbnails.entries.values().filter(|entry| !entry.images.is_empty()).count(), 500);
        thumbnails.set_source(&context, "archive-a", 1);
        assert_eq!(thumbnails.entries.len(), 501);
        thumbnails.set_source(&context, "archive-b", 1);
        assert!(thumbnails.entries.is_empty());
    }
}

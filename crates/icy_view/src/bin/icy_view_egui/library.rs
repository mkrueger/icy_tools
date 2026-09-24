//! Per-user data that outlives a session: star ratings, view history, pinned and recent places.
//! Everything is keyed by the display path, so art inside archives and on 16colo.rs works too.

use icy_view::items::{Item, NavPoint, ProviderType};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const RECENT_LIMIT: usize = 12;
const VIEWED_LIMIT: usize = 50_000;
const SAVE_DELAY: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub path: String,
    #[serde(default)]
    pub web: bool,
}

impl Place {
    pub fn from_point(point: &NavPoint) -> Self {
        Self {
            path: point.path.clone(),
            web: point.provider_type == ProviderType::Web,
        }
    }

    pub fn label(&self) -> String {
        let name = self.path.trim_end_matches('/').rsplit('/').next().unwrap_or_default();
        match (self.web, name.is_empty()) {
            (true, true) => "16colo.rs".into(),
            (true, false) => format!("16colo.rs › {name}"),
            (false, true) => self.path.clone(),
            (false, false) => name.to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Viewed {
    pub count: u32,
    pub last: u64,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Library {
    #[serde(default)]
    pub ratings: HashMap<String, u8>,
    #[serde(default)]
    pub viewed: HashMap<String, Viewed>,
    #[serde(default)]
    pub favorites: Vec<Place>,
    #[serde(default)]
    pub recent: Vec<Place>,
    #[serde(skip)]
    path: Option<PathBuf>,
    #[serde(skip)]
    changed: Option<Instant>,
}

/// Stable identity of an item across folders, archives and the web.
pub fn key(location: &NavPoint, item: &dyn Item) -> String {
    if let Some(path) = item.get_full_path() {
        path
    } else if item.is_virtual_file() || location.provider_type == ProviderType::Web {
        format!("16colo.rs:{}", item.get_file_path())
    } else {
        format!("{}/{}", location.path.trim_end_matches('/'), item.get_file_path())
    }
}

impl Library {
    pub fn load(path: Option<PathBuf>) -> Self {
        let mut library: Self = path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        library.path = path;
        library
    }

    pub fn rating(&self, key: &str) -> u8 {
        self.ratings.get(key).copied().unwrap_or(0)
    }

    pub fn set_rating(&mut self, key: String, rating: u8) {
        if rating == 0 {
            self.ratings.remove(&key);
        } else {
            self.ratings.insert(key, rating.min(5));
        }
        self.touch();
    }

    pub fn viewed(&self, key: &str) -> Option<Viewed> {
        self.viewed.get(key).copied()
    }

    pub fn mark_viewed(&mut self, key: String) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |time| time.as_secs());
        let entry = self.viewed.entry(key).or_default();
        entry.count += 1;
        entry.last = now;
        if self.viewed.len() > VIEWED_LIMIT {
            let mut oldest: Vec<_> = self.viewed.iter().map(|(key, viewed)| (viewed.last, key.clone())).collect();
            oldest.sort();
            for (_, key) in oldest.into_iter().take(self.viewed.len() - VIEWED_LIMIT) {
                self.viewed.remove(&key);
            }
        }
        self.touch();
    }

    pub fn set_viewed(&mut self, key: String, viewed: bool) {
        if viewed {
            self.mark_viewed(key);
        } else {
            self.viewed.remove(&key);
            self.touch();
        }
    }

    pub fn is_favorite(&self, place: &Place) -> bool {
        self.favorites.contains(place)
    }

    pub fn toggle_favorite(&mut self, place: Place) {
        if let Some(index) = self.favorites.iter().position(|favorite| *favorite == place) {
            self.favorites.remove(index);
        } else {
            self.favorites.push(place);
        }
        self.touch();
    }

    pub fn visit(&mut self, place: Place) {
        if self.recent.first() == Some(&place) {
            return;
        }
        self.recent.retain(|recent| *recent != place);
        self.recent.insert(0, place);
        self.recent.truncate(RECENT_LIMIT);
        self.touch();
    }

    fn touch(&mut self) {
        self.changed.get_or_insert_with(Instant::now);
    }

    /// Writes pending changes once they have settled, so browsing doesn't write on every file.
    pub fn save_if_due(&mut self) {
        if self.changed.is_some_and(|changed| changed.elapsed() >= SAVE_DELAY) {
            self.save();
        }
    }

    pub fn save(&mut self) {
        if self.changed.take().is_none() {
            return;
        }
        let Some(path) = &self.path else {
            return;
        };
        match serde_json::to_string(self) {
            Ok(text) => {
                if let Err(error) = std::fs::write(path, text) {
                    log::error!("Error writing {}: {error}", path.display());
                }
            }
            Err(error) => log::error!("Error serializing library: {error}"),
        }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        self.save();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Rate(u8),
    Viewed(bool),
    Pin,
}

/// Place a container item leads to, matching what entering it navigates to.
pub fn place(location: &NavPoint, item: &dyn Item) -> Place {
    Place {
        path: item
            .get_full_path()
            .unwrap_or_else(|| format!("{}/{}", location.path.trim_end_matches('/'), item.get_file_path())),
        web: location.provider_type == ProviderType::Web,
    }
}

/// Right-click menu shared by the list and the tiles.
pub fn context_menu(ui: &mut eframe::egui::Ui, library: &Library, location: &NavPoint, item: &dyn Item) -> Option<Change> {
    use super::text;
    let mut change = None;
    if item.is_container() {
        let pinned = library.is_favorite(&place(location, item));
        if ui.button(text(if pinned { "egui-unpin" } else { "egui-pin" })).clicked() {
            change = Some(Change::Pin);
        }
        return change;
    }
    let key = key(location, item);
    let rating = library.rating(&key);
    ui.menu_button(text("egui-rating"), |ui| {
        for value in 0..=5u8 {
            let label = if value == 0 { text("egui-rating-none") } else { super::colors::stars(value) };
            let button = eframe::egui::Button::new(label).selected(rating == value).shortcut_text(value.to_string());
            if ui.add(button).clicked() {
                change = Some(Change::Rate(value));
            }
        }
    });
    let viewed = library.viewed(&key).is_some();
    if ui.button(text(if viewed { "egui-mark-unviewed" } else { "egui-mark-viewed" })).clicked() {
        change = Some(Change::Viewed(!viewed));
    }
    change
}

/// Round check badge marking art that has been opened before.
pub fn paint_viewed(painter: &eframe::egui::Painter, center: eframe::egui::Pos2, radius: f32, visuals: &eframe::egui::Visuals) {
    use eframe::egui;
    painter.circle(center, radius, visuals.selection.bg_fill, egui::Stroke::new(1.0, visuals.window_fill));
    let size = radius * 0.5;
    painter.line(
        vec![
            center + egui::vec2(-size, 0.0),
            center + egui::vec2(-size * 0.3, size * 0.7),
            center + egui::vec2(size, -size * 0.6),
        ],
        egui::Stroke::new((radius * 0.28).max(1.2), visuals.selection.stroke.color),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_round_trips_ratings_history_and_places() {
        let fixture = crate::tests::Fixture::new();
        let path = fixture.0.join("library.json");
        {
            let mut library = Library::load(Some(path.clone()));
            library.set_rating("/art/a.ans".into(), 4);
            library.set_rating("/art/b.ans".into(), 9);
            library.mark_viewed("/art/a.ans".into());
            library.mark_viewed("/art/a.ans".into());
            library.toggle_favorite(Place {
                path: "/art".into(),
                web: false,
            });
            for index in 0..20 {
                library.visit(Place {
                    path: format!("/art/{index}"),
                    web: false,
                });
            }
            library.visit(Place {
                path: "/art/5".into(),
                web: false,
            });
        }
        let library = Library::load(Some(path));
        assert_eq!(library.rating("/art/a.ans"), 4);
        assert_eq!(library.rating("/art/b.ans"), 5);
        assert_eq!(library.viewed("/art/a.ans").unwrap().count, 2);
        assert!(library.viewed("/art/b.ans").is_none());
        assert_eq!(library.favorites.len(), 1);
        assert_eq!(library.recent.len(), RECENT_LIMIT);
        assert_eq!(library.recent[0].path, "/art/5");
        assert_eq!(library.recent.iter().filter(|place| place.path == "/art/5").count(), 1);
    }

    #[test]
    fn keys_distinguish_archive_members_and_web_files() {
        let zip = NavPoint::file("/art/pack.zip");
        let web = NavPoint::web("2025/pack");
        let file = icy_view::items::ItemFile::new("/art/a.ans".into());
        assert_eq!(key(&zip, &file), "/art/a.ans");
        let remote = icy_view::items::SixteenColorsFile::new("x.ans".into(), "/pack/p/raw".into(), String::new(), String::new());
        assert_eq!(key(&web, &remote), "16colo.rs:/pack/p/raw/x.ans");
        assert_eq!(Place::from_point(&web).label(), "16colo.rs › pack");
        assert_eq!(Place::from_point(&NavPoint::web("")).label(), "16colo.rs");
    }
}

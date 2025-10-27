use std::num::NonZeroUsize;
use std::ops::Deref;

use evictor::Lru;

use crate::Fonts;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub(crate) struct Key {
    pub(crate) style: Modifier,
    pub(crate) glyph: u32,
    pub(crate) font: u64,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) struct CacheRect {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Entry {
    Cached(CacheRect),
    Uncached(CacheRect),
}

impl Entry {
    pub(crate) fn cached(&self) -> bool {
        matches!(self, Entry::Cached(_))
    }
}

impl Deref for Entry {
    type Target = CacheRect;

    fn deref(&self) -> &Self::Target {
        let (Entry::Cached(entry) | Entry::Uncached(entry)) = self;
        entry
    }
}

#[derive(Debug)]
pub(crate) struct Atlas {
    lru: Lru<Key, CacheRect>,
    width: u32,
    height: u32,

    entry_width: u32,
    entry_height: u32,

    next_entry: u32,
    max_entries: u32,
}

impl Atlas {
    pub(crate) fn new(fonts: &Fonts, width: u32, height: u32) -> Self {
        let entry_width = fonts.min_width_px() * 2;
        let entry_height = fonts.height_px();
        let max_entries = ((width / entry_width) * (height / entry_height)).max(1);
        log::debug!("Atlas with WxH {entry_width}x{entry_height} can hold {max_entries}");

        Atlas {
            lru: Lru::new(
                NonZeroUsize::new(max_entries as usize).expect("Max entries must be non-zero"),
            ),
            width,
            height,
            entry_width,
            entry_height,
            next_entry: 0,
            max_entries,
        }
    }

    pub(crate) fn match_fonts(&mut self, fonts: &Fonts) {
        self.clear();
        self.entry_width = fonts.min_width_px() * 2;
        self.entry_height = fonts.height_px();
        self.max_entries = (self.width / self.entry_width) * (self.height / self.entry_height);

        log::debug!(
            "Atlas with WxH {}x{} can hold {}",
            self.entry_width, self.entry_height, self.max_entries
        );
    }

    fn clear(&mut self) {
        self.lru.clear();
        self.next_entry = 0;
    }

    pub(crate) fn try_get(&mut self, key: &Key) -> Option<Entry> {
        self.lru.get(key).copied().map(Entry::Cached)
    }

    pub(crate) fn get(&mut self, key: &Key, width: u32, height: u32) -> Entry {
        debug_assert_eq!(
            self.entry_height, height,
            "Internal height not equal to provided height. Did you forget to call match_fonts?"
        );
        debug_assert_eq!(
            self.entry_width % width,
            0,
            "Internal width not a multiple of provided width. Did you forget to call match_fonts?"
        );

        self.try_get(key).unwrap_or_else(|| {
            let rect = if self.next_entry == self.max_entries {
                self.lru.pop().expect("Atlas has zero max entries!").1
            } else {
                let entry = self.next_entry;
                self.next_entry += 1;
                self.slot_to_rect(entry, width)
            };

            self.lru.insert(*key, rect);
            Entry::Uncached(rect)
        })
    }

    fn slot_to_rect(&self, slot: u32, width: u32) -> CacheRect {
        let x = slot % (self.width / self.entry_width) * self.entry_width;
        let y = slot / (self.width / self.entry_width) * self.entry_height;
        CacheRect {
            x,
            y,
            width,
            height: self.entry_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Modifier;

    use crate::utils::text_atlas::Atlas;
    use crate::utils::text_atlas::Key;
    use crate::Font;
    use crate::Fonts;

    #[test]
    fn reuse() {
        let fonts = Fonts::new(
            Font::new(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/fonts/Fairfax.ttf"
            )))
            .unwrap(),
            24,
        );
        let mut atlas = Atlas::new(&fonts, 24, 24);

        for idx in 0..atlas.max_entries {
            atlas.get(
                &Key {
                    style: Modifier::default(),
                    glyph: idx as _,
                    font: idx as _,
                },
                12,
                24,
            );
        }

        let last_key = Key {
            style: Modifier::default(),
            glyph: u32::MAX,
            font: u32::MAX as _,
        };

        let last_inserted = atlas.get(&last_key, 12, 24);
        let post_insertion = atlas.get(&last_key, 12, 24);

        assert_eq!(*last_inserted, *post_insertion);
    }
}

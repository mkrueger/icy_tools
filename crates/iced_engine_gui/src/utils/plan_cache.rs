use std::num::NonZeroUsize;

use evictor::Lru;
use rustybuzz::Direction;
use rustybuzz::Script;
use rustybuzz::ShapePlan;
use rustybuzz::UnicodeBuffer;

use crate::Font;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    face_id: u64,
    direction: Direction,
    script: Script,
}

pub(crate) struct PlanCache {
    lru: Lru<Key, ShapePlan>,
}

impl PlanCache {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            lru: Lru::new(NonZeroUsize::new(capacity).expect("Capacity must be non-zero")),
        }
    }

    pub(crate) fn get(&mut self, font: &Font, buffer: &mut UnicodeBuffer) -> &ShapePlan {
        buffer.guess_segment_properties();
        let key = Key {
            face_id: font.id(),
            direction: buffer.direction(),
            script: buffer.script(),
        };

        self.lru.get_or_insert_with(key, |_| {
            ShapePlan::new(
                font.font(),
                buffer.direction(),
                Some(buffer.script()),
                buffer.language().as_ref(),
                &[],
            )
        })
    }
}

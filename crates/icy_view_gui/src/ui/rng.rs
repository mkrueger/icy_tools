use std::ops::{Bound, RangeBounds};

use web_time::Instant;

pub struct Rng {
    state: i32,
}

impl Rng {
    // Lehmer random number generator
    pub fn next(&mut self) -> i32 {
        self.state = ((self.state as u64).wrapping_mul(48271) % 0x7fff_ffff) as i32;
        self.state
    }

    pub(crate) fn gen_range<R: RangeBounds<u8>>(&mut self, arg: R) -> u32 {
        let bounds = (arg.start_bound(), arg.end_bound());
        let res = match bounds {
            (Bound::Included(a), Bound::Included(b)) => (*a as usize) + (self.next() as usize) % (*b + 1 - *a) as usize,

            (Bound::Included(a), Bound::Excluded(b)) => (*a as usize) + (self.next() as usize) % (*b - *a) as usize,
            _ => panic!("Unsupported range bounds"),
        };

        res as u32
    }
}

impl Default for Rng {
    fn default() -> Self {
        Self {
            state: Instant::now().duration_since(START_TIME.to_owned()).as_nanos() as i32,
        }
    }
}

lazy_static::lazy_static! {
    static ref START_TIME: Instant = Instant::now();
}

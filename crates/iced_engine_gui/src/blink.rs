use once_cell::sync::Lazy;
use web_time::Instant;

static BLINK_START: Lazy<Instant> = Lazy::new(Instant::now);

pub fn now_ms() -> u128 {
    BLINK_START.elapsed().as_millis() as u128
}

pub struct Blink {
    is_on: bool,
    last_blink: u128,
    blink_rate: u128,
}

impl Blink {
    pub fn new(blink_rate: u128) -> Self {
        Self {
            is_on: false,
            last_blink: 0,
            blink_rate,
        }
    }

    /// Milliseconds since the Blink system started (shared base for update/is_due).
    pub fn now_ms() -> u128 {
        BLINK_START.elapsed().as_millis() as u128
    }

    pub fn is_on(&self) -> bool {
        self.is_on
    }

    /// Attempt a toggle given a current time in ms. Returns true if state changed.
    pub fn update(&mut self, cur_ms: u128) -> bool {
        if cur_ms - self.last_blink > self.blink_rate {
            self.is_on = !self.is_on;
            self.last_blink = cur_ms;
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        self.is_on = true;
        self.last_blink = Blink::now_ms();
    }
}

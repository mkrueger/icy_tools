//! Audio bridge for the YM2149 GIST replayer.
//!
//! The `ym2149` crate only emulates the chip; feeding its samples to the sound card is
//! up to the host. The replayer runs on the music thread and pushes samples into a ring
//! buffer, while rodio pulls from it on the audio callback thread.

use std::{num::NonZero, sync::Arc, time::Duration};

use parking_lot::Mutex;
use rodio::{ChannelCount, Sample, SampleRate, Source};

/// Rate the GIST replayer is clocked at. The rodio mixer resamples to the device rate.
pub const SAMPLE_RATE: u32 = 44100;

/// ~372ms of headroom at 44.1kHz - favours robustness over latency for sound effects.
const RING_BUFFER_SIZE: usize = 16384;

/// Batch size for pulling samples out of the ring buffer, to keep lock contention low.
const READ_CHUNK: usize = 4096;

/// Single-producer / single-consumer sample queue.
pub struct RingBuffer {
    buffer: Vec<f32>,
    write_pos: usize,
    read_pos: usize,
    mask: usize,
}

impl RingBuffer {
    fn new() -> Self {
        let capacity = RING_BUFFER_SIZE.next_power_of_two();
        Self {
            buffer: vec![0.0; capacity],
            write_pos: 0,
            read_pos: 0,
            mask: capacity - 1,
        }
    }

    fn capacity(&self) -> usize {
        self.buffer.len()
    }

    fn available_read(&self) -> usize {
        self.write_pos.wrapping_sub(self.read_pos)
    }

    /// Writes as many samples as fit and returns how many were taken.
    pub fn write(&mut self, samples: &[f32]) -> usize {
        // One slot stays empty so a full buffer is distinguishable from an empty one.
        let available = self.capacity() - self.available_read() - 1;
        let to_write = samples.len().min(available);
        if to_write == 0 {
            return 0;
        }

        let idx = self.write_pos & self.mask;
        if idx + to_write <= self.capacity() {
            self.buffer[idx..idx + to_write].copy_from_slice(&samples[..to_write]);
        } else {
            let first = self.capacity() - idx;
            self.buffer[idx..].copy_from_slice(&samples[..first]);
            self.buffer[..to_write - first].copy_from_slice(&samples[first..to_write]);
        }

        self.write_pos = self.write_pos.wrapping_add(to_write);
        to_write
    }

    /// Reads into `dest` and returns how many samples were filled.
    fn read(&mut self, dest: &mut [f32]) -> usize {
        let to_read = dest.len().min(self.available_read());
        if to_read == 0 {
            return 0;
        }

        let idx = self.read_pos & self.mask;
        if idx + to_read <= self.capacity() {
            dest[..to_read].copy_from_slice(&self.buffer[idx..idx + to_read]);
        } else {
            let first = self.capacity() - idx;
            dest[..first].copy_from_slice(&self.buffer[idx..]);
            dest[first..to_read].copy_from_slice(&self.buffer[..to_read - first]);
        }

        self.read_pos = self.read_pos.wrapping_add(to_read);
        to_read
    }
}

/// Endless rodio source draining [`RingBuffer`]; outputs silence when it runs dry.
struct RingBufferSource {
    ring: Arc<Mutex<RingBuffer>>,
    chunk: Vec<f32>,
    chunk_pos: usize,
}

impl Iterator for RingBufferSource {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        if self.chunk_pos >= self.chunk.len() {
            let read = self.ring.lock().read(&mut self.chunk);
            if read < self.chunk.len() {
                self.chunk[read..].fill(0.0);
            }
            self.chunk_pos = 0;
        }

        let sample = self.chunk[self.chunk_pos];
        self.chunk_pos += 1;
        Some(sample)
    }
}

impl Source for RingBufferSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZero::new(1).unwrap()
    }

    fn sample_rate(&self) -> SampleRate {
        NonZero::new(SAMPLE_RATE).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Attaches a GIST sample sink to `mixer` and returns the producer side of the queue.
pub fn attach(mixer: &rodio::mixer::Mixer) -> Arc<Mutex<RingBuffer>> {
    let ring = Arc::new(Mutex::new(RingBuffer::new()));
    mixer.add(RingBufferSource {
        ring: Arc::clone(&ring),
        chunk: vec![0.0; READ_CHUNK],
        chunk_pos: READ_CHUNK,
    });
    ring
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_read_roundtrips() {
        let mut ring = RingBuffer::new();
        let input: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        assert_eq!(ring.write(&input), 1000);

        let mut out = vec![0.0; 1000];
        assert_eq!(ring.read(&mut out), 1000);
        assert_eq!(out, input);
    }

    #[test]
    fn write_stops_at_capacity() {
        let mut ring = RingBuffer::new();
        let input = vec![1.0; RING_BUFFER_SIZE * 2];
        assert_eq!(ring.write(&input), RING_BUFFER_SIZE - 1);
        assert_eq!(ring.write(&input), 0);
    }

    #[test]
    fn read_wraps_around_the_end() {
        let mut ring = RingBuffer::new();
        let mut scratch = vec![0.0; RING_BUFFER_SIZE - 1];

        // Push the positions close to the end of the backing store.
        ring.write(&scratch);
        ring.read(&mut scratch);

        let input: Vec<f32> = (0..100).map(|i| i as f32).collect();
        assert_eq!(ring.write(&input), 100);

        let mut out = vec![0.0; 100];
        assert_eq!(ring.read(&mut out), 100);
        assert_eq!(out, input);
    }

    #[test]
    fn read_on_empty_buffer_yields_nothing() {
        let mut ring = RingBuffer::new();
        let mut out = vec![9.0; 16];
        assert_eq!(ring.read(&mut out), 0);
    }
}

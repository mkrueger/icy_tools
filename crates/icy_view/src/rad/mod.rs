//! Reality AdLib Tracker (`.rad`) support for icy_view.
//!
//! The OPL3 emulator and RAD replayer are public-domain Opal/RADPlayer ports by
//! Shayde/Reality, ported via kaleidotron (MIT, Copyright (c) 2026 Rick Christy).

pub mod opl3;
pub mod player;

use opl3::Opl3;
use player::RadPlayer;

const MAX_SECONDS: f64 = 600.0;
#[cfg(test)]
const DEFAULT_RATE: u32 = 48_000;

#[derive(Clone, Debug, PartialEq)]
pub struct RadTune {
    data: Vec<u8>,
    version: i32,
    channels: usize,
    tick_hz: f64,
    duration: f64,
    description: Vec<u8>,
    instruments: Vec<Vec<u8>>,
}

impl RadTune {
    pub fn load(data: &[u8]) -> anyhow::Result<Self> {
        let mut player = RadPlayer::new(data).ok_or_else(|| anyhow::anyhow!("not a RAD v1/v2 module"))?;
        if player.order_count() == 0 || player.track_count() == 0 {
            anyhow::bail!("RAD module has no playable order list");
        }
        let tick_hz = player.hz().clamp(1.0, 1000.0);
        let version = player.version();
        let channels = if player.uses_opl3() { 18 } else { 9 };
        let description = expand_description(player.description());
        let instruments = player.instrument_names().into_iter().map(|name| clean_bytes(&name)).collect();
        let duration = duration_from(&mut player, tick_hz);
        if duration <= 0.0 {
            anyhow::bail!("RAD module has no playable duration");
        }
        Ok(Self {
            data: data.to_vec(),
            version,
            channels,
            tick_hz,
            duration,
            description,
            instruments,
        })
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn format_name(&self) -> &'static str {
        match self.version {
            1 => "Reality AdLib Tracker RAD v1",
            2 => "Reality AdLib Tracker RAD v2",
            _ => "Reality AdLib Tracker RAD",
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn speed(&self) -> usize {
        self.tick_hz.round().max(1.0) as usize
    }

    pub fn bpm(&self) -> usize {
        (self.tick_hz * 2.5).round().max(1.0) as usize
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn title(&self) -> Vec<u8> {
        first_line(&self.description).unwrap_or_else(|| b"RAD module".to_vec())
    }

    pub fn message(&self) -> Vec<Vec<u8>> {
        lines(&self.description)
    }

    pub fn instruments(&self) -> Vec<Vec<u8>> {
        self.instruments.clone()
    }

    pub fn renderer(&self, sample_rate: u32) -> anyhow::Result<RadRenderer> {
        RadRenderer::new(&self.data, sample_rate)
    }
}

/// RAD descriptions encode a line break as 0x01 and runs of 2..=31 spaces as the run length.
fn expand_description(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &byte in bytes.iter().take_while(|&&b| b != 0) {
        match byte {
            0x01 => out.push(b'\n'),
            0x02..=0x1F => out.resize(out.len() + byte as usize, b' '),
            _ => out.push(byte),
        }
    }
    out
}

fn clean_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = bytes.iter().map(|b| if *b == 0 { b' ' } else { *b }).collect();
    while out.last() == Some(&b' ') {
        out.pop();
    }
    out
}

fn first_line(bytes: &[u8]) -> Option<Vec<u8>> {
    lines(bytes).into_iter().find(|line| !line.iter().all(|b| b.is_ascii_whitespace()))
}

/// Lines of the description; blank lines inside it are kept because they are part of the layout.
fn lines(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = bytes
        .split(|b| *b == b'\n')
        .map(|line| clean_bytes(line.strip_suffix(b"\r").unwrap_or(line)))
        .collect();
    while lines.last().is_some_and(Vec::is_empty) {
        lines.pop();
    }
    let leading = lines.iter().take_while(|line| line.is_empty()).count();
    lines.drain(..leading);
    lines
}

fn duration_from(player: &mut RadPlayer, tick_hz: f64) -> f64 {
    let max_ticks = (MAX_SECONDS * tick_hz).ceil() as usize;
    let mut ticks = 0usize;
    let mut write = |_: u16, _: u8| {};
    while ticks < max_ticks {
        ticks += 1;
        if !player.update(&mut write) {
            break;
        }
    }
    ticks as f64 / tick_hz
}

pub struct RadRenderer {
    player: RadPlayer,
    chip: Opl3,
    sample_rate: u32,
    frames_per_tick: usize,
    frame_in_tick: usize,
    sample: Option<i16>,
    ended: bool,
    frames: usize,
    max_frames: usize,
}

impl RadRenderer {
    pub fn new(data: &[u8], sample_rate: u32) -> anyhow::Result<Self> {
        let player = RadPlayer::new(data).ok_or_else(|| anyhow::anyhow!("not a RAD v1/v2 module"))?;
        let tick_hz = player.hz().clamp(1.0, 1000.0);
        let sample_rate = sample_rate.max(1);
        Ok(Self {
            player,
            chip: Opl3::new(sample_rate),
            sample_rate,
            frames_per_tick: (sample_rate as f64 / tick_hz).round().max(1.0) as usize,
            frame_in_tick: usize::MAX,
            sample: None,
            ended: false,
            frames: 0,
            max_frames: (MAX_SECONDS * sample_rate as f64) as usize,
        })
    }

    pub fn seek_seconds(&mut self, seconds: f64) {
        let target = (seconds.max(0.0) * self.sample_rate as f64).round() as usize;
        while self.frames < target && self.next_sample().is_some() {}
    }

    pub fn position_seconds(&self) -> f64 {
        self.frames as f64 / self.sample_rate as f64
    }

    pub fn next_f32(&mut self) -> Option<f32> {
        self.next_sample().map(|sample| sample as f32 / 32768.0)
    }

    fn next_sample(&mut self) -> Option<i16> {
        if let Some(right) = self.sample.take() {
            return Some(right);
        }
        if self.ended || self.frames >= self.max_frames {
            return None;
        }
        if self.frame_in_tick >= self.frames_per_tick {
            let mut write = |reg, val| self.chip.write_reg(reg, val);
            if !self.player.update(&mut write) {
                self.ended = true;
                return None;
            }
            self.frame_in_tick = 0;
        }
        let (left, right) = self.chip.sample();
        self.frame_in_tick += 1;
        self.frames += 1;
        self.sample = Some(right);
        Some(left)
    }

    #[cfg(test)]
    fn render_frames(&mut self, frames: usize) -> Vec<f32> {
        (0..frames * 2).filter_map(|_| self.next_f32()).collect()
    }

    #[cfg(test)]
    fn finished(&self) -> bool {
        self.ended || self.frames >= self.max_frames
    }
}

#[cfg(test)]
pub(crate) fn test_rad_v2() -> Vec<u8> {
    let mut data = vec![0u8; 0x10];
    data.push(0x21);
    data.push(0x20 | 3); // BPM present, speed 3.
    data.extend_from_slice(&125u16.to_le_bytes());
    data.extend_from_slice(b"RAD Test Tune\x01A tiny v2 description\0");
    data.push(1); // instrument number
    data.push(4);
    data.extend_from_slice(b"Bell");
    data.push(2); // 4-op algorithm; RAD v2 channels key the first pair in this mode.
    data.push(0x22); // feedback
    data.push(0x01); // riff speed/detune
    data.push(63); // volume
    data.extend_from_slice(&[
        0x21, 0xF1, 0xF1, 0x06, 0x00, // modulator
        0x01, 0xD2, 0x72, 0x03, 0x00, // carrier
        0x01, 0xF1, 0xF1, 0x06, 0x00, 0x01, 0xD2, 0x72, 0x03, 0x00,
    ]);
    data.push(0); // end instruments
    data.push(1); // order count
    data.push(1); // play track 1
    let track = vec![
        0x00, // line 0
        0xF0, // last note, channel 0, note+instrument+effect fields
        0x31, // C-3
        0x01, // instrument 1
        0x0F, // set speed
        0x03, 0x84, // final line 4
        0x80, // channel 0 last empty note
    ];
    data.push(1); // track number
    data.extend_from_slice(&(track.len() as u16).to_le_bytes());
    data.extend_from_slice(&track);
    data
}

#[cfg(test)]
pub(crate) fn test_rad_v1() -> Vec<u8> {
    let mut data = vec![0u8; 0x10];
    data.push(0x10);
    data.push(0x80 | 3); // description present, speed 3.
    data.extend_from_slice(b"RAD v1 Test\0");
    data.push(1);
    data.extend_from_slice(&[0x21, 0x01, 0xF1, 0xD2, 0xF1, 0x72, 0x06, 0x03, 0x03, 0x00, 0x00]);
    data.push(0);
    data.push(1);
    data.push(0);
    let track_pos = data.len() + 64;
    data.extend(std::iter::repeat_n(0u8, 64));
    let table = data.len() - 64;
    data[table..table + 2].copy_from_slice(&(track_pos as u16).to_le_bytes());
    data.extend_from_slice(&[
        0x00, 0x80, // channel 0, last
        0x31, // C-3
        0x10, // instrument 1, no effect
        0x84, 0x80, 0x00, 0x00,
    ]);
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v2_metadata_and_audio() {
        let tune = RadTune::load(&test_rad_v2()).unwrap();
        assert_eq!(tune.format_name(), "Reality AdLib Tracker RAD v2");
        assert_eq!(tune.channels(), 18);
        assert_eq!(tune.title(), b"RAD Test Tune");
        assert_eq!(tune.instruments(), vec![b"Bell".to_vec()]);
        assert!(tune.duration() > 0.0);
        let mut renderer = tune.renderer(DEFAULT_RATE).unwrap();
        let pcm = renderer.render_frames(DEFAULT_RATE as usize / 2);
        let peak = pcm.iter().fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        assert!(peak > 0.0001, "peak {peak}, samples {}", pcm.len());
        while renderer.next_f32().is_some() {}
        assert!(renderer.finished());
    }

    #[test]
    fn parses_v1_metadata_and_audio() {
        let tune = RadTune::load(&test_rad_v1()).unwrap();
        assert_eq!(tune.format_name(), "Reality AdLib Tracker RAD v1");
        assert_eq!(tune.channels(), 9);
        assert_eq!(tune.title(), b"RAD v1 Test");
        let mut renderer = tune.renderer(22_050).unwrap();
        let pcm = renderer.render_frames(22_050 / 2);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.001));
    }

    #[test]
    fn expands_description_control_codes() {
        assert_eq!(expand_description(b"\"Tune\"\x01\x01by\x03me\0junk"), b"\"Tune\"\n\nby   me");
    }

    #[test]
    fn rejects_garbage_and_truncated_rad() {
        assert!(RadTune::load(b"garbage").is_err());
        let mut truncated = vec![0u8; 0x11];
        truncated[0x10] = 0x21;
        assert!(RadTune::load(&truncated).is_err());
    }
}

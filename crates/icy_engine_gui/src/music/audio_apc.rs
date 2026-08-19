//! SyncTERM audio APC (`ESC _ SyncTERM:A;... ESC \`) parsing and playback.
//!
//! Graphics doors such as SyncDoom probe for digital audio with
//! `SyncTERM:Q;libsndfile`, then upload samples and drive a small mixer through
//! the `SyncTERM:A;` verb family. This module owns the wire format, the patch
//! slots and the per-channel players.

use std::{
    io::Cursor,
    num::NonZero,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use once_cell::sync::Lazy;
use rodio::{buffer::SamplesBuffer, mixer::Mixer, Player, Source};

/// Patch slots addressable by `S=`.
pub const PATCH_SLOTS: usize = 256;
/// Mixer channels addressable by `C=`.
pub const CHANNELS: usize = 16;
/// Everything is normalized to this rate, matching SyncTERM's `XPBEEP_SAMPLE_RATE`.
pub const SAMPLE_RATE: u32 = 44100;
/// Feature id reported for `SyncTERM:Q;libsndfile`.
pub const FEATURE_SNDFILE: u16 = 100;
/// Feature id reported for `SyncTERM:Q;libsndfileFormat`.
pub const FEATURE_SNDFILE_FORMAT: u16 = 101;

/// Refuse absurd uploads before they reach the decoder.
const MAX_BLOB_SIZE: usize = 32 * 1024 * 1024;
/// Cap on decoded/synthesized frames per slot (~60s of stereo audio).
const MAX_PATCH_FRAMES: usize = SAMPLE_RATE as usize * 60;

/// Base attenuation applied to APC channels, mirroring `AUDIO_APC_BASE_DB`.
const BASE_DB: f32 = -12.0;
/// SyncTERM's floor for a linear volume of 0%.
const MIN_DB: f32 = -60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveShape {
    Sine,
    Sawtooth,
    Square,
    SineHarmonic,
    SineSaw,
    SineSawChord,
    SineSawHarmonic,
    Silence,
}

impl WaveShape {
    /// Longest-prefix match, so `SIN` does not swallow `SINE_SAW`.
    fn parse(value: &str) -> Option<WaveShape> {
        const SHAPES: &[(&str, WaveShape)] = &[
            ("SINE_SAW_HARM", WaveShape::SineSawHarmonic),
            ("SINE_SAW_CHORD", WaveShape::SineSawChord),
            ("SINE_HARM", WaveShape::SineHarmonic),
            ("SINE_SAW", WaveShape::SineSaw),
            ("SILENCE", WaveShape::Silence),
            ("SAW", WaveShape::Sawtooth),
            ("SIN", WaveShape::Sine),
            ("SQ", WaveShape::Square),
        ];
        SHAPES.iter().find(|(name, _)| *name == value).map(|(_, shape)| *shape)
    }

    fn sample(self, phase: f32) -> f32 {
        let tau = std::f32::consts::TAU;
        match self {
            WaveShape::Silence => 0.0,
            WaveShape::Sine => (phase * tau).sin(),
            WaveShape::Sawtooth => 2.0 * phase - 1.0,
            WaveShape::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            WaveShape::SineHarmonic => (0.5 * (phase * tau).sin() + 0.5 * (phase * tau * 2.0).sin()).clamp(-1.0, 1.0),
            WaveShape::SineSaw => (0.5 * (phase * tau).sin() + 0.5 * (2.0 * phase - 1.0)).clamp(-1.0, 1.0),
            WaveShape::SineSawChord => {
                let third = (phase * tau * 1.25).sin();
                let fifth = (phase * tau * 1.5).sin();
                (0.4 * (phase * tau).sin() + 0.3 * third + 0.3 * fifth).clamp(-1.0, 1.0)
            }
            WaveShape::SineSawHarmonic => {
                let harm = (phase * tau * 2.0).sin();
                (0.4 * (phase * tau).sin() + 0.3 * harm + 0.3 * (2.0 * phase - 1.0)).clamp(-1.0, 1.0)
            }
        }
    }
}

/// A parsed `SyncTERM:A;` command.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioApcCommand {
    /// Load a sample previously stored in the client cache via `SyncTERM:C;S`.
    Load {
        slot: u8,
        file: String,
    },
    LoadBlob {
        slot: u8,
        data: Vec<u8>,
    },
    Synth {
        slot: u8,
        shape: WaveShape,
        frequency: f32,
        frames: usize,
    },
    Copy {
        source: u8,
        destination: u8,
    },
    Queue {
        channel: u8,
        slot: u8,
        fade_in: usize,
        looping: bool,
        left_db: f32,
        right_db: f32,
    },
    Flush {
        channel: u8,
        fade_out: usize,
    },
    Volume {
        channel: u8,
        left_db: f32,
        right_db: f32,
    },
    /// Arm a one-shot `CSI = 7 ; <ch> ; 0 n` notification when the channel drains.
    Update {
        channel: u8,
    },
}

/// A parsed `SyncTERM:Q;` capability query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFeatureQuery {
    Sndfile,
    SndfileFormat { major: u32, subtype: u32 },
}

/// Live per-channel activity, shared between the sound thread and the connection.
///
/// Playback runs on the sound thread while the DSR replies and drain
/// notifications are emitted by the terminal thread, so the state is a bitmask
/// rather than a lock.
#[derive(Debug, Default)]
pub struct AudioApcStatus {
    active: AtomicU32,
}

impl AudioApcStatus {
    pub fn set_active(&self, channel: u8, active: bool) {
        if channel as usize >= CHANNELS {
            return;
        }
        let bit = 1u32 << channel;
        if active {
            self.active.fetch_or(bit, Ordering::Relaxed);
        } else {
            self.active.fetch_and(!bit, Ordering::Relaxed);
        }
    }

    pub fn is_active(&self, channel: u8) -> bool {
        (channel as usize) < CHANNELS && self.active.load(Ordering::Relaxed) & (1u32 << channel) != 0
    }

    pub fn active_mask(&self) -> u32 {
        self.active.load(Ordering::Relaxed)
    }

    pub fn clear(&self) {
        self.active.store(0, Ordering::Relaxed);
    }
}

static STATUS: Lazy<Arc<AudioApcStatus>> = Lazy::new(|| Arc::new(AudioApcStatus::default()));

/// The process-wide channel state. SyncTERM keeps this in file-scope statics too,
/// and there is exactly one sound thread per process.
pub fn status() -> Arc<AudioApcStatus> {
    STATUS.clone()
}

/// Parses `<pct>` (linear 0..100) or `<n>dB`, returning dB. Matches `apc_parse_volume`.
fn parse_volume(value: &str) -> f32 {
    let trimmed = value.trim();
    let digits = trimmed.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let Ok(parsed) = digits.parse::<f32>() else {
        return 0.0;
    };
    if trimmed[digits.len()..].eq_ignore_ascii_case("db") {
        return parsed;
    }
    if parsed <= 0.0 {
        return MIN_DB;
    }
    20.0 * (parsed.min(100.0) / 100.0).log10()
}

/// Parses `<n>` / `<n>ms` (milliseconds), `<n>f` (frames) or `<n>p` (periods).
fn parse_duration(value: &str, frequency: f32) -> usize {
    let trimmed = value.trim();
    let digits_len = trimmed.find(|c: char| !c.is_ascii_digit()).unwrap_or(trimmed.len());
    let Ok(count) = trimmed[..digits_len].parse::<u64>() else {
        return 0;
    };
    let rate = u64::from(SAMPLE_RATE);
    match &trimmed[digits_len..] {
        "" | "ms" => (count.saturating_mul(rate) / 1000) as usize,
        "f" => count as usize,
        "p" => {
            if frequency <= 0.0 {
                0
            } else {
                (count as f32 * SAMPLE_RATE as f32 / frequency + 0.5) as usize
            }
        }
        _ => 0,
    }
}

fn parse_slot(value: &str) -> Option<u8> {
    value.parse::<u16>().ok().filter(|slot| (*slot as usize) < PATCH_SLOTS).map(|slot| slot as u8)
}

fn parse_channel(value: &str) -> Option<u8> {
    value.parse::<u16>().ok().filter(|ch| (*ch as usize) < CHANNELS).map(|ch| ch as u8)
}

/// Splits the argument list into `key=value` pairs, stopping at the first
/// unrecognized token. SyncTERM treats the remainder as positional data.
struct Args<'a> {
    rest: &'a str,
}

impl<'a> Args<'a> {
    fn new(rest: &'a str) -> Self {
        Args { rest }
    }

    /// Consumes the next `;`-delimited token if it is a recognized key.
    fn next_pair(&mut self, known: &[&str]) -> Option<(&'a str, &'a str)> {
        let rest = self.rest.strip_prefix(';')?;
        let token_end = rest.find(';').unwrap_or(rest.len());
        let token = &rest[..token_end];
        let (key, value) = match token.split_once('=') {
            Some((key, value)) => (key, value),
            None => (token, ""),
        };
        if !known.contains(&key) {
            return None;
        }
        self.rest = &rest[token_end..];
        Some((key, value))
    }

    /// The unparsed remainder, minus a single leading separator.
    fn tail(&self) -> &'a str {
        self.rest.strip_prefix(';').unwrap_or(self.rest)
    }
}

/// Parses the payload of an `ESC _ SyncTERM:A;... ESC \` APC.
///
/// `payload` is the whole APC body including the `SyncTERM:A;` prefix.
pub fn parse_audio_apc(payload: &str) -> Option<AudioApcCommand> {
    let body = payload.strip_prefix("SyncTERM:A;")?;
    let verb_end = body.find(';').unwrap_or(body.len());
    let (verb, rest) = body.split_at(verb_end);

    match verb {
        "Load" => {
            let mut args = Args::new(rest);
            let mut slot = None;
            while let Some((key, value)) = args.next_pair(&["S"]) {
                if key == "S" {
                    slot = parse_slot(value);
                }
            }
            let file = args.tail();
            if file.is_empty() {
                return None;
            }
            Some(AudioApcCommand::Load {
                slot: slot?,
                file: file.to_string(),
            })
        }
        "LoadBlob" => {
            let mut args = Args::new(rest);
            let mut slot = None;
            while let Some((key, value)) = args.next_pair(&["S"]) {
                if key == "S" {
                    slot = parse_slot(value);
                }
            }
            let encoded = args.tail();
            if encoded.is_empty() || encoded.len() > MAX_BLOB_SIZE {
                return None;
            }
            use base64::{engine::general_purpose, Engine as _};
            let data = general_purpose::STANDARD.decode(encoded).ok()?;
            Some(AudioApcCommand::LoadBlob { slot: slot?, data })
        }
        "Synth" => {
            let mut args = Args::new(rest);
            let mut slot = None;
            let mut shape = None;
            let mut frequency = 0.0f32;
            let mut duration = "";
            while let Some((key, value)) = args.next_pair(&["S", "W", "F", "T"]) {
                match key {
                    "S" => slot = parse_slot(value),
                    "W" => shape = WaveShape::parse(value),
                    "F" => frequency = value.parse().unwrap_or(0.0),
                    "T" => duration = value,
                    _ => {}
                }
            }
            let shape = shape?;
            let frequency = if shape == WaveShape::Silence { 0.0 } else { frequency };
            let frames = parse_duration(duration, frequency).min(MAX_PATCH_FRAMES);
            if frames == 0 {
                return None;
            }
            Some(AudioApcCommand::Synth {
                slot: slot?,
                shape,
                frequency,
                frames,
            })
        }
        "Copy" => {
            let mut args = Args::new(rest);
            let mut source = None;
            let mut destination = None;
            while let Some((key, value)) = args.next_pair(&["S", "D"]) {
                match key {
                    "S" => source = parse_slot(value),
                    "D" => destination = parse_slot(value),
                    _ => {}
                }
            }
            Some(AudioApcCommand::Copy {
                source: source?,
                destination: destination?,
            })
        }
        "Queue" => {
            let mut args = Args::new(rest);
            let mut channel = None;
            let mut slot = None;
            let mut fade_in = 0;
            let mut looping = false;
            let mut left_db = 0.0f32;
            let mut right_db = 0.0f32;
            while let Some((key, value)) = args.next_pair(&["C", "S", "I", "O", "X", "L", "V", "VL", "VR"]) {
                match key {
                    "C" => channel = parse_channel(value),
                    "S" => slot = parse_slot(value),
                    "I" => fade_in = parse_duration(value, 0.0),
                    "L" => looping = true,
                    "V" => {
                        left_db = parse_volume(value);
                        right_db = left_db;
                    }
                    "VL" => left_db = parse_volume(value),
                    "VR" => right_db = parse_volume(value),
                    _ => {}
                }
            }
            Some(AudioApcCommand::Queue {
                channel: channel?,
                slot: slot?,
                fade_in,
                looping,
                left_db,
                right_db,
            })
        }
        "Flush" => {
            let mut args = Args::new(rest);
            let mut channel = None;
            let mut fade_out = 0;
            while let Some((key, value)) = args.next_pair(&["C", "O"]) {
                match key {
                    "C" => channel = parse_channel(value),
                    "O" => fade_out = parse_duration(value, 0.0),
                    _ => {}
                }
            }
            Some(AudioApcCommand::Flush { channel: channel?, fade_out })
        }
        "Volume" => {
            let mut args = Args::new(rest);
            let mut channel = None;
            let mut left_db = None;
            let mut right_db = None;
            while let Some((key, value)) = args.next_pair(&["C", "V", "VL", "VR", "T"]) {
                match key {
                    "C" => channel = parse_channel(value),
                    "V" => {
                        let db = parse_volume(value);
                        left_db = Some(db);
                        right_db = Some(db);
                    }
                    "VL" => left_db = Some(parse_volume(value)),
                    "VR" => right_db = Some(parse_volume(value)),
                    _ => {}
                }
            }
            Some(AudioApcCommand::Volume {
                channel: channel?,
                left_db: left_db.unwrap_or(0.0),
                right_db: right_db.unwrap_or(0.0),
            })
        }
        "Update" => {
            let mut args = Args::new(rest);
            let mut channel = None;
            while let Some((key, value)) = args.next_pair(&["C"]) {
                if key == "C" {
                    channel = parse_channel(value);
                }
            }
            Some(AudioApcCommand::Update { channel: channel? })
        }
        // `Wait` blocks SyncTERM's render loop until a channel drains; doors use it
        // only for ordering, so honoring it would stall the terminal for no gain.
        "Wait" => None,
        _ => None,
    }
}

/// Parses the payload of an `ESC _ SyncTERM:Q;... ESC \` capability query.
pub fn parse_feature_query(payload: &str) -> Option<AudioFeatureQuery> {
    let body = payload.strip_prefix("SyncTERM:Q;")?;
    if body == "libsndfile" {
        return Some(AudioFeatureQuery::Sndfile);
    }
    let formats = body.strip_prefix("libsndfileFormat;")?;
    let (major, subtype) = formats.split_once(';')?;
    Some(AudioFeatureQuery::SndfileFormat {
        major: major.parse().ok()?,
        subtype: subtype.parse().ok()?,
    })
}

/// libsndfile major format codes (`SF_FORMAT_TYPEMASK >> 16`) we can decode.
///
/// The decoder is symphonia via rodio; it covers WAV/AIFF-style PCM, FLAC and
/// Ogg Vorbis but has no Opus support, so `OGG;OPUS` must answer "no".
pub fn supports_format(major: u32, subtype: u32) -> bool {
    const SF_FORMAT_WAV: u32 = 0x01;
    const SF_FORMAT_AIFF: u32 = 0x02;
    const SF_FORMAT_FLAC: u32 = 0x17;
    const SF_FORMAT_OGG: u32 = 0x20;
    const SF_FORMAT_VORBIS: u32 = 0x60;
    const SF_FORMAT_OPUS: u32 = 0x64;

    match major {
        SF_FORMAT_WAV | SF_FORMAT_AIFF | SF_FORMAT_FLAC => true,
        SF_FORMAT_OGG => subtype == SF_FORMAT_VORBIS && subtype != SF_FORMAT_OPUS,
        _ => false,
    }
}

/// A decoded sample, stored stereo-interleaved at [`SAMPLE_RATE`].
#[derive(Default, Clone)]
struct Patch {
    frames: Vec<f32>,
}

/// Patch slots plus the per-channel players, owned by the sound thread.
pub struct AudioApcState {
    patches: Vec<Patch>,
    players: Vec<Option<Player>>,
    volumes: Vec<(f32, f32)>,
    status: Arc<AudioApcStatus>,
}

impl Default for AudioApcState {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioApcState {
    pub fn new() -> Self {
        AudioApcState {
            patches: vec![Patch::default(); PATCH_SLOTS],
            players: (0..CHANNELS).map(|_| None).collect(),
            volumes: vec![(0.0, 0.0); CHANNELS],
            status: status(),
        }
    }

    /// Drops every player, e.g. when the output device is rebuilt.
    pub fn reset(&mut self) {
        for player in &mut self.players {
            if let Some(player) = player.take() {
                player.stop();
            }
        }
        self.status.clear();
    }

    /// Publishes which channels still have audio pending.
    pub fn refresh_status(&self) {
        for (index, player) in self.players.iter().enumerate() {
            let active = player.as_ref().is_some_and(|player| !player.empty());
            self.status.set_active(index as u8, active);
        }
    }

    fn player(&mut self, mixer: &Mixer, channel: u8) -> Option<&Player> {
        let index = channel as usize;
        if index >= CHANNELS {
            return None;
        }
        if self.players[index].is_none() {
            self.players[index] = Some(Player::connect_new(mixer));
        }
        self.players[index].as_ref()
    }

    fn store(&mut self, slot: u8, frames: Vec<f32>) {
        if frames.is_empty() {
            return;
        }
        self.patches[slot as usize] = Patch { frames };
    }

    /// Decodes an audio file into stereo f32 frames at [`SAMPLE_RATE`].
    fn decode(data: Vec<u8>) -> Option<Vec<f32>> {
        if data.is_empty() || data.len() > MAX_BLOB_SIZE {
            return None;
        }
        let decoder = match rodio::Decoder::new(Cursor::new(data)) {
            Ok(decoder) => decoder,
            Err(err) => {
                log::warn!("Audio APC: cannot decode sample: {err}");
                return None;
            }
        };
        let channels = decoder.channels();
        let rate = decoder.sample_rate();
        let resampled = rodio::conversions::SampleRateConverter::new(decoder, rate, NonZero::new(SAMPLE_RATE)?, channels);

        let mut frames = Vec::new();
        let limit = MAX_PATCH_FRAMES * 2;
        match channels.get() {
            1 => {
                for sample in resampled {
                    if frames.len() >= limit {
                        break;
                    }
                    frames.push(sample);
                    frames.push(sample);
                }
            }
            2 => {
                for sample in resampled {
                    if frames.len() >= limit {
                        break;
                    }
                    frames.push(sample);
                }
            }
            other => {
                // Downmix anything exotic to stereo by averaging each frame.
                let mut buffer = Vec::with_capacity(other as usize);
                for sample in resampled {
                    buffer.push(sample);
                    if buffer.len() == other as usize {
                        let average = buffer.iter().sum::<f32>() / other as f32;
                        frames.push(average);
                        frames.push(average);
                        buffer.clear();
                    }
                    if frames.len() >= limit {
                        break;
                    }
                }
            }
        }
        if frames.len() % 2 == 1 {
            frames.push(0.0);
        }
        (!frames.is_empty()).then_some(frames)
    }

    fn synth(shape: WaveShape, frequency: f32, frames: usize) -> Vec<f32> {
        let mut samples = Vec::with_capacity(frames * 2);
        for index in 0..frames {
            let value = if frequency <= 0.0 {
                0.0
            } else {
                let phase = (index as f32 * frequency / SAMPLE_RATE as f32).fract();
                shape.sample(phase)
            };
            samples.push(value);
            samples.push(value);
        }
        samples
    }

    /// Applies a command. `cache_directory` resolves `Load` file names.
    pub fn handle(&mut self, mixer: Option<&Mixer>, cache_directory: Option<&std::path::Path>, command: AudioApcCommand) {
        match command {
            AudioApcCommand::Load { slot, file } => {
                let Some(directory) = cache_directory else { return };
                let Some(path) = safe_cache_path(directory, &file) else {
                    log::warn!("Audio APC: rejected cache path {file:?}");
                    return;
                };
                let Ok(data) = std::fs::read(path) else { return };
                if let Some(frames) = Self::decode(data) {
                    self.store(slot, frames);
                }
            }
            AudioApcCommand::LoadBlob { slot, data } => {
                if let Some(frames) = Self::decode(data) {
                    self.store(slot, frames);
                }
            }
            AudioApcCommand::Synth {
                slot,
                shape,
                frequency,
                frames,
            } => {
                let samples = Self::synth(shape, frequency, frames);
                self.store(slot, samples);
            }
            AudioApcCommand::Copy { source, destination } => {
                let frames = self.patches[source as usize].frames.clone();
                self.store(destination, frames);
            }
            AudioApcCommand::Queue {
                channel,
                slot,
                fade_in,
                looping,
                left_db,
                right_db,
            } => {
                let frames = std::mem::take(&mut self.patches[slot as usize].frames);
                if frames.is_empty() {
                    return;
                }
                let Some(mixer) = mixer else { return };
                let left = db_to_gain(left_db + BASE_DB);
                let right = db_to_gain(right_db + BASE_DB);
                let panned = apply_pan(frames, left, right);
                let Some(player) = self.player(mixer, channel) else { return };

                let buffer = SamplesBuffer::new(stereo(), sample_rate(), panned);
                let fade = Duration::from_secs_f32(fade_in as f32 / SAMPLE_RATE as f32);
                match (looping, fade_in > 0) {
                    (true, true) => player.append(buffer.repeat_infinite().fade_in(fade)),
                    (true, false) => player.append(buffer.repeat_infinite()),
                    (false, true) => player.append(buffer.fade_in(fade)),
                    (false, false) => player.append(buffer),
                }
                self.status.set_active(channel, true);
            }
            AudioApcCommand::Flush { channel, fade_out: _ } => {
                // rodio has no tail fade on clear; doors use Flush to reclaim a
                // channel before the next one-shot, so stopping promptly matters more.
                if let Some(Some(player)) = self.players.get(channel as usize) {
                    player.clear();
                    player.play();
                }
                self.status.set_active(channel, false);
            }
            AudioApcCommand::Volume { channel, left_db, right_db } => {
                let Some(mixer) = mixer else { return };
                self.volumes[channel as usize] = (left_db, right_db);
                let gain = db_to_gain(left_db.max(right_db) + BASE_DB);
                if let Some(player) = self.player(mixer, channel) {
                    player.set_volume(gain);
                }
            }
            AudioApcCommand::Update { .. } => {}
        }
    }
}

fn stereo() -> rodio::ChannelCount {
    NonZero::new(2u16).expect("2 is non-zero")
}

fn sample_rate() -> rodio::SampleRate {
    NonZero::new(SAMPLE_RATE).expect("sample rate is non-zero")
}

fn db_to_gain(db: f32) -> f32 {
    if db <= MIN_DB {
        0.0
    } else {
        10.0f32.powf(db / 20.0)
    }
}

fn apply_pan(mut frames: Vec<f32>, left: f32, right: f32) -> Vec<f32> {
    for pair in frames.chunks_exact_mut(2) {
        pair[0] *= left;
        pair[1] *= right;
    }
    frames
}

/// Resolves a cache-relative file name, rejecting absolute paths and traversal.
fn safe_cache_path(directory: &std::path::Path, file: &str) -> Option<std::path::PathBuf> {
    if file.is_empty() || file.len() > 128 {
        return None;
    }
    let mut path = directory.to_path_buf();
    for component in file.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return None;
        }
        if component.contains('\\') || component.contains('\0') {
            return None;
        }
        path.push(component);
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_syncdoom_sfx_sequence() {
        assert_eq!(
            parse_audio_apc("SyncTERM:A;Flush;C=4"),
            Some(AudioApcCommand::Flush { channel: 4, fade_out: 0 })
        );
        assert_eq!(
            parse_audio_apc("SyncTERM:A;Load;S=7;doom/sfx/12"),
            Some(AudioApcCommand::Load {
                slot: 7,
                file: "doom/sfx/12".to_string()
            })
        );
        let Some(AudioApcCommand::Queue {
            channel,
            slot,
            left_db,
            right_db,
            looping,
            ..
        }) = parse_audio_apc("SyncTERM:A;Queue;C=4;S=7;VL=-3.0dB;VR=-9.0dB")
        else {
            panic!("expected a Queue command");
        };
        assert_eq!((channel, slot, looping), (4, 7, false));
        assert!((left_db - -3.0).abs() < f32::EPSILON);
        assert!((right_db - -9.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parses_loop_flag_and_linear_volume() {
        let Some(AudioApcCommand::Queue { looping, left_db, .. }) = parse_audio_apc("SyncTERM:A;Queue;C=2;S=1;V=50;L") else {
            panic!("expected a Queue command");
        };
        assert!(looping);
        // 50% linear is about -6 dB.
        assert!((left_db - -6.0206).abs() < 0.01);
    }

    #[test]
    fn parses_synth_shapes_and_durations() {
        let Some(AudioApcCommand::Synth { shape, frequency, frames, .. }) = parse_audio_apc("SyncTERM:A;Synth;S=3;W=SINE_SAW;F=440;T=100ms") else {
            panic!("expected a Synth command");
        };
        assert_eq!(shape, WaveShape::SineSaw);
        assert!((frequency - 440.0).abs() < f32::EPSILON);
        assert_eq!(frames, 4410);

        // `SIN` must not swallow the longer names.
        let Some(AudioApcCommand::Synth { shape, .. }) = parse_audio_apc("SyncTERM:A;Synth;S=0;W=SIN;F=100;T=10f") else {
            panic!("expected a Synth command");
        };
        assert_eq!(shape, WaveShape::Sine);

        // Frame and period units.
        let Some(AudioApcCommand::Synth { frames, .. }) = parse_audio_apc("SyncTERM:A;Synth;S=0;W=SQ;F=441;T=2p") else {
            panic!("expected a Synth command");
        };
        assert_eq!(frames, 200);
    }

    #[test]
    fn silence_forces_zero_frequency() {
        let Some(AudioApcCommand::Synth { frequency, shape, .. }) = parse_audio_apc("SyncTERM:A;Synth;S=0;W=SILENCE;F=440;T=5f") else {
            panic!("expected a Synth command");
        };
        assert_eq!(shape, WaveShape::Silence);
        assert_eq!(frequency, 0.0);
    }

    #[test]
    fn rejects_out_of_range_slots_and_channels() {
        assert!(parse_audio_apc("SyncTERM:A;Queue;C=16;S=0").is_none());
        assert!(parse_audio_apc("SyncTERM:A;Load;S=256;file").is_none());
        assert!(parse_audio_apc("SyncTERM:A;Load;S=0;").is_none());
        assert!(parse_audio_apc("SyncTERM:C;DrawJXL;a.jxl").is_none());
        assert!(parse_audio_apc("SyncTERM:A;Bogus;C=1").is_none());
    }

    #[test]
    fn load_blob_decodes_base64() {
        use base64::{engine::general_purpose, Engine as _};
        let encoded = general_purpose::STANDARD.encode(b"RIFFbogus");
        let Some(AudioApcCommand::LoadBlob { slot, data }) = parse_audio_apc(&format!("SyncTERM:A;LoadBlob;S=9;{encoded}")) else {
            panic!("expected a LoadBlob command");
        };
        assert_eq!(slot, 9);
        assert_eq!(data, b"RIFFbogus");
    }

    #[test]
    fn parses_capability_queries() {
        assert_eq!(parse_feature_query("SyncTERM:Q;libsndfile"), Some(AudioFeatureQuery::Sndfile));
        assert_eq!(
            parse_feature_query("SyncTERM:Q;libsndfileFormat;32;100"),
            Some(AudioFeatureQuery::SndfileFormat { major: 32, subtype: 100 })
        );
        assert_eq!(parse_feature_query("SyncTERM:Q;JXL"), None);
    }

    #[test]
    fn reports_opus_as_unsupported_but_wav_as_supported() {
        // Ogg/Opus - symphonia has no Opus decoder.
        assert!(!supports_format(32, 100));
        // Ogg/Vorbis and plain WAV.
        assert!(supports_format(32, 0x60));
        assert!(supports_format(1, 2));
    }

    #[test]
    fn rejects_cache_path_traversal() {
        let root = std::path::Path::new("/tmp/cache");
        assert!(safe_cache_path(root, "../escape").is_none());
        assert!(safe_cache_path(root, "/etc/passwd").is_none());
        assert!(safe_cache_path(root, "sfx/12").is_some());
    }

    #[test]
    fn status_tracks_channels_independently() {
        let status = AudioApcStatus::default();
        status.set_active(3, true);
        status.set_active(9, true);
        assert!(status.is_active(3));
        assert!(!status.is_active(4));
        status.set_active(3, false);
        assert!(!status.is_active(3));
        assert!(status.is_active(9));
        // Out-of-range writes must not corrupt the mask.
        status.set_active(200, true);
        assert_eq!(status.active_mask(), 1 << 9);
    }
}

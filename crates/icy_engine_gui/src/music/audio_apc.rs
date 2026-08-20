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
        mpsc::{self, Receiver, SyncSender, TrySendError},
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
/// Cap on decoded/synthesized frames per slot. Music tracks are minutes long,
/// and SyncTERM materializes them whole rather than streaming.
const MAX_PATCH_FRAMES: usize = SAMPLE_RATE as usize * 300;
/// Ceiling on all resident patches together. SyncTERM has no such bound, but a
/// terminal should not let a remote system decide how much memory it uses.
const MAX_TOTAL_PATCH_BYTES: usize = 192 * 1024 * 1024;

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
/// Symphonia covers WAV/AIFF-style PCM, FLAC and Ogg Vorbis; Ogg-Opus is decoded
/// through libopus and so depends on the `opus-audio` feature.
pub fn supports_format(major: u32, subtype: u32) -> bool {
    const SF_FORMAT_WAV: u32 = 0x01;
    const SF_FORMAT_AIFF: u32 = 0x02;
    const SF_FORMAT_FLAC: u32 = 0x17;
    const SF_FORMAT_OGG: u32 = 0x20;
    const SF_FORMAT_VORBIS: u32 = 0x60;
    const SF_FORMAT_OPUS: u32 = 0x64;

    match major {
        SF_FORMAT_WAV | SF_FORMAT_AIFF | SF_FORMAT_FLAC => true,
        SF_FORMAT_OGG => matches!(subtype, SF_FORMAT_VORBIS) || (subtype == SF_FORMAT_OPUS && cfg!(feature = "opus-audio")),
        _ => false,
    }
}

/// Interleaves an arbitrary channel count into stereo, capped at [`MAX_PATCH_FRAMES`].
fn to_stereo(samples: impl Iterator<Item = f32>, channels: u16) -> Vec<f32> {
    let limit = MAX_PATCH_FRAMES * 2;
    let mut frames = Vec::new();
    match channels {
        1 => {
            for sample in samples {
                if frames.len() >= limit {
                    break;
                }
                frames.push(sample);
                frames.push(sample);
            }
        }
        2 => {
            for sample in samples {
                if frames.len() >= limit {
                    break;
                }
                frames.push(sample);
            }
        }
        other => {
            // Downmix anything exotic to stereo by averaging each frame.
            let mut buffer = Vec::with_capacity(other as usize);
            for sample in samples {
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
    frames
}

/// An Ogg stream whose first logical page carries an Opus identification header.
fn is_ogg_opus(data: &[u8]) -> bool {
    if !data.starts_with(b"OggS") {
        return false;
    }
    let head = &data[..data.len().min(512)];
    head.windows(8).any(|window| window == b"OpusHead")
}

/// Decodes Ogg-Opus, the music codec of the SyncTERM audio APC.
///
/// Symphonia demuxes the Ogg container but has no Opus decoder, so the packets
/// go through libopus and the result is resampled to [`SAMPLE_RATE`].
#[cfg(feature = "opus-audio")]
fn decode_ogg_opus(data: &[u8]) -> Option<Vec<f32>> {
    use symphonia::core::codecs::CODEC_TYPE_OPUS;
    use symphonia::core::formats::FormatReader;
    use symphonia::core::io::MediaSourceStream;

    /// Opus decodes at a fixed 48 kHz regardless of the source rate.
    const OPUS_RATE: u32 = 48_000;
    /// Longest Opus frame is 120 ms.
    const MAX_FRAME_SAMPLES: usize = 5760;

    let source = MediaSourceStream::new(Box::new(Cursor::new(data.to_vec())), Default::default());
    let mut reader = match symphonia::default::formats::OggReader::try_new(source, &Default::default()) {
        Ok(reader) => reader,
        Err(err) => {
            log::warn!("Audio APC: Ogg demux failed: {err}");
            return None;
        }
    };
    let Some(track) = reader.tracks().iter().find(|track| track.codec_params.codec == CODEC_TYPE_OPUS) else {
        log::warn!("Audio APC: Ogg stream contains no Opus track");
        return None;
    };
    let track_id = track.id;
    let Some(track_channels) = track.codec_params.channels else {
        log::warn!("Audio APC: Opus track has no channel layout");
        return None;
    };
    let channels = track_channels.count().min(2) as u16;
    let mut pre_skip = track.codec_params.delay.unwrap_or(0) as usize;

    let mode = if channels >= 2 { opus::Channels::Stereo } else { opus::Channels::Mono };
    let mut decoder = match opus::Decoder::new(OPUS_RATE, mode) {
        Ok(decoder) => decoder,
        Err(err) => {
            log::warn!("Audio APC: libopus decoder creation failed: {err}");
            return None;
        }
    };
    let mut scratch = vec![0f32; MAX_FRAME_SAMPLES * channels as usize];

    let mut decoded: Vec<f32> = Vec::new();
    let limit = (MAX_PATCH_FRAMES as u64 * u64::from(OPUS_RATE) / u64::from(SAMPLE_RATE)) as usize * channels as usize;
    while let Ok(packet) = reader.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let count = match decoder.decode_float(packet.buf(), &mut scratch, false) {
            Ok(count) => count,
            Err(err) => {
                log::warn!("Audio APC: Opus packet dropped: {err}");
                continue;
            }
        };
        let mut produced = &scratch[..count * channels as usize];
        if pre_skip > 0 {
            // The encoder's priming samples are not part of the music.
            let skip = pre_skip.min(count);
            pre_skip -= skip;
            produced = &produced[skip * channels as usize..];
        }
        decoded.extend_from_slice(produced);
        if decoded.len() >= limit {
            break;
        }
    }
    if decoded.is_empty() {
        log::warn!("Audio APC: Opus stream produced no samples");
        return None;
    }

    let channel_count = NonZero::new(channels)?;
    let resampled = rodio::conversions::SampleRateConverter::new(decoded.into_iter(), NonZero::new(OPUS_RATE)?, NonZero::new(SAMPLE_RATE)?, channel_count);
    let frames = to_stereo(resampled, channels);
    (!frames.is_empty()).then_some(frames)
}

#[cfg(not(feature = "opus-audio"))]
fn decode_ogg_opus(_data: &[u8]) -> Option<Vec<f32>> {
    log::warn!("Audio APC: built without Ogg-Opus support");
    None
}

/// A decoded sample, stored stereo-interleaved at [`SAMPLE_RATE`].
#[derive(Default, Clone)]
struct Patch {
    frames: Vec<f32>,
}

enum DecodeInput {
    File(std::path::PathBuf),
    Blob(Vec<u8>),
}

struct DecodeJob {
    slot: u8,
    generation: u64,
    input: DecodeInput,
}

struct DecodeResult {
    slot: u8,
    generation: u64,
    frames: Option<Vec<f32>>,
}

fn start_decode_worker() -> (SyncSender<DecodeJob>, Receiver<DecodeResult>) {
    let (job_tx, job_rx) = mpsc::sync_channel::<DecodeJob>(8);
    let (result_tx, result_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("audio-apc-decoder".to_string())
        .spawn(move || {
            while let Ok(job) = job_rx.recv() {
                let data = match job.input {
                    DecodeInput::File(path) => std::fs::read(path).ok(),
                    DecodeInput::Blob(data) => Some(data),
                };
                let frames = data.and_then(AudioApcState::decode);
                if result_tx
                    .send(DecodeResult {
                        slot: job.slot,
                        generation: job.generation,
                        frames,
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .expect("failed to start Audio APC decoder");
    (job_tx, result_rx)
}

#[derive(Default)]
struct StereoGainControl {
    left: AtomicU32,
    right: AtomicU32,
}

impl StereoGainControl {
    fn new(left: f32, right: f32) -> Self {
        Self {
            left: AtomicU32::new(left.to_bits()),
            right: AtomicU32::new(right.to_bits()),
        }
    }

    fn set(&self, left: f32, right: f32) {
        self.left.store(left.to_bits(), Ordering::Relaxed);
        self.right.store(right.to_bits(), Ordering::Relaxed);
    }
}

struct StereoGain<S> {
    input: S,
    control: Arc<StereoGainControl>,
    channel: usize,
}

impl<S> StereoGain<S> {
    fn new(input: S, control: Arc<StereoGainControl>) -> Self {
        Self { input, control, channel: 0 }
    }
}

impl<S: Source> Iterator for StereoGain<S> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.input.next()?;
        let gain = if self.channel == 0 {
            f32::from_bits(self.control.left.load(Ordering::Relaxed))
        } else {
            f32::from_bits(self.control.right.load(Ordering::Relaxed))
        };
        self.channel = (self.channel + 1) % self.input.channels().get() as usize;
        Some(sample * gain)
    }
}

impl<S: Source> Source for StereoGain<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.input.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.input.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.input.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }
}

/// Patch slots plus the per-channel players, owned by the sound thread.
pub struct AudioApcState {
    patches: Vec<Patch>,
    players: Vec<Option<Player>>,
    volumes: Vec<Arc<StereoGainControl>>,
    status: Arc<AudioApcStatus>,
    generations: Vec<u64>,
    pending: Vec<bool>,
    deferred: Vec<Vec<AudioApcCommand>>,
    decode_tx: SyncSender<DecodeJob>,
    decode_rx: Receiver<DecodeResult>,
}

impl Default for AudioApcState {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioApcState {
    pub fn new() -> Self {
        let (decode_tx, decode_rx) = start_decode_worker();
        AudioApcState {
            patches: vec![Patch::default(); PATCH_SLOTS],
            players: (0..CHANNELS).map(|_| None).collect(),
            volumes: (0..CHANNELS).map(|_| Arc::new(StereoGainControl::new(1.0, 1.0))).collect(),
            status: status(),
            generations: vec![0; PATCH_SLOTS],
            pending: vec![false; PATCH_SLOTS],
            deferred: vec![Vec::new(); PATCH_SLOTS],
            decode_tx,
            decode_rx,
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

    pub fn poll(&mut self, mixer: Option<&Mixer>) {
        while let Ok(result) = self.decode_rx.try_recv() {
            let slot = result.slot as usize;
            if self.generations[slot] != result.generation {
                continue;
            }
            self.pending[slot] = false;
            if let Some(frames) = result.frames {
                self.store(result.slot, frames);
            }
            let commands = std::mem::take(&mut self.deferred[slot]);
            for command in commands {
                self.handle(mixer, None, command);
            }
        }
        self.refresh_status();
    }

    fn submit_decode(&mut self, slot: u8, input: DecodeInput) {
        let index = slot as usize;
        self.generations[index] = self.generations[index].wrapping_add(1);
        self.pending[index] = true;
        self.patches[index].frames.clear();
        self.deferred[index].clear();
        let job = DecodeJob {
            slot,
            generation: self.generations[index],
            input,
        };
        if let Err(error) = self.decode_tx.try_send(job) {
            self.pending[index] = false;
            self.deferred[index].clear();
            match error {
                TrySendError::Full(_) => log::warn!("Audio APC: decoder queue full, dropping slot {slot}"),
                TrySendError::Disconnected(_) => log::warn!("Audio APC: decoder unavailable, dropping slot {slot}"),
            }
        }
    }

    fn player(&mut self, mixer: &Mixer, channel: u8) -> Option<&Player> {
        let index = channel as usize;
        if index >= CHANNELS {
            return None;
        }
        if self.players[index].is_none() {
            let player = Player::connect_new(mixer);
            player.set_volume(db_to_gain(BASE_DB));
            self.players[index] = Some(player);
        }
        self.players[index].as_ref()
    }

    fn store(&mut self, slot: u8, frames: Vec<f32>) {
        if frames.is_empty() {
            return;
        }
        // Replacing the slot frees its old buffer, so only the others count.
        let resident: usize = self
            .patches
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != slot as usize)
            .map(|(_, patch)| patch.frames.len() * std::mem::size_of::<f32>())
            .sum();
        if resident + frames.len() * std::mem::size_of::<f32>() > MAX_TOTAL_PATCH_BYTES {
            log::warn!("Audio APC: patch memory budget exhausted, dropping slot {slot}");
            return;
        }
        self.patches[slot as usize] = Patch { frames };
    }

    /// Decodes an audio file into stereo f32 frames at [`SAMPLE_RATE`].
    fn decode(data: Vec<u8>) -> Option<Vec<f32>> {
        if data.is_empty() || data.len() > MAX_BLOB_SIZE {
            return None;
        }
        if is_ogg_opus(&data) {
            return decode_ogg_opus(&data);
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
        Some(to_stereo(resampled, channels.get()))
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
        self.poll(mixer);
        match command {
            AudioApcCommand::Load { slot, file } => {
                let Some(directory) = cache_directory else {
                    log::warn!("Audio APC: cannot load {file:?}: no cache directory");
                    return;
                };
                let Some(path) = safe_cache_path(directory, &file) else {
                    log::warn!("Audio APC: rejected cache path {file:?}");
                    return;
                };
                self.submit_decode(slot, DecodeInput::File(path));
            }
            AudioApcCommand::LoadBlob { slot, data } => {
                self.submit_decode(slot, DecodeInput::Blob(data));
            }
            AudioApcCommand::Synth {
                slot,
                shape,
                frequency,
                frames,
            } => {
                let index = slot as usize;
                self.generations[index] = self.generations[index].wrapping_add(1);
                self.pending[index] = false;
                self.deferred[index].clear();
                let samples = Self::synth(shape, frequency, frames);
                self.store(slot, samples);
            }
            AudioApcCommand::Copy { source, destination } => {
                if self.pending[source as usize] {
                    self.deferred[source as usize].push(AudioApcCommand::Copy { source, destination });
                    return;
                }
                let destination_index = destination as usize;
                self.generations[destination_index] = self.generations[destination_index].wrapping_add(1);
                self.pending[destination_index] = false;
                self.deferred[destination_index].clear();
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
                if self.pending[slot as usize] {
                    self.deferred[slot as usize].push(AudioApcCommand::Queue {
                        channel,
                        slot,
                        fade_in,
                        looping,
                        left_db,
                        right_db,
                    });
                    return;
                }
                let frames = std::mem::take(&mut self.patches[slot as usize].frames);
                if frames.is_empty() {
                    return;
                }
                let Some(mixer) = mixer else {
                    log::warn!("Audio APC: cannot queue channel {channel}: no audio mixer");
                    return;
                };
                let left = db_to_gain(left_db);
                let right = db_to_gain(right_db);
                let panned = apply_pan(frames, left, right);
                let volume = self.volumes[channel as usize].clone();
                let Some(player) = self.player(mixer, channel) else { return };

                let buffer = SamplesBuffer::new(stereo(), sample_rate(), panned);
                let fade = Duration::from_secs_f32(fade_in as f32 / SAMPLE_RATE as f32);
                match (looping, fade_in > 0) {
                    (true, true) => player.append(StereoGain::new(buffer.repeat_infinite().fade_in(fade), volume)),
                    (true, false) => player.append(StereoGain::new(buffer.repeat_infinite(), volume)),
                    (false, true) => player.append(StereoGain::new(buffer.fade_in(fade), volume)),
                    (false, false) => player.append(StereoGain::new(buffer, volume)),
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
                self.volumes[channel as usize].set(db_to_gain(left_db), db_to_gain(right_db));
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
    #[ignore = "set ICY_AUDIO_TEST_FILE to exercise a real music asset"]
    fn decodes_external_music_fixture() {
        let path = std::env::var("ICY_AUDIO_TEST_FILE").expect("ICY_AUDIO_TEST_FILE is required");
        let data = std::fs::read(&path).expect("fixture should be readable");
        let started = std::time::Instant::now();
        let decoded = AudioApcState::decode(data).expect("fixture should decode");
        let peak = decoded.iter().fold(0.0f32, |peak, sample| peak.max(sample.abs()));

        eprintln!("decoded {} stereo frames in {:?}, peak={peak}", decoded.len() / 2, started.elapsed());
        assert!(!decoded.is_empty());
        assert!(peak > 0.01, "decoded fixture was silent");

        let path = std::path::PathBuf::from(path);
        let cache_directory = path.parent().unwrap();
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let (mixer, _source) = rodio::mixer::mixer(stereo(), sample_rate());
        let mut state = AudioApcState::new();
        state.handle(Some(&mixer), Some(cache_directory), AudioApcCommand::Load { slot: 0, file });
        state.handle(
            Some(&mixer),
            Some(cache_directory),
            AudioApcCommand::Queue {
                channel: 0,
                slot: 0,
                fade_in: 0,
                looping: true,
                left_db: 0.0,
                right_db: 0.0,
            },
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !state.status.is_active(0) && std::time::Instant::now() < deadline {
            state.poll(Some(&mixer));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(state.status.is_active(0), "looping fixture never reached the player");
    }

    #[test]
    fn stereo_gain_preserves_channels_and_live_updates() {
        let control = Arc::new(StereoGainControl::new(1.0, 0.25));
        let buffer = SamplesBuffer::new(stereo(), sample_rate(), vec![1.0, 1.0, 0.5, 0.5]);
        let mut source = StereoGain::new(buffer, control.clone());

        assert_eq!(source.next(), Some(1.0));
        assert_eq!(source.next(), Some(0.25));
        control.set(0.0, 0.5);
        assert_eq!(source.next(), Some(0.0));
        assert_eq!(source.next(), Some(0.25));
    }

    #[test]
    fn decibel_floor_mutes_without_affecting_other_channel() {
        let frames = apply_pan(vec![1.0, 1.0], db_to_gain(MIN_DB), db_to_gain(-6.0206));
        assert_eq!(frames[0], 0.0);
        assert!((frames[1] - 0.5).abs() < 0.0001);
        assert!((db_to_gain(BASE_DB) - 0.25118864).abs() < 0.0001);
    }

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
    fn reports_formats_matching_the_build() {
        // Ogg/Opus tracks the feature so the capability reply never overstates.
        assert_eq!(supports_format(32, 100), cfg!(feature = "opus-audio"));
        // Ogg/Vorbis and plain WAV.
        assert!(supports_format(32, 0x60));
        assert!(supports_format(1, 2));
        // Unknown container.
        assert!(!supports_format(0x99, 2));
    }

    #[test]
    fn detects_ogg_opus_streams() {
        let mut stream = b"OggS".to_vec();
        stream.extend_from_slice(&[0u8; 22]);
        stream.extend_from_slice(b"OpusHead");
        assert!(is_ogg_opus(&stream));

        // Ogg Vorbis must keep going through the symphonia path.
        let mut vorbis = b"OggS".to_vec();
        vorbis.extend_from_slice(&[0u8; 22]);
        vorbis.extend_from_slice(b"\x01vorbis");
        assert!(!is_ogg_opus(&vorbis));

        assert!(!is_ogg_opus(b"RIFFxxxxWAVE"));
        assert!(!is_ogg_opus(b""));
    }

    #[test]
    fn downmixes_and_caps_channel_layouts() {
        assert_eq!(to_stereo([0.5f32, -0.5].into_iter(), 1), vec![0.5, 0.5, -0.5, -0.5]);
        assert_eq!(to_stereo([0.25f32, 0.75].into_iter(), 2), vec![0.25, 0.75]);
        // Four channels average down to one stereo frame.
        assert_eq!(to_stereo([1.0f32, 0.0, 1.0, 0.0].into_iter(), 4), vec![0.5, 0.5]);
        // A trailing half frame is padded rather than dropped.
        assert_eq!(to_stereo([0.5f32].into_iter(), 2), vec![0.5, 0.0]);
    }

    #[cfg(feature = "opus-audio")]
    #[test]
    fn decodes_ogg_opus_round_trip() {
        // Build a real Ogg-Opus stream so the demux, pre-skip and resample path
        // are all exercised rather than mocked.
        const RATE: u32 = 48_000;
        const PRE_SKIP: u16 = 312;
        const FRAME: usize = 960; // 20 ms
        let serial: u32 = 0x1234_5678;

        let mut head = b"OpusHead".to_vec();
        head.push(1);
        head.push(1);
        head.extend_from_slice(&PRE_SKIP.to_le_bytes());
        head.extend_from_slice(&RATE.to_le_bytes());
        head.extend_from_slice(&0i16.to_le_bytes());
        head.push(0);

        let mut tags = b"OpusTags".to_vec();
        tags.extend_from_slice(&4u32.to_le_bytes());
        tags.extend_from_slice(b"icy0");
        tags.extend_from_slice(&0u32.to_le_bytes());

        let mut encoder = opus::Encoder::new(RATE, opus::Channels::Mono, opus::Application::Audio).unwrap();
        let frames = 50; // one second
        let mut packets = Vec::new();
        for index in 0..frames {
            let pcm: Vec<f32> = (0..FRAME)
                .map(|n| {
                    let t = (index * FRAME + n) as f32 / RATE as f32;
                    (t * 440.0 * std::f32::consts::TAU).sin() * 0.5
                })
                .collect();
            let mut encoded = vec![0u8; 4000];
            let len = encoder.encode_float(&pcm, &mut encoded).unwrap();
            encoded.truncate(len);
            packets.push(encoded);
        }

        let mut stream = Vec::new();
        stream.extend_from_slice(&ogg_page(serial, 0, 0x02, 0, &head));
        stream.extend_from_slice(&ogg_page(serial, 1, 0x00, 0, &tags));
        for (index, packet) in packets.iter().enumerate() {
            let granule = ((index + 1) * FRAME) as u64;
            let last = index + 1 == packets.len();
            let header = if last { 0x04 } else { 0x00 };
            stream.extend_from_slice(&ogg_page(serial, index as u32 + 2, header, granule, packet));
        }

        assert!(is_ogg_opus(&stream));
        let decoded = decode_ogg_opus(&stream).expect("opus stream should decode");

        // One second at 48 kHz, minus pre-skip, resampled to 44.1 kHz and stereo.
        let expected = (FRAME * frames - PRE_SKIP as usize) as f32 / RATE as f32 * SAMPLE_RATE as f32;
        let got = (decoded.len() / 2) as f32;
        assert!((got - expected).abs() < 200.0, "expected about {expected} frames, got {got}");
        assert_eq!(decoded.len() % 2, 0);
        // A 440 Hz tone must not decode to silence.
        let peak = decoded.iter().fold(0f32, |peak, sample| peak.max(sample.abs()));
        assert!(peak > 0.1, "decoded tone was silent (peak {peak})");
    }

    #[cfg(feature = "opus-audio")]
    fn ogg_page(serial: u32, sequence: u32, header_type: u8, granule: u64, packet: &[u8]) -> Vec<u8> {
        let mut segments = Vec::new();
        let mut remaining = packet.len();
        while remaining >= 255 {
            segments.push(255u8);
            remaining -= 255;
        }
        segments.push(remaining as u8);

        let mut page = Vec::new();
        page.extend_from_slice(b"OggS");
        page.push(0);
        page.push(header_type);
        page.extend_from_slice(&granule.to_le_bytes());
        page.extend_from_slice(&serial.to_le_bytes());
        page.extend_from_slice(&sequence.to_le_bytes());
        page.extend_from_slice(&0u32.to_le_bytes()); // checksum placeholder
        page.push(segments.len() as u8);
        page.extend_from_slice(&segments);
        page.extend_from_slice(packet);

        let checksum = ogg_crc(&page);
        page[22..26].copy_from_slice(&checksum.to_le_bytes());
        page
    }

    #[cfg(feature = "opus-audio")]
    fn ogg_crc(data: &[u8]) -> u32 {
        // Ogg uses a non-reflected CRC-32 with polynomial 0x04c11db7.
        let mut crc: u32 = 0;
        for &byte in data {
            crc ^= u32::from(byte) << 24;
            for _ in 0..8 {
                crc = if crc & 0x8000_0000 != 0 { (crc << 1) ^ 0x04c1_1db7 } else { crc << 1 };
            }
        }
        crc
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

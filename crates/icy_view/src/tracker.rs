//! Tracker modules (MOD/S3M/XM/IT): an info sheet for the preview and thumbnails, and a
//! streaming player (xmrsplayer rendered on a worker thread, played through rodio).
//!
//! Names and song messages are read straight from the file because they are CP437 and
//! often carry ASCII art; xmrs decodes them as lossy UTF-8.

use std::{
    num::NonZero,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

use icy_engine::{AttributedChar, Position, TextAttribute, TextBuffer, TextPane};
use parking_lot::Mutex;
use xmrs::{core::module::Module, tracker::format::ModuleFormat};
use xmrsplayer::xmrsplayer::XmrsPlayer;

pub const EXTENSIONS: &[&str] = &["mod", "s3m", "xm", "it"];

const TITLE: u8 = 14;
const TEXT: u8 = 7;
const DIM: u8 = 8;
const WIDTH: i32 = 80;

fn extension(path: &Path) -> Option<String> {
    path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase())
}

pub fn is_tracker_file(path: &Path) -> bool {
    extension(path).is_some_and(|ext| EXTENSIONS.contains(&ext.as_str()))
}

/// The MOD importer accepts nearly anything, so the extension picks the importer and only
/// unknown extensions fall back to content detection.
pub fn load_module(path: &Path, data: &[u8]) -> anyhow::Result<Module> {
    let result = match extension(path).as_deref() {
        Some("mod") => Module::load_mod(data),
        Some("s3m") => Module::load_s3m(data),
        Some("xm") => Module::load_xm(data),
        Some("it") => Module::load_it(data),
        _ => Module::load(data),
    };
    result.map_err(|error| anyhow::anyhow!("not a playable tracker module: {error:?}"))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModuleInfo {
    pub title: Vec<u8>,
    pub format: &'static str,
    pub tracker: Vec<u8>,
    pub channels: usize,
    pub tempo: usize,
    pub bpm: usize,
    pub duration: f64,
    pub instruments: Vec<Vec<u8>>,
    pub samples: Vec<Vec<u8>>,
    pub message: Vec<Vec<u8>>,
}

impl ModuleInfo {
    pub fn new(module: &Module, data: &[u8]) -> Self {
        let mut info = raw_info(module.origin.unwrap_or_default(), data).unwrap_or_else(|| ModuleInfo {
            title: module.name.as_bytes().to_vec(),
            instruments: module.instrument.iter().map(|instrument| instrument.name.as_bytes().to_vec()).collect(),
            message: module.comment.lines().map(|line| line.as_bytes().to_vec()).collect(),
            ..Default::default()
        });
        info.format = format_name(module.origin.unwrap_or_default());
        if info.channels == 0 {
            info.channels = module.get_num_channels();
        }
        info.tempo = module.default_tempo;
        info.bpm = module.default_bpm;
        info.duration = XmrsPlayer::new(module, 48_000, 0).duration_seconds();
        for names in [&mut info.instruments, &mut info.samples, &mut info.message] {
            while names.last().is_some_and(|name| name.iter().all(|b| *b == b' ')) {
                names.pop();
            }
        }
        info
    }

    pub fn title(&self) -> String {
        self.title
            .iter()
            .map(|b| icy_engine::BufferType::CP437.convert_to_unicode(char::from(*b)))
            .collect()
    }
}

fn format_name(format: ModuleFormat) -> &'static str {
    match format {
        ModuleFormat::Mod => "ProTracker MOD",
        ModuleFormat::S3m => "Scream Tracker 3",
        ModuleFormat::Xm => "FastTracker II",
        ModuleFormat::It => "Impulse Tracker",
        _ => "Tracker module",
    }
}

fn bytes(data: &[u8], start: usize, len: usize) -> Option<&[u8]> {
    data.get(start..start.checked_add(len)?)
}

fn u16_at(data: &[u8], at: usize) -> Option<usize> {
    bytes(data, at, 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
}

fn u32_at(data: &[u8], at: usize) -> Option<usize> {
    bytes(data, at, 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

/// Fixed-size name field: NULs become spaces, trailing blanks are dropped.
fn name(data: &[u8], start: usize, len: usize) -> Vec<u8> {
    let mut name: Vec<u8> = bytes(data, start, len)
        .unwrap_or_default()
        .iter()
        .map(|b| if *b == 0 { b' ' } else { *b })
        .collect();
    while name.last() == Some(&b' ') {
        name.pop();
    }
    name
}

fn raw_info(format: ModuleFormat, data: &[u8]) -> Option<ModuleInfo> {
    match format {
        ModuleFormat::Mod => mod_info(data),
        ModuleFormat::S3m => s3m_info(data),
        ModuleFormat::Xm => xm_info(data),
        ModuleFormat::It => it_info(data),
        _ => None,
    }
}

fn mod_info(data: &[u8]) -> Option<ModuleInfo> {
    // 31-sample modules carry a printable format tag ("M.K.", "8CHN", …) at 1080.
    let tag = bytes(data, 1080, 4)?;
    let count = if tag.iter().all(|b| (0x20..0x7F).contains(b)) { 31 } else { 15 };
    let digits = |text: &[u8]| std::str::from_utf8(text).ok().and_then(|text| text.parse::<usize>().ok());
    let channels = match tag {
        b"FLT8" | b"OCTA" | b"OKTA" | b"CD81" => 8,
        [n, b'C', b'H', b'N'] => digits(&[*n]).unwrap_or(4),
        [a, b, b'C', b'H'] | [a, b, b'C', b'N'] => digits(&[*a, *b]).unwrap_or(4),
        _ => 4,
    };
    Some(ModuleInfo {
        title: name(data, 0, 20),
        channels,
        samples: (0..count).map(|index| name(data, 20 + index * 30, 22)).collect(),
        ..Default::default()
    })
}

fn s3m_info(data: &[u8]) -> Option<ModuleInfo> {
    if bytes(data, 0x2C, 4)? != b"SCRM" {
        return None;
    }
    let (orders, instruments) = (u16_at(data, 0x20)?, u16_at(data, 0x22)?);
    let samples = (0..instruments)
        .map(|index| u16_at(data, 0x60 + orders + index * 2).map(|pointer| name(data, pointer * 16 + 0x30, 28)))
        .collect::<Option<_>>()?;
    let channels = bytes(data, 0x40, 32)?.iter().filter(|setting| **setting < 16).count();
    Some(ModuleInfo {
        title: name(data, 0, 28),
        channels,
        samples,
        ..Default::default()
    })
}

fn xm_info(data: &[u8]) -> Option<ModuleInfo> {
    if bytes(data, 0, 17)? != b"Extended Module: " {
        return None;
    }
    let (patterns, instrument_count) = (u16_at(data, 70)?, u16_at(data, 72)?);
    let mut at = 60 + u32_at(data, 60)?;
    for _ in 0..patterns {
        at += u32_at(data, at)? + u16_at(data, at + 7)?;
    }
    let (mut instruments, mut samples) = (Vec::new(), Vec::new());
    for _ in 0..instrument_count {
        let header = u32_at(data, at)?;
        instruments.push(name(data, at + 4, 22));
        let count = u16_at(data, at + 27)?;
        let sample_header = if count > 0 { u32_at(data, at + 29)? } else { 0 };
        at += header;
        let mut sample_data = 0;
        for _ in 0..count {
            sample_data += u32_at(data, at)?;
            samples.push(name(data, at + 18, 22));
            at += sample_header;
        }
        at += sample_data;
    }
    Some(ModuleInfo {
        title: name(data, 17, 20),
        tracker: name(data, 38, 20),
        channels: u16_at(data, 68)?,
        instruments,
        samples,
        ..Default::default()
    })
}

fn it_info(data: &[u8]) -> Option<ModuleInfo> {
    if bytes(data, 0, 4)? != b"IMPM" {
        return None;
    }
    let (orders, instruments, samples) = (u16_at(data, 0x20)?, u16_at(data, 0x22)?, u16_at(data, 0x24)?);
    let pointers = 0xC0 + orders;
    let names = |first: usize, count: usize, offset: usize| -> Option<Vec<Vec<u8>>> {
        (0..count)
            .map(|index| u32_at(data, first + index * 4).map(|pointer| name(data, pointer + offset, 26)))
            .collect()
    };
    let mut message = Vec::new();
    if u16_at(data, 0x2E)? & 1 != 0 {
        let text = bytes(data, u32_at(data, 0x38)?, u16_at(data, 0x36)?).unwrap_or_default();
        let text = text.split(|b| *b == 0).next().unwrap_or_default();
        message = text
            .split(|b| *b == b'\r' || *b == b'\n')
            .map(|line| line.iter().map(|b| if *b < 0x20 { b' ' } else { *b }).collect())
            .collect();
    }
    Some(ModuleInfo {
        title: name(data, 4, 26),
        instruments: names(pointers, instruments, 0x20)?,
        samples: names(pointers + instruments * 4, samples, 0x14)?,
        message,
        ..Default::default()
    })
}

pub fn format_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn write_bytes(buffer: &mut TextBuffer, x: i32, y: i32, text: &[u8], color: u8) {
    for (offset, b) in text.iter().enumerate() {
        let position = Position::new(x + offset as i32, y);
        if position.x < buffer.width() {
            buffer.layers[0].set_char(position, AttributedChar::new(char::from(*b), TextAttribute::from_color(color, 0)));
        }
    }
}

/// Title, format line, song message and the numbered instrument/sample lists.
pub fn render_info(info: &ModuleInfo) -> TextBuffer {
    let cp437 = |text: &str| -> Vec<u8> { text.chars().map(|ch| icy_engine::BufferType::CP437.convert_from_unicode(ch) as u8).collect() };
    let mut details = vec![
        info.format.to_string(),
        format!("{} channels", info.channels),
        format!("Speed {} · {} BPM", info.tempo, info.bpm),
        format_time(info.duration),
    ];
    if !info.tracker.is_empty() {
        details.push(String::from_utf8_lossy(&info.tracker).into_owned());
    }
    // Each line is a list of (column, text, colour) segments.
    let mut lines: Vec<Vec<(i32, Vec<u8>, u8)>> = vec![vec![(1, info.title.clone(), TITLE)], vec![(1, cp437(&details.join(" · ")), TEXT)]];
    let sections: [(&str, &[Vec<u8>], bool); 3] = [
        ("Message", &info.message, false),
        ("Instruments", &info.instruments, true),
        ("Samples", &info.samples, true),
    ];
    for (heading, names, numbered) in sections {
        if names.is_empty() {
            continue;
        }
        lines.push(Vec::new());
        lines.push(vec![(1, heading.as_bytes().to_vec(), DIM)]);
        for (index, name) in names.iter().enumerate() {
            lines.push(if numbered {
                vec![(1, format!("{:02}", index + 1).into_bytes(), DIM), (5, name.clone(), TEXT)]
            } else {
                vec![(1, name.clone(), TEXT)]
            });
        }
    }

    let mut buffer = TextBuffer::new((WIDTH, (lines.len() as i32 + 1).max(25)));
    for (y, segments) in lines.iter().enumerate() {
        for (x, text, color) in segments {
            write_bytes(&mut buffer, *x, y as i32, text, *color);
        }
    }
    buffer
}

/// Parses the module and renders its info sheet (preview fallback and thumbnails).
pub fn render(path: &Path, data: &[u8]) -> anyhow::Result<TextBuffer> {
    let module = load_module(path, data)?;
    Ok(render_info(&ModuleInfo::new(&module, data)))
}

const CHUNK_FRAMES: usize = 2048;
const QUEUE_CHUNKS: usize = 4;

#[derive(Default)]
struct Shared {
    generation: AtomicU64,
    base_seconds: AtomicU64,
    played_frames: AtomicU64,
    sample_rate: AtomicU32,
    paused: AtomicBool,
    finished: AtomicBool,
    error: Mutex<Option<String>>,
}

enum Command {
    Seek(f64),
}

struct Chunk {
    generation: u64,
    samples: Vec<f32>,
    last: bool,
}

/// Plays a module on the default output device until dropped.
pub struct TrackerPlayer {
    commands: mpsc::Sender<Command>,
    shared: Arc<Shared>,
    duration: f64,
}

impl TrackerPlayer {
    pub fn start(module: Module, paused: bool) -> Self {
        let (player, commands) = Self::new(&module);
        player.set_paused(paused);
        let shared = player.shared.clone();
        let spawned = std::thread::Builder::new().name("tracker-audio".into()).spawn(move || {
            // The device sink is not Send, so it lives on this thread.
            let mut sink = match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(sink) => sink,
                Err(error) => {
                    *shared.error.lock() = Some(error.to_string());
                    shared.finished.store(true, Ordering::Relaxed);
                    return;
                }
            };
            sink.log_on_drop(false);
            let rate = sink.config().sample_rate().get();
            let (sender, receiver) = mpsc::sync_channel(QUEUE_CHUNKS);
            sink.mixer().add(StreamSource::new(receiver, shared.clone(), rate));
            synthesize(&module, rate, &shared, &commands, sender);
        });
        if let Err(error) = spawned {
            *player.shared.error.lock() = Some(error.to_string());
        }
        player
    }

    /// No output device (tests, audio off): knows the duration but never advances.
    pub fn silent(module: &Module, paused: bool) -> Self {
        let player = Self::new(module).0;
        player.set_paused(paused);
        player
    }

    fn new(module: &Module) -> (Self, mpsc::Receiver<Command>) {
        let (commands, receiver) = mpsc::channel();
        let player = Self {
            commands,
            shared: Arc::default(),
            duration: XmrsPlayer::new(module, 48_000, 0).duration_seconds(),
        };
        (player, receiver)
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn position(&self) -> f64 {
        let base = f64::from_bits(self.shared.base_seconds.load(Ordering::Relaxed));
        let rate = self.shared.sample_rate.load(Ordering::Relaxed);
        let played = if rate == 0 {
            0.0
        } else {
            self.shared.played_frames.load(Ordering::Relaxed) as f64 / rate as f64
        };
        (base + played).min(self.duration)
    }

    pub fn paused(&self) -> bool {
        self.shared.paused.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Relaxed);
    }

    pub fn finished(&self) -> bool {
        self.shared.finished.load(Ordering::Relaxed)
    }

    pub fn playing(&self) -> bool {
        !self.paused() && !self.finished()
    }

    /// Snaps to the start of the row containing `seconds`; also restarts a finished song.
    pub fn seek(&self, seconds: f64) {
        self.shared.finished.store(false, Ordering::Relaxed);
        let _ = self.commands.send(Command::Seek(seconds.clamp(0.0, self.duration)));
    }

    pub fn error(&self) -> Option<String> {
        self.shared.error.lock().clone()
    }
}

/// Renders chunks ahead of the output until the player is dropped; a seek bumps the
/// generation so the output discards everything rendered before it.
fn synthesize(module: &Module, rate: u32, shared: &Shared, commands: &mpsc::Receiver<Command>, output: mpsc::SyncSender<Chunk>) {
    let mut player = XmrsPlayer::new(module, rate, 0);
    player.set_max_loop_count(1);
    shared.sample_rate.store(rate, Ordering::Relaxed);
    let mut generation = shared.generation.load(Ordering::Relaxed);
    let mut pending: Option<Chunk> = None;
    let mut ended = false;
    loop {
        loop {
            match commands.try_recv() {
                Ok(Command::Seek(seconds)) => {
                    player.seek_seconds(seconds);
                    generation += 1;
                    shared.generation.store(generation, Ordering::Relaxed);
                    shared.base_seconds.store(player.position_seconds().to_bits(), Ordering::Relaxed);
                    shared.played_frames.store(0, Ordering::Relaxed);
                    shared.finished.store(false, Ordering::Relaxed);
                    pending = None;
                    ended = false;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }
        if pending.is_none() && !ended {
            let mut samples = Vec::with_capacity(CHUNK_FRAMES * 2);
            while samples.len() < CHUNK_FRAMES * 2 {
                match player.next() {
                    Some(sample) => samples.push(sample as f32 / 32768.0),
                    None => {
                        ended = true;
                        break;
                    }
                }
            }
            samples.truncate(samples.len() & !1);
            pending = Some(Chunk {
                generation,
                samples,
                last: ended,
            });
        }
        match pending.take() {
            Some(chunk) => match output.try_send(chunk) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(chunk)) => {
                    pending = Some(chunk);
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(mpsc::TrySendError::Disconnected(_)) => return,
            },
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

/// Interleaved stereo fed from the synth thread; silence while paused or starved, so the
/// device stream keeps running and pausing never cuts a frame in half.
struct StreamSource {
    chunks: mpsc::Receiver<Chunk>,
    shared: Arc<Shared>,
    rate: rodio::SampleRate,
    generation: u64,
    samples: Vec<f32>,
    index: usize,
    last: bool,
    right: Option<f32>,
}

impl StreamSource {
    fn new(chunks: mpsc::Receiver<Chunk>, shared: Arc<Shared>, rate: u32) -> Self {
        Self {
            chunks,
            shared,
            rate: NonZero::new(rate).unwrap_or(NonZero::<u32>::MIN),
            generation: u64::MAX,
            samples: Vec::new(),
            index: 0,
            last: false,
            right: None,
        }
    }

    fn frame(&mut self) -> Option<(f32, f32)> {
        if self.shared.paused.load(Ordering::Relaxed) {
            return Some((0.0, 0.0));
        }
        loop {
            let current = self.generation == self.shared.generation.load(Ordering::Relaxed);
            if current && self.index + 1 < self.samples.len() {
                let frame = (self.samples[self.index], self.samples[self.index + 1]);
                self.index += 2;
                self.shared.played_frames.fetch_add(1, Ordering::Relaxed);
                return Some(frame);
            }
            if current && self.last {
                self.last = false;
                self.shared.finished.store(true, Ordering::Relaxed);
            }
            match self.chunks.try_recv() {
                Ok(chunk) => {
                    self.generation = chunk.generation;
                    self.samples = chunk.samples;
                    self.index = 0;
                    self.last = chunk.last;
                }
                Err(mpsc::TryRecvError::Empty) => return Some((0.0, 0.0)),
                Err(mpsc::TryRecvError::Disconnected) => return None,
            }
        }
    }
}

impl Iterator for StreamSource {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<rodio::Sample> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        let (left, right) = self.frame()?;
        self.right = Some(right);
        Some(left)
    }
}

impl rodio::Source for StreamSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        NonZero::new(2).unwrap()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// A 4-channel ProTracker module: 1 looping square-wave sample, 2 orders of one pattern,
/// a note on every 16th row of channel 0. Sample 2's name carries CP437 art bytes.
#[doc(hidden)]
pub fn test_module() -> Vec<u8> {
    let mut data = vec![0u8; 1084];
    data[..9].copy_from_slice(b"test song");
    let sample = 20;
    data[sample..sample + 6].copy_from_slice(b"square");
    data[sample + 22..sample + 24].copy_from_slice(&16u16.to_be_bytes()); // length in words
    data[sample + 25] = 64; // volume
    data[sample + 28..sample + 30].copy_from_slice(&16u16.to_be_bytes()); // loop length
    let art = 20 + 30;
    data[art..art + 4].copy_from_slice(&[0xDB, 0xB2, 0xB1, 0xB0]);
    data[950] = 2; // song length
    data[951] = 127;
    data[1080..1084].copy_from_slice(b"M.K.");
    let mut pattern = vec![0u8; 1024];
    for row in (0..64).step_by(16) {
        let at = row * 16;
        pattern[at..at + 4].copy_from_slice(&[0x00, 0xD6, 0x10, 0x00]); // C-3 (period 214), sample 1
    }
    data.extend(pattern);
    data.extend((0..32).map(|i| if i < 16 { 0x60 } else { 0xA0 }));
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tracker_files_by_extension() {
        assert!(is_tracker_file(Path::new("SONG.MOD")));
        assert!(is_tracker_file(Path::new("a/b.it")));
        assert!(!is_tracker_file(Path::new("art.ans")));
        assert!(!is_tracker_file(Path::new("mod")));
    }

    #[test]
    fn info_reads_raw_names_and_song_details() {
        let data = test_module();
        let module = load_module(Path::new("test.mod"), &data).unwrap();
        let info = ModuleInfo::new(&module, &data);
        assert_eq!(info.title, b"test song");
        assert_eq!(info.format, "ProTracker MOD");
        assert_eq!(info.channels, 4);
        assert_eq!((info.tempo, info.bpm), (6, 125));
        assert_eq!(
            info.samples,
            vec![b"square".to_vec(), vec![0xDB, 0xB2, 0xB1, 0xB0]],
            "trailing empty slots are dropped"
        );
        // 2 orders × 64 rows × 6 ticks at 125 BPM (0.02 s per tick).
        assert!((info.duration - 15.36).abs() < 0.01, "duration {}", info.duration);

        let buffer = render_info(&info);
        let row = |y: i32| (0..WIDTH).map(|x| buffer.layers[0].char_at(Position::new(x, y)).ch).collect::<String>();
        assert!(row(0).starts_with(" test song"));
        assert!(
            row(1).contains("ProTracker MOD") && row(1).contains("4 channels") && row(1).contains("0:15"),
            "{}",
            row(1)
        );
        let art = (0..buffer.height()).find(|y| row(*y).starts_with(" 02")).expect("sample list");
        assert_eq!(
            row(art).chars().skip(5).take(4).map(|ch| ch as u32).collect::<Vec<_>>(),
            vec![0xDB, 0xB2, 0xB1, 0xB0]
        );
    }

    #[test]
    fn rejects_non_modules() {
        assert!(load_module(Path::new("x.xm"), b"not a module").is_err());
        assert!(load_module(Path::new("x.it"), &test_module()).is_err());
    }

    fn pull(source: &mut StreamSource, frames: usize) -> Vec<f32> {
        (0..frames * 2).map(|_| source.next().expect("stream ended")).collect()
    }

    #[test]
    fn stream_plays_pauses_seeks_and_finishes() {
        let data = test_module();
        let module = load_module(Path::new("test.mod"), &data).unwrap();
        let (player, commands) = TrackerPlayer::new(&module);
        let rate = 8_000;
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CHUNKS);
        let mut source = StreamSource::new(receiver, player.shared.clone(), rate);
        let shared = player.shared.clone();
        let synth = std::thread::spawn(move || synthesize(&module, rate, &shared, &commands, sender));
        let wait = |source: &mut StreamSource, done: &dyn Fn(&[f32]) -> bool| {
            for _ in 0..500 {
                if done(&pull(source, 256)) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            panic!("stream never reached the expected state");
        };

        wait(&mut source, &|samples| samples.iter().any(|s| s.abs() > 0.01));
        assert!(player.position() > 0.0);

        player.set_paused(true);
        let position = player.position();
        assert!(pull(&mut source, 1024).iter().all(|s| *s == 0.0), "paused output is silent");
        assert_eq!(player.position(), position, "pausing holds the position");
        player.set_paused(false);

        player.seek(14.0);
        wait(&mut source, &|_| player.position() >= 14.0);
        assert!(player.position() < 15.0, "seek lands near the target: {}", player.position());
        wait(&mut source, &|_| player.finished());
        assert!((player.position() - player.duration()).abs() < 0.2);

        player.seek(0.0);
        assert!(!player.finished(), "seeking restarts a finished song");
        wait(&mut source, &|_| player.position() > 0.0 && player.position() < 1.0);

        drop(player);
        synth.join().unwrap();
        assert!(std::iter::from_fn(|| source.next()).take(CHUNK_FRAMES * 2 * (QUEUE_CHUNKS + 2)).count() < CHUNK_FRAMES * 2 * (QUEUE_CHUNKS + 2));
    }
}

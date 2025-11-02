use std::{
    collections::{HashMap, VecDeque},
    sync::mpsc::{Receiver, SendError, Sender, TryRecvError, channel},
};

use once_cell::sync::Lazy;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;
#[cfg(target_arch = "wasm32")]
use wasm_thread as thread;

use icy_engine::ansi::sound::{AnsiMusic, MusicAction, MusicStyle};
use rodio::{
    Source,
    source::{Function, SignalGenerator, SineWave},
};
use web_time::{Duration, Instant};

use crate::TerminalResult;

use super::Rng;

// DTMF frequency pairs (low freq, high freq) for each key
static DTMF_FREQUENCIES: Lazy<HashMap<char, (f32, f32)>> = Lazy::new(|| {
    let mut m = HashMap::new();
    // Numeric keys
    m.insert('1', (697.0, 1209.0));
    m.insert('2', (697.0, 1336.0));
    m.insert('3', (697.0, 1477.0));
    m.insert('4', (770.0, 1209.0));
    m.insert('5', (770.0, 1336.0));
    m.insert('6', (770.0, 1477.0));
    m.insert('7', (852.0, 1209.0));
    m.insert('8', (852.0, 1336.0));
    m.insert('9', (852.0, 1477.0));
    m.insert('*', (941.0, 1209.0));
    m.insert('0', (941.0, 1336.0));
    m.insert('#', (941.0, 1477.0));

    // Letter keys (for phone systems that support them)
    m.insert('A', (697.0, 1633.0));
    m.insert('B', (770.0, 1633.0));
    m.insert('C', (852.0, 1633.0));
    m.insert('D', (941.0, 1633.0));
    m
});

/// Data that is sent to the connection thread
#[derive(Debug)]
pub enum SoundData {
    PlayMusic(AnsiMusic),
    Beep,
    Clear,
    LineSound,
    PlayDialSound(String),
    _PlayBusySound,

    StartPlay,
    StopPlay,
}

pub struct SoundThread {
    rx: Receiver<SoundData>,
    tx: Sender<SoundData>,
    is_playing: bool,
    rng: Rng,
    pub stop_button: u32,
    last_stop_cycle: Instant,
    restart_count: usize,
}

impl SoundThread {
    pub fn new() -> Self {
        let mut rng = Rng::default();
        let stop_button = rng.gen_range(0..6);
        let (tx, rx) = channel::<SoundData>();
        let mut res = SoundThread {
            rx,
            tx,
            is_playing: false,
            stop_button,
            rng,
            last_stop_cycle: Instant::now(),
            restart_count: 0,
        };
        #[cfg(not(target_arch = "wasm32"))]
        res.start_background_thread();
        res
    }

    pub(crate) fn clear(&self) {
        let _ = self.tx.send(SoundData::Clear);
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.is_playing
    }
}

impl SoundThread {
    pub fn update_state(&mut self) -> TerminalResult<()> {
        if self.no_thread_running() {
            return Ok(());
        }
        if self.last_stop_cycle.elapsed().as_secs() > 5 {
            self.stop_button = self.rng.gen_range(0..6);
            self.last_stop_cycle = Instant::now();
        }
        loop {
            match self.rx.try_recv() {
                Ok(data) => match data {
                    SoundData::StartPlay => self.is_playing = true,
                    SoundData::StopPlay => self.is_playing = false,
                    _ => {}
                },

                Err(err) => match err {
                    TryRecvError::Empty => break,
                    TryRecvError::Disconnected => {
                        self.restart_background_thread();
                        return Err(anyhow::anyhow!("rx.try_recv error: {err}").into());
                    }
                },
            }
        }
        Ok(())
    }
    pub(crate) fn beep(&mut self) -> TerminalResult<()> {
        self.send_data(SoundData::Beep)
    }

    pub fn stop_line_sound(&mut self) -> TerminalResult<()> {
        self.send_data(SoundData::StopPlay)
    }

    pub fn start_line_sound(&mut self) -> TerminalResult<()> {
        self.send_data(SoundData::LineSound)
    }

    pub fn start_dial_sound(&mut self, phone_number: &str) -> TerminalResult<()> {
        self.send_data(SoundData::PlayDialSound(phone_number.to_string()))
    }

    pub(crate) fn play_music(&mut self, music: AnsiMusic) -> TerminalResult<()> {
        self.send_data(SoundData::PlayMusic(music))
    }

    fn send_data(&mut self, data: SoundData) -> TerminalResult<()> {
        if self.no_thread_running() {
            // prevent error spew.
            return Ok(());
        }
        let res = self.tx.send(data);
        if let Err(SendError::<SoundData>(data)) = res {
            if self.restart_background_thread() {
                return self.send_data(data);
            }
            return Err(anyhow::anyhow!("Sound thread crashed too many times.").into());
        }
        Ok(())
    }

    fn start_background_thread(&mut self) {
        let (tx, rx) = channel::<SoundData>();
        let (tx2, rx2) = channel::<SoundData>();

        self.rx = rx2;
        self.tx = tx;
        let mut data = SoundBackgroundThreadData {
            rx,
            tx: tx2,
            music: VecDeque::new(),
            thread_is_running: true,
            last_beep: Instant::now(),
            line_sound_playing: false,
        };

        if let Err(err) = std::thread::Builder::new().name("music_thread".to_string()).spawn(move || {
            while data.thread_is_running {
                data.handle_queue();
                data.handle_receive();
                if data.music.is_empty() {
                    thread::sleep(Duration::from_millis(100));
                }
            }
            log::error!("communication thread closed because it lost connection with the ui thread.");
        }) {
            log::error!("Error in starting music thread: {}", err);
        }
    }
    fn no_thread_running(&self) -> bool {
        self.restart_count > 3
    }

    fn restart_background_thread(&mut self) -> bool {
        if self.no_thread_running() {
            log::error!("sound thread crashed too many times, exiting.");
            return false;
        }
        self.restart_count += 1;
        log::error!("sound thread crashed, restarting.");
        self.start_background_thread();
        true
    }
}

pub struct SoundBackgroundThreadData {
    tx: Sender<SoundData>,
    rx: Receiver<SoundData>,
    thread_is_running: bool,

    music: VecDeque<SoundData>,
    last_beep: Instant,

    line_sound_playing: bool,
}

impl SoundBackgroundThreadData {
    pub fn handle_receive(&mut self) -> bool {
        let mut result = false;
        loop {
            match self.rx.try_recv() {
                Ok(data) => {
                    println!("Sound thread received: {:?}", data);
                    match data {
                        SoundData::PlayMusic(m) => {
                            self.line_sound_playing = false;
                            self.music.push_back(SoundData::PlayMusic(m));
                        }
                        SoundData::Beep => {
                            self.line_sound_playing = false;
                            self.music.push_back(SoundData::Beep);
                        }
                        SoundData::LineSound => {
                            self.music.push_back(SoundData::LineSound);
                        }
                        SoundData::PlayDialSound(phone_number) => {
                            self.music.push_back(SoundData::PlayDialSound(phone_number));
                        }
                        SoundData::_PlayBusySound => {
                            self.line_sound_playing = false;
                            self.music.push_back(SoundData::_PlayBusySound);
                        }
                        SoundData::StopPlay => {
                            self.line_sound_playing = false;
                            result = true;
                        }
                        SoundData::Clear => {
                            result = true;
                            self.music.clear();
                            self.line_sound_playing = false;
                        }
                        _ => {}
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.line_sound_playing = false;
                    self.thread_is_running = false;
                    break;
                }
            }
        }
        result
    }

    fn handle_queue(&mut self) {
        // Check if we're playing line sound continuously
        if self.line_sound_playing {
            // Just return to keep the sound going, checking for stop messages
            thread::sleep(Duration::from_millis(10));
            return;
        }

        let Some(data) = self.music.pop_front() else {
            return;
        };
        match data {
            SoundData::PlayMusic(music) => self.play_music(&music),
            SoundData::Beep => self.beep(),
            SoundData::LineSound => self.play_line_sound(),
            SoundData::PlayDialSound(phone_number) => {
                if self.play_dial_sound(&phone_number) {
                    self.play_busysound();
                }
            }
            SoundData::_PlayBusySound => self.play_busysound(),
            _ => {}
        }
    }

    fn play_busysound(&mut self) {
        let _ = self.tx.send(SoundData::StartPlay);

        let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();
        let sample_rate = stream_handle.config().sample_rate();

        // North American busy signal: 480 Hz + 620 Hz
        // Pattern: 500ms on, 500ms off
        const BUSY_TONE_ON_MS: u64 = 500;
        const BUSY_TONE_OFF_MS: u64 = 500;
        const BUSY_CYCLES: usize = 8; // Play 8 cycles (8 seconds total)

        for _ in 0..BUSY_CYCLES {
            // Check for stop signal
            if self.handle_receive() {
                break;
            }

            // Generate the two tones for busy signal
            let tone1 = SignalGenerator::new(sample_rate, 480.0, Function::Sine).amplify(0.15);
            let tone2 = SignalGenerator::new(sample_rate, 620.0, Function::Sine).amplify(0.15);

            // Mix them together and play for the "on" duration
            let busy_tone = tone1.mix(tone2).take_duration(Duration::from_millis(BUSY_TONE_ON_MS));

            // Play the tone
            stream_handle.mixer().add(busy_tone);

            // Wait for tone duration
            thread::sleep(Duration::from_millis(BUSY_TONE_ON_MS));

            // Silent period
            thread::sleep(Duration::from_millis(BUSY_TONE_OFF_MS));
        }

        let _ = self.tx.send(SoundData::StopPlay);
    }

    fn play_dial_sound(&mut self, phone_number: &str) -> bool {
        let mut res = true;
        let _ = self.tx.send(SoundData::StartPlay);

        let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();
        let sample_rate = stream_handle.config().sample_rate();

        // 1. Initial dial tone (brief)
        let dial_tone = SignalGenerator::new(sample_rate, 350.0, Function::Sine)
            .amplify(0.1)
            .mix(SignalGenerator::new(sample_rate, 440.0, Function::Sine).amplify(0.1))
            .take_duration(Duration::from_millis(500));
        stream_handle.mixer().add(dial_tone);
        thread::sleep(Duration::from_millis(500));

        // Standard DTMF timing
        const TONE_DURATION_MS: u64 = 200; // Duration of each tone
        const INTER_DIGIT_PAUSE_MS: u64 = 100; // Pause between digits

        for ch in phone_number.chars() {
            // Check for stop signal
            if self.handle_receive() {
                res = false;
                break;
            }

            // Convert to uppercase for letter lookup
            let ch_upper = ch.to_ascii_uppercase();

            // Skip non-alphanumeric characters (spaces, dashes, parentheses, etc.)
            if !ch_upper.is_alphanumeric() {
                // Optional: Add a small pause for separators like spaces or dashes
                if ch == ' ' || ch == '-' || ch == '.' {
                    thread::sleep(Duration::from_millis(INTER_DIGIT_PAUSE_MS));
                }
                continue;
            }

            // Look up DTMF frequencies for this character
            if let Some(&(low_freq, high_freq)) = DTMF_FREQUENCIES.get(&ch_upper) {
                // Generate the two tones
                let low_tone = SignalGenerator::new(sample_rate, low_freq, Function::Sine).amplify(0.15);
                let high_tone = SignalGenerator::new(sample_rate, high_freq, Function::Sine).amplify(0.15);

                // Mix them together for DTMF
                let dtmf_tone = low_tone.mix(high_tone).take_duration(Duration::from_millis(TONE_DURATION_MS));

                // Play the tone
                stream_handle.mixer().add(dtmf_tone);

                // Wait for tone duration
                thread::sleep(Duration::from_millis(TONE_DURATION_MS));

                // Inter-digit pause
                thread::sleep(Duration::from_millis(INTER_DIGIT_PAUSE_MS));
            }
            // If character not found in DTMF table, just skip it
        }

        if !res {
            let _ = self.tx.send(SoundData::StopPlay);
        }
        res
    }

    fn play_line_sound(&mut self) {
        self.line_sound_playing = true;
        let _ = self.tx.send(SoundData::StartPlay);

        let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();
        let sample_rate = stream_handle.config().sample_rate();

        // 1. Initial dial tone (brief)
        let dial_tone = SignalGenerator::new(sample_rate, 350.0, Function::Sine)
            .amplify(0.1)
            .mix(SignalGenerator::new(sample_rate, 440.0, Function::Sine).amplify(0.1))
            .take_duration(Duration::from_millis(500));
        stream_handle.mixer().add(dial_tone);
        thread::sleep(Duration::from_millis(500));

        // Phone line dial tone: 350 Hz + 440 Hz combined (North American standard)
        // For a more authentic sound, you could also use 425 Hz (European) or other standards
        let tone1 = SignalGenerator::new(sample_rate, 350.0, Function::Sine).amplify(0.15);
        let tone2 = SignalGenerator::new(sample_rate, 440.0, Function::Sine).amplify(0.15);

        // Mix the two tones together for authentic dial tone
        let mixed = tone1.mix(tone2);

        // Add to mixer - this will play continuously
        stream_handle.mixer().add(mixed);

        // Keep checking for stop signal
        while self.line_sound_playing {
            if self.handle_receive() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        // Clear the mixer when stopping
        //        stream_handle.mixer().clear();
        let _ = self.tx.send(SoundData::StopPlay);
    }

    fn beep(&mut self) {
        if self.last_beep.elapsed().as_millis() > 500 {
            let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();
            let wave = SineWave::new(740.0).amplify(0.2).take_duration(Duration::from_secs(3));

            stream_handle.mixer().add(wave);

            thread::sleep(std::time::Duration::from_millis(200));
        }
        self.last_beep = Instant::now();
    }

    fn play_music(&mut self, music: &AnsiMusic) {
        let _ = self.tx.send(SoundData::StartPlay);
        let mut i = 0;
        let mut cur_style = MusicStyle::Normal;
        let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();
        let sample_rate = stream_handle.config().sample_rate();
        while i < music.music_actions.len() {
            let act = &music.music_actions[i];
            i += 1;
            if self.handle_receive() {
                break;
            }
            let duration = act.get_duration();
            match act {
                MusicAction::SetStyle(style) => {
                    cur_style = *style;
                }
                MusicAction::PlayNote(freq, _length, _dotted) => {
                    let f = *freq;
                    let pause_length = cur_style.get_pause_length(duration);
                    stream_handle.mixer().add(
                        SignalGenerator::new(sample_rate, f, Function::Square)
                            .amplify(0.1)
                            .take_duration(std::time::Duration::from_millis(duration as u64 - pause_length as u64)),
                    );
                }
                MusicAction::Pause(_) => {}
            }
            thread::sleep(std::time::Duration::from_millis(duration as u64));
        }

        let _ = self.tx.send(SoundData::StopPlay);
    }
}

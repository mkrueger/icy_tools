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

use crate::DialTone;
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
    LineSound(DialTone),
    PlayDialSound(bool, DialTone, String),
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

    pub fn start_line_sound(&mut self, tone: DialTone) -> TerminalResult<()> {
        self.send_data(SoundData::LineSound(tone))
    }

    pub fn start_dial_sound(&mut self, tone_dial: bool, tone: DialTone, phone_number: &str) -> TerminalResult<()> {
        self.send_data(SoundData::PlayDialSound(tone_dial, tone, phone_number.to_string()))
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
                        SoundData::LineSound(tone) => {
                            self.music.push_back(SoundData::LineSound(tone));
                        }
                        SoundData::PlayDialSound(tone_dial, tone, phone_number) => {
                            self.music.push_back(SoundData::PlayDialSound(tone_dial, tone, phone_number));
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
            SoundData::LineSound(tone) => self.play_line_sound(tone),
            SoundData::PlayDialSound(tone_dial, tone, phone_number) => {
                if self.play_dial_sound(tone_dial, tone, &phone_number) {
                    self.play_busysound(tone);
                }
            }
            SoundData::_PlayBusySound => self.play_busysound(DialTone::US),
            _ => {}
        }
    }

    fn play_busysound(&mut self, tone: DialTone) {
        let _ = self.tx.send(SoundData::StartPlay);

        let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();
        let sample_rate = stream_handle.config().sample_rate();

        // Region-specific busy signal parameters
        let (freq1, freq2, on_ms, off_ms, cycles) = match tone {
            DialTone::US => {
                // North American busy signal: 480 Hz + 620 Hz
                // Pattern: 500ms on, 500ms off
                (480.0, 620.0, 500, 500, 8)
            }
            DialTone::UK => {
                // UK busy signal: 400 Hz single tone
                // Pattern: 375ms on, 375ms off
                (400.0, 400.0, 375, 375, 8)
            }
            DialTone::Europe => {
                // European busy signal: 425 Hz single tone
                // Pattern: 500ms on, 500ms off
                (425.0, 425.0, 500, 500, 8)
            }
            DialTone::France => {
                // French busy signal: 440 Hz single tone
                // Pattern: 500ms on, 500ms off
                (440.0, 440.0, 500, 500, 8)
            }
            DialTone::Japan => {
                // Japanese busy signal: 400 Hz single tone
                // Pattern: 500ms on, 500ms off
                (400.0, 400.0, 500, 500, 8)
            }
        };

        for _ in 0..cycles {
            // Check for stop signal
            if self.handle_receive() {
                break;
            }

            // Generate the tone(s) for busy signal
            let tone1 = SignalGenerator::new(sample_rate, freq1, Function::Sine).amplify(0.15);

            let busy_tone = if freq1 == freq2 {
                // Single frequency (UK, Europe, France, Japan)
                // Mix with silent tone to maintain consistent type
                let tone2 = SignalGenerator::new(sample_rate, freq2, Function::Sine).amplify(0.0);
                tone1.mix(tone2).take_duration(Duration::from_millis(on_ms))
            } else {
                // Dual frequency (US)
                let tone2 = SignalGenerator::new(sample_rate, freq2, Function::Sine).amplify(0.15);
                tone1.mix(tone2).take_duration(Duration::from_millis(on_ms))
            };

            // Play the tone
            stream_handle.mixer().add(busy_tone);

            // Wait for tone duration
            thread::sleep(Duration::from_millis(on_ms));

            // Silent period
            thread::sleep(Duration::from_millis(off_ms));
        }

        let _ = self.tx.send(SoundData::StopPlay);
    }

    fn play_dial_sound(&mut self, tone_dial: bool, tone: DialTone, phone_number: &str) -> bool {
        let mut res = true;
        let _ = self.tx.send(SoundData::StartPlay);

        let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();
        let sample_rate = stream_handle.config().sample_rate();

        let dial_tone = mix_dial_tone(tone, sample_rate).take_duration(Duration::from_millis(500));
        stream_handle.mixer().add(dial_tone);

        thread::sleep(Duration::from_millis(500));

        // Standard DTMF timing
        const TONE_DURATION_MS: u64 = 200; // Duration of each tone
        const INTER_DIGIT_PAUSE_MS: u64 = 100; // Pause between digits

        if tone_dial {
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
        } else {
            // Pulse (Rotary) dialing using region-specific profile
            let profile = pulse_profile_for(tone);
            let pulse_ms = (1000.0 / profile.pulses_per_second).round() as u64;
            let mut click_ms = (pulse_ms as f32 * profile.break_ratio).round() as u64;
            if click_ms == 0 {
                click_ms = 1;
            }
            let quiet_ms = pulse_ms.saturating_sub(click_ms);

            for ch in phone_number.chars() {
                if self.handle_receive() {
                    res = false;
                    break;
                }

                if !ch.is_ascii_digit() {
                    if ch == ' ' || ch == '-' || ch == '.' {
                        thread::sleep(Duration::from_millis(profile.inter_digit_pause_ms / 2));
                    }
                    continue;
                }

                let num_pulses = match ch {
                    '0' => 10,
                    '1'..='9' => ch.to_digit(10).unwrap() as usize,
                    _ => continue,
                };

                for pulse_idx in 0..num_pulses {
                    if self.handle_receive() {
                        res = false;
                        break;
                    }

                    let click = PulseClick::new(sample_rate, click_ms);
                    stream_handle.mixer().add(click);
                    thread::sleep(Duration::from_millis(click_ms));

                    if pulse_idx < num_pulses - 1 && quiet_ms > 0 {
                        thread::sleep(Duration::from_millis(quiet_ms));
                    }
                }

                thread::sleep(Duration::from_millis(profile.inter_digit_pause_ms));
            }
        }
        if !res {
            let _ = self.tx.send(SoundData::StopPlay);
        }
        res
    }

    fn play_line_sound(&mut self, tone: DialTone) {
        self.line_sound_playing = true;
        let _ = self.tx.send(SoundData::StartPlay);

        let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();
        let sample_rate = stream_handle.config().sample_rate();

        let dial_tone = mix_dial_tone(tone, sample_rate);
        stream_handle.mixer().add(dial_tone);

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

fn mix_dial_tone(tone: DialTone, sample_rate: u32) -> impl Source<Item = f32> + Send {
    match tone {
        DialTone::US => {
            // Phone line dial tone: 350 Hz + 440 Hz combined (North American standard)
            let tone1 = SignalGenerator::new(sample_rate, 350.0, Function::Sine).amplify(0.15);
            let tone2 = SignalGenerator::new(sample_rate, 440.0, Function::Sine).amplify(0.15);
            tone1.mix(tone2)
        }
        DialTone::UK => {
            let tone1 = SignalGenerator::new(sample_rate, 350.0, Function::Sine).amplify(0.15);
            let tone2: rodio::source::Amplify<SignalGenerator> = SignalGenerator::new(sample_rate, 450.0, Function::Sine).amplify(0.15);
            tone1.mix(tone2)
        }
        DialTone::Europe => {
            let tone1 = SignalGenerator::new(sample_rate, 425.0, Function::Sine).amplify(0.15);
            // Mix with silent tone to match the return type
            let tone2 = SignalGenerator::new(sample_rate, 425.0, Function::Sine).amplify(0.0);
            tone1.mix(tone2)
        }
        DialTone::France => {
            let tone1 = SignalGenerator::new(sample_rate, 440.0, Function::Sine).amplify(0.15);
            // Mix with silent tone to match the return type
            let tone2 = SignalGenerator::new(sample_rate, 440.0, Function::Sine).amplify(0.0);
            tone1.mix(tone2)
        }
        DialTone::Japan => {
            let tone1 = SignalGenerator::new(sample_rate, 400.0, Function::Sine).amplify(0.15);
            // Mix with silent tone to match the return type
            let tone2 = SignalGenerator::new(sample_rate, 400.0, Function::Sine).amplify(0.0);
            tone1.mix(tone2)
        }
    } // No semicolon here - we want to return the value
}

struct PulseProfile {
    pulses_per_second: f32, // usually 10.0
    break_ratio: f32,       // fraction of each pulse that is 'break' (click)
    inter_digit_pause_ms: u64,
}

fn pulse_profile_for(tone: DialTone) -> PulseProfile {
    match tone {
        DialTone::US => PulseProfile {
            pulses_per_second: 10.0,
            break_ratio: 0.60,
            inter_digit_pause_ms: 700,
        },
        DialTone::UK => PulseProfile {
            pulses_per_second: 10.0,
            break_ratio: 0.67,
            inter_digit_pause_ms: 700,
        },
        DialTone::Europe => PulseProfile {
            pulses_per_second: 10.0,
            break_ratio: 0.60,
            inter_digit_pause_ms: 700,
        },
        DialTone::France => PulseProfile {
            pulses_per_second: 10.0,
            break_ratio: 0.60,
            inter_digit_pause_ms: 700,
        },
        DialTone::Japan => PulseProfile {
            pulses_per_second: 10.0,
            break_ratio: 0.60,
            inter_digit_pause_ms: 700,
        },
    }
}

struct PulseClick {
    sample_rate: u32,
    total_samples: u32,
    index: u32,
    primary_freq: f32,
    accent_freq: f32,
    thunk_freq: f32,
    bounce_start: f32,
    bounce_end: f32,
    seed: u32,
}

impl PulseClick {
    fn new(sample_rate: u32, duration_ms: u64) -> Self {
        let total_samples = (((duration_ms as f64 / 1000.0) * sample_rate as f64).ceil() as u32).max(1);

        let seed = fastrand::u32(0..1_000_000);
        Self {
            sample_rate,
            total_samples,
            index: 0,
            primary_freq: 4200.0 + fastrand::f32() * 800.0, // Higher, sharper relay chirp
            accent_freq: 7500.0 + fastrand::f32() * 1200.0, // High metallic ring
            thunk_freq: 220.0 + fastrand::f32() * 80.0,     // Lighter mechanical body
            bounce_start: 0.0012 + fastrand::f32() * 0.0004,
            bounce_end: 0.0020 + fastrand::f32() * 0.0005,
            seed,
        }
    }

    #[inline]
    fn time(&self) -> f32 {
        self.index as f32 / self.sample_rate as f32
    }

    #[inline]
    fn ratio(&self) -> f32 {
        self.index as f32 / self.total_samples as f32
    }

    #[inline]
    fn next_sample(&mut self) -> f32 {
        if self.index >= self.total_samples {
            return 0.0;
        }

        let t = self.time();
        let mix = self.ratio();
        let mut sample = 0.0;

        // Very sharp, quick attack—Hayes-style relay snap.
        match self.index {
            0 => sample += 1.4,
            1 => sample -= 1.2,
            2 => sample += 0.7,
            _ => {}
        }

        // Ultra-high chirp (9–11 kHz) for that classic modem "squeak"
        if t < 0.0008 {
            let chirp_freq = 10000.0 + fastrand::f32() * 1500.0;
            let chirp_decay = (-t * 9000.0).exp();
            sample += (t * chirp_freq * std::f32::consts::TAU).sin() * chirp_decay * 0.6;
        }

        // Primary relay "chirp" resonance (brighter, faster decay).
        let primary_decay = (-t * 6800.0).exp();
        sample += (t * self.primary_freq * std::f32::consts::TAU).sin() * primary_decay * 1.0;

        // Accent high ring (very short—modem relays ring briefly).
        let accent_decay = (-t * 8000.0).exp();
        sample += (t * self.accent_freq * std::f32::consts::TAU).sin() * accent_decay * 0.5;

        // Much lighter low-freq thump (Hayes relays are mechanically lighter).
        if t < 0.008 {
            let thunk_decay = (-t * 280.0).exp();
            sample += (t * self.thunk_freq * std::f32::consts::TAU).cos() * thunk_decay * 0.3;
        }

        // Short grit burst (first 5 ms only, quick release).
        if t < 0.005 {
            let noise_env = (-t * 4200.0).exp();
            let mut rng = fastrand::Rng::with_seed(self.seed as u64 + self.index as u64);
            let noise = (rng.f32() * 2.0 - 1.0) * noise_env * 0.22;
            sample += noise;
        }

        // Minimal bounce (Hayes relays don't bounce much—they're crisp).
        if t >= self.bounce_start && t <= self.bounce_end {
            let rel = (t - self.bounce_start) / (self.bounce_end - self.bounce_start);
            let bounce_env = (1.0 - rel).powf(3.0);
            sample += bounce_env * 0.25;
        }

        // Quick decay envelope for percussive, punchy character.
        let envelope = (1.0 - mix).powf(2.2);
        sample *= envelope;

        // Hot drive—keep it loud and snappy.
        sample = (sample * 1.45).clamp(-1.0, 1.0);

        self.index += 1;
        sample
    }
}

impl Iterator for PulseClick {
    type Item = f32;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.total_samples { None } else { Some(self.next_sample()) }
    }
}

impl Source for PulseClick {
    fn channels(&self) -> u16 {
        1
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.total_samples as f64 / self.sample_rate as f64))
    }
    fn current_span_len(&self) -> Option<usize> {
        Some((self.total_samples - self.index) as usize)
    }
}

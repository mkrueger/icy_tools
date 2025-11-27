use std::{
    collections::{HashMap, VecDeque},
    sync::mpsc::{Receiver, SendError, Sender, TryRecvError, channel},
    time::{Duration, Instant},
};

use once_cell::sync::Lazy;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;
#[cfg(target_arch = "wasm32")]
use wasm_thread as thread;

use icy_parser_core::{AnsiMusic, IgsCommand, MusicAction, MusicStyle};
use rodio::{
    Source,
    buffer::SamplesBuffer,
    source::{Function, SignalGenerator, SineWave},
};
use ym2149::Ym2149;

use super::gist::gist_data::GistSoundData;
use super::gist::gist_driver::GistDriver;
use super::sound_effects::sound_data;

use crate::DialTone;
use crate::TerminalResult;

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
    PlayIgs(Box<IgsCommand>),

    /// Fade out sound on specific voice (soft stop)
    SndOff(u8),
    /// Immediately stop sound on specific voice (hard stop)
    StopSnd(u8),
    /// Stop all voices (soft fade)
    SndOffAll,
    /// Stop all voices (hard stop)
    StopSndAll,
}

pub struct SoundThread {
    rx: Receiver<SoundData>,
    tx: Sender<SoundData>,
    is_playing: bool,
    pub stop_button: u32,
    last_stop_cycle: Instant,
    restart_count: usize,
}

impl SoundThread {
    pub fn new() -> Self {
        let stop_button = fastrand::u32(0..6);
        let (tx, rx) = channel::<SoundData>();
        let mut res = SoundThread {
            rx,
            tx,
            is_playing: false,
            stop_button,
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
            self.stop_button = fastrand::u32(0..6);
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

    pub fn play_music(&mut self, music: AnsiMusic) -> TerminalResult<()> {
        self.send_data(SoundData::PlayMusic(music))
    }

    pub fn play_igs(&mut self, music: Box<IgsCommand>) -> TerminalResult<()> {
        self.send_data(SoundData::PlayIgs(music))
    }

    /// Fade out sound on specific voice (soft stop)
    pub fn snd_off(&mut self, voice: u8) -> TerminalResult<()> {
        self.send_data(SoundData::SndOff(voice))
    }

    /// Immediately stop sound on specific voice (hard stop)
    pub fn stop_snd(&mut self, voice: u8) -> TerminalResult<()> {
        self.send_data(SoundData::StopSnd(voice))
    }

    /// Fade out all voices (soft stop)
    pub fn snd_off_all(&mut self) -> TerminalResult<()> {
        self.send_data(SoundData::SndOffAll)
    }

    /// Immediately stop all voices (hard stop)
    pub fn stop_snd_all(&mut self) -> TerminalResult<()> {
        self.send_data(SoundData::StopSndAll)
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
        // Create audio stream before spawning thread
        let (stream, mixer, sample_rate) = match rodio::OutputStreamBuilder::open_default_stream() {
            Ok(handle) => {
                let rate = handle.config().sample_rate();
                let mixer = handle.mixer().clone();
                (Some(handle), Some(mixer), rate)
            }
            Err(e) => {
                log::error!("Failed to open audio stream: {}", e);
                (None, None, 44100)
            }
        };

        let mut data = SoundBackgroundThreadData {
            rx,
            tx: tx2,
            music: VecDeque::new(),
            thread_is_running: true,
            last_beep: Instant::now(),
            line_sound_playing: false,
            stream,
            mixer,
            sample_rate,
            gist_chip: Ym2149::new(),
            gist_driver: GistDriver::new(),
            gist_playing: false,
            gist_tick_accumulator: 0,
        };

        if let Err(err) = std::thread::Builder::new().name("music_thread".to_string()).spawn(move || {
            while data.thread_is_running {
                data.handle_queue();
                data.handle_receive();
                if data.music.is_empty() && !data.gist_playing {
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

    // Persistent audio stream - we keep both the OutputStream (to prevent drop)
    // and a cloned Mixer for direct access without borrow conflicts
    #[allow(dead_code)]
    stream: Option<rodio::OutputStream>,
    mixer: Option<rodio::mixer::Mixer>,
    sample_rate: u32,

    // Persistent GIST state
    gist_chip: Ym2149,
    gist_driver: GistDriver,
    gist_playing: bool,
    gist_tick_accumulator: u32,
}

impl SoundBackgroundThreadData {
    /// Recreate the audio stream to stop all playing sounds
    fn reset_audio_stream(&mut self) {
        // Dropping the old stream stops all sounds
        self.stream = None;
        self.mixer = None;

        // Create new stream
        match rodio::OutputStreamBuilder::open_default_stream() {
            Ok(handle) => {
                self.sample_rate = handle.config().sample_rate();
                self.mixer = Some(handle.mixer().clone());
                self.stream = Some(handle);
            }
            Err(e) => {
                log::error!("Failed to recreate audio stream: {}", e);
            }
        }

        // Reset GIST state
        self.gist_playing = false;
        self.gist_driver = GistDriver::new();
        self.gist_chip = Ym2149::new();
        self.gist_tick_accumulator = 0;
    }

    pub fn handle_receive(&mut self) -> bool {
        let mut result = false;
        loop {
            match self.rx.try_recv() {
                Ok(data) => match data {
                    SoundData::PlayMusic(m) => {
                        self.line_sound_playing = false;
                        self.music.push_back(SoundData::PlayMusic(m));
                    }
                    SoundData::PlayIgs(igs) => {
                        self.line_sound_playing = false;
                        self.music.push_back(SoundData::PlayIgs(igs));
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
                        self.reset_audio_stream();
                        result = true;
                    }
                    SoundData::Clear => {
                        self.music.clear();
                        self.line_sound_playing = false;
                        self.reset_audio_stream();
                        result = true;
                    }
                    SoundData::SndOff(voice) => {
                        self.gist_driver.snd_off(voice as usize);
                    }
                    SoundData::StopSnd(voice) => {
                        self.gist_driver.stop_snd(&mut self.gist_chip, voice as usize);
                        if !self.gist_driver.is_playing() {
                            self.gist_playing = false;
                        }
                    }
                    SoundData::SndOffAll => {
                        for i in 0..3 {
                            self.gist_driver.snd_off(i);
                        }
                    }
                    SoundData::StopSndAll => {
                        self.stop_gist();
                    }
                    _ => {}
                },
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

        // Process GIST audio if playing
        if self.gist_playing {
            self.process_gist_tick();
            // Don't block - process other sounds too
        }

        let Some(data) = self.music.pop_front() else {
            // If GIST is playing, sleep briefly to avoid busy-waiting
            if self.gist_playing {
                thread::sleep(Duration::from_millis(5));
            }
            return;
        };
        match data {
            SoundData::PlayMusic(music) => self.play_music(&music),
            SoundData::PlayIgs(music) => self.play_igs(&music),
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

    /// Process one GIST tick - called frequently from handle_queue
    fn process_gist_tick(&mut self) {
        if !self.gist_driver.is_playing() {
            self.gist_playing = false;
            self.stop_gist();
            println!("!PLAYING");
            return;
        }

        let Some(mixer) = &self.mixer else {
            self.gist_playing = false;
            return;
        };
        let mixer = mixer.clone();

        // Generate enough samples for ~50ms at 200Hz tick rate
        const TICK_RATE: u32 = 200;
        const SAMPLES_PER_TICK: usize = 2205; // ~50ms at 44100Hz for smooth playback

        let mut samples_buffer = Vec::with_capacity(SAMPLES_PER_TICK);

        for _ in 0..SAMPLES_PER_TICK {
            self.gist_tick_accumulator += TICK_RATE;
            if self.gist_tick_accumulator >= self.sample_rate {
                self.gist_tick_accumulator -= self.sample_rate;
                self.gist_driver.tick(&mut self.gist_chip);
            }
            self.gist_chip.clock();
            samples_buffer.push(self.gist_chip.get_sample());
        }

        // Add samples to mixer
        let source = SamplesBuffer::new(1, self.sample_rate, samples_buffer);
        mixer.add(source);

        // Wait for the buffer to play (prevents flooding)
        thread::sleep(Duration::from_millis(45));

        // Check if still playing after tick
        if !self.gist_driver.is_playing() {
            self.gist_playing = false;
        }
    }

    fn play_busysound(&mut self, tone: DialTone) {
        let _ = self.tx.send(SoundData::StartPlay);

        let Some(mixer) = &self.mixer else {
            return;
        };
        let mixer = mixer.clone();
        let sample_rate = self.sample_rate;

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
            mixer.add(busy_tone);

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

        let Some(mixer) = &self.mixer else {
            return false;
        };
        let mixer = mixer.clone();
        let sample_rate = self.sample_rate;

        let dial_tone = mix_dial_tone(tone, sample_rate).take_duration(Duration::from_millis(500));
        mixer.add(dial_tone);

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
                    mixer.add(dtmf_tone);

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

                let kind = if fastrand::bool() {
                    PulseClickKind::Hayes
                } else {
                    PulseClickKind::USRobotics
                };

                for pulse_idx in 0..num_pulses {
                    if self.handle_receive() {
                        res = false;
                        break;
                    }

                    let click = PulseClick::new_kind(kind, sample_rate, click_ms);
                    mixer.add(click);
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

        let Some(mixer) = &self.mixer else {
            return;
        };
        let mixer = mixer.clone();

        let dial_tone = mix_dial_tone(tone, self.sample_rate);
        mixer.add(dial_tone);

        // Keep checking for stop signal
        while self.line_sound_playing {
            if self.handle_receive() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        let _ = self.tx.send(SoundData::StopPlay);
    }

    fn beep(&mut self) {
        if self.last_beep.elapsed().as_millis() > 500 {
            if let Some(mixer) = &self.mixer {
                let wave = SineWave::new(740.0).amplify(0.2).take_duration(Duration::from_secs(3));
                mixer.add(wave);
                thread::sleep(std::time::Duration::from_millis(200));
            }
        }
        self.last_beep = Instant::now();
    }

    fn play_music(&mut self, music: &AnsiMusic) {
        let _ = self.tx.send(SoundData::StartPlay);
        let mut i = 0;
        let mut cur_style = MusicStyle::Normal;
        let Some(mixer) = &self.mixer else {
            return;
        };
        let mixer = mixer.clone();
        let sample_rate = self.sample_rate;
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
                    mixer.add(
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

    fn play_igs(&mut self, music: &IgsCommand) {
        match music {
            IgsCommand::BellsAndWhistles { sound_effect } => {
                self.queue_gist_sound(*sound_effect as usize, None, -1);
            }
            IgsCommand::ChipMusic {
                sound_effect,
                voice,
                volume,
                pitch,
                ..
            } => {
                // Only start the sound here - timing and stop_type are handled by terminal_thread
                self.queue_chip_music(*sound_effect as usize, *voice, *volume, *pitch);
            }
            _ => {
                log::warn!("Unsupported IGS command for music playback: {:?}", music);
            }
        }
    }

    /// Queue a GIST sound effect - will be mixed with any currently playing sounds
    fn queue_gist_sound(&mut self, sound_index: usize, volume: Option<i16>, pitch: i16) {
        let Some(sound_words) = sound_data(sound_index) else {
            log::warn!("Invalid GIST sound index: {}", sound_index);
            return;
        };

        let sound = GistSoundData::from_words(sound_words);
        if sound.duration() == 0 {
            return;
        }

        // Start the sound on the persistent chip/driver (finds free voice automatically)
        if self
            .gist_driver
            .snd_on(&mut self.gist_chip, &sound, None, volume, pitch, i16::MAX - 1)
            .is_some()
        {
            self.gist_playing = true;
        } else {
            log::warn!("Failed to start GIST sound {}", sound_index);
        }
    }

    /// Queue chip music - uses persistent state
    /// Only starts the sound - timing and stop_type are handled by terminal_thread
    fn queue_chip_music(&mut self, sound_index: usize, voice: u8, volume: u8, pitch: u8) {
        let Some(sound_words) = sound_data(sound_index) else {
            log::warn!("Invalid GIST sound index for ChipMusic: {}", sound_index);
            return;
        };

        let sound = GistSoundData::from_words(sound_words);
        let voice_idx = (voice as usize).min(2);

        // Only start sound if pitch > 0
        if pitch > 0 {
            let actual_pitch = pitch as i16;
            let actual_volume = Some(volume.min(15) as i16);

            // Start on the requested voice channel
            if self
                .gist_driver
                .snd_on(&mut self.gist_chip, &sound, Some(voice_idx), actual_volume, actual_pitch, 1)
                .is_some()
            {
                self.gist_playing = true;
            } else {
                log::warn!("Failed to start ChipMusic sound");
            }
        }
    }

    /// Stop all GIST sounds
    fn stop_gist(&mut self) {
        self.gist_driver.stop_all(&mut self.gist_chip);
        self.gist_playing = false;
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

#[derive(Copy, Clone, Debug)]
enum PulseClickKind {
    Hayes,
    USRobotics,
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
    kind: PulseClickKind,
}

impl PulseClick {
    fn new_kind(kind: PulseClickKind, sample_rate: u32, duration_ms: u64) -> Self {
        let total_samples = (((duration_ms as f64 / 1000.0) * sample_rate as f64).ceil() as u32).max(1);
        let seed = fastrand::u32(0..1_000_000);

        // Profile-specific parameter ranges
        let (primary_freq, accent_freq, thunk_freq, bounce_start, bounce_end) = match kind {
            PulseClickKind::Hayes => (
                4200.0 + fastrand::f32() * 800.0,  // sharper relay chirp
                7500.0 + fastrand::f32() * 1200.0, // high metallic ring
                220.0 + fastrand::f32() * 80.0,    // light body
                0.0012 + fastrand::f32() * 0.0004,
                0.0020 + fastrand::f32() * 0.0005,
            ),
            PulseClickKind::USRobotics => (
                3600.0 + fastrand::f32() * 700.0,  // lower, chunkier relay
                5800.0 + fastrand::f32() * 1000.0, // less bright ring
                180.0 + fastrand::f32() * 70.0,    // heavier body
                0.0018 + fastrand::f32() * 0.0005,
                0.0028 + fastrand::f32() * 0.0007,
            ),
        };

        Self {
            sample_rate,
            total_samples,
            index: 0,
            primary_freq,
            accent_freq,
            thunk_freq,
            bounce_start,
            bounce_end,
            seed,
            kind,
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

    fn next_sample(&mut self) -> f32 {
        if self.index >= self.total_samples {
            return 0.0;
        }

        let t = self.time();
        let mix = self.ratio();
        let mut sample = 0.0;

        // Attack pattern differs slightly by modem type
        match self.kind {
            PulseClickKind::Hayes => match self.index {
                0 => sample += 1.4,
                1 => sample -= 1.2,
                2 => sample += 0.7,
                _ => {}
            },
            PulseClickKind::USRobotics => match self.index {
                0 => sample += 1.3,
                1 => sample -= 1.15,
                2 => sample += 0.75,
                3 => sample -= 0.40,
                _ => {}
            },
        }

        // High chirp / sparkle
        match self.kind {
            PulseClickKind::Hayes => {
                if t < 0.0008 {
                    let freq = 10000.0 + fastrand::f32() * 1500.0;
                    let decay = (-t * 9000.0).exp();
                    sample += (t * freq * std::f32::consts::TAU).sin() * decay * 0.6;
                }
            }
            PulseClickKind::USRobotics => {
                if t < 0.0012 {
                    let freq = 7500.0 + fastrand::f32() * 1500.0;
                    let decay = (-t * 7500.0).exp();
                    sample += (t * freq * std::f32::consts::TAU).sin() * decay * 0.5;
                }
            }
        }

        // Primary relay chirp
        let primary_decay = match self.kind {
            PulseClickKind::Hayes => (-t * 6800.0).exp(),
            PulseClickKind::USRobotics => (-t * 5500.0).exp(),
        };
        sample += (t * self.primary_freq * std::f32::consts::TAU).sin()
            * primary_decay
            * match self.kind {
                PulseClickKind::Hayes => 1.0,
                PulseClickKind::USRobotics => 0.95,
            };

        // Accent ring
        let accent_decay = match self.kind {
            PulseClickKind::Hayes => (-t * 8000.0).exp(),
            PulseClickKind::USRobotics => (-t * 6500.0).exp(),
        };
        sample += (t * self.accent_freq * std::f32::consts::TAU).sin()
            * accent_decay
            * match self.kind {
                PulseClickKind::Hayes => 0.5,
                PulseClickKind::USRobotics => 0.45,
            };

        // Low mechanical body
        let thunk_limit = match self.kind {
            PulseClickKind::Hayes => 0.008,
            PulseClickKind::USRobotics => 0.012,
        };
        if t < thunk_limit {
            let thunk_decay = match self.kind {
                PulseClickKind::Hayes => (-t * 280.0).exp(),
                PulseClickKind::USRobotics => (-t * 220.0).exp(),
            };
            sample += (t * self.thunk_freq * std::f32::consts::TAU).cos()
                * thunk_decay
                * match self.kind {
                    PulseClickKind::Hayes => 0.3,
                    PulseClickKind::USRobotics => 0.5,
                };
        }

        // Grit / relay contact noise
        let grit_limit = match self.kind {
            PulseClickKind::Hayes => 0.005,
            PulseClickKind::USRobotics => 0.007,
        };
        if t < grit_limit {
            let noise_env = match self.kind {
                PulseClickKind::Hayes => (-t * 4200.0).exp(),
                PulseClickKind::USRobotics => (-t * 3500.0).exp(),
            };
            let mut rng = fastrand::Rng::with_seed(self.seed as u64 + self.index as u64);
            let noise_amp = match self.kind {
                PulseClickKind::Hayes => 0.22,
                PulseClickKind::USRobotics => 0.25,
            };
            sample += (rng.f32() * 2.0 - 1.0) * noise_env * noise_amp;
        }

        // Bounce
        if t >= self.bounce_start && t <= self.bounce_end {
            let rel = (t - self.bounce_start) / (self.bounce_end - self.bounce_start);
            let bounce_env = match self.kind {
                PulseClickKind::Hayes => (1.0 - rel).powf(3.0),
                PulseClickKind::USRobotics => (1.0 - rel).powf(2.8),
            };
            let bounce_amp = match self.kind {
                PulseClickKind::Hayes => 0.25,
                PulseClickKind::USRobotics => 0.30,
            };
            sample += bounce_env * bounce_amp;
        }

        // Envelope
        let envelope = match self.kind {
            PulseClickKind::Hayes => (1.0 - mix).powf(2.2),
            PulseClickKind::USRobotics => (1.0 - mix).powf(2.4),
        };
        sample *= envelope;

        // Drive
        let driven = sample
            * match self.kind {
                PulseClickKind::Hayes => 1.45,
                PulseClickKind::USRobotics => 1.38,
            };
        sample = driven.clamp(-1.0, 1.0);

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

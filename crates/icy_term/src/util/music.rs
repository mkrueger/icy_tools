use std::{
    collections::VecDeque,
    sync::mpsc::{channel, Receiver, SendError, Sender, TryRecvError},
};

#[cfg(not(target_arch = "wasm32"))]
use std::thread;
#[cfg(target_arch = "wasm32")]
use wasm_thread as thread;

use icy_engine::ansi::sound::{AnsiMusic, MusicAction, MusicStyle};
use rodio::OutputStream;
use web_time::{Duration, Instant};

use crate::TerminalResult;

use super::Rng;

/// Data that is sent to the connection thread
#[derive(Debug)]
pub enum SoundData {
    PlayMusic(AnsiMusic),
    Beep,
    Clear,

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

#[cfg(not(target_arch = "wasm32"))]
impl SoundThread {
    pub(crate) fn update_state(&mut self) -> TerminalResult<()> {
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

#[cfg(target_arch = "wasm32")]
impl SoundThread {
    pub(crate) fn update_state(&mut self) -> TerminalResult<()> {
        Ok(())
    }

    pub(crate) fn beep(&self) -> TerminalResult<()> {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = rodio::Sink::try_new(&stream_handle).unwrap();
        sink.set_volume(0.1);

        let source = rodio::source::SineWave::new(880.);
        sink.append(source);

        thread::sleep(std::time::Duration::from_millis(200));
        Ok(())
    }

    pub(crate) fn play_music(&self, music: AnsiMusic) -> TerminalResult<()> {
        let mut i = 0;
        let mut cur_style = MusicStyle::Normal;

        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = rodio::Sink::try_new(&stream_handle).unwrap();
        sink.set_volume(0.1);

        while i < music.music_actions.len() {
            let act = &music.music_actions[i];
            i += 1;
            match act {
                MusicAction::SetStyle(style) => {
                    cur_style = *style;
                }
                MusicAction::PlayNote(freq, length, dotted) => {
                    let f = *freq;

                    let mut duration = if *dotted { 420_000_u64 } else { 300_000_u64 } / u64::from(*length);

                    let pause_length = match cur_style {
                        MusicStyle::Legato => 0,
                        MusicStyle::Staccato => duration / 4,
                        _ => duration / 8,
                    };
                    duration -= pause_length;
                    {
                        let source = rodio::source::SineWave::new(f);
                        sink.append(source);
                        sink.play();
                        thread::sleep(std::time::Duration::from_millis(duration));
                        sink.clear();
                    }
                    thread::sleep(std::time::Duration::from_millis(pause_length));
                }
                MusicAction::Pause(length) => {
                    let duration = 2 * 250_000 / length;
                    thread::sleep(std::time::Duration::from_millis(u64::from(duration)));
                }
            }
        }
        Ok(())
    }
}

pub struct SoundBackgroundThreadData {
    tx: Sender<SoundData>,
    rx: Receiver<SoundData>,
    thread_is_running: bool,

    music: VecDeque<SoundData>,
}

impl SoundBackgroundThreadData {
    pub fn handle_receive(&mut self) -> bool {
        let mut result = false;
        loop {
            match self.rx.try_recv() {
                Ok(data) => match data {
                    SoundData::PlayMusic(m) => {
                        self.music.push_back(SoundData::PlayMusic(m));
                    }
                    SoundData::Beep => self.music.push_back(SoundData::Beep),
                    SoundData::Clear => {
                        result = true;
                        self.music.clear();
                    }
                    _ => {}
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.thread_is_running = false;
                    break;
                }
            }
        }
        result
    }

    fn handle_queue(&mut self) {
        let Some(data) = self.music.pop_front() else {
            return;
        };
        match data {
            SoundData::PlayMusic(music) => self.play_music(&music),
            SoundData::Beep => SoundBackgroundThreadData::beep(),
            _ => {}
        }
    }

    fn beep() {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = rodio::Sink::try_new(&stream_handle).unwrap();
        sink.set_volume(0.1);

        let source = rodio::source::SineWave::new(880.);
        sink.append(source);

        thread::sleep(std::time::Duration::from_millis(200));
    }

    fn play_music(&mut self, music: &AnsiMusic) {
        let _ = self.tx.send(SoundData::StartPlay);
        let mut i = 0;
        let mut cur_style = MusicStyle::Normal;

        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = rodio::Sink::try_new(&stream_handle).unwrap();
        sink.set_volume(0.1);

        while i < music.music_actions.len() {
            let act = &music.music_actions[i];
            i += 1;
            if self.handle_receive() {
                break;
            }
            match act {
                MusicAction::SetStyle(style) => {
                    cur_style = *style;
                }
                MusicAction::PlayNote(freq, length, dotted) => {
                    let f = *freq;

                    let mut duration = if *dotted { 420_000_i32 } else { 300_000_i32 } / *length;

                    let pause_length = match cur_style {
                        MusicStyle::Legato => 0,
                        MusicStyle::Staccato => duration / 4,
                        _ => duration / 8,
                    };
                    duration -= pause_length;
                    {
                        let source = rodio::source::SineWave::new(f);
                        sink.append(source);
                        sink.play();
                        thread::sleep(std::time::Duration::from_millis(duration as u64));
                        sink.clear();
                    }
                    thread::sleep(std::time::Duration::from_millis(pause_length as u64));
                }
                MusicAction::Pause(length) => {
                    let duration = 2 * 250_000 / length;
                    thread::sleep(std::time::Duration::from_millis(duration as u64));
                }
            }
        }
        let _ = self.tx.send(SoundData::StopPlay);
    }
}

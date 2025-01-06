use std::{
    collections::VecDeque,
    sync::mpsc::{channel, Receiver, SendError, Sender, TryRecvError},
};

#[cfg(not(target_arch = "wasm32"))]
use std::thread;
#[cfg(target_arch = "wasm32")]
use wasm_thread as thread;

use crate::rng::Rng;
use icy_engine::ansi::sound::{AnsiMusic, MusicAction, MusicStyle};
use rodio::{
    cpal::SampleRate,
    source::{Function, SignalGenerator},
    OutputStream, Source,
};
use web_time::{Duration, Instant};

/// Data that is sent to the connection thread
#[derive(Debug)]
pub enum SoundData {
    PlayMusic(AnsiMusic),
    Clear,

    StartPlay,
    StopPlay,
    CurAction(MusicAction),
}

pub struct SoundThread {
    rx: Receiver<SoundData>,
    tx: Sender<SoundData>,
    is_playing: bool,
    rng: Rng,
    pub stop_button: u32,
    last_stop_cycle: Instant,
    restart_count: usize,
    pub cur_action: Option<MusicAction>,
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
            cur_action: None,
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
    pub(crate) fn update_state(&mut self) -> anyhow::Result<()> {
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
                    SoundData::CurAction(act) => self.cur_action = Some(act),
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
    pub(crate) fn play_music(&mut self, music: AnsiMusic) -> anyhow::Result<()> {
        self.send_data(SoundData::PlayMusic(music))
    }

    fn send_data(&mut self, data: SoundData) -> anyhow::Result<()> {
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
            _ => {}
        }
    }

    fn play_music(&mut self, music: &AnsiMusic) {
        let _ = self.tx.send(SoundData::StartPlay);
        let mut i = 0;
        let mut cur_style = MusicStyle::Normal;

        let (_stream, stream_handle) = OutputStream::try_default().unwrap();

        let sample_rate = SampleRate(48000);

        while i < music.music_actions.len() {
            let act = &music.music_actions[i];
            i += 1;
            if self.handle_receive() {
                break;
            }
            let _ = self.tx.send(SoundData::CurAction(act.clone()));
            let duration = act.get_duration();

            match act {
                MusicAction::SetStyle(style) => {
                    cur_style = *style;
                }
                MusicAction::PlayNote(freq, _length, _dotted) => {
                    let f = *freq;
                    let pause_length = cur_style.get_pause_length(duration);
                    if let Err(err) = stream_handle.play_raw(
                        SignalGenerator::new(sample_rate, f, Function::Square)
                            .amplify(0.07)
                            .take_duration(std::time::Duration::from_millis(duration as u64 - pause_length as u64)),
                    ) {
                        log::error!("Error in playing note: {}", err);
                        break;
                    }
                }
                MusicAction::Pause(_) => {}
            }
            thread::sleep(std::time::Duration::from_millis(duration as u64));
        }

        let _ = self.tx.send(SoundData::StopPlay);
    }
}

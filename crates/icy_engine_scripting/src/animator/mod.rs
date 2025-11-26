//! Animation engine for Lua-scripted ANSI animations
//!
//! This module provides the core animation functionality for playing
//! and controlling Lua-scripted animations.

mod lua_runtime;

pub mod lua_layer;
pub use lua_layer::{LuaLayer, LuaScreen};

use std::sync::Arc;
use std::thread;

use icy_engine::{Screen, TextBuffer};
use parking_lot::Mutex;
use web_time::Instant;

use crate::MonitorSettings;

/// Default animation speed in milliseconds (like animated GIFs)
const DEFAULT_SPEED: u32 = 100;

/// Maximum number of frames to prevent memory issues
const MAX_FRAMES: usize = 4096;

/// Log entry from script execution
pub struct LogEntry {
    pub frame: usize,
    pub text: String,
}

/// Animation player and controller
///
/// Manages animation state, playback controls, and frame storage.
pub struct Animator {
    /// Optional scene buffer
    pub scene: Option<TextBuffer>,
    /// Stored animation frames with monitor settings and delay
    pub frames: Vec<(Box<dyn Screen>, MonitorSettings, u32)>,
    /// Current monitor settings for rendering
    pub(crate) current_monitor_settings: MonitorSettings,
    /// Additional buffers
    pub buffers: Vec<TextBuffer>,
    /// Error message from script execution
    pub error: String,
    /// Log entries from script
    pub log: Vec<LogEntry>,

    // Playback controls
    cur_frame: usize,
    is_loop: bool,
    is_playing: bool,
    delay: u32,
    instant: Instant,

    /// Background thread running the Lua script
    pub(crate) run_thread: Option<thread::JoinHandle<()>>,
}

impl Default for Animator {
    fn default() -> Self {
        Self {
            scene: Default::default(),
            frames: Default::default(),
            current_monitor_settings: MonitorSettings::neutral(),
            buffers: Default::default(),
            cur_frame: Default::default(),
            is_loop: Default::default(),
            is_playing: Default::default(),
            delay: DEFAULT_SPEED,
            instant: Instant::now(),
            run_thread: None,
            error: String::new(),
            log: Vec::new(),
        }
    }
}

impl Animator {
    /// Add a new frame from the current screen state
    pub(crate) fn lua_next_frame(&mut self, screen: &Arc<Mutex<Box<dyn Screen>>>) -> mlua::Result<()> {
        if self.frames.len() > MAX_FRAMES {
            return Err(mlua::Error::RuntimeError("Maximum number of frames reached".to_string()));
        }

        let frame = screen.lock().clone_box();
        self.frames.push((frame, self.current_monitor_settings.clone(), self.delay));
        Ok(())
    }

    /// Check if the background script thread is still running
    pub fn is_thread_running(&self) -> bool {
        self.run_thread.is_some() && !self.run_thread.as_ref().unwrap().is_finished()
    }

    /// Check if the animation completed successfully
    pub fn success(&self) -> bool {
        !self.is_thread_running() && self.error.is_empty()
    }

    /// Check if animation is currently playing
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Set playing state
    pub fn set_is_playing(&mut self, is_playing: bool) {
        self.is_playing = is_playing;
    }

    /// Get current frame index
    pub fn get_cur_frame(&self) -> usize {
        self.cur_frame
    }

    /// Set current frame index
    pub fn set_cur_frame(&mut self, cur_frame: usize) {
        if !self.frames.is_empty() {
            self.cur_frame = cur_frame.clamp(0, self.frames.len() - 1);
            self.delay = self.frames[self.cur_frame].2;
        }
    }

    /// Check if animation loops
    pub fn get_is_loop(&self) -> bool {
        self.is_loop
    }

    /// Set loop mode
    pub fn set_is_loop(&mut self, is_loop: bool) {
        self.is_loop = is_loop;
    }

    /// Get current frame delay in milliseconds
    pub fn get_delay(&self) -> u32 {
        self.delay
    }

    /// Set frame delay in milliseconds
    pub fn set_delay(&mut self, delay: u32) {
        self.delay = delay;
    }

    /// Update playback state, returns true if frame changed
    pub fn update_playback(&mut self) -> bool {
        if self.is_playing && self.instant.elapsed().as_millis() > self.delay as u128 {
            self.next_frame();
            self.instant = Instant::now();
            return true;
        }
        false
    }

    /// Start playback
    pub fn start_playback(&mut self) {
        self.is_playing = true;
        self.instant = Instant::now();
    }

    /// Get current frame's buffer and settings
    pub fn get_current_frame(&self) -> Option<(&Box<dyn Screen>, &MonitorSettings)> {
        self.frames.get(self.cur_frame).map(|(scene, settings, _)| (scene, settings))
    }

    /// Get current monitor settings
    pub fn get_current_monitor_settings(&self) -> MonitorSettings {
        self.frames.get(self.cur_frame).map(|(_, settings, _)| settings.clone()).unwrap_or_default()
    }

    /// Get current frame buffer with all metadata (immutable)
    pub fn get_cur_frame_buffer(&self) -> Option<(&Box<dyn Screen>, &MonitorSettings, &u32)> {
        self.frames.get(self.cur_frame).map(|(scene, settings, delay)| (scene, settings, delay))
    }

    /// Get current frame buffer with all metadata (mutable)
    pub fn get_cur_frame_buffer_mut(&mut self) -> Option<(&mut Box<dyn Screen>, &mut MonitorSettings, &mut u32)> {
        self.frames.get_mut(self.cur_frame).map(|(scene, settings, delay)| (scene, settings, delay))
    }

    /// Advance to next frame, returns false if waiting for more frames
    pub fn next_frame(&mut self) -> bool {
        self.cur_frame += 1;

        if self.cur_frame >= self.frames.len() {
            if self.is_thread_running() {
                self.cur_frame -= 1;
                return false;
            }
            if self.is_loop {
                self.delay = DEFAULT_SPEED;
                self.cur_frame = 0;
            } else {
                self.cur_frame -= 1;
                self.is_playing = false;
            }
            return true;
        }

        self.delay = self.frames[self.cur_frame].2;
        true
    }
}

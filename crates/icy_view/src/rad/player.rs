// Reality Adlib Tracker (RAD) replayer, ported from the public-domain
// RADPlayer C++ class by Shayde/Reality (handles RAD v1 `0x10` and v2 `0x21`).
// Ported via kaleidotron (MIT, Copyright (c) 2026 Rick Christy).
//
// This is a faithful translation of the *replayer core* only (the `RADValidate`
// file validator and the AdPlug `Crad2Player` wrapper from rad2.cpp are not
// ported). It parses a `.rad` tune and, tick by tick, emits OPL3 register writes.
//
// The C++ used raw `uint8_t*` pointers walking the tune buffer; here the tune
// bytes are owned in `tune` and every "pointer" is a `usize` byte-offset into
// that slice. All reads go through bounds-checked helpers so a malformed file
// never panics (it stops / returns None instead). Overflowing arithmetic that
// the original relied on is done with `wrapping_*`.
//
// SetOPL3(reg,val) in the reference both records the value in an internal
// register mirror (read back by GetOPL3) *and* calls the user callback. Here
// `set_opl3` records into `opl3_regs` immediately and queues `(reg,val)` into
// `pending`; `update()` drains `pending` in order through the caller's `write`
// closure at the end of the tick. Same writes, same order, same mirror state.

#![allow(dead_code)]
// A faithful C++→Rust port: keep clippy quiet about idioms that mirror the source
// (manual range checks, index loops, etc.) rather than "improving" the translation.
#![allow(clippy::all)]

// Command effect numbers (from the enum in the C++).
const CM_PORTAMENTO_UP: u8 = 0x1;
const CM_PORTAMENTO_DWN: u8 = 0x2;
const CM_TONE_SLIDE: u8 = 0x3;
const CM_TONE_VOL_SLIDE: u8 = 0x5;
const CM_VOL_SLIDE: u8 = 0xA;
const CM_SET_VOL: u8 = 0xC;
const CM_JUMP_TO_LINE: u8 = 0xD;
const CM_SET_SPEED: u8 = 0xF;
const CM_IGNORE: u8 = b'I' - 55; // 18
const CM_MULTIPLIER: u8 = b'M' - 55; // 22
const CM_RIFF: u8 = b'R' - 55; // 27
const CM_TRANSPOSE: u8 = b'T' - 55; // 29
const CM_FEEDBACK: u8 = b'U' - 55; // 30
const CM_VOLUME: u8 = b'V' - 55; // 31

// Key flags.
const F_KEY_ON: u8 = 1 << 0;
const F_KEY_OFF: u8 = 1 << 1;
const F_KEYED_ON: u8 = 1 << 2;

const K_TRACKS: usize = 100;
const K_CHANNELS: usize = 9;
const K_TRACK_LINES: u8 = 64;
const K_RIFF_TRACKS: usize = 10;
const K_INSTRUMENTS: usize = 127;

// Tables, verbatim from the reference.
const NOTE_SIZE: [i8; 8] = [0, 2, 1, 3, 1, 3, 2, 4];
const CHAN_OFFSETS3: [u16; 9] = [0, 1, 2, 0x100, 0x101, 0x102, 6, 7, 8];
const CHN2_OFFSETS3: [u16; 9] = [3, 4, 5, 0x103, 0x104, 0x105, 0x106, 0x107, 0x108];
const NOTE_FREQ: [u16; 12] = [0x16b, 0x181, 0x198, 0x1b0, 0x1ca, 0x1e5, 0x202, 0x220, 0x241, 0x263, 0x287, 0x2ae];
const OP_OFFSETS2: [[u16; 2]; 9] = [
    [0x003, 0x000],
    [0x004, 0x001],
    [0x005, 0x002],
    [0x00B, 0x008],
    [0x00C, 0x009],
    [0x00D, 0x00A],
    [0x013, 0x010],
    [0x014, 0x011],
    [0x015, 0x012],
];
const OP_OFFSETS3: [[u16; 4]; 9] = [
    [0x00B, 0x008, 0x003, 0x000],
    [0x00C, 0x009, 0x004, 0x001],
    [0x00D, 0x00A, 0x005, 0x002],
    [0x10B, 0x108, 0x103, 0x100],
    [0x10C, 0x109, 0x104, 0x101],
    [0x10D, 0x10A, 0x105, 0x102],
    [0x113, 0x110, 0x013, 0x010],
    [0x114, 0x111, 0x014, 0x011],
    [0x115, 0x112, 0x015, 0x012],
];
const ALG_CARRIERS: [[bool; 4]; 7] = [
    [true, false, false, false], // 0 - 2op - op < op
    [true, true, false, false],  // 1 - 2op - op + op
    [true, false, false, false], // 2 - 4op - op < op < op < op
    [true, false, false, true],  // 3 - 4op - op < op < op + op
    [true, false, true, false],  // 4 - 4op - op < op + op < op
    [true, false, true, true],   // 5 - 4op - op < op + op + op
    [true, true, true, true],    // 6 - 4op - op + op + op + op
];

#[derive(Copy, Clone, PartialEq, Eq)]
enum Source {
    None,
    Riff,
    IRiff,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum FxLoc {
    Chan,
    Riff,
    IRiff,
}

#[derive(Copy, Clone, Default)]
struct CEffects {
    port_slide: i8,
    vol_slide: i8,
    tone_slide_freq: u16,
    tone_slide_oct: u8,
    tone_slide_speed: u8,
    tone_slide_dir: i8,
}

impl CEffects {
    fn reset(&mut self) {
        self.port_slide = 0;
        self.vol_slide = 0;
        self.tone_slide_dir = 0;
    }
}

#[derive(Copy, Clone, Default)]
struct CRiff {
    fx: CEffects,
    track: Option<usize>,
    track_start: usize,
    line: u8,
    speed: u8,
    speed_cnt: u8,
    transpose_octave: i8,
    transpose_note: i8,
    last_instrument: u8,
    updated: bool,
}

#[derive(Copy, Clone, Default)]
struct CChannel {
    last_instrument: u8,
    instrument: Option<usize>,
    volume: u8,
    detune_a: u8,
    detune_b: u8,
    key_flags: u8,
    curr_freq: u16,
    curr_octave: i8,
    fx: CEffects,
    riff: CRiff,
    iriff: CRiff,
}

struct CInstrument {
    feedback: [u8; 2],
    panning: [u8; 2],
    algorithm: u8,
    detune: u8,
    volume: u8,
    riff_speed: u8,
    riff: Option<usize>,
    operators: [[u8; 5]; 4],
    name: [u8; 256],
}

impl CInstrument {
    fn new() -> CInstrument {
        CInstrument {
            feedback: [0; 2],
            panning: [0; 2],
            algorithm: 0,
            detune: 0,
            volume: 0,
            riff_speed: 0,
            riff: None,
            operators: [[0; 5]; 4],
            name: [0; 256],
        }
    }
}

// Bounds-checked byte read from a slice (returns 0 past the end).
#[inline]
fn rb(d: &[u8], i: usize) -> u8 {
    if i < d.len() {
        d[i]
    } else {
        0
    }
}

pub struct RadPlayer {
    tune: Vec<u8>,
    version: i32,
    use_opl3: bool,
    description: usize,
    instruments: [CInstrument; K_INSTRUMENTS],
    num_instruments: i32,
    channels: [CChannel; K_CHANNELS],
    play_time: u32,
    order_map: [u32; 4],
    repeating: bool,
    hertz: f32,
    order_list: usize,
    tracks: [Option<usize>; K_TRACKS],
    num_tracks: i32,
    riffs: [[Option<usize>; K_CHANNELS]; K_RIFF_TRACKS],
    track: Option<usize>,
    speed: u8,
    order_list_size: u8,
    speed_cnt: u8,
    order: u8,
    line: u8,
    entrances: i8,
    master_vol: u8,
    line_jump: i8,
    opl3_regs: [u8; 512],

    // Values exported by unpack_note().
    note_num: i8,
    octave_num: i8,
    inst_num: u8,
    effect_num: u8,
    param: u8,

    // Deferred chip reset + queued register writes for the current tick.
    chip_reset_done: bool,
    pending: Vec<(u16, u8)>,
}

impl RadPlayer {
    /// Parse a `.rad` file (v1 `0x10` or v2 `0x21`, version byte at offset 0x10).
    /// Returns `None` if the version byte is invalid.
    pub fn new(tune: &[u8]) -> Option<RadPlayer> {
        // Version check; only version 1.0 (0x10) and 2.1 (0x21) are supported.
        let ver = *tune.get(0x10)?;
        if ver != 0x10 && ver != 0x21 {
            return None;
        }

        // Some old tunes have a truncated final note; the AdPlug loader appends
        // one empty byte so they still "work". Mirror that here.
        let mut data: Vec<u8> = tune.to_vec();
        data.push(0);
        let len = data.len();

        let version = (ver >> 4) as i32;
        let use_opl3 = version >= 2;

        let mut instruments: [CInstrument; K_INSTRUMENTS] = std::array::from_fn(|_| CInstrument::new());

        let mut s: usize = 0x11;

        let flags = rb(&data, s);
        s += 1;
        let speed = flags & 0x1F;

        // Is a BPM value present?
        let mut hertz: f32 = 50.0;
        if version >= 2 && (flags & 0x20) != 0 {
            let bpm = rb(&data, s) as i32 | ((rb(&data, s + 1) as i32) << 8);
            hertz = bpm as f32 * 2.0 / 5.0;
            s += 2;
        }

        // Slow-timer tune?
        if (flags & 0x40) != 0 {
            hertz = 18.2;
        }

        // Skip any description (a null-terminated string).
        let mut description = 0usize;
        if version >= 2 || (flags & 0x80) != 0 {
            description = s;
            while rb(&data, s) != 0 {
                s += 1;
                if s >= len {
                    break;
                }
            }
            s += 1;
        }

        // Unpack the instruments.
        let mut num_instruments: i32 = 0;
        loop {
            if s >= len {
                break;
            }
            let inst_num = rb(&data, s);
            s += 1;
            if inst_num == 0 {
                break;
            }
            if inst_num as i32 > num_instruments {
                num_instruments = inst_num as i32;
            }

            let idx = (inst_num as usize).wrapping_sub(1);
            let mut inst = CInstrument::new();
            let mut alg: u8 = 0;

            if version >= 2 {
                // Version 2 instrument.
                let inst_namelen = rb(&data, s) as usize;
                s += 1;
                for i in 0..inst_namelen {
                    let c = rb(&data, s);
                    s += 1;
                    if i < 256 {
                        inst.name[i] = c;
                    }
                }
                if inst_namelen < 256 {
                    inst.name[inst_namelen] = 0;
                }

                alg = rb(&data, s);
                s += 1;
                inst.algorithm = alg & 7;
                inst.panning[0] = (alg >> 3) & 3;
                inst.panning[1] = (alg >> 5) & 3;

                if inst.algorithm < 7 {
                    let b = rb(&data, s);
                    s += 1;
                    inst.feedback[0] = b & 15;
                    inst.feedback[1] = b >> 4;

                    let b = rb(&data, s);
                    s += 1;
                    inst.detune = b >> 4;
                    inst.riff_speed = b & 15;

                    inst.volume = rb(&data, s);
                    s += 1;

                    for i in 0..4 {
                        for j in 0..5 {
                            inst.operators[i][j] = rb(&data, s);
                            s += 1;
                        }
                    }
                } else {
                    // Ignore MIDI instrument data.
                    s += 6;
                }

                // Instrument riff?
                if (alg & 0x80) != 0 {
                    let size = rb(&data, s) as usize | ((rb(&data, s + 1) as usize) << 8);
                    s += 2;
                    inst.riff = Some(s);
                    s += size;
                } else {
                    inst.riff = None;
                }
            } else {
                // Version 1 instrument.
                inst.name[0] = 0;
                inst.algorithm = rb(&data, s + 8) & 1;
                inst.panning[0] = 0;
                inst.panning[1] = 0;
                inst.feedback[0] = (rb(&data, s + 8) >> 1) & 7;
                inst.feedback[1] = 0;
                inst.detune = 0;
                inst.riff_speed = 0;
                inst.volume = 64;

                inst.operators[0][0] = rb(&data, s);
                inst.operators[1][0] = rb(&data, s + 1);
                inst.operators[0][1] = rb(&data, s + 2);
                inst.operators[1][1] = rb(&data, s + 3);
                inst.operators[0][2] = rb(&data, s + 4);
                inst.operators[1][2] = rb(&data, s + 5);
                inst.operators[0][3] = rb(&data, s + 6);
                inst.operators[1][3] = rb(&data, s + 7);
                inst.operators[0][4] = rb(&data, s + 9);
                inst.operators[1][4] = rb(&data, s + 10);

                inst.riff = None;
                s += 11;
            }

            let _ = alg;
            if idx < K_INSTRUMENTS {
                instruments[idx] = inst;
            }
        }

        // Get order list.
        let order_list_size = rb(&data, s);
        s += 1;
        let order_list = s;
        s += order_list_size as usize;

        // Locate the tracks.
        let mut tracks: [Option<usize>; K_TRACKS] = [None; K_TRACKS];
        let mut num_tracks: i32 = 0;
        if version >= 2 {
            loop {
                if s >= len {
                    break;
                }
                let track_num = rb(&data, s);
                s += 1;
                if track_num as usize >= K_TRACKS {
                    break;
                }
                if track_num as i32 + 1 > num_tracks {
                    num_tracks = track_num as i32 + 1;
                }
                let size = rb(&data, s) as usize | ((rb(&data, s + 1) as usize) << 8);
                s += 2;
                tracks[track_num as usize] = Some(s);
                s += size;
            }
        } else {
            for i in 0..32 {
                if s >= len {
                    break;
                }
                let pos = rb(&data, s) as usize | ((rb(&data, s + 1) as usize) << 8);
                s += 2;
                if pos != 0 {
                    num_tracks = i as i32 + 1;
                    tracks[i] = Some(pos);
                }
            }
        }

        // Locate the riffs (version 2 only).
        let mut riffs: [[Option<usize>; K_CHANNELS]; K_RIFF_TRACKS] = [[None; K_CHANNELS]; K_RIFF_TRACKS];
        if version >= 2 {
            loop {
                if s >= len {
                    break;
                }
                let riffid = rb(&data, s);
                s += 1;
                let riffnum = (riffid >> 4) as usize;
                let channum = (riffid & 15) as usize;
                if riffnum >= K_RIFF_TRACKS || channum > K_CHANNELS {
                    break;
                }
                let size = rb(&data, s) as usize | ((rb(&data, s + 1) as usize) << 8);
                s += 2;
                if channum >= 1 && channum <= K_CHANNELS {
                    riffs[riffnum][channum - 1] = Some(s);
                }
                s += size;
            }
        }

        let mut p = RadPlayer {
            tune: data,
            version,
            use_opl3,
            description,
            instruments,
            num_instruments,
            channels: std::array::from_fn(|_| CChannel::default()),
            play_time: 0,
            order_map: [0; 4],
            repeating: false,
            hertz,
            order_list,
            tracks,
            num_tracks,
            riffs,
            track: None,
            speed,
            order_list_size,
            speed_cnt: 1,
            order: 0,
            line: 0,
            entrances: 0,
            master_vol: 64,
            line_jump: -1,
            opl3_regs: [255; 512],
            note_num: 0,
            octave_num: 0,
            inst_num: 0,
            effect_num: 0,
            param: 0,
            chip_reset_done: false,
            pending: Vec::new(),
        };

        p.reset_play_state();
        Some(p)
    }

    /// The playback tick rate in Hz (call `update` this many times per second).
    pub fn hz(&self) -> f64 {
        self.hertz as f64
    }

    /// RAD major version (1 for `0x10`, 2 for `0x21`).
    pub fn version(&self) -> i32 {
        self.version
    }

    /// Whether the tune uses OPL3 mode. RAD v2 tunes use 18 voices, v1 tunes use 9.
    pub fn uses_opl3(&self) -> bool {
        self.use_opl3
    }

    pub fn instrument_count(&self) -> usize {
        self.num_instruments.clamp(0, K_INSTRUMENTS as i32) as usize
    }

    pub fn track_count(&self) -> usize {
        self.num_tracks.clamp(0, K_TRACKS as i32) as usize
    }

    pub fn order_count(&self) -> usize {
        self.order_list_size as usize
    }

    /// Null-terminated module description bytes from the file.
    pub fn description(&self) -> &[u8] {
        if self.description == 0 || self.description >= self.tune.len() {
            return &[];
        }
        let end = self.tune[self.description..]
            .iter()
            .position(|b| *b == 0)
            .map(|len| self.description + len)
            .unwrap_or(self.tune.len());
        &self.tune[self.description..end]
    }

    /// Non-empty instrument names in instrument-number order.
    pub fn instrument_names(&self) -> Vec<Vec<u8>> {
        let count = self.num_instruments.clamp(0, K_INSTRUMENTS as i32) as usize;
        (0..count)
            .filter_map(|index| {
                let name = &self.instruments[index].name;
                let end = name.iter().position(|b| *b == 0).unwrap_or(name.len());
                let mut bytes = name[..end].to_vec();
                while bytes.last() == Some(&b' ') {
                    bytes.pop();
                }
                (!bytes.is_empty()).then_some(bytes)
            })
            .collect()
    }

    // --- OPL3 register mirror + queued writes ------------------------------

    #[inline]
    fn set_opl3(&mut self, reg: u16, val: u8) {
        if (reg as usize) < 512 {
            self.opl3_regs[reg as usize] = val;
        }
        self.pending.push((reg, val));
    }

    #[inline]
    fn get_opl3(&self, reg: u16) -> u8 {
        if (reg as usize) < 512 {
            self.opl3_regs[reg as usize]
        } else {
            0
        }
    }

    #[inline]
    fn rd(&self, off: usize) -> u8 {
        rb(&self.tune, off)
    }

    // --- accessors that select an effect / riff slot at runtime ------------

    #[inline]
    fn fx_ref(&self, ch: usize, loc: FxLoc) -> &CEffects {
        match loc {
            FxLoc::Chan => &self.channels[ch].fx,
            FxLoc::Riff => &self.channels[ch].riff.fx,
            FxLoc::IRiff => &self.channels[ch].iriff.fx,
        }
    }

    #[inline]
    fn fx_mut(&mut self, ch: usize, loc: FxLoc) -> &mut CEffects {
        match loc {
            FxLoc::Chan => &mut self.channels[ch].fx,
            FxLoc::Riff => &mut self.channels[ch].riff.fx,
            FxLoc::IRiff => &mut self.channels[ch].iriff.fx,
        }
    }

    // cr == true selects the channel riff (Riff), false the instrument riff (IRiff),
    // mirroring the reference's `chan_riff` argument.
    #[inline]
    fn rmut(&mut self, ch: usize, cr: bool) -> &mut CRiff {
        if cr {
            &mut self.channels[ch].riff
        } else {
            &mut self.channels[ch].iriff
        }
    }

    // --- reset -------------------------------------------------------------

    // The play-state portion of the reference's Stop(): does NOT emit register
    // writes (there is no callback yet). The chip reset writes are deferred to
    // the first update() (see reset_chip).
    fn reset_play_state(&mut self) {
        self.play_time = 0;
        self.repeating = false;
        for i in 0..4 {
            self.order_map[i] = 0;
        }

        self.speed_cnt = 1;
        self.order = 0;
        self.track = self.get_track();
        self.line = 0;
        self.entrances = 0;
        self.master_vol = 64;

        for i in 0..K_CHANNELS {
            let chan = &mut self.channels[i];
            chan.last_instrument = 0;
            chan.instrument = None;
            chan.volume = 0;
            chan.detune_a = 0;
            chan.detune_b = 0;
            chan.key_flags = 0;
            chan.riff.speed_cnt = 0;
            chan.iriff.speed_cnt = 0;
        }
    }

    // The register-writing portion of the reference's Stop(): clears the chip
    // and configures OPL3 mode. Queued through set_opl3 like everything else.
    fn reset_chip(&mut self) {
        for reg in 0x20u16..0xF6u16 {
            // Ensure envelopes decay all the way.
            let val: u8 = if reg >= 0x60 && reg < 0xA0 { 0xFF } else { 0 };
            self.set_opl3(reg, val);
            self.set_opl3(reg + 0x100, val);
        }

        // Configure OPL3.
        self.set_opl3(1, 0x20); // Allow waveforms
        self.set_opl3(8, 0); // No split point
        self.set_opl3(0xbd, 0); // No drums, etc.
        self.set_opl3(0x104, 0); // Everything 2-op by default
        self.set_opl3(0x105, 1); // OPL3 mode on
    }

    // --- main tick ---------------------------------------------------------

    /// Advance one tick, emitting OPL register writes via `write`. Returns
    /// `false` once the song has played through (looped back to the start).
    pub fn update(&mut self, write: &mut dyn FnMut(u16, u8)) -> bool {
        self.pending.clear();

        // Lazy chip reset before the very first tick.
        if !self.chip_reset_done {
            self.reset_chip();
            self.chip_reset_done = true;
        }

        // Run riffs.
        for i in 0..K_CHANNELS {
            self.tick_riff(i, false);
            self.tick_riff(i, true);
        }

        // Run main track.
        self.play_line();

        // Run effects.
        for i in 0..K_CHANNELS {
            self.continue_fx(i, FxLoc::IRiff);
            self.continue_fx(i, FxLoc::Riff);
            self.continue_fx(i, FxLoc::Chan);
        }

        self.play_time = self.play_time.wrapping_add(1);

        // Flush queued register writes in order.
        for k in 0..self.pending.len() {
            let (reg, val) = self.pending[k];
            write(reg, val);
        }
        self.pending.clear();

        // The reference's Update() returns `Repeating`; the AdPlug wrapper plays
        // while !Repeating. We return false once the order list wraps to an
        // already-visited order (loop detected in get_track).
        !self.repeating
    }

    // --- note / track unpacking --------------------------------------------

    // Unpacks a single RAD note, advancing `s`. Exports into note_num/octave_num/
    // inst_num/effect_num/param, updates `last_instrument`, returns the last-channel flag.
    fn unpack_note(&mut self, s: &mut usize, last_instrument: &mut u8) -> bool {
        let chanid = self.rd(*s);
        *s += 1;

        self.inst_num = 0;
        self.effect_num = 0;
        self.param = 0;

        let mut note: u8 = 0;

        if self.version >= 2 {
            // Version 2 notes.
            if (chanid & 0x40) != 0 {
                let n = self.rd(*s);
                *s += 1;
                note = n & 0x7F;
                if (n & 0x80) != 0 {
                    self.inst_num = *last_instrument;
                }
            }
            if (chanid & 0x20) != 0 {
                self.inst_num = self.rd(*s);
                *s += 1;
                *last_instrument = self.inst_num;
            }
            if (chanid & 0x10) != 0 {
                self.effect_num = self.rd(*s);
                *s += 1;
                self.param = self.rd(*s);
                *s += 1;
            }
        } else {
            // Version 1 notes.
            let n = self.rd(*s);
            *s += 1;
            note = n & 0x7f;
            if (n & 0x80) != 0 {
                self.inst_num = 16;
            }
            let n = self.rd(*s);
            *s += 1;
            self.inst_num |= n >> 4;
            if self.inst_num != 0 {
                *last_instrument = self.inst_num;
            }
            self.effect_num = n & 0xf;
            if self.effect_num != 0 {
                self.param = self.rd(*s);
                *s += 1;
            }
        }

        self.note_num = (note & 15) as i8;
        self.octave_num = (note >> 4) as i8;

        (chanid & 0x80) != 0
    }

    // Get the current track as indicated by the order list, and detect repeats.
    fn get_track(&mut self) -> Option<usize> {
        if self.order >= self.order_list_size {
            self.order = 0;
        }

        let mut track_num = self.rd(self.order_list + self.order as usize);

        // Jump marker? (single-level only, to avoid infinite loops.)
        if (track_num & 0x80) != 0 {
            self.order = track_num & 0x7F;
            track_num = self.rd(self.order_list + self.order as usize) & 0x7F;
        }

        // Check for tune repeat, and mark the order in the order map.
        if self.order < 128 {
            let byte = (self.order >> 5) as usize;
            let bit = 1u32 << (self.order & 31);
            if self.order_map[byte] & bit != 0 {
                self.repeating = true;
            } else {
                self.order_map[byte] |= bit;
            }
        }

        self.tracks.get(track_num as usize).copied().flatten()
    }

    // Skip through a track until we reach `linenum` or the next higher line.
    // Returns None if there is no such line.
    fn skip_to_line(&self, mut trk: usize, linenum: u8, chan_riff: bool) -> Option<usize> {
        let end = self.tune.len();
        loop {
            // Safety guard (not in the reference, which assumes validated data):
            // past the end every read is 0, which never terminates the walk.
            if trk >= end {
                return None;
            }
            let lineid = self.rd(trk);
            if (lineid & 0x7F) >= linenum {
                return Some(trk);
            }
            if (lineid & 0x80) != 0 {
                break;
            }
            trk += 1;

            // Skip channel notes.
            loop {
                if trk >= end {
                    return None;
                }
                let chanid = self.rd(trk);
                trk += 1;
                if self.version >= 2 {
                    trk += NOTE_SIZE[((chanid >> 4) & 7) as usize] as usize;
                } else if self.rd(trk + 1) & 0xf != 0 {
                    // v1 note with param
                    trk += 3;
                } else {
                    // v1 note without param
                    trk += 2;
                }
                if (chanid & 0x80) != 0 || chan_riff {
                    break;
                }
            }
        }

        None
    }

    // Plays one line of the current track and advances pointers.
    fn play_line(&mut self) {
        self.speed_cnt = self.speed_cnt.wrapping_sub(1);
        if self.speed_cnt > 0 {
            return;
        }
        self.speed_cnt = self.speed;

        // Reset channel effects.
        for i in 0..K_CHANNELS {
            self.channels[i].fx.reset();
        }

        self.line_jump = -1;

        // At the right line?
        if let Some(mut trk) = self.track {
            if (self.rd(trk) & 0x7F) <= self.line {
                let lineid = self.rd(trk);
                trk += 1;

                // Run through channels.
                let mut overran = false;
                loop {
                    // Safety guard (not in the reference): a truncated line has
                    // no 0x80 terminator, so cap the walk at the buffer end.
                    if trk >= self.tune.len() {
                        overran = true;
                        break;
                    }
                    let channum = (self.rd(trk) & 15) as usize;
                    let last;
                    if channum < K_CHANNELS {
                        let mut li = self.channels[channum].last_instrument;
                        last = self.unpack_note(&mut trk, &mut li);
                        self.channels[channum].last_instrument = li;
                        let n = self.note_num;
                        let o = self.octave_num;
                        let inst = self.inst_num as u16;
                        let eff = self.effect_num;
                        let par = self.param;
                        self.play_note(channum, n, o, inst, eff, par, Source::None, 0);
                    } else {
                        let mut li = 0u8;
                        last = self.unpack_note(&mut trk, &mut li);
                    }
                    if last {
                        break;
                    }
                }

                // Was this the last line?
                if (lineid & 0x80) != 0 || overran {
                    self.track = None;
                } else {
                    self.track = Some(trk);
                }
            }
        }

        // Move to next line.
        self.line = self.line.wrapping_add(1);
        if self.line >= K_TRACK_LINES || self.line_jump >= 0 {
            if self.line_jump >= 0 {
                self.line = self.line_jump as u8;
            } else {
                self.line = 0;
            }

            // Move to the next track in the order list.
            self.order = self.order.wrapping_add(1);
            self.track = self.get_track();
            if self.line > 0 {
                if let Some(t) = self.track {
                    self.track = self.skip_to_line(t, self.line, false);
                }
            }
        }
    }

    // Runs the tone-slide effect body (shared by the cmToneSlide `goto` path and
    // the cmToneVolSlide fall-through).
    fn toneslide_effect(&mut self, channum: usize, loc: FxLoc, param: u8) {
        let speed = param;
        if speed != 0 {
            self.fx_mut(channum, loc).tone_slide_speed = speed;
        }
        self.get_slide_dir(channum, loc);
    }

    // Play a single note (and process its effect command).
    #[allow(clippy::too_many_arguments)]
    fn play_note(&mut self, channum: usize, notenum: i8, octave: i8, instnum: u16, cmd: u8, param: u8, src: Source, op: i32) {
        // Recursion detector: riffs can trigger other riffs and loop.
        if self.entrances >= 8 {
            return;
        }
        self.entrances += 1;

        let fxloc = match src {
            Source::None => FxLoc::Chan,
            Source::Riff => FxLoc::Riff,
            Source::IRiff => FxLoc::IRiff,
        };

        let mut transposing = false;

        // For tone-slides the note is the target; skip everything else (the
        // reference `goto toneslide`s straight into the effect body).
        if cmd == CM_TONE_SLIDE {
            if notenum > 0 && notenum <= 12 {
                let fx = self.fx_mut(channum, fxloc);
                fx.tone_slide_oct = octave as u8;
                fx.tone_slide_freq = NOTE_FREQ[(notenum - 1) as usize];
            }
            self.toneslide_effect(channum, fxloc, param);
            self.entrances -= 1;
            return;
        }

        // Playing a new instrument?
        if instnum > 0 {
            let idx = (instnum as usize).wrapping_sub(1);
            if idx < K_INSTRUMENTS {
                let oldinst = self.channels[channum].instrument;
                let inst_new = Some(idx);
                self.channels[channum].instrument = inst_new;

                let alg = self.instruments[idx].algorithm;
                if alg < 7 {
                    self.load_instrument_opl3(channum);

                    // Bounce the channel.
                    self.channels[channum].key_flags |= F_KEY_OFF | F_KEY_ON;

                    self.channels[channum].iriff.fx.reset();

                    if src != Source::IRiff || inst_new != oldinst {
                        let riff = self.instruments[idx].riff;
                        let riff_speed = self.instruments[idx].riff_speed;

                        if riff.is_some() && riff_speed > 0 {
                            self.channels[channum].iriff.track = riff;
                            self.channels[channum].iriff.track_start = riff.unwrap();
                            self.channels[channum].iriff.line = 0;
                            self.channels[channum].iriff.speed = riff_speed;
                            self.channels[channum].iriff.last_instrument = 0;

                            // Note given with the riff command transposes the riff.
                            if notenum >= 1 && notenum <= 12 {
                                self.channels[channum].iriff.transpose_octave = octave;
                                self.channels[channum].iriff.transpose_note = notenum;
                                transposing = true;
                            } else {
                                self.channels[channum].iriff.transpose_octave = 3;
                                self.channels[channum].iriff.transpose_note = 12;
                            }

                            // Do first tick of riff.
                            self.channels[channum].iriff.speed_cnt = 1;
                            self.tick_riff(channum, false);
                        } else {
                            self.channels[channum].iriff.speed_cnt = 0;
                        }
                    }
                } else {
                    // Ignore MIDI instruments.
                    self.channels[channum].instrument = None;
                }
            }
        }

        // Starting a channel riff?
        if cmd == CM_RIFF || cmd == CM_TRANSPOSE {
            self.channels[channum].riff.fx.reset();

            let p0 = (param / 10) as usize;
            let p1 = param % 10;
            let track = if p1 > 0 {
                self.riffs.get(p0).and_then(|row| row.get((p1 - 1) as usize)).copied().flatten()
            } else {
                None
            };
            self.channels[channum].riff.track = track;

            if let Some(t) = track {
                self.channels[channum].riff.track_start = t;
                self.channels[channum].riff.line = 0;
                self.channels[channum].riff.speed = self.speed;
                self.channels[channum].riff.last_instrument = 0;

                if cmd == CM_TRANSPOSE && notenum >= 1 && notenum <= 12 {
                    self.channels[channum].riff.transpose_octave = octave;
                    self.channels[channum].riff.transpose_note = notenum;
                    transposing = true;
                } else {
                    self.channels[channum].riff.transpose_octave = 3;
                    self.channels[channum].riff.transpose_note = 12;
                }

                self.channels[channum].riff.speed_cnt = 1;
                self.tick_riff(channum, true);
            } else {
                self.channels[channum].riff.speed_cnt = 0;
            }
        }

        // Play the note.
        if !transposing && notenum > 0 {
            // Key-off?
            if notenum == 15 {
                self.channels[channum].key_flags |= F_KEY_OFF;
            }

            let alg_ok = match self.channels[channum].instrument {
                None => true,
                Some(i) => self.instruments[i].algorithm < 7,
            };
            if alg_ok {
                self.play_note_opl3(channum, octave, notenum);
            }
        }

        // Process effect.
        match cmd {
            CM_SET_VOL => self.set_volume(channum, param),
            CM_SET_SPEED => match src {
                Source::None => {
                    self.speed = param;
                    self.speed_cnt = param;
                }
                Source::Riff => {
                    self.channels[channum].riff.speed = param;
                    self.channels[channum].riff.speed_cnt = param;
                }
                Source::IRiff => {
                    self.channels[channum].iriff.speed = param;
                    self.channels[channum].iriff.speed_cnt = param;
                }
            },
            CM_PORTAMENTO_UP => {
                self.fx_mut(channum, fxloc).port_slide = param as i8;
            }
            CM_PORTAMENTO_DWN => {
                self.fx_mut(channum, fxloc).port_slide = (param as i8).wrapping_neg();
            }
            CM_TONE_VOL_SLIDE | CM_VOL_SLIDE => {
                let mut val = param as i8;
                if val >= 50 {
                    val = -(val - 50);
                }
                self.fx_mut(channum, fxloc).vol_slide = val;
                if cmd == CM_TONE_VOL_SLIDE {
                    // Fall through to the tone-slide body.
                    self.toneslide_effect(channum, fxloc, param);
                }
            }
            CM_JUMP_TO_LINE => {
                if param < K_TRACK_LINES && src == Source::None {
                    self.line_jump = param as i8;
                }
            }
            CM_MULTIPLIER => {
                if src == Source::IRiff {
                    self.load_inst_multiplier(channum, op, param);
                }
            }
            CM_VOLUME => {
                if src == Source::IRiff {
                    self.load_inst_volume(channum, op, param);
                }
            }
            CM_FEEDBACK => {
                if src == Source::IRiff {
                    let which = param / 10;
                    let fb = param % 10;
                    self.load_inst_feedback(channum, which, fb);
                }
            }
            _ => {}
        }

        self.entrances -= 1;
    }

    // Sets the OPL3 registers for a given instrument.
    fn load_instrument_opl3(&mut self, channum: usize) {
        let idx = match self.channels[channum].instrument {
            Some(i) => i,
            None => return,
        };

        let alg = self.instruments[idx].algorithm;
        self.channels[channum].volume = self.instruments[idx].volume;
        self.channels[channum].detune_a = (self.instruments[idx].detune + 1) >> 1;
        self.channels[channum].detune_b = self.instruments[idx].detune >> 1;

        // Turn on 4-op mode for algorithms 2 and 3.
        if self.use_opl3 && channum < 6 {
            let mask = 1u8 << channum;
            let cur = self.get_opl3(0x104);
            let bit = if alg == 2 || alg == 3 { mask } else { 0 };
            self.set_opl3(0x104, (cur & !mask) | bit);
        }

        // Left/right/feedback/algorithm.
        if self.use_opl3 {
            let p1 = self.instruments[idx].panning[1];
            let fb1 = self.instruments[idx].feedback[1];
            let v1 = (((p1 ^ 3) as u16) << 4) | ((fb1 as u16) << 1) | (if alg == 3 || alg == 5 || alg == 6 { 1 } else { 0 });
            self.set_opl3(0xC0 + CHAN_OFFSETS3[channum], v1 as u8);

            let p0 = self.instruments[idx].panning[0];
            let fb0 = self.instruments[idx].feedback[0];
            let v2 = (((p0 ^ 3) as u16) << 4) | ((fb0 as u16) << 1) | (if alg == 1 || alg == 6 { 1 } else { 0 });
            self.set_opl3(0xC0 + CHN2_OFFSETS3[channum], v2 as u8);
        } else {
            let p0 = self.instruments[idx].panning[0];
            let fb0 = self.instruments[idx].feedback[0];
            let v = (((p0 ^ 3) as u16) << 4) | ((fb0 as u16) << 1) | (if alg == 1 { 1 } else { 0 });
            self.set_opl3(0xC0 + channum as u16, v as u8);
        }

        // Load the operators.
        const BLANK: [u8; 5] = [0, 0x3F, 0, 0xF0, 0];
        let n = if self.use_opl3 { 4 } else { 2 };
        for i in 0..n {
            let op: [u8; 5] = if alg < 2 && i >= 2 { BLANK } else { self.instruments[idx].operators[i] };
            let reg = if self.use_opl3 { OP_OFFSETS3[channum][i] } else { OP_OFFSETS2[channum][i] };

            let mut vol: u16 = (!op[1] & 0x3F) as u16;

            // Volume scaling for carriers.
            if ALG_CARRIERS[alg as usize][i] {
                vol = vol * self.instruments[idx].volume as u16 / 64;
                vol = vol * self.master_vol as u16 / 64;
            }

            self.set_opl3(reg + 0x20, op[0]);
            self.set_opl3(reg + 0x40, (op[1] & 0xC0) | (((vol ^ 0x3F) & 0x3F) as u8));
            self.set_opl3(reg + 0x60, op[2]);
            self.set_opl3(reg + 0x80, op[3]);
            self.set_opl3(reg + 0xE0, op[4]);
        }
    }

    // Play a note on the OPL3 hardware.
    fn play_note_opl3(&mut self, channum: usize, octave: i8, note: i8) {
        let (o1, o2): (u16, u16) = if self.use_opl3 {
            (CHAN_OFFSETS3[channum], CHN2_OFFSETS3[channum])
        } else {
            (0, channum as u16)
        };

        // Key off the channel.
        if self.channels[channum].key_flags & F_KEY_OFF != 0 {
            self.channels[channum].key_flags &= !(F_KEY_OFF | F_KEYED_ON);
            if self.use_opl3 {
                let v = self.get_opl3(0xB0 + o1) & !0x20;
                self.set_opl3(0xB0 + o1, v);
            }
            let v = self.get_opl3(0xB0 + o2) & !0x20;
            self.set_opl3(0xB0 + o2, v);
        }

        if note > 12 || note < 1 {
            return;
        }

        let op4 = self.use_opl3
            && match self.channels[channum].instrument {
                Some(i) => self.instruments[i].algorithm >= 2,
                None => false,
            };

        let freq0 = NOTE_FREQ[(note - 1) as usize];
        self.channels[channum].curr_freq = freq0;
        self.channels[channum].curr_octave = octave;

        // Detune both channels in opposite directions so tuning is retained.
        let freq = freq0.wrapping_add(self.channels[channum].detune_a as u16);
        let frq2 = freq0.wrapping_sub(self.channels[channum].detune_b as u16);

        // Frequency low byte.
        if op4 {
            self.set_opl3(0xA0 + o1, (frq2 & 0xFF) as u8);
        }
        self.set_opl3(0xA0 + o2, (freq & 0xFF) as u8);

        // Frequency high bits + octave + key on.
        if self.channels[channum].key_flags & F_KEY_ON != 0 {
            self.channels[channum].key_flags = (self.channels[channum].key_flags & !F_KEY_ON) | F_KEYED_ON;
        }
        let keyed: u16 = if self.channels[channum].key_flags & F_KEYED_ON != 0 { 0x20 } else { 0 };
        if op4 {
            self.set_opl3(0xB0 + o1, ((frq2 >> 8) | ((octave as u16) << 2) | keyed) as u8);
        } else if self.use_opl3 {
            self.set_opl3(0xB0 + o1, 0);
        }
        self.set_opl3(0xB0 + o2, ((freq >> 8) | ((octave as u16) << 2) | keyed) as u8);
    }

    // Tick a channel/instrument riff.
    fn tick_riff(&mut self, channum: usize, chan_riff: bool) {
        self.rmut(channum, chan_riff).updated = true;

        if self.rmut(channum, chan_riff).speed_cnt == 0 {
            self.rmut(channum, chan_riff).fx.reset();
            return;
        }

        let sc = self.rmut(channum, chan_riff).speed_cnt.wrapping_sub(1);
        self.rmut(channum, chan_riff).speed_cnt = sc;
        if sc > 0 {
            return;
        }
        let sp = self.rmut(channum, chan_riff).speed;
        self.rmut(channum, chan_riff).speed_cnt = sp;

        let line = self.rmut(channum, chan_riff).line;
        let nl = line.wrapping_add(1);
        self.rmut(channum, chan_riff).line = nl;
        if nl >= K_TRACK_LINES {
            self.rmut(channum, chan_riff).speed_cnt = 0;
        }

        self.rmut(channum, chan_riff).fx.reset();

        // Is this the current line in the track?
        let mut trk = self.rmut(channum, chan_riff).track;
        let mut lineid: u8 = 0;
        if let Some(mut t) = trk {
            if (self.rd(t) & 0x7F) == line {
                lineid = self.rd(t);
                t += 1;

                // The current riff may be clobbered by recursive riffs.
                self.rmut(channum, chan_riff).updated = false;

                if chan_riff {
                    // Channel riff: play the current note.
                    let mut li = self.rmut(channum, chan_riff).last_instrument;
                    self.unpack_note(&mut t, &mut li);
                    self.rmut(channum, chan_riff).last_instrument = li;
                    let tn = self.rmut(channum, chan_riff).transpose_note;
                    let to = self.rmut(channum, chan_riff).transpose_octave;
                    self.transpose(tn, to);
                    let n = self.note_num;
                    let o = self.octave_num;
                    let inst = self.inst_num as u16;
                    let eff = self.effect_num;
                    let par = self.param;
                    self.play_note(channum, n, o, inst, eff, par, Source::Riff, 0);
                } else {
                    // Instrument riff: each "channel" is an extra effect, not a
                    // separate physical channel.
                    loop {
                        // Safety guard (not in the reference): stop at buffer end.
                        if t >= self.tune.len() {
                            break;
                        }
                        let col = (self.rd(t) & 15) as i32;
                        let mut li = self.rmut(channum, chan_riff).last_instrument;
                        let last = self.unpack_note(&mut t, &mut li);
                        self.rmut(channum, chan_riff).last_instrument = li;
                        if self.effect_num != CM_IGNORE {
                            let tn = self.rmut(channum, chan_riff).transpose_note;
                            let to = self.rmut(channum, chan_riff).transpose_octave;
                            self.transpose(tn, to);
                        }
                        let opv = if col > 0 { (col - 1) & 3 } else { 0 };
                        let n = self.note_num;
                        let o = self.octave_num;
                        let inst = self.inst_num as u16;
                        let eff = self.effect_num;
                        let par = self.param;
                        self.play_note(channum, n, o, inst, eff, par, Source::IRiff, opv);
                        if last {
                            break;
                        }
                    }
                }

                // Exit if a recursive call replaced or stopped this riff.
                if self.rmut(channum, chan_riff).updated {
                    return;
                }
                self.rmut(channum, chan_riff).updated = true;

                if (lineid & 0x80) != 0 {
                    trk = None;
                } else {
                    trk = Some(t);
                }
                self.rmut(channum, chan_riff).track = trk;
            } else {
                trk = Some(t);
            }
        }

        // Special case: if the next line has a jump command, run it now.
        let mut t = match trk {
            Some(t) => t,
            None => return,
        };
        let val = self.rd(t);
        t += 1;
        if (val & 0x7F) != self.rmut(channum, chan_riff).line {
            return;
        }

        let mut dummy = lineid; // dummy last_instrument here
        self.unpack_note(&mut t, &mut dummy);
        if self.effect_num == CM_JUMP_TO_LINE && self.param < K_TRACK_LINES {
            self.rmut(channum, chan_riff).line = self.param;
            let ts = self.rmut(channum, chan_riff).track_start;
            let dst = self.skip_to_line(ts, self.param, chan_riff);
            self.rmut(channum, chan_riff).track = dst;
        }
    }

    // Continue effects that operate continuously (slides).
    fn continue_fx(&mut self, channum: usize, loc: FxLoc) {
        let fx = *self.fx_ref(channum, loc);

        if fx.port_slide != 0 {
            self.portamento(channum, loc, fx.port_slide, false);
        }

        if fx.vol_slide != 0 {
            let mut vol = (self.channels[channum].volume as i8).wrapping_sub(fx.vol_slide);
            if vol < 0 {
                vol = 0;
            }
            self.set_volume(channum, vol as u8);
        }

        if fx.tone_slide_dir != 0 {
            self.portamento(channum, loc, fx.tone_slide_dir, true);
        }
    }

    // Set the volume of a channel.
    fn set_volume(&mut self, channum: usize, vol_in: u8) {
        let mut vol = vol_in;
        if vol > 64 {
            vol = 64;
        }
        self.channels[channum].volume = vol;

        // Scale to master volume.
        vol = ((vol as u16 * self.master_vol as u16) / 64) as u8;

        let idx = match self.channels[channum].instrument {
            Some(i) => i,
            None => return,
        };
        let alg = self.instruments[idx].algorithm;

        for i in 0..4 {
            if !ALG_CARRIERS[alg as usize][i] {
                continue;
            }
            let op1 = self.instruments[idx].operators[i][1];
            let opvol = ((((op1 & 63) ^ 63) as u16 * vol as u16) / 64) as u8;
            let reg = 0x40 + if self.use_opl3 { OP_OFFSETS3[channum][i] } else { OP_OFFSETS2[channum][i] };
            let cur = self.get_opl3(reg);
            self.set_opl3(reg, (cur & 0xC0) | (opvol ^ 0x3F));
        }
    }

    // Start a tone-slide (compute its direction).
    fn get_slide_dir(&mut self, channum: usize, loc: FxLoc) {
        let fx = *self.fx_ref(channum, loc);
        let mut speed = fx.tone_slide_speed as i8;
        if speed > 0 {
            let oct = fx.tone_slide_oct;
            let freq = fx.tone_slide_freq;
            let oldfreq = self.channels[channum].curr_freq;
            let oldoct = self.channels[channum].curr_octave as u8;

            if oldoct > oct {
                speed = -speed;
            } else if oldoct == oct {
                if oldfreq > freq {
                    speed = -speed;
                } else if oldfreq == freq {
                    speed = 0;
                }
            }
        }
        self.fx_mut(channum, loc).tone_slide_dir = speed;
    }

    // Load a multiplier value into an operator (OPL3 numbering; v2 only).
    fn load_inst_multiplier(&mut self, channum: usize, op: i32, mult: u8) {
        let reg = 0x20 + OP_OFFSETS3[channum][op as usize];
        let cur = self.get_opl3(reg);
        self.set_opl3(reg, (cur & 0xF0) | (mult & 15));
    }

    // Load a volume value into an operator (OPL3 numbering; v2 only).
    fn load_inst_volume(&mut self, channum: usize, op: i32, vol: u8) {
        let reg = 0x40 + OP_OFFSETS3[channum][op as usize];
        let cur = self.get_opl3(reg);
        self.set_opl3(reg, (cur & 0xC0) | ((vol & 0x3F) ^ 0x3F));
    }

    // Load a feedback value into an instrument (OPL3 numbering; v2 only).
    fn load_inst_feedback(&mut self, channum: usize, which: u8, fb: u8) {
        if which == 0 {
            let reg = 0xC0 + CHN2_OFFSETS3[channum];
            let cur = self.get_opl3(reg);
            self.set_opl3(reg, (cur & 0x31) | ((fb & 7) << 1));
        } else if which == 1 {
            let reg = 0xC0 + CHAN_OFFSETS3[channum];
            let cur = self.get_opl3(reg);
            self.set_opl3(reg, (cur & 0x31) | ((fb & 7) << 1));
        }
    }

    // Adjust the pitch of a channel's note, optionally clamped (tone slides).
    fn portamento(&mut self, channum: usize, loc: FxLoc, amount: i8, toneslide: bool) {
        let mut freq = self.channels[channum].curr_freq;
        let mut oct = self.channels[channum].curr_octave as u8;

        freq = freq.wrapping_add(amount as u16);

        if freq < 0x156 {
            if oct > 0 {
                oct -= 1;
                freq = freq.wrapping_add(0x2AE - 0x156);
            } else {
                freq = 0x156;
            }
        } else if freq > 0x2AE {
            if oct < 7 {
                oct += 1;
                freq -= 0x2AE - 0x156;
            } else {
                freq = 0x2AE;
            }
        }

        if toneslide {
            let ts_oct = self.fx_ref(channum, loc).tone_slide_oct;
            let ts_freq = self.fx_ref(channum, loc).tone_slide_freq;
            if amount >= 0 {
                if oct > ts_oct || (oct == ts_oct && freq >= ts_freq) {
                    freq = ts_freq;
                    oct = ts_oct;
                }
            } else if oct < ts_oct || (oct == ts_oct && freq <= ts_freq) {
                freq = ts_freq;
                oct = ts_oct;
            }
        }

        self.channels[channum].curr_freq = freq;
        self.channels[channum].curr_octave = oct as i8;

        // Apply detunes.
        let frq2 = freq.wrapping_sub(self.channels[channum].detune_b as u16);
        let freqd = freq.wrapping_add(self.channels[channum].detune_a as u16);

        let chan_offset = if self.use_opl3 { CHN2_OFFSETS3[channum] } else { channum as u16 };
        self.set_opl3(0xA0 + chan_offset, (freqd & 0xFF) as u8);
        let cur = self.get_opl3(0xB0 + chan_offset);
        self.set_opl3(0xB0 + chan_offset, (((freqd >> 8) & 3) | ((oct as u16) << 2) | ((cur as u16) & 0xE0)) as u8);

        if self.use_opl3 {
            let chan_offset2 = CHAN_OFFSETS3[channum];
            self.set_opl3(0xA0 + chan_offset2, (frq2 & 0xFF) as u8);
            let cur2 = self.get_opl3(0xB0 + chan_offset2);
            self.set_opl3(0xB0 + chan_offset2, (((frq2 >> 8) & 3) | ((oct as u16) << 2) | ((cur2 as u16) & 0xE0)) as u8);
        }
    }

    // Transpose the note exported by unpack_note().
    fn transpose(&mut self, note: i8, octave: i8) {
        if self.note_num >= 1 && self.note_num <= 12 {
            let toct = octave - 3;
            if toct != 0 {
                self.octave_num += toct;
                if self.octave_num < 0 {
                    self.octave_num = 0;
                } else if self.octave_num > 7 {
                    self.octave_num = 7;
                }
            }

            let tnot = note - 12;
            if tnot != 0 {
                self.note_num += tnot;
                if self.note_num < 1 {
                    self.note_num += 12;
                    if self.octave_num > 0 {
                        self.octave_num -= 1;
                    } else {
                        self.note_num = 1;
                    }
                }
            }
        }
    }
}

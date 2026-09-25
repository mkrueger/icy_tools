// OPL3 FM-synth emulator — a faithful Rust port of "Opal", the OPL3 emulator from
// Reality Adlib Tracker v2.0a, released into the public domain by Shayde/Reality
// (with fixes by JP Cimalando / OpenMPT). Original: opal.h, a single C++ class.
// Ported via kaleidotron (MIT, Copyright (c) 2026 Rick Christy).
//
// This is a mechanical, correctness-critical translation. The integer types,
// wrapping arithmetic, lookup tables, operator/channel routing, envelope
// generator, phase accumulators and the internal sample-rate conversion are all
// preserved from the C++ source. The one structural change forced by Rust is that
// the C++ back-pointers (`Operator::Chip`, `Operator::Chan`, `Channel::Chip`) are
// removed: operators and channels live in the `Opl3` chip as flat arrays indexed by
// number, and the chip-level state each method needs is passed in as parameters
// (or the method is promoted to a chip method). The MATH is identical.
//
// Missing (same as Opal): timers/interrupts, OPL3-enable bit (always on), CSW mode,
// test register, percussion mode.

#![allow(clippy::all)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

// ---- Constants -------------------------------------------------------------

const OPL3_SAMPLE_RATE: i32 = 49716;
const NUM_CHANNELS: usize = 18;
const NUM_OPERATORS: usize = 36;

// Envelope stages (C++ enum: EnvOff = -1, then EnvAtt, EnvDec, EnvSus, EnvRel).
const ENV_OFF: i32 = -1;
const ENV_ATT: i32 = 0;
const ENV_DEC: i32 = 1;
const ENV_SUS: i32 = 2;
const ENV_REL: i32 = 3;

// ---- Static lookup tables (verbatim from opal.h) ---------------------------

static RATE_TABLES: [[u16; 8]; 4] = [
    [1, 0, 1, 0, 1, 0, 1, 0],
    [1, 0, 1, 0, 0, 0, 1, 0],
    [1, 0, 0, 0, 1, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0],
];

static EXP_TABLE: [u16; 256] = [
    1018, 1013, 1007, 1002, 996, 991, 986, 980, 975, 969, 964, 959, 953, 948, 942, 937, 932, 927, 921, 916, 911, 906, 900, 895, 890, 885, 880, 874, 869, 864,
    859, 854, 849, 844, 839, 834, 829, 824, 819, 814, 809, 804, 799, 794, 789, 784, 779, 774, 770, 765, 760, 755, 750, 745, 741, 736, 731, 726, 722, 717, 712,
    708, 703, 698, 693, 689, 684, 680, 675, 670, 666, 661, 657, 652, 648, 643, 639, 634, 630, 625, 621, 616, 612, 607, 603, 599, 594, 590, 585, 581, 577, 572,
    568, 564, 560, 555, 551, 547, 542, 538, 534, 530, 526, 521, 517, 513, 509, 505, 501, 496, 492, 488, 484, 480, 476, 472, 468, 464, 460, 456, 452, 448, 444,
    440, 436, 432, 428, 424, 420, 416, 412, 409, 405, 401, 397, 393, 389, 385, 382, 378, 374, 370, 367, 363, 359, 355, 352, 348, 344, 340, 337, 333, 329, 326,
    322, 318, 315, 311, 308, 304, 300, 297, 293, 290, 286, 283, 279, 276, 272, 268, 265, 262, 258, 255, 251, 248, 244, 241, 237, 234, 231, 227, 224, 220, 217,
    214, 210, 207, 204, 200, 197, 194, 190, 187, 184, 181, 177, 174, 171, 168, 164, 161, 158, 155, 152, 148, 145, 142, 139, 136, 133, 130, 126, 123, 120, 117,
    114, 111, 108, 105, 102, 99, 96, 93, 90, 87, 84, 81, 78, 75, 72, 69, 66, 63, 60, 57, 54, 51, 48, 45, 42, 40, 37, 34, 31, 28, 25, 22, 20, 17, 14, 11, 8, 6,
    3, 0,
];

static LOG_SIN_TABLE: [u16; 256] = [
    2137, 1731, 1543, 1419, 1326, 1252, 1190, 1137, 1091, 1050, 1013, 979, 949, 920, 894, 869, 846, 825, 804, 785, 767, 749, 732, 717, 701, 687, 672, 659, 646,
    633, 621, 609, 598, 587, 576, 566, 556, 546, 536, 527, 518, 509, 501, 492, 484, 476, 468, 461, 453, 446, 439, 432, 425, 418, 411, 405, 399, 392, 386, 380,
    375, 369, 363, 358, 352, 347, 341, 336, 331, 326, 321, 316, 311, 307, 302, 297, 293, 289, 284, 280, 276, 271, 267, 263, 259, 255, 251, 248, 244, 240, 236,
    233, 229, 226, 222, 219, 215, 212, 209, 205, 202, 199, 196, 193, 190, 187, 184, 181, 178, 175, 172, 169, 167, 164, 161, 159, 156, 153, 151, 148, 146, 143,
    141, 138, 136, 134, 131, 129, 127, 125, 122, 120, 118, 116, 114, 112, 110, 108, 106, 104, 102, 100, 98, 96, 94, 92, 91, 89, 87, 85, 83, 82, 80, 78, 77, 75,
    74, 72, 70, 69, 67, 66, 64, 63, 62, 60, 59, 57, 56, 55, 53, 52, 51, 49, 48, 47, 46, 45, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27,
    26, 25, 24, 23, 23, 22, 21, 20, 20, 19, 18, 17, 17, 16, 15, 15, 14, 13, 13, 12, 12, 11, 10, 10, 9, 9, 8, 8, 7, 7, 7, 6, 6, 5, 5, 5, 4, 4, 4, 3, 3, 3, 2, 2,
    2, 2, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0,
];

// Key-scale-level table (Operator::ComputeKeyScaleLevel), 8 rows x 16.
static LEVTAB: [u8; 128] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 12, 16, 20, 24, 28, 32, 0, 0, 0, 0, 0, 12, 20, 28, 32, 40, 44, 48, 52, 56,
    60, 64, 0, 0, 0, 20, 32, 44, 52, 60, 64, 72, 76, 80, 84, 88, 92, 96, 0, 0, 32, 52, 64, 76, 84, 92, 96, 104, 108, 112, 116, 120, 124, 128, 0, 32, 64, 84,
    96, 108, 116, 124, 128, 136, 140, 144, 148, 152, 156, 160, 0, 64, 96, 116, 128, 140, 148, 156, 160, 168, 172, 176, 180, 184, 188, 192, 0, 96, 128, 148,
    160, 172, 180, 188, 192, 200, 204, 208, 212, 216, 220, 224,
];

// Frequency-multiplier table * 2 (Operator::SetFrequencyMultiplier).
static MUL_TIMES_2: [u16; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];

// Key-scale shift table (Operator::SetKeyScale).
static KSL_SHIFT: [u16; 4] = [8, 1, 2, 0];

// Register byte -> operator index within a bank (Opal::Port op_lookup). -1 = none.
static OP_LOOKUP: [i8; 32] = [
    0, 1, 2, 3, 4, 5, -1, -1, 6, 7, 8, 9, 10, 11, -1, -1, 12, 13, 14, 15, 16, 17, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
];

// ---- muldivr (OpenMPT's Util::muldivr) -------------------------------------
// Rounded multiply-divide: (a * b + c/2) / c, computed in 64 bits.
#[inline]
fn muldivr(a: i32, b: i32, c: i32) -> i32 {
    ((a as i64 * b as i64 + (c / 2) as i64) / c as i64) as i32
}

#[inline]
fn clamp16(v: i32) -> i16 {
    if v < -0x8000 {
        -0x8000
    } else if v > 0x7FFF {
        0x7FFF
    } else {
        v as i16
    }
}

// ---- Operator --------------------------------------------------------------

#[derive(Clone, Copy)]
struct Operator {
    phase: u32,            // current offset in the selected waveform
    waveform: u16,         // waveform id
    freq_mult_times2: u16, // frequency multiplier * 2
    envelope_stage: i32,   // Env* stage
    envelope_level: i16,   // 0 - 0x1FF, 0 being loudest
    output_level: u16,     // 0 - 0xFF (val * 4)
    attack_rate: u16,
    decay_rate: u16,
    sustain_level: u16,
    release_rate: u16,
    attack_shift: u16,
    attack_mask: u16,
    attack_add: u16,
    attack_tab: usize, // index into RATE_TABLES (rate_low)
    decay_shift: u16,
    decay_mask: u16,
    decay_add: u16,
    decay_tab: usize,
    release_shift: u16,
    release_mask: u16,
    release_add: u16,
    release_tab: usize,
    key_scale_shift: u16,
    key_scale_level: u16,
    out: [i16; 2],
    key_on: bool,
    key_scale_rate: bool, // KSR: affects envelope rate scaling
    sustain_mode: bool,
    tremolo_enable: bool,
    vibrato_enable: bool,
}

impl Operator {
    fn new() -> Self {
        Operator {
            phase: 0,
            waveform: 0,
            freq_mult_times2: 1,
            envelope_stage: ENV_OFF,
            envelope_level: 0x1FF,
            output_level: 0,
            attack_rate: 0,
            decay_rate: 0,
            sustain_level: 0,
            release_rate: 0,
            attack_shift: 0,
            attack_mask: 0,
            attack_add: 0,
            attack_tab: 0,
            decay_shift: 0,
            decay_mask: 0,
            decay_add: 0,
            decay_tab: 0,
            release_shift: 0,
            release_mask: 0,
            release_add: 0,
            release_tab: 0,
            key_scale_shift: 0,
            key_scale_level: 0,
            out: [0, 0],
            key_on: false,
            key_scale_rate: false,
            sustain_mode: false,
            tremolo_enable: false,
            vibrato_enable: false,
        }
    }

    // Opal::Operator::Output. `_keyscalenum` matches the C++ signature but is unused
    // there too. `tremolo_level`/`clock` are the chip-level state the C++ read via
    // the Master back-pointer.
    fn output(&mut self, _keyscalenum: u16, phase_step: u32, vibrato: i16, mod_in: i16, fbshift: i16, tremolo_level: u16, clock: u16) -> i16 {
        // Advance wave phase.
        let mut ps = phase_step;
        if self.vibrato_enable {
            ps = ps.wrapping_add(vibrato as u32);
        }
        self.phase = self.phase.wrapping_add(ps.wrapping_mul(self.freq_mult_times2 as u32) / 2);

        let level: u16 = (((self.envelope_level as i32)
            + (self.output_level as i32)
            + (self.key_scale_level as i32)
            + (if self.tremolo_enable { tremolo_level as i32 } else { 0 }))
            << 3) as u16;

        match self.envelope_stage {
            // Attack stage
            ENV_ATT => {
                let idx = ((clock >> (self.attack_shift as u32)) & 7) as usize;
                let mut add: u16 = (((self.attack_add >> (RATE_TABLES[self.attack_tab][idx] as u32)) as i32) * (!(self.envelope_level as i32)) >> 3) as u16;
                if self.attack_rate == 0 {
                    add = 0;
                }
                if self.attack_mask != 0 && (clock & self.attack_mask) != 0 {
                    add = 0;
                }
                self.envelope_level = (self.envelope_level as i32 + add as i32) as i16;
                if self.envelope_level <= 0 {
                    self.envelope_level = 0;
                    self.envelope_stage = ENV_DEC;
                }
            }

            // Decay stage
            ENV_DEC => {
                let idx = ((clock >> (self.decay_shift as u32)) & 7) as usize;
                let mut add: u16 = self.decay_add >> (RATE_TABLES[self.decay_tab][idx] as u32);
                if self.decay_rate == 0 {
                    add = 0;
                }
                if self.decay_mask != 0 && (clock & self.decay_mask) != 0 {
                    add = 0;
                }
                self.envelope_level = (self.envelope_level as i32 + add as i32) as i16;
                if self.envelope_level as i32 >= self.sustain_level as i32 {
                    self.envelope_level = self.sustain_level as i16;
                    self.envelope_stage = ENV_SUS;
                }
            }

            // Sustain stage — falls through to release when not in sustain mode.
            ENV_SUS => {
                if !self.sustain_mode {
                    if self.release_step(clock) {
                        return 0;
                    }
                }
                // sustain_mode: break (no change)
            }

            // Release stage
            ENV_REL => {
                if self.release_step(clock) {
                    return 0;
                }
            }

            // Envelope, and therefore the operator, is not running.
            _ => {
                self.out = [0, 0];
                return 0;
            }
        }

        // Feedback? Modulate by a blend of the last two samples.
        let mut mod_v = mod_in;
        if fbshift != 0 {
            let fb_sum = (self.out[0] as i32 + self.out[1] as i32) >> (fbshift as u32);
            mod_v = (mod_v as i32 + fb_sum) as i16;
        }

        let phase: u16 = ((self.phase >> 10) as u16).wrapping_add(mod_v as u16);
        let mut offset: u16 = phase & 0xFF;
        let mut logsin: u16;
        let mut negate = false;

        match self.waveform {
            // Standard sine wave
            0 => {
                if phase & 0x100 != 0 {
                    offset ^= 0xFF;
                }
                logsin = LOG_SIN_TABLE[offset as usize];
                negate = (phase & 0x200) != 0;
            }

            // Half sine wave
            1 => {
                if phase & 0x200 != 0 {
                    offset = 0;
                } else if phase & 0x100 != 0 {
                    offset ^= 0xFF;
                }
                logsin = LOG_SIN_TABLE[offset as usize];
            }

            // Positive sine wave
            2 => {
                if phase & 0x100 != 0 {
                    offset ^= 0xFF;
                }
                logsin = LOG_SIN_TABLE[offset as usize];
            }

            // Quarter positive sine wave
            3 => {
                if phase & 0x100 != 0 {
                    offset = 0;
                }
                logsin = LOG_SIN_TABLE[offset as usize];
            }

            // Double-speed sine wave
            4 => {
                if phase & 0x200 != 0 {
                    offset = 0;
                } else {
                    if phase & 0x80 != 0 {
                        offset ^= 0xFF;
                    }
                    offset = (offset + offset) & 0xFF;
                    negate = (phase & 0x100) != 0;
                }
                logsin = LOG_SIN_TABLE[offset as usize];
            }

            // Double-speed positive sine wave
            5 => {
                if phase & 0x200 != 0 {
                    offset = 0;
                } else {
                    offset = (offset + offset) & 0xFF;
                    if phase & 0x80 != 0 {
                        offset ^= 0xFF;
                    }
                }
                logsin = LOG_SIN_TABLE[offset as usize];
            }

            // Square wave
            6 => {
                logsin = 0;
                negate = (phase & 0x200) != 0;
            }

            // Exponentiation wave
            _ => {
                logsin = phase & 0x1FF;
                if phase & 0x200 != 0 {
                    logsin ^= 0x1FF;
                    negate = true;
                }
                logsin <<= 3;
            }
        }

        let mut mix: u16 = (logsin as i32 + level as i32) as u16;
        if mix > 0x1FFF {
            mix = 0x1FFF;
        }

        // Exponential: table read at the 8 LSBs, + hidden bit 1024, shifted by the
        // remaining MSBs (the exponent).
        let mut v: i16 = ((EXP_TABLE[(mix & 0xFF) as usize] as u32 + 1024u32) >> ((mix >> 8) as u32)) as i16;
        v = v.wrapping_add(v);
        if negate {
            v = !v;
        }

        // Keep last two results for feedback.
        self.out[1] = self.out[0];
        self.out[0] = v;

        v
    }

    // The release-stage body, shared by ENV_REL and the ENV_SUS fall-through.
    // Returns true when the operator has finished (caller must `return 0`).
    #[inline]
    fn release_step(&mut self, clock: u16) -> bool {
        let idx = ((clock >> (self.release_shift as u32)) & 7) as usize;
        let mut add: u16 = self.release_add >> (RATE_TABLES[self.release_tab][idx] as u32);
        if self.release_rate == 0 {
            add = 0;
        }
        if self.release_mask != 0 && (clock & self.release_mask) != 0 {
            add = 0;
        }
        self.envelope_level = (self.envelope_level as i32 + add as i32) as i16;
        if self.envelope_level >= 0x1FF {
            self.envelope_level = 0x1FF;
            self.envelope_stage = ENV_OFF;
            self.out = [0, 0];
            return true;
        }
        false
    }

    // ---- Self-only setters (no chip/channel state needed) ----
    fn set_tremolo_enable(&mut self, on: bool) {
        self.tremolo_enable = on;
    }
    fn set_vibrato_enable(&mut self, on: bool) {
        self.vibrato_enable = on;
    }
    fn set_sustain_mode(&mut self, on: bool) {
        self.sustain_mode = on;
    }
    fn set_frequency_multiplier(&mut self, scale: u16) {
        self.freq_mult_times2 = MUL_TIMES_2[(scale & 15) as usize];
    }
    fn set_output_level(&mut self, level: u16) {
        self.output_level = level * 4;
    }
    fn set_sustain_level(&mut self, level: u16) {
        self.sustain_level = if level < 15 { level } else { 31 };
        self.sustain_level *= 16;
    }
    fn set_waveform(&mut self, wave: u16) {
        self.waveform = wave & 7;
    }

    // Opal::Operator::SetKeyOn.
    fn set_key_on(&mut self, on: bool) {
        if self.key_on == on {
            return;
        }
        self.key_on = on;
        if on {
            // The highest attack rate is instant; it bypasses the attack phase.
            if self.attack_rate == 15 {
                self.envelope_stage = ENV_DEC;
                self.envelope_level = 0;
            } else {
                self.envelope_stage = ENV_ATT;
            }
            self.phase = 0;
        } else if self.envelope_stage != ENV_OFF && self.envelope_stage != ENV_REL {
            self.envelope_stage = ENV_REL;
        }
    }
}

// ---- Channel ---------------------------------------------------------------

#[derive(Clone, Copy)]
struct OplChannel {
    op: [i32; 4], // operator indices into Opl3::op; -1 = none
    freq: u16,    // frequency (actually a phase-stepping value)
    octave: u16,  // block
    phase_step: u32,
    key_scale_number: u16,
    feedback_shift: u16,
    modulation_type: u16,
    channel_pair: i32, // secondary channel index in 4-op mode; -1 = none
    enable: bool,
    left_enable: bool,
    right_enable: bool,
}

impl OplChannel {
    fn new() -> Self {
        OplChannel {
            op: [-1, -1, -1, -1],
            freq: 0,
            octave: 0,
            phase_step: 0,
            key_scale_number: 0,
            feedback_shift: 0,
            modulation_type: 0,
            channel_pair: -1,
            enable: true,
            left_enable: true,
            right_enable: true,
        }
    }

    // Opal::Channel::ComputePhaseStep.
    fn compute_phase_step(&mut self) {
        self.phase_step = (self.freq as u32) << (self.octave as u32);
    }

    // ---- Self-only setters ----
    fn set_frequency_low(&mut self, freq: u16) {
        self.freq = (self.freq & 0x300) | (freq & 0xFF);
        self.compute_phase_step();
    }
    fn set_left_enable(&mut self, on: bool) {
        self.left_enable = on;
    }
    fn set_right_enable(&mut self, on: bool) {
        self.right_enable = on;
    }
    fn set_feedback(&mut self, val: u16) {
        self.feedback_shift = if val != 0 { 9 - val } else { 0 };
    }
    fn set_modulation_type(&mut self, t: u16) {
        self.modulation_type = t;
    }
}

// ---- The chip --------------------------------------------------------------

pub struct Opl3 {
    sample_rate: i32,
    sample_accum: i32,
    last_output: [i16; 2],
    curr_output: [i16; 2],
    chan: [OplChannel; NUM_CHANNELS],
    op: [Operator; NUM_OPERATORS],
    // Which channel each operator belongs to for rate/key-scale computation.
    // This replaces the C++ Operator::Chan back-pointer. Because operators are
    // shared between a 4-op primary and its 2-op secondary, the *last* channel to
    // claim an operator during Init wins — exactly as the C++ SetChannel calls do.
    chan_of_op: [usize; NUM_OPERATORS],
    clock: u16,
    tremolo_clock: u16,
    tremolo_level: u16,
    vibrato_tick: u16,
    vibrato_clock: u16,
    note_sel: bool,
    tremolo_depth: bool,
    vibrato_depth: bool,
}

impl Opl3 {
    /// Create the chip, resampling its native ~49716 Hz output to `sample_rate`.
    pub fn new(sample_rate: u32) -> Self {
        let mut chip = Opl3 {
            sample_rate: OPL3_SAMPLE_RATE,
            sample_accum: 0,
            last_output: [0, 0],
            curr_output: [0, 0],
            chan: [OplChannel::new(); NUM_CHANNELS],
            op: [Operator::new(); NUM_OPERATORS],
            chan_of_op: [0; NUM_OPERATORS],
            clock: 0,
            tremolo_clock: 0,
            tremolo_level: 0,
            vibrato_tick: 0,
            vibrato_clock: 0,
            note_sel: false,
            tremolo_depth: false,
            vibrato_depth: false,
        };
        chip.init(sample_rate as i32);
        chip
    }

    // Opal::Init.
    fn init(&mut self, sample_rate: i32) {
        // Add the operators to the channels. Some channels can't use all operators.
        let chan_ops: [i32; NUM_CHANNELS] = [0, 1, 2, 6, 7, 8, 12, 13, 14, 18, 19, 20, 24, 25, 26, 30, 31, 32];
        for i in 0..NUM_CHANNELS {
            let op = chan_ops[i];
            if i < 3 || (i >= 9 && i < 12) {
                self.set_operators(i, op, op + 3, op + 6, op + 9);
            } else {
                self.set_operators(i, op, op + 3, -1, -1);
            }
        }

        // Initialise operator rate data (depends on channel state, hence not in ctor).
        for i in 0..NUM_OPERATORS {
            self.op_compute_rates(i);
        }

        self.set_sample_rate(sample_rate);
    }

    // Opal::Channel::SetOperators (plus the SetChannel back-pointer bookkeeping).
    fn set_operators(&mut self, ci: usize, a: i32, b: i32, c: i32, d: i32) {
        self.chan[ci].op = [a, b, c, d];
        if a >= 0 {
            self.chan_of_op[a as usize] = ci;
        }
        if b >= 0 {
            self.chan_of_op[b as usize] = ci;
        }
        if c >= 0 {
            self.chan_of_op[c as usize] = ci;
        }
        if d >= 0 {
            self.chan_of_op[d as usize] = ci;
        }
    }

    // Opal::SetSampleRate.
    fn set_sample_rate(&mut self, mut sample_rate: i32) {
        if sample_rate == 0 {
            sample_rate = OPL3_SAMPLE_RATE;
        }
        self.sample_rate = sample_rate;
        self.sample_accum = 0;
        self.last_output = [0, 0];
        self.curr_output = [0, 0];
    }

    /// Write one OPL register (0x000..0x1FF; the high bit selects the second bank).
    /// Opal::Port.
    pub fn write_reg(&mut self, reg: u16, val: u8) {
        let type_ = reg & 0xE0;

        // BD: the one-off register stuck in the middle of the register array.
        if reg == 0xBD {
            self.tremolo_depth = (val & 0x80) != 0;
            self.vibrato_depth = (val & 0x40) != 0;
            return;
        }

        // Global registers
        if type_ == 0x00 {
            if reg == 0x104 {
                // 4-OP enables.
                let mut mask: u8 = 1;
                for i in 0..6 {
                    // The 4-op channels are 0, 1, 2, 9, 10, 11.
                    let chan = if i < 3 { i } else { i + 6 } as usize;
                    let secondary = chan + 3;
                    if val & mask != 0 {
                        self.chan[chan].channel_pair = secondary as i32;
                        self.chan[secondary].enable = false;
                    } else {
                        self.chan[chan].channel_pair = -1;
                        self.chan[secondary].enable = true;
                    }
                    mask <<= 1;
                }
            } else if reg == 0x08 {
                // CSW / Note-sel.
                self.note_sel = (val & 0x40) != 0;
                for i in 0..NUM_CHANNELS {
                    self.chan_compute_key_scale_number(i);
                }
            }

        // Channel registers
        } else if type_ >= 0xA0 && type_ <= 0xC0 {
            let mut chan_num = (reg & 15) as usize;
            if chan_num >= 9 {
                return;
            }
            if reg & 0x100 != 0 {
                chan_num += 9;
            }

            // Registers Ax and Bx affect both channels of a 4-op pair.
            let pair = self.chan[chan_num].channel_pair;
            let numchans = if pair >= 0 { 2 } else { 1 };
            let chans = [chan_num as i32, pair];

            match reg & 0xF0 {
                // Frequency low
                0xA0 => {
                    for j in 0..numchans {
                        let cj = chans[j] as usize;
                        self.chan[cj].set_frequency_low(val as u16);
                    }
                }
                // Key-on / Octave / Frequency high
                0xB0 => {
                    for j in 0..numchans {
                        let cj = chans[j] as usize;
                        self.chan_set_key_on(cj, (val & 0x20) != 0);
                        self.chan_set_octave(cj, ((val >> 2) & 7) as u16);
                        self.chan_set_frequency_high(cj, (val & 3) as u16);
                    }
                }
                // Stereo enables / feedback / modulation type
                0xC0 => {
                    self.chan[chan_num].set_right_enable((val & 0x20) != 0);
                    self.chan[chan_num].set_left_enable((val & 0x10) != 0);
                    self.chan[chan_num].set_feedback(((val >> 1) & 7) as u16);
                    self.chan[chan_num].set_modulation_type((val & 1) as u16);
                }
                _ => {}
            }

        // Operator registers
        } else if (type_ >= 0x20 && type_ <= 0x80) || type_ == 0xE0 {
            let op_num_i = OP_LOOKUP[(reg & 0x1F) as usize];
            if op_num_i < 0 {
                return;
            }
            let mut op_num = op_num_i as usize;
            if reg & 0x100 != 0 {
                op_num += 18;
            }

            match type_ {
                // Tremolo / vibrato / sustain-mode / envelope-scaling / freq-mult
                0x20 => {
                    self.op[op_num].set_tremolo_enable((val & 0x80) != 0);
                    self.op[op_num].set_vibrato_enable((val & 0x40) != 0);
                    self.op[op_num].set_sustain_mode((val & 0x20) != 0);
                    self.op_set_envelope_scaling(op_num, (val & 0x10) != 0);
                    self.op[op_num].set_frequency_multiplier((val & 15) as u16);
                }
                // Key scale / output level
                0x40 => {
                    self.op_set_key_scale(op_num, (val >> 6) as u16);
                    self.op[op_num].set_output_level((val & 0x3F) as u16);
                }
                // Attack rate / decay rate
                0x60 => {
                    self.op_set_attack_rate(op_num, (val >> 4) as u16);
                    self.op_set_decay_rate(op_num, (val & 15) as u16);
                }
                // Sustain level / release rate
                0x80 => {
                    self.op[op_num].set_sustain_level((val >> 4) as u16);
                    self.op_set_release_rate(op_num, (val & 15) as u16);
                }
                // Waveform
                0xE0 => {
                    self.op[op_num].set_waveform((val & 7) as u16);
                }
                _ => {}
            }
        }
    }

    /// Generate one output stereo sample pair (already resampled to `sample_rate`).
    /// Opal::Sample.
    pub fn sample(&mut self) -> (i16, i16) {
        // If the destination rate is higher than the OPL3 rate, skip ahead.
        while self.sample_accum >= self.sample_rate {
            self.last_output[0] = self.curr_output[0];
            self.last_output[1] = self.curr_output[1];

            let (l, r) = self.output_chip();
            self.curr_output[0] = l;
            self.curr_output[1] = r;

            self.sample_accum -= self.sample_rate;
        }

        // Mix with the partial accumulation.
        let fract = muldivr(self.sample_accum, 65536, self.sample_rate);
        let l_diff = self.curr_output[0] as i32 - self.last_output[0] as i32;
        let r_diff = self.curr_output[1] as i32 - self.last_output[1] as i32;
        let left = (self.last_output[0] as i32).wrapping_add(fract.wrapping_mul(l_diff) / 65536) as i16;
        let right = (self.last_output[1] as i32).wrapping_add(fract.wrapping_mul(r_diff) / 65536) as i16;

        self.sample_accum += OPL3_SAMPLE_RATE;

        (left, right)
    }

    // Opal::Output — final output from the chip at the OPL3 sample rate.
    fn output_chip(&mut self) -> (i16, i16) {
        let mut leftmix: i32 = 0;
        let mut rightmix: i32 = 0;

        for i in 0..NUM_CHANNELS {
            let (cl, cr) = self.channel_output(i);
            leftmix += cl as i32;
            rightmix += cr as i32;
        }

        let left = clamp16(leftmix);
        let right = clamp16(rightmix);

        self.clock = self.clock.wrapping_add(1);

        // Tremolo: a 13,440-sample triangle wave, peak 26, trough 0.
        self.tremolo_clock = (self.tremolo_clock + 1) % 13440;
        self.tremolo_level = (if self.tremolo_clock < 13440 / 2 {
            self.tremolo_clock
        } else {
            13440 - self.tremolo_clock
        }) / 256;
        if !self.tremolo_depth {
            self.tremolo_level >>= 2;
        }

        // Vibrato: an 8-sample triangle wave cycled every 1,024 samples.
        self.vibrato_tick = self.vibrato_tick.wrapping_add(1);
        if self.vibrato_tick >= 1024 {
            self.vibrato_tick = 0;
            self.vibrato_clock = (self.vibrato_clock + 1) & 7;
        }

        (left, right)
    }

    // Opal::Channel::Output — lifted to a chip method so it can touch its operators.
    fn channel_output(&mut self, ci: usize) -> (i16, i16) {
        let ch = self.chan[ci]; // Copy; the channel is not mutated here.
        if !ch.enable {
            return (0, 0);
        }

        let mut vibrato: i16 = ((ch.freq >> 7) & 7) as i16;
        if !self.vibrato_depth {
            vibrato >>= 1;
        }

        // 0  3  7  3  0  -3  -7  -3
        let clk = self.vibrato_clock;
        if clk & 3 == 0 {
            vibrato = 0; // Positions 0 and 4 are zero.
        } else {
            if clk & 1 != 0 {
                vibrato >>= 1; // Odd positions are half magnitude.
            }
            vibrato = ((vibrato as i32) << (ch.octave as u32)) as i16;
            if clk & 4 != 0 {
                vibrato = vibrato.wrapping_neg(); // Second-half positions negate.
            }
        }

        let ks = ch.key_scale_number;
        let ps = ch.phase_step;
        let fb = ch.feedback_shift as i16;
        let tl = self.tremolo_level;
        let clock = self.clock;
        let i0 = ch.op[0] as usize;
        let i1 = ch.op[1] as usize;

        let pair_mod_type = if ch.channel_pair >= 0 {
            Some(self.chan[ch.channel_pair as usize].modulation_type)
        } else {
            None
        };

        let out: i16;

        if let Some(pair_mt) = pair_mod_type {
            // Running in 4-op mode.
            let i2 = ch.op[2] as usize;
            let i3 = ch.op[3] as usize;

            if pair_mt == 0 {
                if ch.modulation_type == 0 {
                    // feedback -> mod -> mod -> mod -> carrier
                    let mut o = self.op[i0].output(ks, ps, vibrato, 0, fb, tl, clock);
                    o = self.op[i1].output(ks, ps, vibrato, o, 0, tl, clock);
                    o = self.op[i2].output(ks, ps, vibrato, o, 0, tl, clock);
                    o = self.op[i3].output(ks, ps, vibrato, o, 0, tl, clock);
                    out = o;
                } else {
                    // (feedback -> carrier) + (mod -> mod -> carrier)
                    let o = self.op[i0].output(ks, ps, vibrato, 0, fb, tl, clock);
                    let mut acc = self.op[i1].output(ks, ps, vibrato, 0, 0, tl, clock);
                    acc = self.op[i2].output(ks, ps, vibrato, acc, 0, tl, clock);
                    out = o.wrapping_add(self.op[i3].output(ks, ps, vibrato, acc, 0, tl, clock));
                }
            } else if ch.modulation_type == 0 {
                // (feedback -> mod -> carrier) + (mod -> carrier)
                let mut o = self.op[i0].output(ks, ps, vibrato, 0, fb, tl, clock);
                o = self.op[i1].output(ks, ps, vibrato, o, 0, tl, clock);
                let acc = self.op[i2].output(ks, ps, vibrato, 0, 0, tl, clock);
                out = o.wrapping_add(self.op[i3].output(ks, ps, vibrato, acc, 0, tl, clock));
            } else {
                // (feedback -> carrier) + (mod -> carrier) + carrier
                let mut o = self.op[i0].output(ks, ps, vibrato, 0, fb, tl, clock);
                let acc = self.op[i1].output(ks, ps, vibrato, 0, 0, tl, clock);
                o = o.wrapping_add(self.op[i2].output(ks, ps, vibrato, acc, 0, tl, clock));
                o = o.wrapping_add(self.op[i3].output(ks, ps, vibrato, 0, 0, tl, clock));
                out = o;
            }
        } else {
            // Standard 2-op mode.
            if ch.modulation_type == 0 {
                // Frequency (phase) modulation.
                let o = self.op[i0].output(ks, ps, vibrato, 0, fb, tl, clock);
                out = self.op[i1].output(ks, ps, vibrato, o, 0, tl, clock);
            } else {
                // Additive.
                let o = self.op[i0].output(ks, ps, vibrato, 0, fb, tl, clock);
                out = o.wrapping_add(self.op[i1].output(ks, ps, vibrato, 0, 0, tl, clock));
            }
        }

        let left = if ch.left_enable { out } else { 0 };
        let right = if ch.right_enable { out } else { 0 };
        (left, right)
    }

    // Opal::Channel::SetFrequencyHigh.
    fn chan_set_frequency_high(&mut self, ci: usize, freq: u16) {
        self.chan[ci].freq = (self.chan[ci].freq & 0xFF) | ((freq & 3) << 8);
        self.chan[ci].compute_phase_step();
        // Only the high bits of Freq affect the Key Scale No.
        self.chan_compute_key_scale_number(ci);
    }

    // Opal::Channel::SetOctave.
    fn chan_set_octave(&mut self, ci: usize, oct: u16) {
        self.chan[ci].octave = oct & 7;
        self.chan[ci].compute_phase_step();
        self.chan_compute_key_scale_number(ci);
    }

    // Opal::Channel::SetKeyOn.
    fn chan_set_key_on(&mut self, ci: usize, on: bool) {
        let o0 = self.chan[ci].op[0];
        let o1 = self.chan[ci].op[1];
        if o0 >= 0 {
            self.op[o0 as usize].set_key_on(on);
        }
        if o1 >= 0 {
            self.op[o1 as usize].set_key_on(on);
        }
    }

    // Opal::Channel::ComputeKeyScaleNumber.
    fn chan_compute_key_scale_number(&mut self, ci: usize) {
        let freq = self.chan[ci].freq;
        let octave = self.chan[ci].octave;
        let lsb = if self.note_sel { freq >> 9 } else { (freq >> 8) & 1 };
        self.chan[ci].key_scale_number = (octave << 1) | lsb;

        // Channel operators recompute their rates and key-scale levels.
        for k in 0..4 {
            let oi = self.chan[ci].op[k];
            if oi < 0 {
                continue;
            }
            let oi = oi as usize;
            self.op_compute_rates(oi);
            self.op_compute_key_scale_level(oi);
        }
    }

    // ---- Operator setters that need channel state (promoted to chip methods) ----
    fn op_set_envelope_scaling(&mut self, oi: usize, on: bool) {
        self.op[oi].key_scale_rate = on;
        self.op_compute_rates(oi);
    }
    fn op_set_key_scale(&mut self, oi: usize, scale: u16) {
        self.op[oi].key_scale_shift = KSL_SHIFT[scale as usize];
        self.op_compute_key_scale_level(oi);
    }
    fn op_set_attack_rate(&mut self, oi: usize, rate: u16) {
        self.op[oi].attack_rate = rate;
        self.op_compute_rates(oi);
    }
    fn op_set_decay_rate(&mut self, oi: usize, rate: u16) {
        self.op[oi].decay_rate = rate;
        self.op_compute_rates(oi);
    }
    fn op_set_release_rate(&mut self, oi: usize, rate: u16) {
        self.op[oi].release_rate = rate;
        self.op_compute_rates(oi);
    }

    // Opal::Operator::ComputeRates. Uses the operator's owning channel key-scale no.
    fn op_compute_rates(&mut self, oi: usize) {
        let ci = self.chan_of_op[oi];
        let ksn = self.chan[ci].key_scale_number;
        let op = &mut self.op[oi];
        let shift: u32 = if op.key_scale_rate { 0 } else { 2 };
        let ksr = (ksn >> shift) as i32;

        let combined = op.attack_rate as i32 * 4 + ksr;
        let rate_high = combined >> 2;
        let rate_low = combined & 3;
        op.attack_shift = if rate_high < 12 { (12 - rate_high) as u16 } else { 0 };
        op.attack_mask = ((1i32 << (op.attack_shift as u32)) - 1) as u16;
        op.attack_add = if rate_high < 12 { 1 } else { (1i32 << ((rate_high - 12) as u32)) as u16 };
        op.attack_tab = rate_low as usize;
        // Attack rate of 15 is always instant.
        if op.attack_rate == 15 {
            op.attack_add = 0xFFF;
        }

        let combined = op.decay_rate as i32 * 4 + ksr;
        let rate_high = combined >> 2;
        let rate_low = combined & 3;
        op.decay_shift = if rate_high < 12 { (12 - rate_high) as u16 } else { 0 };
        op.decay_mask = ((1i32 << (op.decay_shift as u32)) - 1) as u16;
        op.decay_add = if rate_high < 12 { 1 } else { (1i32 << ((rate_high - 12) as u32)) as u16 };
        op.decay_tab = rate_low as usize;

        let combined = op.release_rate as i32 * 4 + ksr;
        let rate_high = combined >> 2;
        let rate_low = combined & 3;
        op.release_shift = if rate_high < 12 { (12 - rate_high) as u16 } else { 0 };
        op.release_mask = ((1i32 << (op.release_shift as u32)) - 1) as u16;
        op.release_add = if rate_high < 12 { 1 } else { (1i32 << ((rate_high - 12) as u32)) as u16 };
        op.release_tab = rate_low as usize;
    }

    // Opal::Operator::ComputeKeyScaleLevel.
    fn op_compute_key_scale_level(&mut self, oi: usize) {
        let ci = self.chan_of_op[oi];
        let octave = self.chan[ci].octave;
        let freq = self.chan[ci].freq;
        // Top four bits of frequency combined with the octave/block.
        let i = ((octave << 4) | (freq >> 6)) as usize;
        self.op[oi].key_scale_level = (LEVTAB[i] as u16) >> (self.op[oi].key_scale_shift as u32);
    }
}

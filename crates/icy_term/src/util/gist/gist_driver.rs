//! GIST Sound Driver - Cycle-accurate port from gistdrvr.s
//!
//! Critical 68000 semantics that must be preserved:
//! - MULS.W uses only the low 16 bits of both operands
//! - SWAP exchanges high and low words of a 32-bit register
//! - ASR.L is arithmetic (sign-extending) shift right
//! - CMP.L uses signed comparison (BGT, BLT, BGE, BLE)
//! - OR.W at offset 26 reads the HIGH word of the long at that offset

use ym2149::Ym2149;

use super::gist_data::{GistSoundData, GistVoiceTemplate};

const NUM_VOICES: usize = 3;

const YM_FREQS: [u16; 85] = [
    3822, 3608, 3405, 3214, 3034, 2863, 2703, 2551, 2408, 2273, 2145, 2025, 1911, 1804, 1703, 1607, 1517, 1432, 1351, 1276, 1204, 1136, 1073, 1012, 956, 902,
    851, 804, 758, 716, 676, 638, 602, 568, 536, 506, 478, 451, 426, 402, 379, 358, 338, 319, 301, 284, 268, 253, 239, 225, 213, 201, 190, 179, 169, 159, 150,
    142, 134, 127, 119, 113, 106, 100, 95, 89, 84, 80, 75, 71, 67, 63, 60, 56, 53, 50, 47, 45, 42, 40, 38, 36, 34, 32, 30,
];

const DIV_15: [i16; 16] = [0, 18, 35, 52, 69, 86, 103, 120, 137, 154, 171, 188, 205, 222, 239, 256];

const MIXER_MASK: [u8; 3] = [0xf6, 0xed, 0xdb];

/// Voice state - maps directly to the 140-byte structure in the original driver
/// All offsets are in bytes from the structure start
#[derive(Clone, Debug, Default)]
struct Voice {
    /// Offset 0: Duration counter / in-use flag
    inuse: i16,
    /// Offset 2: Tone period
    freq: i16,
    /// Offset 4: Noise period
    noise_freq: i16,
    /// Offset 6: Volume level (0-15)
    volume: i16,
    /// Offset 8: Volume envelope phase (0=none, 1=attack, 2=decay, 3=sustain, 4=release)
    vol_phase: i16,
    /// Offset 10: Volume attack step (added each tick)
    vol_attack: i32,
    /// Offset 14: Volume decay step
    vol_decay: i32,
    /// Offset 18: Volume sustain level (decay target)
    vol_sustain: i32,
    /// Offset 22: Volume release step
    vol_release: i32,
    /// Offset 26: Volume LFO limit (positive)
    vol_lfo_limit: i32,
    /// Offset 30: Volume LFO step (negated when limit reached)
    vol_lfo_step: i32,
    /// Offset 34: Volume LFO delay (ticks before LFO starts)
    vol_lfo_delay: i16,
    /// Offset 36: Frequency envelope phase
    freq_phase: i16,
    /// Offset 38: Frequency attack step
    freq_attack: i32,
    /// Offset 42: Frequency attack target
    freq_attack_target: i32,
    /// Offset 46: Frequency decay step
    freq_decay: i32,
    /// Offset 50: Frequency decay target
    freq_decay_target: i32,
    /// Offset 54: Frequency release step
    freq_release: i32,
    /// Offset 58: Frequency LFO positive limit
    freq_lfo_limit: i32,
    /// Offset 62: Frequency LFO step
    freq_lfo_step: i32,
    /// Offset 66: Frequency LFO reset value (positive direction)
    freq_lfo_reset_pos: i32,
    /// Offset 70: Frequency LFO negative limit
    freq_lfo_limit_neg: i32,
    /// Offset 74: Frequency LFO reset value (negative direction)
    freq_lfo_reset_neg: i32,
    /// Offset 78: Frequency LFO delay
    freq_lfo_delay: i16,
    /// Offset 80: Noise envelope phase
    noise_phase: i16,
    /// Offset 82: Noise attack step
    noise_attack: i32,
    /// Offset 86: Noise attack target
    noise_attack_target: i32,
    /// Offset 90: Noise decay step
    noise_decay: i32,
    /// Offset 94: Noise decay target
    noise_decay_target: i32,
    /// Offset 98: Noise release step
    noise_release: i32,
    /// Offset 102: Noise LFO limit
    noise_lfo_limit: i32,
    /// Offset 106: Noise LFO step
    noise_lfo_step: i32,
    /// Offset 110: Noise LFO delay
    noise_lfo_delay: i16,
    /// Offset 112: Pitch (-1 = use duration, >=0 = play indefinitely)
    pitch: i16,
    /// Offset 114: Priority
    priority: i16,
    /// Offset 116: Volume envelope accumulator
    vol_env_acc: i32,
    /// Offset 120: Volume LFO accumulator
    vol_lfo_acc: i32,
    /// Offset 124: Frequency envelope accumulator
    freq_env_acc: i32,
    /// Offset 128: Frequency LFO accumulator
    freq_lfo_acc: i32,
    /// Offset 132: Noise envelope accumulator
    noise_env_acc: i32,
    /// Offset 136: Noise LFO accumulator
    noise_lfo_acc: i32,
}

impl Voice {
    fn load_from_template(&mut self, tpl: &GistVoiceTemplate, pitch: i16, priority: i16, volume: Option<i16>) {
        // Copy sound parameters
        self.freq = tpl.initial_freq;
        self.noise_freq = tpl.initial_noise_freq;
        self.volume = volume.unwrap_or(tpl.initial_volume);
        self.vol_phase = tpl.vol_phase;
        self.vol_attack = tpl.vol_attack;
        self.vol_decay = tpl.vol_decay;
        self.vol_sustain = tpl.vol_sustain;
        self.vol_release = tpl.vol_release;
        self.vol_lfo_limit = tpl.vol_lfo_limit;
        self.vol_lfo_step = tpl.vol_lfo_step;
        self.vol_lfo_delay = tpl.vol_lfo_delay;
        self.freq_phase = tpl.freq_env_phase;
        self.freq_attack = tpl.freq_attack;
        self.freq_attack_target = tpl.freq_attack_target;
        self.freq_decay = tpl.freq_decay;
        self.freq_decay_target = tpl.freq_decay_target;
        self.freq_release = tpl.freq_release;
        self.freq_lfo_limit = tpl.freq_lfo_limit;
        self.freq_lfo_step = tpl.freq_lfo_step;
        self.freq_lfo_reset_pos = tpl.freq_lfo_reset_positive;
        self.freq_lfo_limit_neg = tpl.freq_lfo_negative_limit;
        self.freq_lfo_reset_neg = tpl.freq_lfo_reset_negative;
        self.freq_lfo_delay = tpl.freq_lfo_delay;
        self.noise_phase = tpl.noise_env_phase;
        self.noise_attack = tpl.noise_attack;
        self.noise_attack_target = tpl.noise_attack_target;
        self.noise_decay = tpl.noise_decay;
        self.noise_decay_target = tpl.noise_decay_target;
        self.noise_release = tpl.noise_release;
        self.noise_lfo_limit = tpl.noise_lfo_limit;
        self.noise_lfo_step = tpl.noise_lfo_step;
        self.noise_lfo_delay = tpl.noise_lfo_delay;

        // Set runtime fields
        self.pitch = pitch;
        self.priority = priority;

        // Clear all accumulators (from snd_on: sndptr->o116 = sndptr->o120 = ... = 0)
        self.vol_env_acc = 0;
        self.vol_lfo_acc = 0;
        self.freq_env_acc = 0;
        self.freq_lfo_acc = 0;
        self.noise_env_acc = 0;
        self.noise_lfo_acc = 0;
    }
}

pub struct GistDriver {
    voices: [Voice; NUM_VOICES],
    mixer: u8,
    tick_count: u32,
}

impl Default for GistDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl GistDriver {
    pub fn new() -> Self {
        Self {
            voices: Default::default(),
            mixer: 0x3f,
            tick_count: 0,
        }
    }

    pub fn is_playing(&self) -> bool {
        self.voices.iter().any(|v| v.inuse != 0)
    }

    pub fn stop_all(&mut self, chip: &mut Ym2149) {
        for (i, v) in self.voices.iter_mut().enumerate() {
            v.inuse = 0;
            v.priority = 0;
            chip.write_register(8 + i as u8, 0);
        }
        self.mixer = 0x3f;
        chip.write_register(7, self.mixer);
    }

    pub fn snd_off(&mut self, voice_idx: usize) {
        if voice_idx < NUM_VOICES && self.voices[voice_idx].inuse != 0 {
            self.voices[voice_idx].inuse = 1;
            self.voices[voice_idx].pitch = -1;
        }
    }

    /// Immediately stop a voice (hard stop, no fadeout)
    pub fn stop_snd(&mut self, chip: &mut Ym2149, voice_idx: usize) {
        if voice_idx < NUM_VOICES {
            let v = &mut self.voices[voice_idx];
            v.inuse = 0;
            v.priority = 0;

            // Set volume to 0
            chip.write_register(8 + voice_idx as u8, 0);

            // Disable both tone and noise for this voice in the mixer
            // Bit 0-2: tone enable (0=enabled, 1=disabled) for voices A,B,C
            // Bit 3-5: noise enable (0=enabled, 1=disabled) for voices A,B,C
            let tone_disable = 1 << voice_idx;
            let noise_disable = 8 << voice_idx;
            self.mixer |= tone_disable | noise_disable;
            chip.write_register(7, self.mixer);

            // Reset voice state to prevent any residual processing
            v.vol_phase = 0;
            v.freq_phase = 0;
            v.noise_phase = 0;
            v.vol_env_acc = 0;
            v.vol_lfo_acc = 0;
            v.freq_env_acc = 0;
            v.freq_lfo_acc = 0;
            v.noise_env_acc = 0;
            v.noise_lfo_acc = 0;
        }
    }

    pub fn snd_on(
        &mut self,
        chip: &mut Ym2149,
        sound: &GistSoundData,
        requested_voice: Option<usize>,
        volume: Option<i16>,
        pitch: i16,
        priority: i16,
    ) -> Option<usize> {
        let duration = sound.duration();
        if duration == 0 {
            return requested_voice.or(Some(0));
        }

        let voice_idx = self.pick_voice(requested_voice, priority)?;

        // stop_snd equivalent
        self.voices[voice_idx].inuse = 0;
        self.voices[voice_idx].priority = 0;
        chip.write_register(8 + voice_idx as u8, 0);

        // Load template
        let tpl = GistVoiceTemplate::from(sound);
        self.voices[voice_idx].load_from_template(&tpl, pitch, priority, volume);

        let v = &mut self.voices[voice_idx];

        // Configure tone
        let tonemask: u8;
        if v.freq >= 0 {
            tonemask = 0;
            if pitch >= 0 {
                let mut p = pitch;
                while p > 108 {
                    p -= 12;
                }
                while p < 24 {
                    p += 12;
                }
                if let Some(&freq) = YM_FREQS.get((p - 24) as usize) {
                    v.freq = freq as i16;
                }
                // After setting frequency from explicit pitch, reset v.pitch to -1
                // so that duration decrement logic works in tick()
                v.pitch = -1;
            }
            chip.write_register((voice_idx * 2) as u8, (v.freq & 0xff) as u8);
            chip.write_register((voice_idx * 2 + 1) as u8, ((v.freq >> 8) & 0x0f) as u8);
        } else {
            tonemask = 1 << voice_idx;
            // When tone disabled, clear freq envelope/LFO
            v.freq_phase = 0;
            v.freq_lfo_limit = 0;
        }

        // Configure noise
        let noisemask: u8;
        if v.noise_freq >= 0 {
            noisemask = 0;
            chip.write_register(6, (v.noise_freq & 0x1f) as u8);
        } else {
            noisemask = 8 << voice_idx;
            // When noise disabled, clear noise envelope/LFO
            v.noise_phase = 0;
            v.noise_lfo_limit = 0;
        }

        // Update mixer
        self.mixer = (self.mixer & MIXER_MASK[voice_idx]) | tonemask | noisemask;
        chip.write_register(7, self.mixer);

        // If no volume envelope, set initial volume and max envelope accumulator
        if v.vol_phase == 0 {
            v.vol_env_acc = 0x000F_0000;
            chip.write_register(8 + voice_idx as u8, (v.volume & 0x0f) as u8);
        }

        // Finally set duration
        v.inuse = duration;

        Some(voice_idx)
    }

    fn pick_voice(&self, requested: Option<usize>, priority: i16) -> Option<usize> {
        if let Some(idx) = requested {
            if idx < NUM_VOICES && self.voices[idx].priority <= priority {
                return Some(idx);
            }
            if idx >= NUM_VOICES {
                return None;
            }
        }

        // Find free voice
        for i in 0..NUM_VOICES {
            if self.voices[i].inuse == 0 {
                return Some(i);
            }
        }

        // All in use - find lowest priority
        let mut best = if self.voices[0].priority < self.voices[1].priority { 0 } else { 1 };
        if self.voices[2].priority <= self.voices[best].priority {
            best = 2;
        }
        if self.voices[best].priority > priority { None } else { Some(best) }
    }

    /// Main interrupt handler - called 200 times per second
    /// This is a precise translation of timer_irq from gistdrvr.s
    pub fn tick(&mut self, chip: &mut Ym2149) {
        self.tick_count += 1;

        // Original processes voices 2, 1, 0 (dbf d2,vcloop)
        // This matters because all voices share the noise register (6)
        for voice_idx in (0..NUM_VOICES).rev() {
            let v = &mut self.voices[voice_idx];

            // vcloop: tst.w (a0) / beq endloop
            if v.inuse == 0 {
                continue;
            }

            // ===== VOLUME ENVELOPE (offset 8, 10-22, 116) =====
            // move.w 8(a0),d0  ; vol_phase
            // move.l 116(a0),d1 ; vol_env_acc
            let mut d1 = v.vol_env_acc;

            match v.vol_phase {
                1 => {
                    // Attack: add.l 10(a0),d1
                    d1 = d1.wrapping_add(v.vol_attack);
                    // cmp.l #0x000F0000,d1 / blt.s endve
                    if d1 >= 0x000F_0000 {
                        d1 = 0x000F_0000;
                        v.vol_phase += 1;
                    }
                }
                2 => {
                    // Decay: add.l 14(a0),d1
                    d1 = d1.wrapping_add(v.vol_decay);
                    // cmp.l 18(a0),d1 / bgt.s endve
                    // Note: BGT is signed, and decay step is typically negative
                    if d1 <= v.vol_sustain {
                        d1 = v.vol_sustain;
                        v.vol_phase += 1;
                    }
                }
                4 => {
                    // Release: add.l 22(a0),d1
                    //let old_d1 = d1;
                    d1 = d1.wrapping_add(v.vol_release);
                    // tst.l d1 / bgt.s endve
                    if d1 <= 0 {
                        d1 = 0;
                        v.vol_phase = 0;
                        v.inuse = 1;
                    }
                }
                _ => {}
            }
            v.vol_env_acc = d1;

            // ===== VOLUME LFO (offset 26-34, 120) =====
            // lva: move.l 26(a0),d0 / beq.s do_vol
            if v.vol_lfo_limit != 0 {
                // tst.w 34(a0) / beq.s do_lv / subq.w #1,34(a0) / bra.s do_vol
                if v.vol_lfo_delay > 0 {
                    v.vol_lfo_delay -= 1;
                } else {
                    // do_lv: move.l 120(a0),d1 / add.l 30(a0),d1
                    let mut lfo = v.vol_lfo_acc.wrapping_add(v.vol_lfo_step);
                    let limit = v.vol_lfo_limit;

                    // cmp.l d0,d1 / bge.s do_lv1
                    if lfo >= limit {
                        // do_lv1: move.l d0,d1 / neg.l 30(a0)
                        lfo = limit;
                        v.vol_lfo_step = v.vol_lfo_step.wrapping_neg();
                    } else {
                        // neg.l d0 / cmp.l d0,d1 / bgt.s enddo_lv
                        let neg_limit = limit.wrapping_neg();
                        if lfo <= neg_limit {
                            lfo = neg_limit;
                            v.vol_lfo_step = v.vol_lfo_step.wrapping_neg();
                        }
                    }
                    v.vol_lfo_acc = lfo;
                }
            }

            // ===== WRITE VOLUME TO CHIP =====
            // do_vol: move.w 8(a0),d0 / or.w 26(a0),d0 / beq.s fe
            // Note: or.w 26(a0) reads the HIGH word of vol_lfo_limit
            let vol_lfo_limit_hi = (v.vol_lfo_limit >> 16) as i16;
            if v.vol_phase != 0 || vol_lfo_limit_hi != 0 {
                // move.w 6(a0),d0 / add.w d0,d0 / move.w 0(a2,d0.w),d0
                let vol_idx = (v.volume.clamp(0, 15)) as usize;
                let mut d0: i32 = DIV_15[vol_idx] as i32;

                // move.l 116(a0),d1 / add.l 120(a0),d1
                let d1 = v.vol_env_acc.wrapping_add(v.vol_lfo_acc);

                // bpl.s do_vol1
                let level: u8 = if d1 < 0 {
                    // moveq.l #0,d0
                    0
                } else {
                    // do_vol1: asr.l #8,d1
                    let shifted = d1 >> 8;
                    // muls.w d1,d0 - multiply low 16 bits
                    let d1_lo = shifted as i16;
                    let d0_lo = d0 as i16;
                    d0 = (d0_lo as i32) * (d1_lo as i32);
                    // swap d0 - get high word
                    d0 = (d0 >> 16) & 0xffff;
                    // Handle sign extension from swap
                    if d0 > 0x7fff {
                        d0 = (d0 as i16) as i32;
                    }
                    // cmp.w #15,d0 / ble.s do_vol2
                    if d0 > 15 {
                        15
                    } else if d0 < 0 {
                        0
                    } else {
                        d0 as u8
                    }
                };

                chip.write_register(8 + voice_idx as u8, level);
            }

            // ===== FREQUENCY ENVELOPE (offset 36-54, 124) =====
            // fe: move.w 36(a0),d0 / move.l 124(a0),d1
            let mut d1 = v.freq_env_acc;

            match v.freq_phase {
                1 => {
                    // add.l 38(a0),d1
                    d1 = d1.wrapping_add(v.freq_attack);
                    // tst.w 38(a0) / bmi.s fea1
                    let step_hi = (v.freq_attack >> 16) as i16;
                    if step_hi >= 0 {
                        // cmp.l 42(a0),d1 / blt.s endfe
                        if d1 >= v.freq_attack_target {
                            d1 = v.freq_attack_target;
                            v.freq_phase += 1;
                        }
                    } else {
                        // fea1: cmp.l 42(a0),d1 / bgt.s endfe
                        if d1 <= v.freq_attack_target {
                            d1 = v.freq_attack_target;
                            v.freq_phase += 1;
                        }
                    }
                }
                2 => {
                    d1 = d1.wrapping_add(v.freq_decay);
                    let step_hi = (v.freq_decay >> 16) as i16;
                    if step_hi >= 0 {
                        if d1 >= v.freq_decay_target {
                            d1 = v.freq_decay_target;
                            v.freq_phase += 1;
                        }
                    } else {
                        if d1 <= v.freq_decay_target {
                            d1 = v.freq_decay_target;
                            v.freq_phase += 1;
                        }
                    }
                }
                4 => {
                    d1 = d1.wrapping_add(v.freq_release);
                    let step_hi = (v.freq_release >> 16) as i16;
                    // tst.w 54(a0) / bmi.s fer1
                    if step_hi >= 0 {
                        // tst.l d1 / bmi.s endfe
                        if d1 >= 0 {
                            d1 = 0;
                        }
                    } else {
                        // fer1: tst.l d1 / bgt.s endfe
                        if d1 <= 0 {
                            d1 = 0;
                        }
                    }
                }
                _ => {}
            }
            v.freq_env_acc = d1;

            // ===== FREQUENCY LFO (offset 58-78, 128) =====
            // lfa: move.l 58(a0),d0 / beq.s do_fr
            if v.freq_lfo_limit != 0 {
                // tst.w 78(a0) / beq.s do_lf / subq.w #1,78(a0) / bra.s do_fr
                if v.freq_lfo_delay > 0 {
                    v.freq_lfo_delay -= 1;
                } else {
                    // do_lf: move.l 62(a0),d1 / bmi.s do_lf2
                    let step = v.freq_lfo_step;

                    if step >= 0 {
                        // Positive direction
                        // add.l 128(a0),d1 - add acc to step
                        let (lfo, carry) = (step as u32).overflowing_add(v.freq_lfo_acc as u32);
                        let lfo = lfo as i32;

                        // bcc.s do_lf1 / move.l 66(a0),62(a0)
                        if carry {
                            v.freq_lfo_step = v.freq_lfo_reset_pos;
                        }

                        // cmp.l d0,d1 / blt.s enddo_lf
                        let limit = v.freq_lfo_limit;
                        if lfo >= limit {
                            // do_lf4: move.l d0,d1 / neg.l 62(a0)
                            v.freq_lfo_acc = limit;
                            v.freq_lfo_step = v.freq_lfo_step.wrapping_neg();
                        } else {
                            v.freq_lfo_acc = lfo;
                        }
                    } else {
                        // Negative direction
                        // move.l 70(a0),d0 - get negative limit
                        let neg_limit = v.freq_lfo_limit_neg;

                        // add.l 128(a0),d1 - add acc to step
                        let (lfo, carry) = (step as u32).overflowing_add(v.freq_lfo_acc as u32);
                        let lfo = lfo as i32;

                        // bcs.s do_lf3 / move.l 74(a0),62(a0)
                        // Note: BCS means "branch if carry SET" - opposite of BCC
                        if !carry {
                            v.freq_lfo_step = v.freq_lfo_reset_neg;
                        }

                        // cmp.l d0,d1 / bgt.s enddo_lf
                        if lfo <= neg_limit {
                            // do_lf4: move.l d0,d1 / neg.l 62(a0)
                            v.freq_lfo_acc = neg_limit;
                            v.freq_lfo_step = v.freq_lfo_step.wrapping_neg();
                        } else {
                            v.freq_lfo_acc = lfo;
                        }
                    }
                }
            }

            // ===== WRITE FREQUENCY TO CHIP =====
            // do_fr: move.w 36(a0),d0 / or.w 58(a0),d0 / beq.s nfe
            let freq_lfo_limit_hi = (v.freq_lfo_limit >> 16) as i16;
            if v.freq_phase != 0 || freq_lfo_limit_hi != 0 {
                if v.freq >= 0 {
                    // move.l 128(a0),d0 / add.l 124(a0),d0
                    let combined = v.freq_lfo_acc.wrapping_add(v.freq_env_acc);

                    // swap d0
                    let hi = ((combined as u32) >> 16) as i16;

                    // muls.w 2(a0),d0
                    let mut d0: i32 = (hi as i32) * (v.freq as i32);

                    // asl.l #4,d0
                    d0 = d0.wrapping_shl(4);

                    // swap d0
                    d0 = ((d0 as u32) >> 16) as i16 as i32;

                    // bpl.s do_fr0 / addq.w #1,d0
                    if d0 < 0 {
                        d0 = d0.wrapping_add(1);
                    }

                    // add.w 2(a0),d0
                    d0 = ((d0 as i16).wrapping_add(v.freq)) as i32;

                    // bpl.s do_fr1
                    if d0 < 0 {
                        d0 = 0;
                    }

                    // cmp.w #0x0FFF,d0 / ble.s do_fr2
                    if d0 > 0x0fff {
                        d0 = 0x0fff;
                    }

                    chip.write_register((voice_idx * 2) as u8, (d0 & 0xff) as u8);
                    chip.write_register((voice_idx * 2 + 1) as u8, ((d0 >> 8) & 0x0f) as u8);
                }
            }

            // ===== NOISE ENVELOPE (offset 80-98, 132) =====
            let mut d1 = v.noise_env_acc;

            match v.noise_phase {
                1 => {
                    d1 = d1.wrapping_add(v.noise_attack);
                    let step_hi = (v.noise_attack >> 16) as i16;
                    if step_hi >= 0 {
                        if d1 >= v.noise_attack_target {
                            d1 = v.noise_attack_target;
                            v.noise_phase += 1;
                        }
                    } else {
                        if d1 <= v.noise_attack_target {
                            d1 = v.noise_attack_target;
                            v.noise_phase += 1;
                        }
                    }
                }
                2 => {
                    d1 = d1.wrapping_add(v.noise_decay);
                    let step_hi = (v.noise_decay >> 16) as i16;
                    if step_hi >= 0 {
                        if d1 >= v.noise_decay_target {
                            d1 = v.noise_decay_target;
                            v.noise_phase += 1;
                        }
                    } else {
                        if d1 <= v.noise_decay_target {
                            d1 = v.noise_decay_target;
                            v.noise_phase += 1;
                        }
                    }
                }
                4 => {
                    d1 = d1.wrapping_add(v.noise_release);
                    let step_hi = (v.noise_release >> 16) as i16;
                    if step_hi >= 0 {
                        if d1 >= 0 {
                            d1 = 0;
                        }
                    } else {
                        if d1 <= 0 {
                            d1 = 0;
                        }
                    }
                }
                _ => {}
            }
            v.noise_env_acc = d1;

            // ===== NOISE LFO (offset 102-110, 136) =====
            if v.noise_lfo_limit != 0 {
                if v.noise_lfo_delay > 0 {
                    v.noise_lfo_delay -= 1;
                } else {
                    let mut lfo = v.noise_lfo_acc.wrapping_add(v.noise_lfo_step);
                    let limit = v.noise_lfo_limit;

                    if lfo >= limit {
                        lfo = limit;
                        v.noise_lfo_step = v.noise_lfo_step.wrapping_neg();
                    } else {
                        let neg_limit = limit.wrapping_neg();
                        if lfo <= neg_limit {
                            lfo = neg_limit;
                            v.noise_lfo_step = v.noise_lfo_step.wrapping_neg();
                        }
                    }
                    v.noise_lfo_acc = lfo;
                }
            }

            // ===== WRITE NOISE TO CHIP =====
            let noise_lfo_limit_hi = (v.noise_lfo_limit >> 16) as i16;
            if v.noise_phase != 0 || noise_lfo_limit_hi != 0 {
                if v.noise_freq >= 0 {
                    // move.l 136(a0),d0 / add.l 132(a0),d0
                    let combined = v.noise_lfo_acc.wrapping_add(v.noise_env_acc);

                    // swap d0
                    let hi = ((combined as u32) >> 16) as i16;

                    // add.w 4(a0),d0
                    let mut d0 = (hi as i32) + (v.noise_freq as i32);

                    // bpl.s do_nfr1
                    if d0 < 0 {
                        d0 = 0;
                    }

                    // cmp.b #31,d0 / ble.s do_nfr2
                    if d0 > 31 {
                        d0 = 31;
                    }

                    chip.write_register(6, d0 as u8);
                }
            }

            // ===== DURATION HANDLING =====
            // dec_dur: tst.w 112(a0) / bpl.s endloop
            if v.pitch >= 0 {
                continue;
            }

            // subq.w #1,(a0)
            v.inuse = v.inuse.wrapping_sub(1);

            // bne.s endloop
            if v.inuse != 0 {
                continue;
            }

            // clr.w 114(a0)
            v.priority = 0;

            // tst.w 8(a0) / bne.s dec_dur1
            if v.vol_phase == 0 {
                chip.write_register(8 + voice_idx as u8, 0);
                continue;
            }

            // dec_dur1: subq.w #1,(a0)
            v.inuse = -1;

            // moveq.l #4,d0 / move.w d0,8(a0)
            v.vol_phase = 4;

            // tst.w 36(a0) / beq.s dec_dur2
            if v.freq_phase != 0 {
                v.freq_phase = 4;
                // move.w 54(a0),d1 / move.w 124(a0),d3 / eor.w d1,d3 / bmi.s dec_dur2
                let release_hi = (v.freq_release >> 16) as i16;
                let acc_hi = (v.freq_env_acc >> 16) as i16;
                // If signs are the same (XOR result is positive), negate
                if (release_hi ^ acc_hi) >= 0 {
                    v.freq_release = v.freq_release.wrapping_neg();
                }
            }

            // tst.w 80(a0) / beq.s endloop
            if v.noise_phase != 0 {
                v.noise_phase = 4;
                let release_hi = (v.noise_release >> 16) as i16;
                let acc_hi = (v.noise_env_acc >> 16) as i16;
                if (release_hi ^ acc_hi) >= 0 {
                    v.noise_release = v.noise_release.wrapping_neg();
                }
            }
        }

        // Debug output disabled
    }
}

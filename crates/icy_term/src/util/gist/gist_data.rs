#[derive(Clone, Copy, Debug)]
pub struct GistSoundData {
    words: [i16; 56],
}

impl GistSoundData {
    pub fn from_words(src: &[i16; 56]) -> Self {
        Self { words: *src }
    }

    pub fn words(&self) -> &[i16; 56] {
        &self.words
    }

    pub fn duration(&self) -> i16 {
        self.words[0]
    }

    #[allow(dead_code)]
    pub fn word(&self, index: usize) -> i16 {
        self.words[index]
    }

    #[allow(dead_code)]
    pub fn long(&self, index: usize) -> i32 {
        let hi = self.words[index] as i32;
        let lo = self.words[index + 1] as u16 as i32;
        (hi << 16) | lo
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)]
pub struct GistVoiceTemplate {
    pub duration: i16,
    pub initial_freq: i16,
    pub initial_noise_freq: i16,
    pub initial_volume: i16,
    pub vol_phase: i16,
    pub vol_attack: i32,
    pub vol_decay: i32,
    pub vol_sustain: i32,
    pub vol_release: i32,
    pub vol_lfo_limit: i32,
    pub vol_lfo_step: i32,
    pub vol_lfo_delay: i16,
    pub freq_env_phase: i16,
    pub freq_attack: i32,
    pub freq_attack_target: i32,
    pub freq_decay: i32,
    pub freq_decay_target: i32,
    pub freq_release: i32,
    pub freq_lfo_limit: i32,
    pub freq_lfo_step: i32,
    pub freq_lfo_reset_positive: i32,
    pub freq_lfo_negative_limit: i32,
    pub freq_lfo_reset_negative: i32,
    pub freq_lfo_delay: i16,
    pub noise_env_phase: i16,
    pub noise_attack: i32,
    pub noise_attack_target: i32,
    pub noise_decay: i32,
    pub noise_decay_target: i32,
    pub noise_release: i32,
    pub noise_lfo_limit: i32,
    pub noise_lfo_step: i32,
    pub noise_lfo_delay: i16,
}

impl From<&GistSoundData> for GistVoiceTemplate {
    fn from(data: &GistSoundData) -> Self {
        fn long(words: &[i16; 56], index: usize) -> i32 {
            let hi = words[index] as i32;
            let lo = words[index + 1] as u16 as i32;
            (hi << 16) | lo
        }

        let words = data.words();
        Self {
            duration: words[0],
            initial_freq: words[1],
            initial_noise_freq: words[2],
            initial_volume: words[3],
            vol_phase: words[4],
            vol_attack: long(words, 5),
            vol_decay: long(words, 7),
            vol_sustain: long(words, 9),
            vol_release: long(words, 11),
            vol_lfo_limit: long(words, 13),
            vol_lfo_step: long(words, 15),
            vol_lfo_delay: words[17],
            freq_env_phase: words[18],
            freq_attack: long(words, 19),
            freq_attack_target: long(words, 21),
            freq_decay: long(words, 23),
            freq_decay_target: long(words, 25),
            freq_release: long(words, 27),
            freq_lfo_limit: long(words, 29),
            freq_lfo_step: long(words, 31),
            freq_lfo_reset_positive: long(words, 33),
            freq_lfo_negative_limit: long(words, 35),
            freq_lfo_reset_negative: long(words, 37),
            freq_lfo_delay: words[39],
            noise_env_phase: words[40],
            noise_attack: long(words, 41),
            noise_attack_target: long(words, 43),
            noise_decay: long(words, 45),
            noise_decay_target: long(words, 47),
            noise_release: long(words, 49),
            noise_lfo_limit: long(words, 51),
            noise_lfo_step: long(words, 53),
            noise_lfo_delay: words[55],
        }
    }
}

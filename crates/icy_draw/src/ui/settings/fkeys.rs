//! F-key character set mappings
//!
//! Moebius treats F1..F12 as 12 slots within a selectable character set.
//! Default sets and initial set index are derived from the embedded Moebius defaults.

use std::fs::{File, create_dir_all};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Settings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FKeySets {
    /// Character sets, each containing 12 CP437 codes.
    pub sets: Vec<[u16; 12]>,
    /// Currently selected set index.
    pub current_set: usize,
}

impl Default for FKeySets {
    fn default() -> Self {
        let sets: Vec<[u16; 12]> = vec![
            [218, 191, 192, 217, 196, 179, 195, 180, 193, 194, 32, 32],
            [201, 187, 200, 188, 205, 186, 204, 185, 202, 203, 32, 32],
            [213, 184, 212, 190, 205, 179, 198, 181, 207, 209, 32, 32],
            [214, 183, 211, 189, 196, 186, 199, 182, 208, 210, 32, 32],
            [197, 206, 216, 215, 232, 232, 155, 156, 153, 239, 32, 32],
            [176, 177, 178, 219, 223, 220, 221, 222, 254, 250, 32, 32],
            [1, 2, 3, 4, 5, 6, 240, 14, 15, 32, 32, 32],
            [24, 25, 30, 31, 16, 17, 18, 29, 20, 21, 32, 32],
            [174, 175, 242, 243, 169, 170, 253, 246, 171, 172, 32, 32],
            [227, 241, 244, 245, 234, 157, 228, 248, 251, 252, 32, 32],
            [224, 225, 226, 229, 230, 231, 235, 236, 237, 238, 32, 32],
            [128, 135, 165, 164, 152, 159, 247, 249, 173, 168, 32, 32],
            [131, 132, 133, 160, 166, 134, 142, 143, 145, 146, 32, 32],
            [136, 137, 138, 130, 144, 140, 139, 141, 161, 158, 32, 32],
            [147, 148, 149, 162, 167, 150, 129, 151, 163, 154, 32, 32],
            [47, 92, 40, 41, 123, 125, 91, 93, 96, 39, 32, 32],
            [32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32],
            [32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32],
            [32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32],
            [32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32],
        ];

        let default_set = 5usize; // Moebius `default_fkeys`
        Self {
            sets,
            current_set: default_set,
        }
    }
}

impl FKeySets {
    pub fn load() -> Self {
        let Some(path) = Self::get_file_path() else {
            return Self::default();
        };

        if !path.exists() {
            return Self::default();
        }

        match File::open(&path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                serde_json::from_reader(reader).unwrap_or_else(|_| Self::default())
            }
            Err(e) => {
                log::warn!("Failed to load fkeys: {}", e);
                Self::default()
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::get_file_path() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }

        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &self)?;
        Ok(())
    }

    fn get_file_path() -> Option<PathBuf> {
        Settings::config_dir().map(|dirs| dirs.join("fkeys.json"))
    }

    pub fn set_count(&self) -> usize {
        self.sets.len().max(1)
    }

    pub fn current_set(&self) -> usize {
        self.current_set
    }

    pub fn clamp_current_set(&mut self) {
        let count = self.set_count();
        if self.current_set >= count {
            self.current_set = 0;
        }
    }

    pub fn code_at(&self, set: usize, slot: usize) -> u16 {
        self.sets.get(set).and_then(|s| s.get(slot)).copied().unwrap_or(32)
    }

    pub fn set_code_at(&mut self, set: usize, slot: usize, code: u16) {
        if let Some(set_data) = self.sets.get_mut(set) {
            if let Some(slot_data) = set_data.get_mut(slot) {
                *slot_data = code;
            }
        }
    }

    /// Returns the codes for the currently selected set.
    pub fn current_set_codes(&self) -> [u16; 12] {
        self.sets.get(self.current_set).copied().unwrap_or([32; 12])
    }

    /// Returns the default codes for a given set index.
    pub fn default_set_at(set_idx: usize) -> [u16; 12] {
        let defaults = Self::default();
        defaults.sets.get(set_idx).copied().unwrap_or([32; 12])
    }

    /// Checks if the given set is at default values.
    pub fn is_set_default(&self, set_idx: usize) -> bool {
        let current = self.sets.get(set_idx).copied().unwrap_or([32; 12]);
        let default = Self::default_set_at(set_idx);
        current == default
    }

    /// Resets the given set to default values.
    pub fn reset_set(&mut self, set_idx: usize) {
        if let Some(set_data) = self.sets.get_mut(set_idx) {
            *set_data = Self::default_set_at(set_idx);
        }
    }
}

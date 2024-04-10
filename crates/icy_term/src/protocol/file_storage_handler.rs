use std::{collections::HashMap, fs::File, io::Write, path::PathBuf};

use crate::TerminalResult;

pub trait FileStorageHandler {
    fn open_unnamed_file(&mut self);
    fn open_file(&mut self, file_name: &str, total_size: usize);
    fn append(&mut self, data: &[u8]);
    fn close(&mut self);
    fn remove_cpm_eof(&mut self);

    fn current_file_name(&self) -> Option<String>;
    fn current_file_length(&self) -> usize;
    fn set_current_size_to(&mut self, size: usize);
    fn get_current_file_total_size(&self) -> usize;
}

#[derive(Clone)]
pub struct TestStorageHandler {
    cur_file_name: Option<String>,
    cur_file_size: usize,
    pub file: HashMap<String, Vec<u8>>,
}

impl TestStorageHandler {
    pub fn new() -> Self {
        Self {
            cur_file_name: None,
            cur_file_size: 0,
            file: HashMap::new(),
        }
    }
}
pub const CPMEOF: u8 = 0x1A;

impl FileStorageHandler for TestStorageHandler {
    fn open_unnamed_file(&mut self) {
        self.open_file("No name", 0);
    }

    fn open_file(&mut self, file_name: &str, total_size: usize) {
        let fn_string = file_name.to_string();
        self.cur_file_name = Some(fn_string.clone());
        self.cur_file_size = total_size;
        self.file.insert(fn_string, Vec::new());
    }

    fn current_file_name(&self) -> Option<String> {
        self.cur_file_name.clone()
    }

    fn set_current_size_to(&mut self, size: usize) {
        if let Some(file_name) = &self.cur_file_name {
            self.file.get_mut(file_name).unwrap().resize(size, 0);
        }
    }

    fn remove_cpm_eof(&mut self) {
        if let Some(file_name) = &self.cur_file_name {
            let m = self.file.get_mut(file_name).unwrap();
            while m.ends_with(&[CPMEOF]) {
                m.pop();
            }
        }
    }

    fn append(&mut self, data: &[u8]) {
        if let Some(file_name) = &self.cur_file_name {
            self.file.get_mut(file_name).unwrap().extend_from_slice(data);
        }
    }

    fn close(&mut self) {
        self.cur_file_name = None;
        self.cur_file_size = 0;
    }

    fn current_file_length(&self) -> usize {
        if let Some(file_name) = &self.cur_file_name {
            self.file.get(file_name).unwrap().len()
        } else {
            0
        }
    }
    fn get_current_file_total_size(&self) -> usize {
        self.cur_file_size
    }
}

pub struct DiskStorageHandler {
    cur_file_name: Option<String>,
    cur_total_file_size: usize,
    current_file_length: usize,
    cpm_length: usize,
    output_path: PathBuf,
    file: Option<File>,
}

impl DiskStorageHandler {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new() -> TerminalResult<Self> {
        let Some(user_dirs) = directories::UserDirs::new() else {
            return Err(anyhow::anyhow!("Failed to get user directories"));
        };
        let Some(output_path) = user_dirs.download_dir() else {
            return Err(anyhow::anyhow!("Failed to get user directories"));
        };

        Ok(Self {
            output_path: output_path.to_path_buf(),
            cur_file_name: None,
            cur_total_file_size: 0,
            current_file_length: 0,
            cpm_length: 0,
            file: None,
        })
    }
}

impl FileStorageHandler for DiskStorageHandler {
    fn open_unnamed_file(&mut self) {
        let mut num = 0;
        let f = "x_modem_transferred_file.0".to_string();
        let mut file_name: PathBuf = self.output_path.join(f.clone());

        while file_name.exists() {
            file_name = self.output_path.join(format!("x_modem_transferred_file.{num}"));
            num += 1;
        }

        self.open_file(&format!("x_modem_transferred_file.{num}"), 0);
    }

    fn open_file(&mut self, file_name: &str, total_size: usize) {
        let fn_string = file_name.to_string();
        self.cur_file_name = Some(fn_string.clone());
        self.cur_total_file_size = total_size;

        let f = if file_name.is_empty() { "new_file".to_string() } else { fn_string };

        let mut file_name: PathBuf = self.output_path.join(f.clone());
        let mut i = 1;
        while file_name.exists() {
            file_name = self.output_path.join(&format!("{}.{i}", f.clone()));
            i += 1;
        }
        let fs = std::fs::File::create(file_name).unwrap();
        self.file = Some(fs);
        self.current_file_length = 0;
    }

    fn remove_cpm_eof(&mut self) {
        if let Some(file) = &mut self.file {
            if let Err(err) = file.set_len((self.current_file_length - self.cpm_length) as u64) {
                log::error!("Failed to set file length: {err}");
            }
            self.current_file_length -= self.cpm_length;
            self.cpm_length = 0;
        }
    }

    fn current_file_name(&self) -> Option<String> {
        self.cur_file_name.clone()
    }

    fn set_current_size_to(&mut self, size: usize) {
        self.file.as_ref().unwrap().set_len(size as u64).unwrap();
        self.current_file_length = size;
    }

    fn append(&mut self, data: &[u8]) {
        self.cpm_length = data.len() - data.iter().rev().take_while(|d| **d == CPMEOF).count();

        self.file.as_ref().unwrap().write_all(data).unwrap();
        self.current_file_length += data.len();
    }

    fn close(&mut self) {
        self.file = None;
        self.cur_file_name = None;
        self.cur_total_file_size = 0;
        self.current_file_length = 0;
    }

    fn current_file_length(&self) -> usize {
        self.current_file_length
    }
    fn get_current_file_total_size(&self) -> usize {
        self.cur_total_file_size
    }
}

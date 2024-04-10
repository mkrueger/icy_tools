use std::{path::Path, sync::mpsc::Sender};

use gifski::{progress::NoProgress, Repeat};
use imgref::ImgVec;
use rgb::RGBA8;

use crate::TerminalResult;

use super::encoding::AnimationEncoder;

pub struct GifEncoder {}

impl AnimationEncoder for GifEncoder {
    fn label(&self) -> String {
        "GIF".to_string()
    }
    fn extension(&self) -> String {
        "gif".to_string()
    }

    fn encode(&self, path: &Path, frames: Vec<(Vec<u8>, u32)>, width: usize, height: usize, sender: Sender<usize>) -> TerminalResult<()> {
        let settings = gifski::Settings {
            width: Some(width as u32),
            height: Some(height as u32),
            quality: 100,
            fast: true,
            repeat: Repeat::Infinite,
        };

        let (c, w) = gifski::new(settings)?;
        let mut time = 0.0;
        let mut pb = NoProgress {};
        let path = path.to_path_buf();
        let fs = std::fs::File::create(path)?;
        std::thread::spawn(move || w.write(fs, &mut pb).unwrap());

        for (frame_idx, (data, duration)) in frames.into_iter().enumerate() {
            sender.send(frame_idx)?;
            let mut n = 0;
            let mut d = Vec::new();
            while n < data.len() {
                d.push(rgb::RGBA::new(data[n], data[n + 1], data[n + 2], data[n + 3]));
                n += 4;
            }

            let img: ImgVec<RGBA8> = imgref::Img::new(d, width, height);
            c.add_frame_rgba(frame_idx, img, time / 1000.0)?;
            time += duration as f64;
        }

        Ok(())
    }
}

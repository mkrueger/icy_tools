use ndarray::Array3;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};
use video_rs::{EncoderSettings, Locator, Time};

use crate::TerminalResult;

use super::encoding::AnimationEncoder;

pub struct Mp4Encoder {}

impl AnimationEncoder for Mp4Encoder {
    fn label(&self) -> String {
        "MP4".to_string()
    }
    fn extension(&self) -> String {
        "mp4".to_string()
    }
    fn encode(&self, path: &Path, frames: Vec<(Vec<u8>, u32)>, width: usize, height: usize, sender: Sender<usize>) -> TerminalResult<()> {
        let height = height + (height % 2);

        let destination: Locator = PathBuf::from(path).into();

        let settings = EncoderSettings::for_h264_yuv420p(width, height, false);

        let mut encoder = video_rs::Encoder::new(&destination, settings)?;
        let mut position = Time::zero();

        for (i, (data, delay)) in frames.iter().enumerate() {
            sender.send(i)?;
            let f = Array3::from_shape_fn((height, width, 3), |(y, x, c)| {
                let idx = x * 4 + y * width * 4 + c;
                if idx >= data.len() {
                    return 0;
                }
                data[idx]
            });
            encoder.encode(&f, &position)?;

            let duration: Time = Time::from_secs(1.0 / 1000.0 * *delay as f32);
            position = position.aligned_with(&duration).add();
        }
        encoder.finish()?;
        Ok(())
    }
}

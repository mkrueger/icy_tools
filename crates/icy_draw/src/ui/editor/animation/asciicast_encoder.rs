use icy_engine::{ColorOptimizer, SaveOptions, StringGenerator, TextPane};
use icy_engine_gui::animations::Animator;
use std::{
    fs::File,
    io::Write,
    path::Path,
    sync::{Arc, mpsc::Sender},
};

use super::encoding::AnimationEncoder;
use crate::TerminalResult;

pub struct AsciiCast {}

impl AnimationEncoder for AsciiCast {
    fn label(&self) -> String {
        "AsciiCast".to_string()
    }
    fn extension(&self) -> String {
        "cast".to_string()
    }
    fn encode(&self, _path: &Path, _frames: Vec<(Vec<u8>, u32)>, _width: usize, _height: usize, _sender: Sender<usize>) -> TerminalResult<()> {
        panic!("unsupported");
    }

    fn direct_encoding(&self, path: &Path, animator: Arc<std::sync::Mutex<Animator>>) -> TerminalResult<bool> {
        let Some(file_stem) = path.file_stem() else {
            return Err(anyhow::anyhow!("invalid file name"));
        };
        let Some(name) = file_stem.to_str() else {
            return Err(anyhow::anyhow!("invalid file name"));
        };
        let mut f = File::create(path)?;
        let frame_count = animator.lock().unwrap().frames.len();
        {
            let buf = &animator.lock().unwrap().frames[0].0;
            f.write_all(format!("{{\"version\": 2, \"width\": {}, \"height\": {}, \"timestamp\": 0, \"title\": \"{}\", \"env\": {{\"TERM\": \"IcyTerm\", \"SHELL\": \"/bin/icy_play\"}}, \"theme\": {{ \"fg\": \"{}\", \"bg\": \"{}\", \"palette\": \"{}\" }}  }}\n", 
                buf.get_width(),
                buf.get_height(),
                name,
                buf.palette.get_color(7).to_hex(),
                buf.palette.get_color(0).to_hex(),
                buf.palette.color_iter().take(16).fold(String::new(), |mut s, x| {
                    if !s.is_empty() {
                        s.push(':');
                    }
                    s.push_str(x.to_hex().as_str());
                    s
                })).as_bytes())?;
        }
        let mut timestamp = 0.0;

        for frame in 0..frame_count {
            let mut opt: SaveOptions = SaveOptions::new();
            opt.control_char_handling = icy_engine::ControlCharHandling::FilterOut;
            opt.longer_terminal_output = true;
            opt.compress = true;
            opt.use_cursor_forward = false;
            opt.preserve_line_length = true;
            opt.modern_terminal_output = true;

            let mut str_gen = StringGenerator::new(opt.clone());
            {
                let optimizer = ColorOptimizer::new(&animator.lock().unwrap().frames[frame].0, &opt);
                let buf = optimizer.optimize(&animator.lock().unwrap().frames[frame].0);
                str_gen.generate(&buf, &buf);
            }
            str_gen.line_offsets.push(str_gen.get_data().len());

            let data = str_gen.get_data();
            let mut cur = 0;
            for i in &str_gen.line_offsets {
                if cur < *i {
                    output_line(&mut f, cur, *i, data, timestamp)?;
                }
                cur = *i;
            }
            timestamp += animator.lock().unwrap().frames[frame].2 as f64;
        }

        Ok(true)
    }
}

fn output_line(f: &mut File, from: usize, to: usize, data: &[u8], timestamp: f64) -> TerminalResult<()> {
    f.write_all(format!("[{}, \"o\", ", timestamp / 1000.0).as_bytes())?;
    let s = String::from_utf8_lossy(&data[from..to]);
    f.write_all(serde_json::to_string(&s)?.as_bytes())?;
    f.write_all(b"]\n")?;
    Ok(())
}

use clap::{Parser, Subcommand};
use icy_engine::{Buffer, SaveOptions, TextPane, editor::EditState, update_crc32};
use icy_engine_gui::animations::Animator;
use std::{fs, path::PathBuf, thread, time::Duration};

use crate::com::Com;

mod com;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Terminal {
    IcyTerm,
    SyncTerm,
    Unknown,
    Name(String),
}

impl Terminal {
    pub fn use_dcs(&self) -> bool {
        matches!(self, Terminal::IcyTerm)
    }

    fn can_repeat_rle(&self) -> bool {
        matches!(self, Terminal::IcyTerm | Terminal::SyncTerm)
    }
}

#[derive(Parser)]
pub struct Cli {
    #[arg(help = "If true modern terminal output (UTF8) is used.", long, default_value_t = false)]
    utf8: bool,

    #[arg(help = "Use lf instead of positioning at end of line.", long, default_value_t = false)]
    use_lf: bool,

    #[arg(help = "File to play/show.", required = true)]
    path: Option<PathBuf>,

    #[arg(help = "Socket port address for i/o", long)]
    port: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Clone, Copy)]
enum Commands {
    #[command(about = "Plays the animation (default)")]
    Play,

    #[command(about = "Show a specific frame of the animation")]
    ShowFrame { frame: usize },
}

pub fn get_line_checksums(buf: &Buffer) -> Vec<u32> {
    let mut result = Vec::new();
    for y in 0..buf.get_height() {
        let mut crc = 0;
        for x in 0..buf.get_width() {
            let ch = buf.get_char((x, y));
            crc = update_crc32(crc, ch.ch as u8);
            let fg = buf.palette.get_color(ch.attribute.get_foreground()).get_rgb();
            crc = update_crc32(crc, fg.0);
            crc = update_crc32(crc, fg.1);
            crc = update_crc32(crc, fg.2);
            let bg = buf.palette.get_color(ch.attribute.get_background()).get_rgb();
            crc = update_crc32(crc, bg.0);
            crc = update_crc32(crc, bg.1);
            crc = update_crc32(crc, bg.2);
            crc = update_crc32(crc, ch.attribute.attr as u8);
            crc = update_crc32(crc, (ch.attribute.attr >> 8) as u8);
        }
        result.push(crc);
    }
    result
}

fn main() {
    let args = Cli::parse();

    let mut io: Box<dyn Com> = if let Some(port) = args.port.clone() {
        Box::new(com::SocketCom::connect("127.0.0.1:".to_string() + port.as_str()).unwrap())
    } else {
        Box::new(com::StdioCom::start().unwrap())
    };

    if let Some(path) = args.path.clone() {
        let parent = Some(path.parent().unwrap().to_path_buf());

        let Some(ext) = path.extension() else {
            println!("Error: File extension not found.");
            return;
        };
        let ext = ext.to_string_lossy().to_ascii_lowercase();
        let mut term = Terminal::Unknown;

        match ext.as_str() {
            "icyanim" => match fs::read_to_string(path) {
                Ok(txt) => {
                    let animator = Animator::run(&parent, txt);
                    animator.lock().unwrap().set_is_playing(true);

                    let mut opt: SaveOptions = SaveOptions::default();
                    if args.utf8 {
                        opt.modern_terminal_output = true;
                    }
                    match args.command.unwrap_or(Commands::Play) {
                        Commands::Play => {
                            io.write(b"\x1B[0c").unwrap();
                            match io.read(true) {
                                Ok(Some(data)) => {
                                    let txt: String = String::from_utf8_lossy(&data).to_string();
                                    term = if txt.contains("73;99;121;84;101;114;109") {
                                        Terminal::IcyTerm
                                    } else if txt.contains("67;84;101;114") {
                                        Terminal::SyncTerm
                                    } else {
                                        Terminal::Name(txt)
                                    }
                                } // 67;84;101;114;109;1;316
                                Ok(_) | Err(_) => {
                                    // ignore (timeout)
                                }
                            }
                            // flush.
                            while let Ok(Some(_)) = io.read(false) {}
                            let mut checksums = Vec::new();

                            // turn caret off
                            let _ = io.write(b"\x1b[?25l");

                            while animator.lock().unwrap().is_playing() {
                                if let Ok(Some(v)) = io.read(false) {
                                    if v.contains(&b'\x1b') || v.contains(&b'\n') || v.contains(&b' ') {
                                        break;
                                    }
                                }
                                if let Some((buffer, _, delay)) = animator.lock().unwrap().get_cur_frame_buffer_mut() {
                                    let new_checksums = get_line_checksums(buffer);
                                    let mut skip_lines: Vec<usize> = Vec::new();
                                    if checksums.len() == new_checksums.len() {
                                        for i in 0..checksums.len() {
                                            if checksums[i] == new_checksums[i] {
                                                skip_lines.push(i);
                                            }
                                        }
                                    }
                                    show_buffer(&mut io, buffer, false, &args, &term, skip_lines).unwrap();
                                    checksums = new_checksums;
                                    std::thread::sleep(Duration::from_millis(*delay as u64));
                                } else {
                                    thread::sleep(Duration::from_millis(10));
                                }
                                while !animator.lock().unwrap().next_frame() {
                                    thread::sleep(Duration::from_millis(10));
                                }
                            }
                            let _ = io.write(b"\x1b[?25h\x1b[0;0 D");
                        }
                        Commands::ShowFrame { frame } => {
                            show_buffer(&mut io, &mut animator.lock().unwrap().frames[frame].0, true, &args, &term, Vec::new()).unwrap();
                        }
                    }
                }
                Err(e) => {
                    println!("Error opening file: {e}");
                }
            },
            _ => {
                let mut buffer = Buffer::load_buffer(&path, true, None);
                if let Ok(buffer) = &mut buffer {
                    show_buffer(&mut io, buffer, true, &args, &Terminal::Unknown, Vec::new()).unwrap();
                }
            }
        }
    }
}

fn show_buffer(io: &mut Box<dyn Com>, buffer: &mut Buffer, single_frame: bool, cli: &Cli, terminal: &Terminal, skip_lines: Vec<usize>) -> anyhow::Result<()> {
    let mut opt: SaveOptions = SaveOptions::default();
    if cli.utf8 {
        opt.modern_terminal_output = true;
    }
    opt.control_char_handling = icy_engine::ControlCharHandling::FilterOut;
    opt.longer_terminal_output = !cli.use_lf;
    opt.compress = true;
    opt.use_cursor_forward = false;
    opt.preserve_line_length = true;
    opt.use_repeat_sequences = terminal.can_repeat_rle();
    opt.lossles_output = true;
    opt.skip_lines = Some(skip_lines);
    opt.alt_rgb = cli.utf8;
    opt.save_sauce = false;
    opt.always_use_rgb = cli.utf8;

    if matches!(terminal, Terminal::IcyTerm) {
        opt.control_char_handling = icy_engine::ControlCharHandling::IcyTerm;
    }
    let bytes = if cli.utf8 && buffer.ice_mode == icy_engine::IceMode::Ice {
        let mut state = EditState::from_buffer(buffer.flat_clone(true));
        let _ = state.set_ice_mode(icy_engine::IceMode::Unlimited);
        state.get_buffer_mut().to_bytes("ans", &opt)?
    } else {
        buffer.to_bytes("ans", &opt)?
    };

    if !single_frame && terminal.use_dcs() {
        io.write(b"\x1BP0;1;0!z")?;
    }
    io.write(&bytes)?;
    /*for i in 0..buffer.get_height() {
        io.write(format!("\x1b[{};1H{}:", i + 1, i).as_bytes())?;
    }
    */
    //io.write(format!("\x1b[23;1HTerminal:{:?}", terminal).as_bytes())?;
    if !single_frame && terminal.use_dcs() {
        io.write(b"\x1b\\\x1b[0*z")?;
    }
    Ok(())
}

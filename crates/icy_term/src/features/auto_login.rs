use icy_net::iemsi::IEmsi;
use web_time::Instant;

use crate::{util::PatternRecognizer, Res};

pub struct AutoLogin {
    pub logged_in: bool,
    pub disabled: bool,
    pub iemsi: IEmsi,
    last_char_recv: Instant,
    first_char_recv: Option<Instant>,
    continue_time: Instant,

    login_expr: Vec<u8>,
    cur_expr_idx: usize,
    got_name: bool,
    name_recognizer: PatternRecognizer,
    login_recognizer: PatternRecognizer,

    user_name: String,
    password: String,
}

impl AutoLogin {
    pub fn new(login_expr: &str, user_name: String, password: String) -> Self {
        Self {
            logged_in: false,
            disabled: false,
            iemsi: IEmsi::default(),
            first_char_recv: None,
            last_char_recv: Instant::now(),
            continue_time: Instant::now(),
            login_expr: login_expr.as_bytes().to_vec(),
            cur_expr_idx: 0,
            got_name: false,
            name_recognizer: PatternRecognizer::from(b"NAME", true),
            login_recognizer: PatternRecognizer::from(b"LOGIN:", true),
            user_name,
            password,
        }
    }
    /*
        pub fn run_command(&mut self, con: &mut Connection) -> TerminalResult<bool> {
            let ch = *self.login_expr.get(self.cur_expr_idx + 1).unwrap();
            match ch {
                b'D' => {
                    // Delay for x seconds. !D4= Delay for 4 seconds
                    let ch = self.login_expr[self.cur_expr_idx + 2];
                    self.continue_time = self.last_char_recv + Duration::from_secs(u64::from(ch - b'0'));
                    self.cur_expr_idx += 3;
                }
                b'E' => {
                    // wait until data came in
                    match self.first_char_recv {
                        Some(_) => {
                            if Instant::now().duration_since(self.last_char_recv).as_millis() < 500 {
                                return Ok(true);
                            }
                        }
                        _ => return Ok(true),
                    }
                    self.cur_expr_idx += 2;
                    return Ok(true);
                }
                b'W' => {
                    // Wait for one of the name questions defined arrives
                    if self.got_name {
                        self.cur_expr_idx += 2;
                    }
                    return Ok(false);
                }
                b'N' => {
                    // Send full user name of active user
                    self.cur_expr_idx += 2;
                    con.send((self.user_name.clone() + "\r").as_bytes().to_vec())?;
                }
                b'F' => {
                    // Send first name of active user
                    self.cur_expr_idx += 2;
                    con.send((self.user_name.clone() + "first\r").as_bytes().to_vec())?;
                    // TODO
                }
                b'L' => {
                    // Send last name of active user
                    self.cur_expr_idx += 2;
                    con.send((self.user_name.clone() + "last\r").as_bytes().to_vec())?;
                    // TODO
                }
                b'P' => {
                    // Send password from active user
                    self.cur_expr_idx += 2;
                    con.send((self.password.clone() + "\r").as_bytes().to_vec())?;
                    self.logged_in = true;
                }
                b'I' => {
                    // Disable IEMSI in this session
                    self.cur_expr_idx += 2;
                    self.iemsi.aborted = true;
                }
                ch => {
                    con.send(vec![ch])?;
                    self.cur_expr_idx += 1;
                }
            }

            Ok(true)
        }
*/
        pub fn try_login(&mut self, ch: u8) -> Res<Option<Vec<u8>>> {
            /*
            if self.logged_in || self.disabled {
                return Ok(());
            }
            if self.user_name.is_empty() || self.password.is_empty() {
                self.logged_in = true;
                return Ok(());
            }

            if self.first_char_recv.is_none() && (ch.is_ascii_uppercase() || ch.is_ascii_lowercase()) {
                self.first_char_recv = Some(Instant::now());
            }

            self.last_char_recv = Instant::now();
            self.got_name |= self.name_recognizer.push_ch(ch) | self.login_recognizer.push_ch(ch);

           */
            Ok(self.iemsi.try_login(&self.user_name, &self.password, ch)?)
        }
/* 
        pub fn run_autologin(&mut self, con: &mut Connection) -> TerminalResult<()> {
            if self.logged_in && self.cur_expr_idx >= self.login_expr.len() || self.disabled {
                return Ok(());
            }
            if self.user_name.is_empty() || self.password.is_empty() {
                self.logged_in = true;
                return Ok(());
            }

            if Instant::now() < self.continue_time {
                return Ok(());
            }
            if self.cur_expr_idx < self.login_expr.len() {
                match self.login_expr.get(self.cur_expr_idx).unwrap() {
                    b'!' => {
                        self.run_command(con)?;
                    }
                    b'\\' => {
                        while self.cur_expr_idx < self.login_expr.len() && self.login_expr[self.cur_expr_idx] == b'\\' {
                            self.cur_expr_idx += 1; // escape
                            let ch = self.login_expr.get(self.cur_expr_idx).unwrap();
                            match ch {
                                b'e' => {
                                    con.send(vec![0x1B])?;
                                }
                                b'n' => {
                                    con.send(vec![b'\n'])?;
                                }
                                b'r' => {
                                    con.send(vec![b'\r'])?;
                                }
                                b't' => {
                                    con.send(vec![b'\t'])?;
                                }
                                ch => {
                                    self.cur_expr_idx += 1; // escape
                                    return Err(anyhow::anyhow!("invalid escape sequence in autologin string: {:?}", *ch as char).into());
                                }
                            }
                            self.cur_expr_idx += 1; // escape
                        }
                        self.last_char_recv = Instant::now();
                    }
                    ch => {
                        con.send(vec![*ch])?;
                        self.cur_expr_idx += 1;
                    }
                }
            }
            Ok(())
        }

    */
}

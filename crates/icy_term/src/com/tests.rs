use std::collections::HashMap;
use std::sync::Arc;

use eframe::epaint::mutex::Mutex;

use super::{Com, TermComResult};

pub struct TestCom {
    name: String,
    write_buf: Arc<Mutex<std::collections::VecDeque<u8>>>,
    read_buf: Arc<Mutex<std::collections::VecDeque<u8>>>,
    pub cmd_table: HashMap<u8, String>,
    silent: bool,
}

pub fn indent_receiver() {
    print!("\t\t\t\t\t\t");
}

impl Com for TestCom {
    fn get_name(&self) -> &'static str {
        "Test_Com"
    }

    fn set_terminal_type(&mut self, _terminal: crate::addresses::Terminal) {}

    fn read_data(&mut self) -> TermComResult<Option<Vec<u8>>> {
        let result: Vec<u8> = self.read_buf.lock().drain(0..).collect();

        if !self.silent {
            if self.name == "receiver" {
                indent_receiver();
            }

            if result.len() == 1 {
                if let Some(cmd) = self.cmd_table.get(&result[0]) {
                    println!("{} reads {}({} 0x{})", self.name, cmd, result[0], result[0]);
                } else {
                    println!("{} reads {} 0x{:X}", self.name, result[0], result[0]);
                }
            } else {
                println!("{} reads {:?} #{}", self.name, result, result.len());
            }
        }

        Ok(Some(result))
    }

    fn send(&mut self, buf: &[u8]) -> TermComResult<usize> {
        if !self.silent {
            if self.name == "receiver" {
                indent_receiver();
            }

            if buf.len() == 1 {
                if let Some(cmd) = self.cmd_table.get(&buf[0]) {
                    println!("{} writes {}({} 0x{})", self.name, cmd, buf[0], buf[0]);
                } else {
                    println!("{} writes {} 0x{:X}", self.name, buf[0], buf[0]);
                }
            } else {
                println!("{} writes {:?} #{}", self.name, buf, buf.len());
            }
        }
        self.write_buf.lock().extend(buf.iter());
        Ok(buf.len())
    }

    fn disconnect(&mut self) -> TermComResult<()> {
        // nothing
        Ok(())
    }

    fn default_port(&self) -> u16 {
        0
    }
}

#[cfg(test)]
pub struct TestChannel {
    pub sender: Box<dyn Com>,
    pub receiver: Box<dyn Com>,
}

#[cfg(test)]
impl TestChannel {
    pub fn new(silent: bool) -> Self {
        TestChannel::from_cmd_table(HashMap::new(), silent)
    }

    pub fn from_cmd_table(cmd_table: HashMap<u8, String>, silent: bool) -> Self {
        let b1 = Arc::new(Mutex::new(std::collections::VecDeque::new()));
        let b2 = Arc::new(Mutex::new(std::collections::VecDeque::new()));
        Self {
            sender: Box::new(TestCom {
                name: "sender".to_string(),
                read_buf: b1.clone(),
                write_buf: b2.clone(),
                cmd_table: cmd_table.clone(),
                silent,
            }),
            receiver: Box::new(TestCom {
                name: "receiver".to_string(),
                read_buf: b2,
                write_buf: b1,
                cmd_table,
                silent,
            }),
        }
    }
}

mod communication_tests {
    use crate::com::TestChannel;
    #[test]
    fn test_simple() {
        let mut test = TestChannel::new(false);
        let t = b"Hello World";
        let _ = test.sender.send(t);
        assert_eq!(t.to_vec(), test.receiver.read_data().unwrap().unwrap());
        let _ = test.receiver.send(t);
        assert_eq!(t.to_vec(), test.sender.read_data().unwrap().unwrap());
    }
}

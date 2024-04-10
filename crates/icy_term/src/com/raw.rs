#![allow(dead_code)]

use super::{Com, OpenConnectionData, TermComResult};
use std::{
    io::{self, ErrorKind, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

pub struct ComRawImpl {
    tcp_stream: TcpStream,
}

impl ComRawImpl {
    pub fn connect(connection_data: &OpenConnectionData) -> TermComResult<Self> {
        let addr = connection_data.address.to_string();

        let Some(a) = connection_data.address.to_socket_addrs()?.next() else {
            return Err(Box::new(io::Error::new(ErrorKind::InvalidInput, format!("Invalid address: {addr}"))));
        };

        let tcp_stream = TcpStream::connect_timeout(&a, Duration::from_millis(500))?;

        tcp_stream.set_write_timeout(Some(Duration::from_millis(500)))?;
        tcp_stream.set_read_timeout(Some(Duration::from_millis(500)))?;
        tcp_stream.set_nonblocking(false)?;

        Ok(Self { tcp_stream })
    }
}

impl Com for ComRawImpl {
    fn get_name(&self) -> &'static str {
        "Raw"
    }

    fn default_port(&self) -> u16 {
        0
    }

    fn set_terminal_type(&mut self, _terminal: crate::addresses::Terminal) {}

    fn read_data(&mut self) -> TermComResult<Option<Vec<u8>>> {
        let mut buf = [0; 1024 * 256];
        self.tcp_stream.set_nonblocking(true)?;
        match self.tcp_stream.read(&mut buf) {
            Ok(size) => Ok(Some(buf[0..size].to_vec())),
            Err(ref e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    return Ok(None);
                }
                Err(Box::new(io::Error::new(ErrorKind::ConnectionAborted, format!("Connection aborted: {e}"))))
            }
        }
    }

    fn send(&mut self, buf: &[u8]) -> TermComResult<usize> {
        let r = self.tcp_stream.write_all(buf);

        match r {
            Ok(()) => Ok(buf.len()),
            Err(ref e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    std::thread::sleep(Duration::from_millis(100));
                    return self.send(buf);
                }
                Err(Box::new(io::Error::new(ErrorKind::ConnectionAborted, format!("Connection aborted: {e}"))))
            }
        }
    }

    fn disconnect(&mut self) -> TermComResult<()> {
        self.tcp_stream.shutdown(std::net::Shutdown::Both)?;
        Ok(())
    }
}

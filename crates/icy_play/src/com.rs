use std::{
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

pub trait Com {
    fn write(&mut self, buf: &[u8]) -> anyhow::Result<usize>;
    fn read(&mut self, block: bool) -> anyhow::Result<Option<Vec<u8>>>;
}

pub struct StdioCom {}

impl StdioCom {
    pub fn start() -> anyhow::Result<Self> {
        Ok(Self {})
    }
}
impl Com for StdioCom {
    fn write(&mut self, buf: &[u8]) -> anyhow::Result<usize> {
        std::io::stdout().write_all(buf)?;
        Ok(buf.len())
    }

    fn read(&mut self, _block: bool) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(None)
    }
}

pub struct SocketCom {
    tcp_stream: TcpStream,
}

impl SocketCom {
    pub fn connect<A: ToSocketAddrs>(address: A) -> anyhow::Result<Self> {
        let tcp_stream = TcpStream::connect(address)?;
        tcp_stream.set_read_timeout(Some(Duration::from_millis(500)))?;
        Ok(Self { tcp_stream })
    }
}

impl Com for SocketCom {
    fn write(&mut self, buf: &[u8]) -> anyhow::Result<usize> {
        self.tcp_stream.write_all(buf)?;
        Ok(buf.len())
    }

    fn read(&mut self, block: bool) -> anyhow::Result<Option<Vec<u8>>> {
        let mut buf = [0; 1024 * 256];
        self.tcp_stream.set_nonblocking(!block)?;
        if self.tcp_stream.peek(&mut buf)? == 0 {
            return Ok(None);
        }

        match self.tcp_stream.read(&mut buf) {
            Ok(size) => {
                self.tcp_stream.set_nonblocking(true)?;
                Ok(Some(buf[0..size].to_vec()))
            }
            Err(ref e) => {
                self.tcp_stream.set_nonblocking(true)?;
                if e.kind() == io::ErrorKind::WouldBlock {
                    return Ok(None);
                }
                Err(anyhow::anyhow!(format!("Connection aborted: {e}")))
            }
        }
    }
}

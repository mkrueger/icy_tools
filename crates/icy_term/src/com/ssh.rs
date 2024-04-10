#![allow(dead_code)]

use super::{Com, OpenConnectionData, TermComResult};
use libssh_rs::{Channel, Session, SshOption};
use std::{
    io::ErrorKind,
    io::{Read, Write},
    sync::{Arc, Mutex},
};
use web_time::Duration;
pub struct SSHComImpl {
    session: Session,
    channel: Arc<Mutex<Channel>>,
}

const SUPPORTED_CIPHERS: &str = "aes128-ctr,aes192-ctr,aes256-ctr,aes128-gcm,aes128-gcm@openssh.com,aes256-gcm,aes256-gcm@openssh.com,aes256-cbc,aes192-cbc,aes128-cbc,blowfish-cbc,3des-cbc,arcfour256,arcfour128,cast128-cbc,arcfour";
const SUPPORTED_KEY_EXCHANGES: &str = "ecdh-sha2-nistp256,ecdh-sha2-nistp384,ecdh-sha2-nistp521,diffie-hellman-group14-sha1,diffie-hellman-group1-sha1";

impl SSHComImpl {
    pub fn connect(connection_data: &OpenConnectionData) -> TermComResult<Self> {
        let session = Session::new()?;
        let (host, port) = Self::parse_address(&connection_data.address)?;

        session.set_option(SshOption::Hostname(host))?;
        session.set_option(SshOption::Port(port))?;
        session.set_option(SshOption::KeyExchange(SUPPORTED_KEY_EXCHANGES.to_string()))?;
        session.set_option(SshOption::CiphersCS(SUPPORTED_CIPHERS.to_string()))?;
        session.set_option(SshOption::CiphersSC(SUPPORTED_CIPHERS.to_string()))?;
        session.set_option(SshOption::Timeout(Duration::from_millis(5000)))?;
        session.set_option(SshOption::LogLevel(libssh_rs::LogLevel::Warning))?;

        session.connect()?;

        //  :TODO: SECURITY: verify_known_hosts() implemented here -- ie: user must accept & we save somewhere

        session.userauth_password(Some(connection_data.user_name.as_str()), Some(connection_data.password.as_str()))?;

        let chan = session.new_channel()?;
        chan.open_session()?;
        let terminal_type = connection_data.terminal.to_string().to_lowercase();
        chan.request_pty(
            terminal_type.as_str(),
            connection_data.window_size.width as u32,
            connection_data.window_size.height as u32,
        )?;
        chan.request_shell()?;
        session.set_blocking(false);

        Ok(Self {
            session,
            channel: Arc::new(Mutex::new(chan)),
        })
    }

    fn default_port() -> u16 {
        22
    }

    fn parse_address(addr: &str) -> TermComResult<(String, u16)> {
        let components: Vec<&str> = addr.split(':').collect();
        match components.first() {
            Some(host) => match components.get(1) {
                Some(port_str) => {
                    let port = port_str.parse()?;
                    Ok(((*host).to_string(), port))
                }
                _ => Ok(((*host).to_string(), Self::default_port())),
            },
            _ => Err(Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid address"))),
        }
    }
}

impl Com for SSHComImpl {
    fn get_name(&self) -> &'static str {
        "SSH"
    }

    fn default_port(&self) -> u16 {
        SSHComImpl::default_port()
    }

    fn set_terminal_type(&mut self, _terminal: crate::addresses::Terminal) {}

    fn read_data(&mut self) -> TermComResult<Option<Vec<u8>>> {
        let mut buf = [0; 1024 * 256];
        match self.channel.lock() {
            Ok(locked) => {
                let mut stdout = locked.stdout();
                match stdout.read(&mut buf) {
                    Ok(size) => Ok(Some(buf[0..size].to_vec())),
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            return Ok(None);
                        }
                        Err(Box::new(std::io::Error::new(ErrorKind::ConnectionAborted, format!("Connection aborted: {e}"))))
                    }
                }
            }
            Err(err) => Err(Box::new(std::io::Error::new(
                ErrorKind::ConnectionAborted,
                format!("Can't lock channel: {err}"),
            ))),
        }
    }

    fn send(&mut self, buf: &[u8]) -> TermComResult<usize> {
        match self.channel.lock() {
            Ok(locked) => {
                locked.stdin().write_all(buf)?;
                Ok(buf.len())
            }
            Err(err) => Err(Box::new(std::io::Error::new(
                ErrorKind::ConnectionAborted,
                format!("Can't lock channel: {err}"),
            ))),
        }
    }

    fn disconnect(&mut self) -> TermComResult<()> {
        self.session.disconnect();
        Ok(())
    }
}

/* Trushh:
#![allow(dead_code)]

use super::{Com, TermComResult};
use crate::address_mod::Address;
use async_trait::async_trait;
use std::{collections::VecDeque, sync::Arc, time::Duration};
use thrussh::{
    client::{self, Channel},
    ChannelId, Disconnect,
};
use thrussh_keys::key;

pub struct SSHCom {
    channel: Option<Channel>,
    cur_data: VecDeque<u8>,
    inner: Option<client::Handle<Client>>,
}

impl SSHCom {
    pub fn new() -> Self {
        Self {
            channel: None,
            cur_data: VecDeque::new(),
            inner: None,
        }
    }
}
struct Client {}

impl client::Handler for Client {
    type Error = thrussh::Error;
    type FutureUnit = futures::future::Ready<Result<(Self, client::Session), Self::Error>>;
    type FutureBool = futures::future::Ready<Result<(Self, bool), Self::Error>>;

    fn finished_bool(self, b: bool) -> Self::FutureBool {
        futures::future::ready(Ok((self, b)))
    }

    fn finished(self, session: client::Session) -> Self::FutureUnit {
        futures::future::ready(Ok((self, session)))
    }

    fn check_server_key(self, server_public_key: &key::PublicKey) -> Self::FutureBool {
        println!("check server key: {:?}", server_public_key);
        self.finished_bool(true)
    }

    fn auth_banner(self, banner: &str, session: client::Session) -> Self::FutureUnit {
        println!("--------");
        println!("{banner}");

        self.finished(session)
    }

    fn channel_open_confirmation(
        self,
        id: ChannelId,
        _max_packet_size: u32,
        _window_size: u32,
        session: client::Session,
    ) -> Self::FutureUnit {
        println!("channel_open_confirmation: {id:?}");
        self.finished(session)
    }

    fn data(self, _channel: ChannelId, data: &[u8], session: client::Session) -> Self::FutureUnit {
        println!("got data : {data:?}");
        self.finished(session)
    }
}

#[async_trait]
impl Com for SSHCom {
    fn get_name(&self) -> &'static str {
        "SSH"
    }
    fn set_terminal_type(&mut self, _terminal: crate::address_mod::Terminal) {}

    async fn connect(&mut self, addr: &Address, _timeout: Duration) -> TermComResult<bool> {
        println!("connect!!!");
        let config = thrussh::client::Config::default();
        let config = Arc::new(config);
        let handler = Client {};
        let mut session = thrussh::client::connect(config, &addr.address, handler)
            .await
            .unwrap();
        let auth_res = session
            .authenticate_password(&addr.user_name, &addr.password)
            .await?;
        println!("authenticate result: {}", auth_res);

        let channel = session.channel_open_session().await.unwrap();

        self.channel = Some(channel);
        self.inner = Some(session);
        Ok(true)
    }

    async fn read_data(&mut self) -> TermComResult<Vec<u8>> {
        if let Some(msg) = self.channel.as_mut().unwrap().wait().await {
            if let thrussh::ChannelMsg::Data { ref data } = msg {
                return Ok(data.to_vec());
            }
        }

        Ok(Vec::new())
    }

    async fn read_u8(&mut self) -> TermComResult<u8> {
        Ok(0)
    }

    async fn read_exact(&mut self, _len: usize) -> TermComResult<Vec<u8>> {
        Ok(Vec::new())
    }

    async fn send<'a>(&mut self, buf: &'a [u8]) -> TermComResult<usize> {
        self.channel.as_mut().unwrap().data(buf).await?;
        Ok(buf.len())
    }

    async fn disconnect(&mut self) -> TermComResult<()> {
        self.inner
            .as_mut()
            .unwrap()
            .disconnect(Disconnect::ByApplication, "", "English")
            .await?;
        Ok(())
    }
}
    */

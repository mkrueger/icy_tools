use crate::addresses;
use std::sync::Arc;

use super::{Com, OpenConnectionData, TermComResult};

use http::Uri;
use rustls::RootCertStore;
use std::io::ErrorKind;
use std::net::TcpStream;
use tungstenite::{client::IntoClientRequest, stream::MaybeTlsStream, Error, Message, WebSocket};
/*
struct NoCertVerifier {}

impl ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
}*/
pub struct WebSocketComImpl {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl WebSocketComImpl {
    pub fn connect(connection_data: &OpenConnectionData) -> TermComResult<Self> {
        let is_secure = connection_data.protocol == addresses::Protocol::WebSocket(true);

        // build an ws:// or wss:// address
        //  :TODO: default port if not supplied in address
        let url = format!("{}://{}", Self::schema_prefix(is_secure), connection_data.address);

        let req = Uri::try_from(url)?.into_client_request()?;

        let mut root_store: RootCertStore = RootCertStore::empty();

        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let config = rustls::ClientConfig::builder().with_root_certificates(root_store).with_no_client_auth();

        // enable this line to test non-secure (ie: invalid certs) wss:// -- we could make this an option in the UI
        //config.dangerous().set_certificate_verifier(Arc::new(NoCertVerifier{}));

        let config = Arc::new(config);

        let stream = TcpStream::connect(connection_data.address.clone())?;
        let connector: tungstenite::Connector = tungstenite::Connector::Rustls(config);
        let (mut socket, _) = tungstenite::client_tls_with_config(req, stream, None, Some(connector))?;

        let s = socket.get_mut();
        match s {
            MaybeTlsStream::Plain(s) => {
                s.set_nonblocking(true)?;
            }
            MaybeTlsStream::Rustls(tls) => {
                tls.sock.set_nonblocking(true)?;
            }
            _ => (),
        }

        Ok(Self { socket })
    }

    fn schema_prefix(is_secure: bool) -> &'static str {
        if is_secure {
            "wss"
        } else {
            "ws"
        }
    }
}

impl Com for WebSocketComImpl {
    fn get_name(&self) -> &'static str {
        "WebSocket"
    }

    fn default_port(&self) -> u16 {
        443 // generally secure by default
    }

    fn set_terminal_type(&mut self, _terminal: crate::addresses::Terminal) {}

    fn read_data(&mut self) -> TermComResult<Option<Vec<u8>>> {
        match self.socket.read() {
            Ok(msg) => Ok(Some(msg.into_data())),
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(Box::new(std::io::Error::new(ErrorKind::ConnectionAborted, format!("Connection aborted: {e}")))),
        }
    }
    fn send(&mut self, buf: &[u8]) -> TermComResult<usize> {
        let msg = Message::binary(buf);
        self.socket.send(msg)?; // write + flush
        Ok(buf.len())
    }

    fn disconnect(&mut self) -> TermComResult<()> {
        Ok(self.socket.close(None)?)
    }
}

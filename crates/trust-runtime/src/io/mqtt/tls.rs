//! The TLS client of a computer's `mqtt` driver (specification 32 §3), built with rustls, the same
//! on every computer: the broker's certificate authorities from `tls_ca_path` and no others, the
//! client's certificate and key for mutual TLS, and the ALPN protocols. The key stays in the
//! runtime's memory. The shared rules check the paths as text; the files are read here, when the
//! parameters are checked and when the driver starts, so that a file that cannot be read or does
//! not hold what its key names refuses the configuration before any worker starts (specification 32
//! §2). The broker's host is the name that the client verifies; the worker gives it with each
//! connection, and carries the connection's bytes through [`TlsConnection`].

use std::fs;
use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::version::{TLS12, TLS13};
use rustls::{ClientConfig, ClientConnection, RootCertStore};
use trust_runtime_services::io_drivers::mqtt::MqttParams;

use crate::error::RuntimeError;
use crate::security::ensure_rustls_crypto_provider;

/// The TLS client of a driver, which each of its connections uses.
#[derive(Debug, Clone)]
pub(super) struct TlsClient(Arc<ClientConfig>);

/// The TLS client that `params` ask for, or `None` without TLS. The files are read first, the
/// authority's, then the client's certificate and key; their contents are checked after, in the
/// same order.
///
/// # Errors
///
/// `mqtt <key> '<path>' cannot be read: <reason>` for a file that cannot be read, and the refusal
/// of an authority file without a certificate that can be trusted, of a certificate file without a
/// certificate, of a key file without a private key, and of a key that does not belong to the
/// certificate.
pub(super) fn client(params: &MqttParams) -> Result<Option<TlsClient>, RuntimeError> {
    let Some(tls) = &params.tls else {
        return Ok(None);
    };
    let authority = read(&tls.ca_path, "tls_ca_path")?;
    let pair = match &tls.client_auth {
        None => None,
        Some(auth) => Some((
            read(&auth.cert_path, "tls_client_cert_path")?,
            read(&auth.key_path, "tls_client_key_path")?,
        )),
    };
    let not_an_authority =
        |reason: String| refused(format!("mqtt tls_ca_path is not a valid PEM CA: {reason}"));
    let mut roots = RootCertStore::empty();
    for certificate in certificates(&authority).map_err(not_an_authority)? {
        roots
            .add(certificate)
            .map_err(|error| not_an_authority(error.to_string()))?;
    }
    ensure_rustls_crypto_provider()?;
    let builder = ClientConfig::builder_with_protocol_versions(&[&TLS13, &TLS12])
        .with_root_certificates(roots);
    let mut config = match pair {
        None => builder.with_no_client_auth(),
        Some((chain, key)) => {
            let chain = certificates(&chain).map_err(|reason| {
                refused(format!(
                    "mqtt tls_client_cert_path is not a valid PEM certificate: {reason}"
                ))
            })?;
            let key = PrivateKeyDer::from_pem_slice(&key).map_err(|error| {
                refused(format!(
                    "mqtt tls_client_key_path is not a valid PEM private key: {error}"
                ))
            })?;
            builder.with_client_auth_cert(chain, key).map_err(|error| {
                refused(format!(
                    "mqtt mTLS client certificate/key are not a usable pair: {error}"
                ))
            })?
        }
    };
    if let Some(alpn) = &tls.alpn {
        config.alpn_protocols = alpn
            .iter()
            .map(|protocol| protocol.as_bytes().to_vec())
            .collect();
    }
    Ok(Some(TlsClient(Arc::new(config))))
}

/// The certificates of the PEM text `pem`, in their order; why there are none.
fn certificates(pem: &[u8]) -> Result<Vec<CertificateDer<'static>>, String> {
    let certificates = CertificateDer::pem_slice_iter(pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if certificates.is_empty() {
        return Err(String::from("it holds no certificate"));
    }
    Ok(certificates)
}

/// The bytes of the file at `path`, which `key` names.
fn read(path: &str, key: &str) -> Result<Vec<u8>, RuntimeError> {
    fs::read(path).map_err(|error| refused(format!("mqtt {key} '{path}' cannot be read: {error}")))
}

/// The refusal of the configuration for `reason`.
fn refused(reason: String) -> RuntimeError {
    RuntimeError::InvalidConfig(reason.into())
}

impl TlsClient {
    /// A connection over `socket` that verifies the broker's certificate for `host`, its handshake
    /// not begun; or why there is none, such as a host that is no TLS server name.
    pub(super) fn connection(
        &self,
        host: &str,
        socket: TcpStream,
    ) -> Result<TlsConnection, String> {
        let name = ServerName::try_from(host.to_owned())
            .map_err(|error| format!("'{host}' is no TLS server name: {error}"))?;
        let session = ClientConnection::new(Arc::clone(&self.0), name)
            .map_err(|error| format!("the TLS session cannot start: {error}"))?;
        Ok(TlsConnection { session, socket })
    }

    /// The configuration that the client's connections use.
    #[cfg(test)]
    pub(super) fn config(&self) -> &ClientConfig {
        &self.0
    }
}

/// A TLS connection to the broker: the client's session and the TCP connection under it. Its reads
/// and writes wait only when the socket does. A write takes the bytes into the session's records
/// once the socket took the records before them, and the records go out as far as the socket takes
/// them, the rest with the next read or write; a read takes the records that came.
pub(super) struct TlsConnection {
    session: ClientConnection,
    socket: TcpStream,
}

impl TlsConnection {
    /// Whether the handshake is still going on.
    pub(super) fn handshaking(&self) -> bool {
        self.session.is_handshaking()
    }

    /// Carries the handshake on, as far as the socket's reads and writes go before their time
    /// limits.
    pub(super) fn handshake(&mut self) -> io::Result<()> {
        self.session.complete_io(&mut self.socket).map(drop)
    }

    /// The TCP connection under it.
    pub(super) fn socket(&self) -> &TcpStream {
        &self.socket
    }

    /// Sends TLS's close notification, as far as the socket takes it.
    pub(super) fn close(&mut self) {
        self.session.send_close_notify();
        let _ = self.push();
    }

    /// Writes the session's records to the socket until it takes no more without waiting.
    fn push(&mut self) -> io::Result<()> {
        while self.session.wants_write() {
            match self.session.write_tls(&mut self.socket) {
                Ok(0) => return Err(ErrorKind::WriteZero.into()),
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

impl Read for TlsConnection {
    /// The bytes that came from the broker: the session's plaintext, once it took the records that
    /// the socket holds when it had none. `WouldBlock` when nothing came. The broker's end of the
    /// connection, with or without TLS's close notification, reads as its end.
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.push()?;
        loop {
            match self.session.reader().read(buffer) {
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(0),
                done => return done,
            }
            self.session.read_tls(&mut self.socket)?;
            self.session
                .process_new_packets()
                .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))?;
        }
    }
}

impl Write for TlsConnection {
    /// Takes `bytes` into the session's records and writes the records as far as the socket takes
    /// them; `WouldBlock`, taking nothing, while records of earlier bytes wait for the socket.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.push()?;
        if self.session.wants_write() {
            return Err(ErrorKind::WouldBlock.into());
        }
        let taken = self.session.writer().write(bytes)?;
        self.push()?;
        Ok(taken)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.push()
    }
}

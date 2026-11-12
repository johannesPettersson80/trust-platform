use crate::connection::Connection;
use crate::util::refined_tcp_stream::Stream as RefinedStream;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

/// A wrapper around an owned Rustls connection and corresponding stream.
///
/// Uses an internal Mutex to permit disparate reader & writer threads to access the stream independently.
pub(crate) struct RustlsStream(
    Arc<Mutex<rustls::StreamOwned<rustls::ServerConnection, Connection>>>,
);

impl RustlsStream {
    pub(crate) fn peer_addr(&mut self) -> std::io::Result<Option<SocketAddr>> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .sock
            .peer_addr()
    }

    pub(crate) fn shutdown(&mut self, how: Shutdown) -> std::io::Result<()> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .sock
            .shutdown(how)
    }
}

impl Clone for RustlsStream {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Read for RustlsStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .read(buf)
    }
}

impl Write for RustlsStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .flush()
    }
}

pub(crate) struct RustlsContext(Arc<rustls::ServerConfig>);

impl RustlsContext {
    /// The server configuration of the PEM certificate chain `certificates` and the PEM private key
    /// `private_key`, with the aws-lc-rs provider and its safe default protocol versions.
    ///
    /// Vendored for truST (`third_party/tiny_http/VENDORED.md`): tiny_http 0.12.0 took rustls 0.20,
    /// parsed the PEM with rustls-pemfile 0.2 and panicked on a key it could not read; this takes
    /// rustls 0.23 and the PEM parser of its `pki_types`, and returns an error for an empty or
    /// malformed chain, a missing or malformed key, and a key that does not match the certificate.
    pub(crate) fn from_pem(
        certificates: Vec<u8>,
        private_key: Zeroizing<Vec<u8>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        use rustls::pki_types::pem::PemObject;
        use rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let certificate_chain = CertificateDer::pem_slice_iter(&certificates)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Couldn't read the certificate chain: {error}"))?;
        if certificate_chain.is_empty() {
            return Err("Couldn't extract certificate chain from config.".into());
        }
        let private_key = PrivateKeyDer::from_pem_slice(&private_key)
            .map_err(|error| format!("Couldn't read the private key: {error}"))?;

        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let tls_conf = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(certificate_chain, private_key)?;

        Ok(Self(Arc::new(tls_conf)))
    }

    pub(crate) fn accept(
        &self,
        stream: Connection,
    ) -> Result<RustlsStream, Box<dyn Error + Send + Sync + 'static>> {
        let connection = rustls::ServerConnection::new(self.0.clone())?;
        Ok(RustlsStream(Arc::new(Mutex::new(
            rustls::StreamOwned::new(connection, stream),
        ))))
    }
}

impl From<RustlsStream> for RefinedStream {
    fn from(stream: RustlsStream) -> Self {
        Self::Https(stream)
    }
}

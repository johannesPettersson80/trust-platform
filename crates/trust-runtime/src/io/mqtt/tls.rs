//! The TLS client of a computer's `mqtt` driver (specification 32 §3), built with the TLS library
//! that the computer uses, `native-tls`: the broker's certificate authority from `tls_ca_path`, the
//! client's certificate and key for mutual TLS, and the ALPN protocols. The shared rules check the
//! paths as text; the files are read here, when the parameters are checked and when the driver
//! starts, so that a file that cannot be read or does not hold what its key names refuses the
//! configuration before any worker starts (specification 32 §2). The broker's host is the name
//! that the client verifies; the worker gives it with each connection.

use std::fs;

use native_tls::{Certificate, Identity, TlsConnector};
use smol_str::SmolStr;
use trust_runtime_services::io_drivers::mqtt::MqttParams;

use crate::error::RuntimeError;

/// The TLS client that `params` ask for, or `None` without TLS. The files are read first, the
/// authority's, then the client's certificate and key; their contents are checked after.
///
/// # Errors
///
/// `mqtt <key> '<path>' cannot be read: <reason>` for a file that cannot be read, and the refusal
/// of an authority that is not a PEM certificate, of a certificate and a key that are not a PKCS#8
/// PEM pair, and of a client that the TLS library cannot build.
pub(super) fn client(params: &MqttParams) -> Result<Option<TlsConnector>, RuntimeError> {
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
    let mut builder = TlsConnector::builder();
    let authority = Certificate::from_pem(&authority)
        .map_err(|error| refused(format!("mqtt tls_ca_path is not a valid PEM CA: {error}")))?;
    builder.add_root_certificate(authority);
    if let Some((certificate, key)) = pair {
        let identity = Identity::from_pkcs8(&certificate, &key).map_err(|error| {
            refused(format!(
                "mqtt mTLS client certificate/key are not valid PKCS#8 PEM: {error}"
            ))
        })?;
        builder.identity(identity);
    }
    if let Some(alpn) = &tls.alpn {
        let protocols: Vec<&str> = alpn.iter().map(SmolStr::as_str).collect();
        builder.request_alpns(&protocols);
    }
    builder
        .build()
        .map(Some)
        .map_err(|error| refused(format!("mqtt TLS setup failed: {error}")))
}

/// The bytes of the file at `path`, which `key` names.
fn read(path: &str, key: &str) -> Result<Vec<u8>, RuntimeError> {
    fs::read(path).map_err(|error| refused(format!("mqtt {key} '{path}' cannot be read: {error}")))
}

/// The refusal of the configuration for `reason`.
fn refused(reason: String) -> RuntimeError {
    RuntimeError::InvalidConfig(reason.into())
}

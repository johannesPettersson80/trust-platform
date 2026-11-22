//! How a runtime verifies another runtime's HTTPS certificate (specification 11, "A runtime's trust
//! in another runtime's certificate"): against the CA bundle `[runtime.tls] ca_path` names, and that
//! bundle alone, or against the public roots when no bundle is named. Every HTTPS client of the web
//! layer that calls another runtime builds its agent with this configuration.

use ureq::tls::{parse_pem, Certificate, PemItem, RootCerts, TlsConfig};

use crate::error::RuntimeError;
use crate::security::TlsMaterials;

/// The TLS configuration of every call this runtime makes to another runtime, built once when the
/// web server starts. A named bundle that holds no certificate is refused, which stops the start.
pub(super) fn peer_tls_config(materials: Option<&TlsMaterials>) -> Result<TlsConfig, RuntimeError> {
    let Some(materials) = materials.filter(|materials| materials.ca_path.is_some()) else {
        return Ok(TlsConfig::default());
    };
    let mut certificates: Vec<Certificate<'static>> = Vec::new();
    for item in parse_pem(&materials.ca_pem) {
        match item {
            Ok(PemItem::Certificate(certificate)) => certificates.push(certificate),
            Ok(_) => {}
            Err(error) => {
                return Err(RuntimeError::ControlError(
                    format!("parse tls ca certificate: {error}").into(),
                ))
            }
        }
    }
    if certificates.is_empty() {
        return Err(RuntimeError::ControlError(
            "parse tls ca certificate: no certificates found".into(),
        ));
    }
    Ok(TlsConfig::builder()
        .root_certs(RootCerts::new_with_certs(&certificates))
        .build())
}

#[cfg(test)]
#[path = "peer_tls_tests.rs"]
mod tests;

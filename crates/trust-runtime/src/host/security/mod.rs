//! Security roles and authorization helpers.

#![allow(missing_docs)]

pub mod pairing;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use serde::{Deserialize, Serialize};

use crate::config::{TlsConfig, TlsMode};
use crate::error::RuntimeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessRole {
    Viewer,
    Operator,
    Engineer,
    Admin,
}

impl AccessRole {
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "viewer" => Some(Self::Viewer),
            "operator" => Some(Self::Operator),
            "engineer" => Some(Self::Engineer),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Operator => "operator",
            Self::Engineer => "engineer",
            Self::Admin => "admin",
        }
    }

    pub fn allows(self, required: Self) -> bool {
        self >= required
    }
}

/// Compare two secrets in constant time with respect to `expected` length.
#[must_use]
pub fn constant_time_eq(expected: &str, provided: &str) -> bool {
    let expected_bytes = expected.as_bytes();
    let provided_bytes = provided.as_bytes();
    let mut diff = expected_bytes.len() ^ provided_bytes.len();
    for (idx, expected_byte) in expected_bytes.iter().enumerate() {
        let provided_byte = provided_bytes.get(idx).copied().unwrap_or_default();
        diff |= (*expected_byte ^ provided_byte) as usize;
    }
    diff == 0
}

#[derive(Debug, Clone)]
pub struct TlsMaterials {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub ca_path: Option<PathBuf>,
    pub certificate_pem: Vec<u8>,
    pub private_key_pem: Vec<u8>,
    pub ca_pem: Vec<u8>,
}

impl TlsMaterials {
    #[must_use]
    pub fn tiny_http_ssl_config(&self) -> tiny_http::SslConfig {
        tiny_http::SslConfig {
            certificate: self.certificate_pem.clone(),
            private_key: self.private_key_pem.clone(),
        }
    }
}

pub fn load_tls_materials(
    config: &TlsConfig,
    project_root: Option<&Path>,
) -> Result<Option<TlsMaterials>, RuntimeError> {
    if matches!(config.mode, TlsMode::Disabled) {
        return Ok(None);
    }
    let cert_path = resolve_tls_path(
        config
            .cert_path
            .as_ref()
            .ok_or_else(|| RuntimeError::ControlError("missing tls cert_path".into()))?,
        project_root,
    )?;
    let key_path = resolve_tls_path(
        config
            .key_path
            .as_ref()
            .ok_or_else(|| RuntimeError::ControlError("missing tls key_path".into()))?,
        project_root,
    )?;
    let ca_path = config
        .ca_path
        .as_ref()
        .map(|path| resolve_tls_path(path, project_root))
        .transpose()?;
    let certificate_pem = std::fs::read(&cert_path).map_err(|err| {
        RuntimeError::ControlError(format!("read tls cert '{}': {err}", cert_path.display()).into())
    })?;
    let private_key_pem = std::fs::read(&key_path).map_err(|err| {
        RuntimeError::ControlError(format!("read tls key '{}': {err}", key_path.display()).into())
    })?;
    let ca_pem = if let Some(path) = ca_path.as_ref() {
        std::fs::read(path).map_err(|err| {
            RuntimeError::ControlError(format!("read tls ca '{}': {err}", path.display()).into())
        })?
    } else {
        certificate_pem.clone()
    };

    Ok(Some(TlsMaterials {
        cert_path,
        key_path,
        ca_path,
        certificate_pem,
        private_key_pem,
        ca_pem,
    }))
}

pub fn rustls_server_config(
    materials: &TlsMaterials,
) -> Result<Arc<rustls::ServerConfig>, RuntimeError> {
    ensure_rustls_crypto_provider()?;
    let certs = parse_pem_certs(&materials.certificate_pem, "tls certificate")?;
    let key = parse_pem_key(&materials.private_key_pem, "tls private key")?;
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| {
            RuntimeError::ControlError(format!("build tls server config: {err}").into())
        })?;
    Ok(Arc::new(config))
}

pub fn rustls_client_config(
    materials: &TlsMaterials,
) -> Result<Arc<rustls::ClientConfig>, RuntimeError> {
    ensure_rustls_crypto_provider()?;
    let certs = parse_pem_certs(&materials.ca_pem, "tls ca certificate")?;
    let mut roots = rustls::RootCertStore::empty();
    for cert in certs {
        roots.add(cert).map_err(|err| {
            RuntimeError::ControlError(format!("invalid tls ca certificate: {err}").into())
        })?;
    }
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

fn ensure_rustls_crypto_provider() -> Result<(), RuntimeError> {
    static INSTALL_RESULT: OnceLock<Result<(), String>> = OnceLock::new();
    let result = INSTALL_RESULT.get_or_init(|| {
        if rustls::crypto::CryptoProvider::get_default().is_some() {
            return Ok(());
        }
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .map_err(|_| "install default rustls crypto provider (aws-lc-rs)".to_string())
    });
    result
        .as_ref()
        .map_err(|message| RuntimeError::ControlError(message.clone().into()))
        .map(|_| ())
}

fn resolve_tls_path(path: &Path, project_root: Option<&Path>) -> Result<PathBuf, RuntimeError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let root = project_root.ok_or_else(|| {
        RuntimeError::ControlError("relative tls path requires project root".into())
    })?;
    Ok(root.join(path))
}

fn parse_pem_certs(pem: &[u8], label: &str) -> Result<Vec<CertificateDer<'static>>, RuntimeError> {
    let certs = CertificateDer::pem_slice_iter(pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| RuntimeError::ControlError(format!("parse {label}: {err}").into()))?;
    if certs.is_empty() {
        return Err(RuntimeError::ControlError(
            format!("parse {label}: no certificates found").into(),
        ));
    }
    Ok(certs)
}

fn parse_pem_key(pem: &[u8], label: &str) -> Result<PrivateKeyDer<'static>, RuntimeError> {
    PrivateKeyDer::from_pem_slice(pem)
        .map_err(|err| RuntimeError::ControlError(format!("parse {label}: {err}").into()))
}

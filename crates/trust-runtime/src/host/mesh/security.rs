//! Apply the runtime's existing mesh token and TLS material to the actual Zenoh session.
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use serde_json::json;

use crate::config::MeshConfig;
use crate::security::TlsMaterials;

/// Zenoh reads its password dictionary while opening; remove the private temporary file after it.
pub(super) struct AuthenticationFile(PathBuf);
impl Drop for AuthenticationFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub(super) fn configure(
    zenoh: &mut zenoh::Config,
    config: &MeshConfig,
    materials: Option<&TlsMaterials>,
) -> Result<Option<AuthenticationFile>, String> {
    let set = |zenoh: &mut zenoh::Config, key: &str, value: serde_json::Value| {
        zenoh
            .insert_json5(key, &value.to_string())
            .map_err(|_| format!("cannot configure {key}"))
    };
    if config.tls {
        let materials =
            materials.ok_or("mesh TLS requires the runtime certificate and key material")?;
        // Validate the material and initialize the same rustls provider used by web/control TLS.
        crate::security::rustls_server_config(materials).map_err(|error| error.to_string())?;
        set(
            zenoh,
            "transport/link/tls",
            json!({
                "root_ca_certificate_base64": STANDARD.encode(&materials.ca_pem),
                "listen_certificate_base64": STANDARD.encode(&materials.certificate_pem),
                "listen_private_key_base64": STANDARD.encode(&materials.private_key_pem),
                "verify_name_on_connect": true,
            }),
        )?;
    }
    // A rejected/unreachable peer must not prevent the serving runtime from starting.
    set(zenoh, "connect/timeout_ms", json!(1000))?;
    set(zenoh, "connect/exit_on_failure", json!(false))?;
    let Some(token) = config.auth_token.as_ref() else {
        return Ok(None);
    };
    if token.trim().is_empty() {
        return Err("mesh authentication token must not be empty".into());
    }
    // Encoding prevents a token containing separators/newlines from adding dictionary users.
    let password = URL_SAFE_NO_PAD.encode(token.as_bytes());
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| "cannot obtain mesh authentication randomness")?;
    let path = std::env::temp_dir().join(format!(
        "trust-mesh-auth-{}-{}",
        std::process::id(),
        URL_SAFE_NO_PAD.encode(random)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|error| format!("cannot create mesh authentication file: {error}"))?;
    let guard = AuthenticationFile(path);
    file.write_all(format!("trust-mesh:{password}\n").as_bytes())
        .map_err(|error| format!("cannot write mesh authentication file: {error}"))?;
    drop(file);
    set(
        zenoh,
        "transport/auth/usrpwd",
        json!({
            "user": "trust-mesh", "password": password, "dictionary_file": guard.0.to_string_lossy(),
        }),
    )?;
    Ok(Some(guard))
}

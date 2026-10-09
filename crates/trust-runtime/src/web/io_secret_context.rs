//! Check destination identity before a redacted edit can reuse stored credentials.

use super::{RuntimeError, SECRET_MARKER};

pub(super) fn check(
    driver: &str,
    requested: &toml::Value,
    stored: Option<&toml::Value>,
) -> Result<(), RuntimeError> {
    if !has_retained_secret(requested) {
        return Ok(());
    }
    let denied = || {
        RuntimeError::InvalidConfig(
        "stored I/O credentials require unchanged authentication context; enter replacement credentials".into(),
    )
    };
    let stored = stored.ok_or_else(denied)?;
    let same = match driver.trim().to_ascii_lowercase().as_str() {
        "mqtt" | "mqtt-tcp" => crate::io::MqttIoDriver::same_credential_context(requested, stored)
            .map_err(|_| denied())?,
        _ => {
            let mut projection = stored.clone();
            redact(&mut projection);
            requested == &projection
        }
    };
    if same {
        Ok(())
    } else {
        Err(denied())
    }
}

fn has_retained_secret(value: &toml::Value) -> bool {
    match value {
        toml::Value::Table(table) => table.iter().any(|(key, child)| {
            (crate::security::is_secret_param_key(key) && child.as_str() == Some(SECRET_MARKER))
                || has_retained_secret(child)
        }),
        toml::Value::Array(values) => values.iter().any(has_retained_secret),
        _ => false,
    }
}

fn redact(value: &mut toml::Value) {
    match value {
        toml::Value::Table(table) => {
            for (key, child) in table {
                if crate::security::is_secret_param_key(key) {
                    *child = toml::Value::String(SECRET_MARKER.to_owned());
                } else {
                    redact(child);
                }
            }
        }
        toml::Value::Array(values) => values.iter_mut().for_each(redact),
        _ => {}
    }
}

#[cfg(test)]
mod tests;

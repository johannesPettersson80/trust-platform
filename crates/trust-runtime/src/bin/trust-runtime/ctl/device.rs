//! `trust-runtime ctl device-get`, `device-set <file>` and `device-reset`: a chip's own settings,
//! `device.toml` (specification 34 TARGET-02; specification 11, the device settings contract).
//!
//! `device-set` reads the `device.toml` file on this computer and sends it as a JSON object of the
//! same shape; the chip checks it against its own interfaces and stores it while its program is
//! stopped, and the settings apply at its next boot. `device-get` prints the stored settings as
//! `device.toml` text, with `password_set` in place of the Wi-Fi password, which a chip never sends
//! back. `device-reset` resets the chip, which then boots with its stored settings.

use std::path::Path;

use anyhow::{anyhow, Context};
use serde_json::{json, Value};

use super::{ensure_control_response_ok, send_request, ResolvedControlTarget};
use crate::cli::ControlAction;

/// Runs one of the device actions against `target`.
pub(super) fn run(target: &ResolvedControlTarget, action: &ControlAction) -> anyhow::Result<()> {
    let auth = target.auth_token.as_deref();
    let request = match action {
        ControlAction::DeviceGet => json!({"id": 1, "type": "device.get", "auth": auth}),
        ControlAction::DeviceSet { file } => {
            json!({"id": 1, "type": "device.set", "auth": auth,
                "params": {"settings": settings_of(file)?}})
        }
        ControlAction::DeviceReset => json!({"id": 1, "type": "device.reset", "auth": auth}),
        _ => return Err(anyhow!("not a device action")),
    };
    let response = send_request(&target.endpoint, &request)?;
    ensure_control_response_ok(&response)?;
    let result = &response["result"];
    println!("{}", render(action, result)?);
    Ok(())
}

/// The settings of the `device.toml` file at `path`, as the JSON object `device.set` takes.
fn settings_of(path: &Path) -> anyhow::Result<Value> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read the device settings {}", path.display()))?;
    let table: toml::Table =
        toml::from_str(&text).with_context(|| format!("{} is not a TOML file", path.display()))?;
    serde_json::to_value(table).context("convert the device settings")
}

/// What the command prints for `result`.
fn render(action: &ControlAction, result: &Value) -> anyhow::Result<String> {
    Ok(match action {
        ControlAction::DeviceGet => {
            let Some(settings) = result.get("settings").filter(|value| !value.is_null()) else {
                return Ok("no device settings are stored".to_string());
            };
            let table: toml::Table =
                serde_json::from_value(settings.clone()).context("read the device settings")?;
            let sequence = result["sequence"].as_u64().unwrap_or_default();
            format!(
                "# device settings, sequence {sequence}\n{}",
                toml::to_string(&table).context("write the device settings")?
            )
        }
        ControlAction::DeviceSet { .. } => format!(
            "stored sequence={}; the settings apply at the device's next boot (trust-runtime ctl \
             device-reset)",
            result["sequence"]
        ),
        _ => "resetting".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `device.toml` file is sent as the JSON object of the same shape, and the stored settings are
    /// printed back as TOML with their sequence. Catches a file sent with its sections renamed or as
    /// text, which the chip would refuse, and settings printed as raw JSON.
    #[test]
    fn device_toml_goes_as_its_json_and_comes_back_as_toml() {
        let path = std::env::temp_dir().join(format!("trust-device-{}.toml", std::process::id()));
        std::fs::write(
            &path,
            "[device]\nname = \"line-3\"\n\n[network.wifi]\nssid = \"Plant\"\npassword = \"secret-pass\"\n",
        )
        .expect("the file");
        let settings = settings_of(&path).expect("the settings");
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            settings,
            json!({"device": {"name": "line-3"},
                "network": {"wifi": {"ssid": "Plant", "password": "secret-pass"}}})
        );
        let printed = render(
            &ControlAction::DeviceGet,
            &json!({"settings": {"device": {"name": "line-3"},
                "network": {"wifi": {"ssid": "Plant", "password_set": true}}}, "sequence": 2}),
        )
        .expect("rendered");
        assert_eq!(
            printed,
            "# device settings, sequence 2\n[device]\nname = \"line-3\"\n\n[network.wifi]\nssid = \"Plant\"\npassword_set = true\n"
        );
        assert_eq!(
            render(
                &ControlAction::DeviceGet,
                &json!({"settings": null, "sequence": null})
            )
            .expect("rendered"),
            "no device settings are stored"
        );
    }
}

//! The device settings requests (specification 34 TARGET-02; specification 11, the device settings
//! contract): `device.get`, `device.set` and `device.reset`. A chip keeps its settings, `device.toml`,
//! in its flash; they apply at its next boot, which `device.reset` starts. A runtime that keeps no
//! device settings, a computer among them, answers each with `unsupported request`.

use alloc::string::ToString;

use serde_json::{json, Value};
use trust_runtime_core::scheduler::ResourceState;

use crate::device_settings::DeviceSettings;
use crate::ports::ControlHost;
use crate::protocol::ControlResponse;
use crate::resource::replies::{hung_refusal, resource_not_running};
use crate::status::invalid_params;

/// The refusal of a runtime that keeps no device settings.
fn unsupported(id: u64) -> ControlResponse {
    ControlResponse::error(id, "unsupported request".to_string())
}

/// `device.get` (Engineer): the stored settings without the Wi-Fi password, with their sequence, or
/// nulls when none are stored.
pub(super) fn get<H: ControlHost>(id: u64, host: &H) -> ControlResponse {
    if host.device_interfaces().is_none() {
        return unsupported(id);
    }
    match host.device_settings() {
        Ok(Some((sequence, settings))) => ControlResponse::ok(
            id,
            json!({"settings": settings.reported(), "sequence": sequence}),
        ),
        Ok(None) => ControlResponse::ok(id, json!({"settings": null, "sequence": null})),
        Err(error) => ControlResponse::error(id, error),
    }
}

/// `device.set {settings}` (Admin): checks the settings against the device's interfaces and stores
/// them while the resource is Stopped. They apply at the device's next boot.
///
/// The first failing check is the reply, in this order: a runtime without device settings
/// (`unsupported request`); the params (`missing params`, then `settings` not an object, then the
/// settings' own reading); a hung resource; a resource that is not Stopped, as
/// `resource_not_running`; the check against the interfaces; and the flash.
pub(super) fn set<H: ControlHost>(id: u64, params: Option<&Value>, host: &H) -> ControlResponse {
    let Some(interfaces) = host.device_interfaces() else {
        return unsupported(id);
    };
    let Some(params) = params else {
        return ControlResponse::error(id, "missing params".to_string());
    };
    let Some(value) = params.get("settings").filter(|value| value.is_object()) else {
        return invalid_params(id, "settings must be an object");
    };
    let settings = match DeviceSettings::from_value(value) {
        Ok(settings) => settings,
        Err(error) => return invalid_params(id, &error),
    };
    if host.is_hung() {
        return hung_refusal(id);
    }
    let state = host.resource_state();
    if !matches!(state, ResourceState::Stopped) {
        return resource_not_running(id, state);
    }
    if let Err(refusal) = settings.check(&interfaces) {
        return ControlResponse::error(id, refusal);
    }
    match host.store_device_settings(&settings) {
        Ok(sequence) => ControlResponse::ok(
            id,
            json!({"status": "stored", "sequence": sequence, "applies": "boot"}),
        ),
        Err(error) => ControlResponse::error(id, error),
    }
}

/// `device.reset` (Admin): the device resets once the reply is sent, and boots with its stored
/// settings, while the resource is Stopped (its outputs then at their stop values).
pub(super) fn reset<H: ControlHost>(id: u64, host: &H) -> ControlResponse {
    if host.device_interfaces().is_none() {
        return unsupported(id);
    }
    if host.is_hung() {
        return hung_refusal(id);
    }
    let state = host.resource_state();
    if !matches!(state, ResourceState::Stopped) {
        return resource_not_running(id, state);
    }
    if host.request_device_reset() {
        ControlResponse::ok(id, json!({"status": "resetting"}))
    } else {
        unsupported(id)
    }
}

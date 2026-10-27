//! Agent-facing runtime validation helpers.

use serde_json::{json, Value as JsonValue};
use trust_runtime::config::RuntimeBundle;
use trust_runtime::control::endpoint::project_control_endpoint_text;
use trust_runtime::control::ControlEndpoint;
use trust_runtime::io::IoDriverRegistry;
use trust_runtime::security::load_tls_materials;

pub(crate) fn validate_json_payload(bundle: std::path::PathBuf) -> anyhow::Result<JsonValue> {
    let bundle = RuntimeBundle::load(&bundle)?;
    let _tls_materials = load_tls_materials(&bundle.runtime.tls, Some(bundle.root.as_path()))?;
    // An omitted endpoint is the platform default, which generates its own token when it is TCP.
    if let Some(configured) = bundle.runtime.control_endpoint.as_deref() {
        let control_endpoint = ControlEndpoint::parse(configured)?;
        if matches!(control_endpoint, ControlEndpoint::Tcp(_))
            && bundle.runtime.control_auth_token.is_none()
        {
            anyhow::bail!("tcp control endpoint requires runtime.control.auth_token");
        }
    }
    let registry = IoDriverRegistry::default_registry();
    for driver in &bundle.io.drivers {
        registry
            .validate(driver.name.as_str(), &driver.params)
            .map_err(anyhow::Error::from)?;
    }
    // The program image is admitted as the computer admits it at its start (specification 44
    // §10.2, §8.8).
    trust_runtime::native::admit_on_this_computer(&bundle.image)?;
    let io_drivers = bundle
        .io
        .drivers
        .iter()
        .map(|driver| driver.name.to_string())
        .collect::<Vec<_>>();
    Ok(json!({
        "version": 1,
        "command": "validate",
        "status": "ok",
        "project": bundle.root.display().to_string(),
        "resource": bundle.runtime.resource_name.to_string(),
        "control_endpoint": project_control_endpoint_text(
            &bundle.root,
            bundle.runtime.control_endpoint.as_deref(),
        ),
        "io_driver": io_drivers.first().cloned().unwrap_or_default(),
        "io_drivers": io_drivers,
    }))
}

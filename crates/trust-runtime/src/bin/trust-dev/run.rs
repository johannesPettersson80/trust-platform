//! Agent-facing runtime validation helpers.

use serde_json::{json, Value as JsonValue};
use trust_runtime::bytecode::BytecodeModule;
use trust_runtime::config::RuntimeBundle;
use trust_runtime::control::ControlEndpoint;
use trust_runtime::io::IoDriverRegistry;
use trust_runtime::security::load_tls_materials;

pub(crate) fn validate_json_payload(bundle: std::path::PathBuf) -> anyhow::Result<JsonValue> {
    let bundle = RuntimeBundle::load(&bundle)?;
    let _tls_materials = load_tls_materials(&bundle.runtime.tls, Some(bundle.root.as_path()))?;
    let control_endpoint = ControlEndpoint::parse(bundle.runtime.control_endpoint.as_str())?;
    if matches!(control_endpoint, ControlEndpoint::Tcp(_))
        && bundle.runtime.control_auth_token.is_none()
    {
        anyhow::bail!("tcp control endpoint requires runtime.control.auth_token");
    }
    let registry = IoDriverRegistry::default_registry();
    for driver in &bundle.io.drivers {
        registry
            .validate(driver.name.as_str(), &driver.params)
            .map_err(anyhow::Error::from)?;
    }
    let module = BytecodeModule::decode(&bundle.bytecode)?;
    module.validate()?;
    let metadata = module.metadata()?;
    let _resource = metadata
        .resource(bundle.runtime.resource_name.as_str())
        .or_else(|| metadata.primary_resource())
        .ok_or_else(|| anyhow::anyhow!("bytecode metadata missing resource definitions"))?;
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
        "control_endpoint": bundle.runtime.control_endpoint.to_string(),
        "io_driver": io_drivers.first().cloned().unwrap_or_default(),
        "io_drivers": io_drivers,
    }))
}

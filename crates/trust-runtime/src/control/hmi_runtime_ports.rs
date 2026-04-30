pub(super) struct RuntimeHmiReadResult {
    pub(super) schema: crate::hmi::HmiSchemaResult,
    pub(super) values: crate::hmi::HmiValuesResult,
}

pub(super) struct RuntimeHmiQueuedWrite {
    pub(super) id: String,
    pub(super) path: String,
}

pub(super) fn read_hmi_schema_port(
    state: &ControlState,
) -> Result<crate::hmi::HmiSchemaResult, String> {
    let metadata = state
        .metadata
        .lock()
        .map_err(|_| "metadata unavailable".to_string())?;
    let snapshot = load_runtime_snapshot(state);
    let descriptor = hmi_descriptor_snapshot(state);
    Ok(build_hmi_schema_for_port(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        &descriptor,
    ))
}

pub(super) fn read_hmi_values_port(
    state: &ControlState,
    ids: Option<&[String]>,
) -> Result<RuntimeHmiReadResult, String> {
    let metadata = state
        .metadata
        .lock()
        .map_err(|_| "metadata unavailable".to_string())?;
    let snapshot = load_runtime_snapshot(state);
    let descriptor = hmi_descriptor_snapshot(state);
    let schema = build_hmi_schema_for_port(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        &descriptor,
    );
    let values = crate::hmi::build_values(
        state.resource_name.as_str(),
        &metadata,
        snapshot.as_ref(),
        true,
        ids,
    );
    Ok(RuntimeHmiReadResult { schema, values })
}

pub(super) fn queue_hmi_runtime_write_port(
    state: &ControlState,
    target: &str,
    requested_value: &serde_json::Value,
) -> Result<RuntimeHmiQueuedWrite, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("missing params.id".to_string());
    }

    let descriptor = hmi_descriptor_snapshot(state);
    let customization = descriptor.customization;
    if !customization.write_enabled() {
        return Err("hmi.write disabled in read-only mode".to_string());
    }
    if customization.write_allowlist().is_empty() {
        return Err("hmi.write allowlist is empty".to_string());
    }

    let metadata = state
        .metadata
        .lock()
        .map_err(|_| "metadata unavailable".to_string())?;
    let snapshot =
        load_runtime_snapshot(state).ok_or_else(|| "runtime snapshot unavailable".to_string())?;
    let point =
        crate::hmi::resolve_write_point(state.resource_name.as_str(), &metadata, Some(&snapshot), target)
            .ok_or_else(|| format!("unknown hmi target '{target}'"))?;
    let allowed = customization.write_target_allowed(point.id.as_str())
        || customization.write_target_allowed(point.path.as_str());
    if !allowed {
        return Err("hmi.write target is not in allowlist".to_string());
    }

    let template = crate::hmi::resolve_write_value_template(&point, &snapshot).ok_or_else(|| {
        format!("hmi.write target '{}' is currently unavailable", point.id)
    })?;
    let value = parse_hmi_write_value(requested_value, &template)
        .ok_or_else(|| format!("invalid hmi.write value for target '{}'", point.id))?;

    match &point.binding {
        crate::hmi::HmiWriteBinding::ProgramVar { program, variable } => {
            let Value::Instance(instance_id) = snapshot
                .storage
                .get_global(program.as_str())
                .ok_or_else(|| {
                    format!("hmi.write target '{}' is currently unavailable", point.id)
                })?
            else {
                return Err(format!(
                    "hmi.write target '{}' is currently unavailable",
                    point.id
                ));
            };
            state
                .debug
                .enqueue_instance_write(*instance_id, variable.clone(), value);
        }
        crate::hmi::HmiWriteBinding::Global { name } => {
            state.debug.enqueue_global_write(name.clone(), value);
        }
    }

    Ok(RuntimeHmiQueuedWrite {
        id: point.id,
        path: point.path,
    })
}

fn build_hmi_schema_for_port(
    resource_name: &str,
    metadata: &crate::RuntimeMetadata,
    snapshot: Option<&crate::debug::DebugSnapshot>,
    descriptor: &HmiRuntimeDescriptor,
) -> crate::hmi::HmiSchemaResult {
    let mut result = crate::hmi::build_schema(
        resource_name,
        metadata,
        snapshot,
        true,
        Some(&descriptor.customization),
    );
    result.schema_revision = descriptor.schema_revision;
    result.descriptor_error = descriptor.last_error.clone();
    result
}

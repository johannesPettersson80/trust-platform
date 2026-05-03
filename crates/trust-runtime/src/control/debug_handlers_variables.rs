pub(super) fn handle_debug_scopes(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: DebugScopesParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    debug!("control debug.scopes frame_id={}", params.frame_id);
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let requested_frame = crate::memory::FrameId(params.frame_id);
    if !snapshot
        .storage
        .frames()
        .iter()
        .any(|frame| frame.id == requested_frame)
    {
        return ControlResponse::error(id, format!("unknown frame id {}", params.frame_id));
    }
    let location = state
        .debug
        .frame_location(requested_frame)
        .or_else(|| state.debug.last_stop().and_then(|stop| stop.location))
        .and_then(|loc| location_to_source(&loc, state));
    let (has_globals, has_retain, has_instances) = (
        !snapshot.storage.globals().is_empty(),
        !snapshot.storage.retain().is_empty(),
        !snapshot.storage.instances().is_empty(),
    );
    debug!(
        "control debug.scopes frame_id={} has_globals={} has_retain={} has_instances={}",
        params.frame_id, has_globals, has_retain, has_instances
    );
    let io_snapshot = match state.io_snapshot.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return ControlResponse::error(id, "I/O snapshot unavailable".to_string()),
    };
    let has_io = crate::debug::dap::io_scope_available(io_snapshot.as_ref());

    let mut handles = match state.debug_variables.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "debug variables unavailable".into()),
    };
    handles.clear();

    let mut scopes = Vec::new();
    let locals_ref = handles.alloc(VariableHandle::Locals(requested_frame));
    scopes.push(DebugScope {
        name: "Locals".to_string(),
        variables_reference: locals_ref,
        expensive: false,
        source: location.as_ref().map(|(source, _, _)| source.clone()),
        line: location.as_ref().map(|(_, line, _)| *line),
        column: location.as_ref().map(|(_, _, column)| *column),
        end_line: None,
        end_column: None,
    });
    if has_globals {
        let globals_ref = handles.alloc(VariableHandle::Globals);
        scopes.push(DebugScope {
            name: "Globals".to_string(),
            variables_reference: globals_ref,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });
    }
    if has_retain {
        let retain_ref = handles.alloc(VariableHandle::Retain);
        scopes.push(DebugScope {
            name: "Retain".to_string(),
            variables_reference: retain_ref,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });
    }
    if has_io {
        let io_ref = handles.alloc(VariableHandle::IoRoot);
        scopes.push(DebugScope {
            name: "I/O".to_string(),
            variables_reference: io_ref,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });
    }
    if has_instances {
        let instances_ref = handles.alloc(VariableHandle::Instances);
        scopes.push(DebugScope {
            name: "Instances".to_string(),
            variables_reference: instances_ref,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });
    }

    debug!(
        "control debug.scopes result={:?}",
        scopes
            .iter()
            .map(|scope| scope.name.as_str())
            .collect::<Vec<_>>()
    );
    ControlResponse::ok(id, json!({ "scopes": scopes }))
}

pub(super) fn handle_debug_variables(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: DebugVariablesParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    debug!(
        "control debug.variables reference={}",
        params.variables_reference
    );
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let io_snapshot = match state.io_snapshot.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return ControlResponse::error(id, "I/O snapshot unavailable".to_string()),
    };
    let mut handles = match state.debug_variables.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "debug variables unavailable".into()),
    };
    let Some(handle) = handles.get(params.variables_reference).cloned() else {
        return ControlResponse::error(
            id,
            format!("unknown variables reference {}", params.variables_reference),
        );
    };
    debug!("control debug.variables handle={:?}", handle);
    let variables = match handle {
        VariableHandle::Locals(frame_id) => {
            let Some(frame) = snapshot
                .storage
                .frames()
                .iter()
                .find(|frame| frame.id == frame_id)
            else {
                return ControlResponse::error(id, format!("unknown frame id {}", frame_id.0));
            };
            let mut entries = Vec::new();
            if let Some(instance_id) = frame.instance_id {
                let Some(instance) = snapshot.storage.get_instance(instance_id) else {
                    return ControlResponse::error(
                        id,
                        format!("unknown instance id {}", instance_id.0),
                    );
                };
                entries.extend(
                    instance
                        .variables
                        .iter()
                        .map(|(name, value)| (name.to_string(), value.clone())),
                );
            }
            entries.extend(
                frame
                    .variables
                    .iter()
                    .map(|(name, value)| (name.to_string(), value.clone())),
            );
            crate::debug::dap::variables_from_entries(&mut handles, entries)
        }
        VariableHandle::Globals => {
            let entries = snapshot
                .storage
                .globals()
                .iter()
                .map(|(name, value)| (name.to_string(), value.clone()))
                .collect::<Vec<_>>();
            crate::debug::dap::variables_from_entries(&mut handles, entries)
        }
        VariableHandle::Retain => {
            let entries = snapshot
                .storage
                .retain()
                .iter()
                .map(|(name, value)| (name.to_string(), value.clone()))
                .collect::<Vec<_>>();
            crate::debug::dap::variables_from_entries(&mut handles, entries)
        }
        VariableHandle::Instances => {
            let instances = snapshot
                .storage
                .instances()
                .iter()
                .map(|(id, data)| (*id, format!("{}#{}", data.type_name, id.0)))
                .collect::<Vec<_>>();
            crate::debug::dap::variables_from_instances(&mut handles, instances)
        }
        VariableHandle::Instance(instance_id) => {
            let Some(instance) = snapshot.storage.get_instance(instance_id) else {
                return ControlResponse::error(id, format!("unknown instance id {}", instance_id.0));
            };
            let mut entries = instance
                .variables
                .iter()
                .map(|(name, value)| (name.to_string(), value.clone()))
                .collect::<Vec<_>>();
            if let Some(parent) = instance.parent {
                entries.push(("parent".to_string(), Value::Instance(parent)));
            }
            crate::debug::dap::variables_from_entries(&mut handles, entries)
        }
        VariableHandle::Struct(value) => {
            crate::debug::dap::variables_from_struct(&mut handles, value)
        }
        VariableHandle::Array(value) => {
            crate::debug::dap::variables_from_array(&mut handles, value)
        }
        VariableHandle::Reference(value_ref) => {
            let Some(value) = snapshot.storage.read_by_ref(value_ref).cloned() else {
                return ControlResponse::error(id, "reference target unavailable".to_string());
            };
            vec![crate::debug::dap::variable_from_value(
                &mut handles,
                "*".to_string(),
                value,
                None,
            )]
        }
        VariableHandle::IoRoot => {
            let Some(state) = io_snapshot.as_ref() else {
                return ControlResponse::error(id, "I/O snapshot unavailable".to_string());
            };
            let inputs_ref = handles.alloc(VariableHandle::IoInputs);
            let outputs_ref = handles.alloc(VariableHandle::IoOutputs);
            let memory_ref = handles.alloc(VariableHandle::IoMemory);
            vec![
                DebugVariable {
                    name: "Inputs".to_string(),
                    value: format!("{} items", state.inputs.len()),
                    r#type: None,
                    variables_reference: inputs_ref,
                    evaluate_name: None,
                },
                DebugVariable {
                    name: "Outputs".to_string(),
                    value: format!("{} items", state.outputs.len()),
                    r#type: None,
                    variables_reference: outputs_ref,
                    evaluate_name: None,
                },
                DebugVariable {
                    name: "Memory".to_string(),
                    value: format!("{} items", state.memory.len()),
                    r#type: None,
                    variables_reference: memory_ref,
                    evaluate_name: None,
                },
            ]
        }
        VariableHandle::IoInputs => {
            let Some(state) = io_snapshot.as_ref() else {
                return ControlResponse::error(id, "I/O snapshot unavailable".to_string());
            };
            crate::debug::dap::variables_from_io_entries(&state.inputs)
        }
        VariableHandle::IoOutputs => {
            let Some(state) = io_snapshot.as_ref() else {
                return ControlResponse::error(id, "I/O snapshot unavailable".to_string());
            };
            crate::debug::dap::variables_from_io_entries(&state.outputs)
        }
        VariableHandle::IoMemory => {
            let Some(state) = io_snapshot.as_ref() else {
                return ControlResponse::error(id, "I/O snapshot unavailable".to_string());
            };
            crate::debug::dap::variables_from_io_entries(&state.memory)
        }
    };
    debug!("control debug.variables result_count={}", variables.len());
    ControlResponse::ok(id, json!({ "variables": variables }))
}

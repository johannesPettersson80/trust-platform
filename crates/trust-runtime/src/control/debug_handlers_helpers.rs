fn debug_stop_to_json(
    stop: crate::debug::DebugStop,
    state: &ControlState,
) -> Option<serde_json::Value> {
    let reason = match stop.reason {
        crate::debug::DebugStopReason::Breakpoint => "breakpoint",
        crate::debug::DebugStopReason::Step => "step",
        crate::debug::DebugStopReason::Pause => "pause",
        crate::debug::DebugStopReason::Entry => "entry",
    };
    let mut payload = json!({
        "reason": reason,
        "thread_id": stop.thread_id,
        "breakpoint_generation": stop.breakpoint_generation,
    });
    if let Some(location) = stop.location {
        if let Some(text) = state.sources.source_text(location.file_id) {
            let (line, column) = location_to_line_col(text, &location);
            if let Some(source) = state
                .sources
                .files()
                .iter()
                .find(|file| file.id == location.file_id)
            {
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("file_id".to_string(), json!(location.file_id));
                    obj.insert("line".to_string(), json!(line));
                    obj.insert("column".to_string(), json!(column));
                    obj.insert(
                        "path".to_string(),
                        json!(source.path.to_string_lossy().to_string()),
                    );
                }
            }
        }
    }
    Some(payload)
}

fn location_to_source(
    location: &crate::debug::SourceLocation,
    state: &ControlState,
) -> Option<(DebugSource, u32, u32)> {
    let source_text = state.sources.source_text(location.file_id)?;
    let (line, column) = location_to_line_col(source_text, location);
    let source = state
        .sources
        .files()
        .iter()
        .find(|file| file.id == location.file_id)?;
    let name = source
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string());
    let path = Some(source.path.to_string_lossy().to_string());
    Some((DebugSource { name, path }, line, column))
}

fn location_to_stack_frame(
    frame_id: u32,
    frame_name: &str,
    location: &crate::debug::SourceLocation,
    state: &ControlState,
) -> Option<serde_json::Value> {
    let (source, line, column) = location_to_source(location, state)?;
    Some(json!({
        "id": frame_id,
        "name": frame_name,
        "source": source,
        "line": line,
        "column": column,
    }))
}

fn evaluate_with_snapshot(
    expr: &crate::program_model::Expr,
    registry: &trust_hir::types::TypeRegistry,
    frame_id: Option<crate::memory::FrameId>,
    snapshot: &crate::debug::DebugSnapshot,
    _using: &[smol_str::SmolStr],
    state: &ControlState,
) -> Result<Value, RuntimeError> {
    let metadata = state
        .metadata
        .lock()
        .map_err(|_| RuntimeError::ControlError("metadata unavailable".into()))?;
    let profile = metadata.profile();
    let mut storage = snapshot.storage.clone();
    let eval = |storage: &mut crate::memory::VariableStorage,
                instance_id: Option<crate::memory::InstanceId>|
     -> Result<Value, RuntimeError> {
        crate::helper_eval::eval_storage_expr_with_stdlib(
            storage,
            registry,
            &profile,
            instance_id,
            Some(metadata.stdlib()),
            expr,
        )
    };

    if let Some(frame_id) = frame_id {
        storage
            .with_frame(frame_id, |storage| {
                let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                eval(storage, instance_id)
            })
            .ok_or(RuntimeError::InvalidFrame(frame_id.0))?
    } else {
        eval(&mut storage, None)
    }
}
